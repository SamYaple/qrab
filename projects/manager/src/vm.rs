use anyhow::Result;
use std::mem::ManuallyDrop;
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
///
/// The `--preconfig` flag and `x-exit-preconfig` command were introduced in
/// QEMU 3.0.0 (August 2018). The `RunState::Preconfig` enum variant existed
/// from 3.0.0 to 5.2.x and was removed in 6.0.0 (the preconfig *mode* still
/// works, but QEMU no longer reports a distinct "preconfig" run state).
pub const PRECONFIG_MIN_VERSION: (u32, u32, u32) = (3, 0, 0);

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Shared state for a QEMU process with a QMP connection.
struct VmInner {
    qmp: Qmp,
    events: mpsc::Receiver<QmpEvent>,
    serial_log: Option<PathBuf>,
    child: Child,
    qmp_sock: PathBuf,
}

impl VmInner {
    async fn shutdown(&mut self) {
        self.qmp.shutdown().await;
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
        let _ = tokio::fs::remove_file(&self.qmp_sock).await;
        if let Some(ref s) = self.serial_log {
            let _ = tokio::fs::remove_file(s).await;
        }
    }

    fn drop_cleanup(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_file(&self.qmp_sock);
        if let Some(ref s) = self.serial_log {
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
///
/// # Example
///
/// ```no_run
/// # use manager::{Arch, MachineConfig, VmBuilder};
/// # async fn example() -> anyhow::Result<()> {
/// let preconfig = VmBuilder::new(Arch::X86_64, MachineConfig::None)
///     .spawn()
///     .await?;
///
/// // Issue preconfig-safe QMP commands here (object-add, blockdev-add, etc.)
///
/// let vm = preconfig.start().await?;
/// // VM is now running -- full QMP access
/// vm.shutdown().await;
/// # Ok(())
/// # }
/// ```
pub struct VmBuilder {
    arch: Arch,
    config: MachineConfig,
    kernel: Option<PathBuf>,
    initrd: Option<PathBuf>,
    append: Option<String>,
    serial: bool,
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
    ///
    /// Requires QEMU >= 3.0.0 for `--preconfig` support.
    pub async fn spawn(self) -> Result<PreconfigVm> {
        let id = SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst);
        let tmpdir = std::env::temp_dir();
        let qmp_sock = tmpdir.join(format!("qrab-qmp-{}-{}.sock", std::process::id(), id));
        let serial_log = if self.serial {
            Some(tmpdir.join(format!(
                "qrab-serial-{}-{}.log",
                std::process::id(),
                id,
            )))
        } else {
            None
        };

        let _ = tokio::fs::remove_file(&qmp_sock).await;
        if let Some(ref s) = serial_log {
            let _ = tokio::fs::remove_file(s).await;
        }

        let mut args: Vec<String> = Vec::new();

        // Always start in preconfig mode
        args.push("--preconfig".into());

        // Machine
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
                                format!(
                                    "if=pflash,format=raw,readonly=on,file={}",
                                    fw.display()
                                ),
                            ]);
                        }
                    }
                }
            }
        }

        // CPU
        if let Some(cpu) = self.arch.cpu() {
            args.extend(["-cpu".into(), cpu.into()]);
        }

        args.extend(["-nographic".into(), "-nodefaults".into()]);

        // QMP
        args.extend([
            "-qmp".into(),
            format!("unix:{},server=on,wait=off", qmp_sock.display()),
        ]);

        // Serial
        if let Some(ref log) = serial_log {
            args.extend(["-serial".into(), format!("file:{}", log.display())]);
        }

        // Direct kernel boot
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

        let child = Command::new(self.arch.binary())
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        // Wait for QMP socket
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

        // Verify QEMU version supports preconfig
        let version = qapi::qemu_version();
        if version < PRECONFIG_MIN_VERSION {
            anyhow::bail!(
                "QEMU {}.{}.{} does not support --preconfig (requires >= {}.{}.{})",
                version.0, version.1, version.2,
                PRECONFIG_MIN_VERSION.0, PRECONFIG_MIN_VERSION.1, PRECONFIG_MIN_VERSION.2,
            );
        }

        Ok(PreconfigVm {
            inner: ManuallyDrop::new(VmInner {
                qmp,
                events,
                serial_log,
                child,
                qmp_sock,
            }),
        })
    }
}

