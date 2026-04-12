use anyhow::Result;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use qapi::QapiCommand;
use qapi_protocol::{Qmp, QmpEvent};

use crate::arch::{Arch, MachineConfig};

/// Minimum QEMU version required for `--preconfig` support.
pub const PRECONFIG_MIN_VERSION: (u32, u32, u32) = (3, 0, 0);

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

/// QEMU process handle with cleanup logic.
///
/// Implements `Drop` to ensure the QEMU process is killed and temp files
/// are removed even if the owning `PreconfigVm` or `Vm` is dropped without
/// calling `shutdown()`. The `cleanup()` method provides async cleanup;
/// `Drop` provides best-effort sync cleanup as a safety net.
struct VmProcess {
    child: Option<Child>,
    qmp_sock: PathBuf,
    serial_log: Option<PathBuf>,
    vnc_sock: Option<PathBuf>,
}

impl VmProcess {
    /// Async cleanup: kill the process, wait for exit, remove temp files.
    /// Safe to call multiple times (idempotent).
    async fn cleanup(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        self.child = None;
        let _ = tokio::fs::remove_file(&self.qmp_sock).await;
        if let Some(ref s) = self.serial_log {
            let _ = tokio::fs::remove_file(s).await;
        }
        if let Some(ref s) = self.vnc_sock {
            let _ = tokio::fs::remove_file(s).await;
        }
    }
}

impl Drop for VmProcess {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
        let _ = std::fs::remove_file(&self.qmp_sock);
        if let Some(ref s) = self.serial_log {
            let _ = std::fs::remove_file(s);
        }
        if let Some(ref s) = self.vnc_sock {
            let _ = std::fs::remove_file(s);
        }
    }
}

/// Builder for configuring and spawning a QEMU virtual machine.
///
/// `VmBuilder` always spawns QEMU in preconfig mode (`--preconfig`), returning
/// a [`PreconfigVm`] handle. This allows configuration of the VM via QMP before
/// guest execution begins. Call [`PreconfigVm::start()`] to exit preconfig and
/// transition to a fully running [`Vm`].
///
/// Requires QEMU >= 3.0.0. The version is checked after connecting and the
/// spawn will fail with an error if the QEMU binary is too old.
pub struct VmBuilder {
    arch: Arch,
    config: MachineConfig,
    kernel: Option<PathBuf>,
    initrd: Option<PathBuf>,
    append: Option<String>,
    serial: bool,
    display: bool,
    no_shutdown: bool,
    firmware: Option<PathBuf>,
    extra_args: Vec<String>,
}

impl VmBuilder {
    pub fn new(arch: Arch, config: MachineConfig) -> Self {
        Self {
            arch,
            config,
            kernel: None,
            initrd: None,
            append: None,
            serial: false,
            display: false,
            no_shutdown: false,
            firmware: None,
            extra_args: Vec::new(),
        }
    }

    /// Use direct kernel boot (bypasses bootloader).
    pub fn kernel(mut self, kernel: PathBuf, initrd: PathBuf, append: &str) -> Self {
        self.kernel = Some(kernel);
        self.initrd = Some(initrd);
        self.append = Some(append.to_string());
        self
    }

    /// Enable serial output to a log file.
    pub fn serial(mut self) -> Self {
        self.serial = true;
        self
    }

    /// Enable VGA display with a VNC server on a Unix socket.
    pub fn display(mut self) -> Self {
        self.display = true;
        self
    }

    /// Don't exit QEMU on guest shutdown.
    pub fn no_shutdown(mut self) -> Self {
        self.no_shutdown = true;
        self
    }

    /// Override firmware path for UEFI boot.
    pub fn firmware(mut self, path: PathBuf) -> Self {
        self.firmware = Some(path);
        self
    }

