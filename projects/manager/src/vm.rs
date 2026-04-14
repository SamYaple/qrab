use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use qapi::QapiCommand;
use qapi_protocol::{Qmp, QmpEvent};

use crate::arch::{Arch, MachineConfig};

/// Minimum QEMU version. 7.1.0 added `allow_preconfig` to `blockdev-add`,
/// which qrab requires for QMP-driven dynamic block device configuration
/// during preconfig instead of using `-drive` CLI args.
pub const PRECONFIG_MIN_VERSION: (u32, u32, u32) = (7, 1, 0);

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Best-effort async removal of the three temp files qrab creates per VM.
/// Used by `VmProcess::cleanup`, by `VmProcess::Drop` when a tokio runtime
/// is available, and by `VmBuilder::spawn` pre-bail cleanup paths where
/// VmProcess has not yet been constructed.
async fn cleanup_temp_files_async(
    qmp_sock: &Path,
    serial_log: &Option<PathBuf>,
    vnc_sock: &Option<PathBuf>,
) {
    let _ = tokio::fs::remove_file(qmp_sock).await;
    if let Some(s) = serial_log {
        let _ = tokio::fs::remove_file(s).await;
    }
    if let Some(s) = vnc_sock {
        let _ = tokio::fs::remove_file(s).await;
    }
}

/// Best-effort sync removal of the three temp files qrab creates per VM.
/// Used only by `VmProcess::Drop` when no tokio runtime is available; the
/// async variant is preferred inside a runtime because blocking
/// `std::fs::remove_file` from a Drop firing in an async context would
/// stall the runtime thread.
fn cleanup_temp_files_sync(
    qmp_sock: &Path,
    serial_log: &Option<PathBuf>,
    vnc_sock: &Option<PathBuf>,
) {
    let _ = std::fs::remove_file(qmp_sock);
    if let Some(s) = serial_log {
        let _ = std::fs::remove_file(s);
    }
    if let Some(s) = vnc_sock {
        let _ = std::fs::remove_file(s);
    }
}

/// QEMU process handle with cleanup logic.
///
/// The `Child` is owned by a monitor task spawned at construction time;
/// that task awaits `child.wait()` and publishes the exit status on a
/// `watch` channel so callers can observe unexpected QEMU death
/// programmatically instead of waiting for the next QMP command to fail
/// cryptically. `cleanup()` signals the monitor to kill the child and
/// waits for it to finish. `Drop` is a best-effort fallback that triggers
/// the same shutdown signal without blocking.
struct VmProcess {
    shutdown: Option<oneshot::Sender<()>>,
    monitor: Option<JoinHandle<()>>,
    exit_watch: watch::Receiver<Option<ExitStatus>>,
    qmp_sock: PathBuf,
    serial_log: Option<PathBuf>,
    vnc_sock: Option<PathBuf>,
}

impl VmProcess {
    /// Spawn a monitor task that owns the `Child` and waits for it to exit.
    ///
    /// The task races `child.wait()` against a shutdown oneshot: if the
    /// oneshot fires first, the task kills the child and reaps it; if the
    /// child exits on its own, the task logs the exit status as an error
    /// and publishes it on the returned `watch::Receiver` so the user is
    /// not left hanging on a dead QMP connection.
    fn spawn_monitor(
        mut child: Child,
    ) -> (
        oneshot::Sender<()>,
        JoinHandle<()>,
        watch::Receiver<Option<ExitStatus>>,
    ) {
        let (tx, rx) = oneshot::channel::<()>();
        let (exit_tx, exit_rx) = watch::channel(None);
        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = rx => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                }
                res = child.wait() => {
                    match res {
                        Ok(status) => {
                            tracing::error!(
                                "QEMU process exited unexpectedly: {:?}",
                                status
                            );
                            let _ = exit_tx.send(Some(status));
                        }
                        Err(e) => tracing::error!(
                            "failed to wait on QEMU process: {}",
                            e
                        ),
                    }
                }
            }
        });
        (tx, handle, exit_rx)
    }

    /// Async cleanup: signal monitor to kill the child, wait for exit,
    /// remove temp files. Safe to call multiple times (idempotent).
    async fn cleanup(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(h) = self.monitor.take() {
            let _ = h.await;
        }
        cleanup_temp_files_async(&self.qmp_sock, &self.serial_log, &self.vnc_sock).await;
    }
}