/// A QEMU VM in preconfig state, before guest execution.
///
/// In this state, only commands marked `allow-preconfig` in the QEMU schema
/// are accepted by QEMU. Non-preconfig commands will be rejected by QEMU
/// with a "CommandNotFound" error.
///
/// The type-level separation between `PreconfigVm` and [`Vm`] communicates
/// the lifecycle phase clearly: use `PreconfigVm` to configure, then call
/// [`start()`](PreconfigVm::start) to transition to a running [`Vm`].
///
/// Typical preconfig commands include:
/// - `object-add` / `object-del` -- create/destroy backend objects
/// - `blockdev-add` / `blockdev-del` -- configure block devices
/// - `set-numa-node` -- configure NUMA topology
/// - `query-status`, `query-name`, `query-commands` -- introspection
///
/// Call [`start()`](PreconfigVm::start) to send `x-exit-preconfig` and
/// transition to a running [`Vm`].
pub struct PreconfigVm {
    inner: ManuallyDrop<VmInner>,
}

impl PreconfigVm {
    /// Access the QMP handle directly.
    pub fn qmp(&self) -> &Qmp {
        &self.inner.qmp
    }

    /// Access the event receiver.
    pub fn events(&mut self) -> &mut mpsc::Receiver<QmpEvent> {
        &mut self.inner.events
    }

    /// Execute a QMP command during the preconfig phase.
    ///
    /// Any command can be submitted, but QEMU will reject commands that are
    /// not marked `allow-preconfig` in the schema with a "CommandNotFound"
    /// error. See the QEMU documentation for the list of preconfig-safe
    /// commands.
    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        Ok(self.inner.qmp.execute(cmd).await?)
    }

    /// Exit preconfig mode and start guest execution.
    ///
    /// Sends `x-exit-preconfig` to QEMU, which transitions the VM from the
    /// preconfig state to running. Returns a [`Vm`] handle with full QMP
    /// access.
    pub async fn start(mut self) -> Result<Vm> {
        self.inner.qmp.execute(&qapi::XExitPreconfig {}).await?;

        // SAFETY: we take ownership of inner and forget self so Drop doesn't run.
        let inner = unsafe { ManuallyDrop::take(&mut self.inner) };
        std::mem::forget(self);

        Ok(Vm {
            inner: ManuallyDrop::new(inner),
        })
    }

    /// Shut down without starting guest execution.
    ///
    /// Disconnects QMP and kills the QEMU process. Use this if configuration
    /// fails and you need to abort.
    pub async fn shutdown(mut self) {
        self.inner.shutdown().await;
        // SAFETY: we consumed inner above; prevent Drop from double-cleaning.
        unsafe { ManuallyDrop::drop(&mut self.inner) };
        std::mem::forget(self);
    }
}

impl Drop for PreconfigVm {
    fn drop(&mut self) {
        self.inner.drop_cleanup();
    }
}

/// A running QEMU virtual machine with a QMP connection.
///
/// Created by calling [`PreconfigVm::start()`] after completing preconfig
/// setup. All QMP commands are available in this state.
pub struct Vm {
    inner: ManuallyDrop<VmInner>,
}

impl Vm {
    /// Access the QMP handle directly.
    pub fn qmp(&self) -> &Qmp {
        &self.inner.qmp
    }

    /// Access the event receiver.
    pub fn events(&mut self) -> &mut mpsc::Receiver<QmpEvent> {
        &mut self.inner.events
    }

    /// Path to the serial log file, if serial was enabled.
    pub fn serial_log(&self) -> Option<&PathBuf> {
        self.inner.serial_log.as_ref()
    }

    /// Execute any QMP command and return the typed response.
    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        Ok(self.inner.qmp.execute(cmd).await?)
    }

    /// Graceful shutdown: disconnect QMP, then kill the process.
    pub async fn shutdown(mut self) {
        self.inner.shutdown().await;
        // SAFETY: we consumed inner above; prevent Drop from double-cleaning.
        unsafe { ManuallyDrop::drop(&mut self.inner) };
        std::mem::forget(self);
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        self.inner.drop_cleanup();
    }
}