    /// Add an extra QEMU argument.
    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.extra_args.push(a.into());
        self
    }

    /// Spawn the QEMU process in preconfig mode and connect QMP.
    ///
    /// Returns a [`PreconfigVm`] handle for issuing configuration commands
    /// before guest execution. Call [`PreconfigVm::start()`] to begin
    /// execution.
    pub async fn spawn(self) -> Result<PreconfigVm> {
        let id = SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst);
        let tmpdir = std::env::temp_dir();
        let qmp_sock = tmpdir.join(format!("qrab-qmp-{}-{}.sock", std::process::id(), id));
        let serial_log = if self.serial {
            Some(tmpdir.join(format!("qrab-serial-{}-{}.log", std::process::id(), id,)))
        } else {
            None
        };

        let _ = tokio::fs::remove_file(&qmp_sock).await;
        if let Some(ref s) = serial_log {
            let _ = tokio::fs::remove_file(s).await;
        }

        let mut args: Vec<String> = Vec::new();
        if self.display {
            // With display, skip --preconfig: VNC must see VGA during machine
            // init to register the graphic console. Use -S (start paused) so
            // the caller can still configure before guest execution.
            args.push("-S".into());
        } else {
            args.push("--preconfig".into());
        }

        match &self.config {
            MachineConfig::None => {
                args.extend(["-machine".into(), "none".into()]);
            }
            MachineConfig::Bios { memory } => {
                let machine = self
                    .arch
                    .bios_machine()
                    .expect("BIOS boot not supported for this architecture");
                args.extend(["-machine".into(), machine.into()]);
                args.extend(["-m".into(), memory.clone()]);
            }
            MachineConfig::Uefi { memory } => {
                let machine = self.arch.uefi_machine();
                args.extend(["-machine".into(), machine.into()]);
                args.extend(["-m".into(), memory.clone()]);
                if let Some(ref fw) = self.firmware {
                    match self.arch {
                        Arch::X86_64 | Arch::I386 => {
                            args.extend(["-bios".into(), fw.display().to_string()]);
                        }
                        Arch::Aarch64 | Arch::Arm => {
                            args.extend([
                                "-drive".into(),
                                format!("if=pflash,format=raw,readonly=on,file={}", fw.display()),
                            ]);
                        }
                    }
                }
            }
        }

        if let Some(cpu) = self.arch.cpu() {
            args.extend(["-cpu".into(), cpu.into()]);
        }

        let vnc_sock = if self.display {
            let sock = tmpdir.join(format!("qrab-vnc-{}-{}.sock", std::process::id(), id));
            let _ = tokio::fs::remove_file(&sock).await;
            args.push("-nodefaults".into());
            args.extend([
                "-display".into(),
                format!("vnc=unix:{}", sock.display()),
            ]);
            args.extend(["-vga".into(), "std".into()]);
            Some(sock)
        } else {
            args.extend(["-nographic".into(), "-nodefaults".into()]);
            None
        };

        args.extend([
            "-qmp".into(),
            format!("unix:{},server=on,wait=off", qmp_sock.display()),
        ]);

        if let Some(ref log) = serial_log {
            args.extend(["-serial".into(), format!("file:{}", log.display())]);
        }

        if let Some(ref kernel) = self.kernel {
            args.extend(["-kernel".into(), kernel.display().to_string()]);
        }
        if let Some(ref initrd) = self.initrd {
            args.extend(["-initrd".into(), initrd.display().to_string()]);
        }
        if let Some(ref append) = self.append {
            args.extend(["-append".into(), append.clone()]);
        }

        if self.no_shutdown {
            args.push("-no-shutdown".into());
        }

        args.extend(self.extra_args);

        eprintln!(
            "QEMU spawn: {} {}",
            self.arch.binary().display(),
            args.join(" ")
        );

        let child = Command::new(self.arch.binary())
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        for _ in 0..50 {
            if qmp_sock.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if !qmp_sock.exists() {
            anyhow::bail!("QMP socket did not appear: {}", qmp_sock.display());
        }

        let (qmp, events) = Qmp::new(&qmp_sock).await?;

        if !self.display {
            let version = qapi::qemu_version();
            if version < PRECONFIG_MIN_VERSION {
                anyhow::bail!(
                    "QEMU {}.{}.{} does not support --preconfig (requires >= {}.{}.{})",
                    version.0,
                    version.1,
                    version.2,
                    PRECONFIG_MIN_VERSION.0,
                    PRECONFIG_MIN_VERSION.1,
                    PRECONFIG_MIN_VERSION.2,
                );
            }
        }

        Ok(PreconfigVm {
            qmp,
            events,
            process: VmProcess {
                child: Some(child),
                qmp_sock,
                serial_log,
                vnc_sock,
            },
            paused_mode: self.display,
        })
    }
}