impl Drop for VmProcess {
    fn drop(&mut self) {
        // Signal the monitor task to kill + reap the child. If we're inside
        // a tokio runtime the monitor task will continue running (detached)
        // and reap properly. If not, abort the handle — this drops the
        // `Child` inside the task which triggers `kill_on_drop(true)`, at
        // best effort with no reaper if there is no runtime.
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        let runtime = tokio::runtime::Handle::try_current().ok();
        if let Some(h) = self.monitor.take() {
            if runtime.is_none() {
                h.abort();
            }
        }
        // Prefer async file removal when a runtime is available: blocking on
        // `std::fs::remove_file` from within a Drop that fires inside an
        // async context would stall the tokio runtime thread. Fall back to
        // the sync path only when there is no runtime to dispatch to.
        if let Some(handle) = runtime {
            let qmp_sock = self.qmp_sock.clone();
            let serial_log = self.serial_log.clone();
            let vnc_sock = self.vnc_sock.clone();
            handle.spawn(async move {
                cleanup_temp_files_async(&qmp_sock, &serial_log, &vnc_sock).await;
            });
        } else {
            cleanup_temp_files_sync(&self.qmp_sock, &self.serial_log, &self.vnc_sock);
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
        // Validate the firmware path before touching any temp files or
        // spawning QEMU. Without this check a missing or directory path
        // only surfaces as a cryptic late failure from QEMU itself. The
        // check deliberately does not police relative or `..` components:
        // rejecting those is policy, not safety.
        if let Some(ref fw) = self.firmware {
            anyhow::ensure!(
                fw.is_file(),
                "firmware path {} is not a regular file",
                fw.display()
            );
        }

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
                let machine = self.arch.bios_machine().ok_or_else(|| {
                    anyhow::anyhow!(
                        "BIOS boot not supported for architecture {:?}",
                        self.arch
                    )
                })?;
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

        tracing::debug!(
            binary = %self.arch.binary().display(),
            args = %args.join(" "),
            "spawning QEMU"
        );

        let mut child = Command::new(self.arch.binary())
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        // Forward QEMU stderr to tracing. Without this, security warnings,
        // missing-firmware messages, and KVM init failures are silently
        // dropped. Best-effort: if the forwarder hits an I/O error we simply
        // stop; the task exits naturally when QEMU closes stderr on exit.
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                loop {
                    match lines.next_line().await {
                        Ok(Some(line)) => {
                            tracing::warn!(target: "qemu", "{}", line);
                        }
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
            });
        }

        for _ in 0..50 {
            if qmp_sock.exists() {
                break;
            }
            // If QEMU exits immediately (bad arg, missing binary, OOM), the
            // socket will never appear. Poll the child so we bail fast with
            // a useful exit status instead of waiting out the full 5s loop.
            if let Ok(Some(status)) = child.try_wait() {
                cleanup_temp_files_async(&qmp_sock, &serial_log, &vnc_sock).await;
                anyhow::bail!("QEMU exited before QMP socket appeared: {status}");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if !qmp_sock.exists() {
            // Mirror the pre-bail cleanup at the other `spawn` failure sites:
            // VmProcess is not yet constructed, so its Drop will not sweep the
            // temp files QEMU may have created. Remove them explicitly before
            // bailing so we do not leak sockets and logs in /tmp.
            cleanup_temp_files_async(&qmp_sock, &serial_log, &vnc_sock).await;
            anyhow::bail!("QMP socket did not appear: {}", qmp_sock.display());
        }

        // Restrict socket permissions to the current user: QEMU creates the
        // QMP socket with umask-dependent permissions, which on typical
        // systems leave it readable/writable by other local users. Tighten
        // to 0600 so only the owning user can connect. Same for the VNC
        // socket when a display was requested.
        #[cfg(unix)]
        {
            tokio::fs::set_permissions(
                &qmp_sock,
                std::fs::Permissions::from_mode(0o600),
            )
            .await?;
            if let Some(ref sock) = vnc_sock {
                if sock.exists() {
                    tokio::fs::set_permissions(
                        sock,
                        std::fs::Permissions::from_mode(0o600),
                    )
                    .await?;
                }
            }
        }

        // Connect QMP. On failure, VmProcess has not yet been constructed,
        // so its Drop will not sweep the temp files QEMU has already created.
        // Remove them explicitly before propagating the error so we do not
        // leak sockets and logs in /tmp.
        let (qmp, events) = match Qmp::new(&qmp_sock).await {
            Ok(pair) => pair,
            Err(e) => {
                cleanup_temp_files_async(&qmp_sock, &serial_log, &vnc_sock).await;
                return Err(e);
            }
        };

        if !self.display {
            let version = qapi::qemu_version();
            if version < PRECONFIG_MIN_VERSION {
                // VmProcess is not yet constructed, so its Drop will not
                // sweep the temp files QEMU has already created. Remove
                // them explicitly before bailing so we do not leak sockets
                // and logs in /tmp.
                cleanup_temp_files_async(&qmp_sock, &serial_log, &vnc_sock).await;
                anyhow::bail!(
                    "QEMU {}.{}.{} is below required {}.{}.{} for blockdev preconfig support",
                    version.0,
                    version.1,
                    version.2,
                    PRECONFIG_MIN_VERSION.0,
                    PRECONFIG_MIN_VERSION.1,
                    PRECONFIG_MIN_VERSION.2,
                );
            }
        }

        let (shutdown, monitor, exit_watch) = VmProcess::spawn_monitor(child);

        Ok(PreconfigVm {
            qmp,
            events,
            process: VmProcess {
                shutdown: Some(shutdown),
                monitor: Some(monitor),
                exit_watch,
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

    /// Watch channel observing the QEMU process exit status.
    ///
    /// The receiver starts with `None` and transitions to `Some(status)`
    /// exactly once, when the monitor task observes `child.wait()`
    /// returning. Callers can `.await` `changed()` or poll
    /// `.has_changed()` to detect an unexpected QEMU crash without
    /// waiting for a cryptic QMP command failure.
    pub fn exit_watch(&self) -> watch::Receiver<Option<ExitStatus>> {
        self.process.exit_watch.clone()
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

    /// Watch channel observing the QEMU process exit status.
    ///
    /// The receiver starts with `None` and transitions to `Some(status)`
    /// exactly once, when the monitor task observes `child.wait()`
    /// returning. Callers can `.await` `changed()` or poll
    /// `.has_changed()` to detect an unexpected QEMU crash without
    /// waiting for a cryptic QMP command failure.
    pub fn exit_watch(&self) -> watch::Receiver<Option<ExitStatus>> {
        self.process.exit_watch.clone()
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