/// A QEMU VM in preconfig state, before guest execution.
///
/// In this state, only commands marked `allow-preconfig` in the QEMU schema
/// are accepted by QEMU. Non-preconfig commands will be rejected with a
/// "CommandNotFound" error.
///
/// Use the `qmp` field for direct QMP access, or [`execute()`](Self::execute)
/// for compile-time preconfig safety enforcement.
///
/// Call [`start()`](Self::start) to send `x-exit-preconfig` and transition
/// to a running [`Vm`].
pub struct PreconfigVm {
    /// QMP protocol client for sending commands to QEMU.
    pub qmp: Qmp,
    /// Channel for receiving asynchronous QMP events from QEMU.
    pub events: mpsc::Receiver<QmpEvent>,
    process: VmProcess,
    /// True when started with `-S` (paused) instead of `--preconfig`.
    paused_mode: bool,
}

impl PreconfigVm {
    /// Path to the serial log file, if serial was enabled.
    pub fn serial_log(&self) -> Option<&PathBuf> {
        self.process.serial_log.as_ref()
    }

    /// Path to the VNC Unix socket, if display was enabled.
    pub fn vnc_sock(&self) -> Option<&PathBuf> {
        self.process.vnc_sock.as_ref()
    }

    /// Execute a preconfig-safe QMP command.
    ///
    /// Only commands implementing [`PreconfigSafe`](qapi::PreconfigSafe) can be
    /// used here. For raw QMP access without the trait bound, use
    /// `self.qmp.execute()` directly.
    pub async fn execute<C: QapiCommand + qapi::PreconfigSafe>(
        &self,
        cmd: &C,
    ) -> Result<C::Returns> {
        self.qmp.execute(cmd).await
    }

    /// Exit preconfig/paused mode and start guest execution.
    ///
    /// Sends `x-exit-preconfig` or `cont` depending on how the VM was
    /// started. Returns a [`Vm`] handle with full QMP access.
    pub async fn start(self) -> Result<Vm> {
        if self.paused_mode {
            self.qmp.execute(&qapi::commands::Cont {}).await?;
        } else {
            self.qmp.execute(&qapi::commands::XExitPreconfig {}).await?;
        }
        Ok(Vm {
            qmp: self.qmp,
            events: self.events,
            process: self.process,
        })
    }

    /// Shut down without starting guest execution.
    pub async fn shutdown(mut self) {
        self.qmp.shutdown().await;
        self.process.cleanup().await;
    }
}

/// A running QEMU virtual machine with a QMP connection.
///
/// Created by calling [`PreconfigVm::start()`] after completing preconfig
/// setup. All QMP commands are available in this state.
///
/// Use the `qmp` field for direct QMP access, or the ergonomic methods
/// (`stop`, `cont`, `reset`, etc.) for common operations.
pub struct Vm {
    /// QMP protocol client for sending commands to QEMU.
    pub qmp: Qmp,
    /// Channel for receiving asynchronous QMP events from QEMU.
    pub events: mpsc::Receiver<QmpEvent>,
    process: VmProcess,
}

impl Vm {
    /// Path to the serial log file, if serial was enabled.
    pub fn serial_log(&self) -> Option<&PathBuf> {
        self.process.serial_log.as_ref()
    }

    /// Path to the VNC Unix socket, if display was enabled.
    pub fn vnc_sock(&self) -> Option<&PathBuf> {
        self.process.vnc_sock.as_ref()
    }

    /// Execute any QMP command and return the typed response.
    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        self.qmp.execute(cmd).await
    }

    // -- Ergonomic lifecycle methods --

    /// Pause guest execution (sends `stop`).
    pub async fn stop(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::Stop {}).await?;
        Ok(())
    }

    /// Resume guest execution (sends `cont`).
    pub async fn cont(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::Cont {}).await?;
        Ok(())
    }

    /// Reset the guest (sends `system_reset`).
    pub async fn reset(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::SystemReset {}).await?;
        Ok(())
    }

    /// Request guest powerdown via ACPI (sends `system_powerdown`).
    pub async fn powerdown(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::SystemPowerdown {}).await?;
        Ok(())
    }

    /// Send `quit` to QEMU, requesting the process to exit.
    pub async fn quit(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::Quit {}).await?;
        Ok(())
    }

    /// Graceful shutdown: disconnect QMP, kill the process, clean up temp files.
    pub async fn shutdown(mut self) {
        self.qmp.shutdown().await;
        self.process.cleanup().await;
    }
}
