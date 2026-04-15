use anyhow::Result;
use std::os::fd::{AsRawFd, IntoRawFd};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
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
use crate::namespace::{InitConfig, KvmFd, NsHelper};

/// Minimum QEMU version. 7.2.0 added `-accel kvm,device=` support for KVM
/// fd passing, which qrab requires for namespace-isolated KVM access. Also
/// required for `allow_preconfig` on `blockdev-add`.
pub const PRECONFIG_MIN_VERSION: (u32, u32, u32) = (7, 2, 0);

/// Validate a memory size string for QEMU's `-m` option.
///
/// QEMU accepts decimal integers optionally followed by a single unit
/// suffix (`K`/`M`/`G`/`T`, case-insensitive) and an optional trailing
/// `B` (e.g. `"512M"`, `"2GB"`, `"1024"`). A bare integer is treated by
/// QEMU as MiB, which matches `-m`'s historical default.
///
/// This check catches common typos such as `"512MB "` with trailing
/// whitespace, `"2 G"` with internal whitespace, `"abc"`, and empty
/// strings, which would otherwise only surface as a late QEMU failure
/// after we have already created temp files and started spawning.
///
/// Intentionally permissive about suffixes `T`/`t`: QEMU's parser
/// accepts terabytes, so we do too.
fn validate_memory(mem: &str) -> Result<()> {
    if mem.is_empty() {
        anyhow::bail!("memory size is empty");
    }

    let bytes = mem.as_bytes();
    let mut i = 0;

    // Leading decimal digits are required.
    let digit_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digit_start {
        anyhow::bail!(
            "memory size {:?} must start with a decimal digit",
            mem
        );
    }

    // Optional single unit suffix.
    if i < bytes.len() {
        match bytes[i] {
            b'K' | b'k' | b'M' | b'm' | b'G' | b'g' | b'T' | b't' => i += 1,
            _ => anyhow::bail!(
                "memory size {:?} has invalid unit suffix (expected one of K/M/G/T)",
                mem
            ),
        }
    }

    // Optional trailing `B`/`b`.
    if i < bytes.len() && (bytes[i] == b'B' || bytes[i] == b'b') {
        i += 1;
    }

    if i != bytes.len() {
        anyhow::bail!(
            "memory size {:?} has trailing characters after valid prefix",
            mem
        );
    }

    Ok(())
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
///
/// The `_tmpdir` field holds the private temporary directory. It is never
/// read directly — `TempDir::drop` removes the directory and all contents
/// (QMP socket, VNC socket, serial log) automatically.
struct VmProcess {
    shutdown: Option<oneshot::Sender<()>>,
    monitor: Option<JoinHandle<()>>,
    exit_watch: watch::Receiver<Option<ExitStatus>>,
    _tmpdir: tempfile::TempDir,
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

    /// Async cleanup: signal monitor to kill the child, wait for exit.
    /// Temp files cleaned up automatically when `_tmpdir` drops.
    /// Safe to call multiple times (idempotent).
    async fn cleanup(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(h) = self.monitor.take() {
            let _ = h.await;
        }
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
        if let Some(h) = self.monitor.take() {
            if tokio::runtime::Handle::try_current().is_err() {
                h.abort();
            }
        }
        // _tmpdir drops here, removing the directory and all contents.
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
    accel: Option<Accel>,
    smp: Option<u32>,
    extra_args: Vec<String>,
}

/// QEMU accelerator selection for [`VmBuilder::accel`].
///
/// Maps to the `-accel` CLI flag. The default when no accelerator is
/// configured is TCG, matching qrab's pre-existing behavior; callers
/// that want host-CPU speed must opt in with `Kvm` or `KvmOrTcg`.
#[derive(Debug, Clone, Copy)]
pub enum Accel {
    /// Require KVM. Spawn fails if KVM is unavailable.
    Kvm,
    /// Use TCG (QEMU's software emulator). Slow but always available.
    Tcg,
    /// Prefer KVM, fall back to TCG if KVM is unavailable.
    KvmOrTcg,
}

impl VmBuilder {
    #[must_use]
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
            accel: None,
            smp: None,
            extra_args: Vec::new(),
        }
    }

    /// Use direct kernel boot (bypasses bootloader).
    #[must_use]
    pub fn kernel(mut self, kernel: PathBuf, initrd: PathBuf, append: &str) -> Self {
        self.kernel = Some(kernel);
        self.initrd = Some(initrd);
        self.append = Some(append.to_string());
        self
    }

    /// Enable serial output to a log file.
    #[must_use]
    pub fn serial(mut self) -> Self {
        self.serial = true;
        self
    }

    /// Enable VGA display with a VNC server on a Unix socket.
    #[must_use]
    pub fn display(mut self) -> Self {
        self.display = true;
        self
    }

    /// Don't exit QEMU on guest shutdown.
    #[must_use]
    pub fn no_shutdown(mut self) -> Self {
        self.no_shutdown = true;
        self
    }

    /// Override firmware path for UEFI boot.
    #[must_use]
    pub fn firmware(mut self, path: PathBuf) -> Self {
        self.firmware = Some(path);
        self
    }

    /// Select the QEMU accelerator (`-accel`).
    ///
    /// Defaults to TCG (software emulation) when not set. Callers that
    /// want host-speed execution must opt in via [`Accel::Kvm`] or
    /// [`Accel::KvmOrTcg`].
    #[must_use]
    pub fn accel(mut self, accel: Accel) -> Self {
        self.accel = Some(accel);
        self
    }

    /// Set the number of guest CPUs (`-smp cpus=N`).
    ///
    /// When not set, QEMU's own default (1) applies.
    #[must_use]
    pub fn smp(mut self, cpus: u32) -> Self {
        self.smp = Some(cpus);
        self
    }

    /// Add an extra QEMU argument.
    #[must_use]
    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.extra_args.push(a.into());
        self
    }

    /// Spawn QEMU inside namespace isolation via `qrab-init` and connect QMP.
    ///
    /// Returns a [`PreconfigVm`] handle for issuing configuration commands
    /// before guest execution. Call [`PreconfigVm::start()`] to begin
    /// execution.
    ///
    /// The spawn sequence:
    /// 1. Create private tmpdir for sockets/logs
    /// 2. Open `/dev/kvm` fd (based on accel mode)
    /// 3. Build QEMU args with KVM fd passing
    /// 4. Create sync + config pipes
    /// 5. Spawn `qrab-init` with `pre_exec` namespace setup
    /// 6. Write config JSON over pipe
    /// 7. Run `NsHelper` to set up uid/gid maps
    /// 8. Wait for QMP socket, connect, check version
    #[must_use = "spawning a VM without using the returned handle immediately kills it on drop"]
    pub async fn spawn(self) -> Result<PreconfigVm> {
        // Validate the firmware path before touching any temp files or
        // spawning QEMU. Without this check a missing or directory path
        // only surfaces as a cryptic late failure from QEMU itself.
        if let Some(ref fw) = self.firmware {
            anyhow::ensure!(
                fw.is_file(),
                "firmware path {} is not a regular file",
                fw.display()
            );
        }

        // --- Private tmpdir (Security #3 — no predictable socket paths) ---
        let tmpdir = tempfile::TempDir::new_in(std::env::temp_dir())?;
        let qmp_sock = tmpdir.path().join("qmp.sock");
        let serial_log = if self.serial {
            Some(tmpdir.path().join("serial.log"))
        } else {
            None
        };

        // --- KVM fd (Industry #49 — auto-detect) ---
        let kvm_fd = match self.accel {
            Some(Accel::Kvm) => Some(
                KvmFd::open()
                    .map_err(|e| anyhow::anyhow!("{e} — is process in kvm group?"))?,
            ),
            Some(Accel::Tcg) | None => None,
            Some(Accel::KvmOrTcg) => match KvmFd::open() {
                Ok(fd) => {
                    tracing::info!("KVM available, using hardware acceleration");
                    Some(fd)
                }
                Err(e) => {
                    tracing::warn!("KVM unavailable ({e}), falling back to TCG");
                    None
                }
            },
        };

        // --- Build QEMU args ---
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
                validate_memory(memory)?;
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
                validate_memory(memory)?;
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

        // KVM fd passing via -add-fd + -accel kvm,device=/dev/fdset/1
        // or plain -accel tcg when no KVM fd.
        if let Some(ref fd) = kvm_fd {
            args.extend([
                "-add-fd".into(),
                format!("fd={},set=1", fd.raw_fd()),
                "-accel".into(),
                "kvm,device=/dev/fdset/1".into(),
            ]);
        } else {
            args.extend(["-accel".into(), "tcg".into()]);
        }

        if let Some(cpus) = self.smp {
            args.extend(["-smp".into(), format!("cpus={}", cpus)]);
        }

        if let Some(cpu) = self.arch.cpu() {
            args.extend(["-cpu".into(), cpu.into()]);
        }

        let vnc_sock = if self.display {
            let sock = tmpdir.path().join("vnc.sock");
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

        // --- Pipes for namespace sync + config transport ---
        let (sync_parent_r, sync_child_w) = nix::unistd::pipe()
            .map_err(|e| anyhow::anyhow!("sync pipe (to parent): {e}"))?;
        let (ack_child_r, ack_parent_w) = nix::unistd::pipe()
            .map_err(|e| anyhow::anyhow!("ack pipe (to child): {e}"))?;
        let (cfg_r, cfg_w) = nix::unistd::pipe()
            .map_err(|e| anyhow::anyhow!("config pipe: {e}"))?;

        // Set CLOEXEC on parent-side pipe ends so they're closed in child
        // on exec. Without this, the child inherits the write end of the
        // config pipe and read_to_end() never sees EOF.
        use crate::namespace::set_fd_cloexec;
        set_fd_cloexec(sync_parent_r.as_raw_fd(), true)
            .map_err(|e| anyhow::anyhow!("set CLOEXEC sync_parent_r: {e}"))?;
        set_fd_cloexec(ack_parent_w.as_raw_fd(), true)
            .map_err(|e| anyhow::anyhow!("set CLOEXEC ack_parent_w: {e}"))?;
        set_fd_cloexec(cfg_w.as_raw_fd(), true)
            .map_err(|e| anyhow::anyhow!("set CLOEXEC cfg_w: {e}"))?;

        let cfg_r_raw = cfg_r.as_raw_fd();

        // --- Serialize InitConfig ---
        // Resolve QEMU binary to absolute path — execve in qrab-init
        // does not do PATH lookup.
        let qemu_binary = which::which(self.arch.binary())
            .map_err(|_| anyhow::anyhow!("QEMU binary not found: {}", self.arch.binary().display()))?;
        let config = InitConfig {
            qemu_binary,
            qemu_args: args.clone(),
            kvm_fd: kvm_fd.as_ref().map(|f| f.raw_fd() as i32),
        };

        tracing::debug!(
            binary = %self.arch.binary().display(),
            args = %args.join(" "),
            kvm = kvm_fd.is_some(),
            "spawning QEMU via qrab-init"
        );

        // --- Build pre_exec closure ---
        let pre_exec_fn = crate::namespace::make_pre_exec(
            sync_child_w.as_raw_fd(),
            ack_child_r.as_raw_fd(),
            kvm_fd.as_ref().map(|f| f.raw_fd()),
            cfg_r_raw,
        );

        // --- Spawn qrab-init ---
        let init_bin = crate::namespace::qrab_init_binary()?;
        let ack_r_raw = ack_child_r.as_raw_fd();
        let mut child = unsafe {
            Command::new(&init_bin)
                .arg(cfg_r_raw.to_string())
                .arg(ack_r_raw.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .pre_exec(pre_exec_fn)
                .spawn()?
        };

        // Close child-side pipe ends in parent.
        drop(sync_child_w);
        drop(ack_child_r);
        drop(cfg_r);

        // --- Write config JSON to pipe ---
        let config_json = serde_json::to_vec(&config)?;
        nix::unistd::write(&cfg_w, &config_json)
            .map_err(|e| anyhow::anyhow!("write config pipe: {e}"))?;
        drop(cfg_w);

        // --- NsHelper: uid/gid map setup ---
        // NsHelper takes ownership via from_raw_fd internally, so pass raw fds
        // and leak the OwnedFds to avoid double-close.
        let sync_parent_r_raw = sync_parent_r.into_raw_fd();
        let ack_parent_w_raw = ack_parent_w.into_raw_fd();
        NsHelper::run(sync_parent_r_raw, ack_parent_w_raw).await?;

        // --- Drop parent's KVM fd copy ---
        drop(kvm_fd);

        // Forward qrab-init stderr to tracing (QEMU stderr flows through).
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

        // --- Wait for QMP socket ---
        for _ in 0..50 {
            if tokio::fs::try_exists(&qmp_sock).await.unwrap_or(false) {
                break;
            }
            if let Ok(Some(status)) = child.try_wait() {
                // tmpdir drops on return, cleaning up.
                anyhow::bail!("qrab-init exited before QMP socket appeared: {status}");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if !tokio::fs::try_exists(&qmp_sock).await.unwrap_or(false) {
            anyhow::bail!("QMP socket did not appear: {}", qmp_sock.display());
        }

        // Restrict socket permissions to owner only.
        #[cfg(unix)]
        {
            tokio::fs::set_permissions(
                &qmp_sock,
                std::fs::Permissions::from_mode(0o600),
            )
            .await?;
            if let Some(ref sock) = vnc_sock {
                if tokio::fs::try_exists(sock).await.unwrap_or(false) {
                    tokio::fs::set_permissions(
                        sock,
                        std::fs::Permissions::from_mode(0o600),
                    )
                    .await?;
                }
            }
        }

        // --- Connect QMP ---
        let (qmp, events) = Qmp::new(&qmp_sock).await?;

        if !self.display {
            let version = qapi::qemu_version();
            if version < PRECONFIG_MIN_VERSION {
                anyhow::bail!(
                    "QEMU {}.{}.{} is below required {}.{}.{} \
                     (-accel kvm,device= and blockdev preconfig support)",
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
                _tmpdir: tmpdir,
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
    /// Path to the QMP Unix socket.
    #[must_use]
    pub fn qmp_sock(&self) -> &Path {
        &self.process.qmp_sock
    }

    /// Path to the serial log file, if serial was enabled.
    #[must_use]
    pub fn serial_log(&self) -> Option<&PathBuf> {
        self.process.serial_log.as_ref()
    }

    /// Path to the VNC Unix socket, if display was enabled.
    #[must_use]
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
    #[must_use]
    pub fn exit_watch(&self) -> watch::Receiver<Option<ExitStatus>> {
        self.process.exit_watch.clone()
    }

    /// Execute a preconfig-safe QMP command.
    ///
    /// Only commands implementing [`PreconfigSafe`](qapi::PreconfigSafe) can be
    /// used here. For raw QMP access without the trait bound, use
    /// `self.qmp.execute()` directly.
    #[must_use = "QMP command results should be checked for errors"]
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
    #[must_use = "dropping the returned Vm immediately kills the guest"]
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

    /// Shut down without starting guest execution (force-kill).
    ///
    /// For graceful shutdown, use [`graceful_shutdown()`](Self::graceful_shutdown).
    pub async fn shutdown(mut self) {
        self.qmp.shutdown().await;
        self.process.cleanup().await;
    }

    /// Graceful shutdown for preconfig state.
    ///
    /// In preconfig, no guest is running so there is no ACPI to signal.
    /// This just cleanly disconnects QMP and stops the process.
    pub async fn graceful_shutdown(mut self, _timeout: Duration) {
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
    /// Path to the QMP Unix socket.
    #[must_use]
    pub fn qmp_sock(&self) -> &Path {
        &self.process.qmp_sock
    }

    /// Path to the serial log file, if serial was enabled.
    #[must_use]
    pub fn serial_log(&self) -> Option<&PathBuf> {
        self.process.serial_log.as_ref()
    }

    /// Path to the VNC Unix socket, if display was enabled.
    #[must_use]
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
    #[must_use]
    pub fn exit_watch(&self) -> watch::Receiver<Option<ExitStatus>> {
        self.process.exit_watch.clone()
    }

    /// Execute any QMP command and return the typed response.
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        self.qmp.execute(cmd).await
    }

    // -- Ergonomic lifecycle methods --

    /// Pause guest execution (sends `stop`).
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn stop(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::Stop {}).await?;
        Ok(())
    }

    /// Resume guest execution (sends `cont`).
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn cont(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::Cont {}).await?;
        Ok(())
    }

    /// Reset the guest (sends `system_reset`).
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn reset(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::SystemReset {}).await?;
        Ok(())
    }

    /// Request guest powerdown via ACPI (sends `system_powerdown`).
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn powerdown(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::SystemPowerdown {}).await?;
        Ok(())
    }

    /// Send `quit` to QEMU, requesting the process to exit.
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn quit(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::Quit {}).await?;
        Ok(())
    }

    /// Force-kill shutdown: disconnect QMP, kill the process immediately.
    ///
    /// For graceful shutdown with ACPI powerdown, use [`graceful_shutdown()`](Self::graceful_shutdown).
    pub async fn shutdown(mut self) {
        self.qmp.shutdown().await;
        self.process.cleanup().await;
    }

    /// Graceful shutdown with escalation chain.
    ///
    /// 1. Send `system_powerdown` (ACPI power button) via QMP
    /// 2. Wait for SHUTDOWN event (up to `timeout`)
    /// 3. If timeout: send `quit` QMP command
    /// 4. Wait 5s for qrab-init to exit
    /// 5. If still alive: kill qrab-init (SIGKILL kills entire PID namespace)
    ///
    /// The existing `shutdown()` method remains as the immediate/forced variant.
    pub async fn graceful_shutdown(mut self, timeout: Duration) {
        // Step 1: ACPI powerdown
        let _ = self.qmp.execute(&qapi::commands::SystemPowerdown {}).await;

        // Step 2: Wait for SHUTDOWN event or timeout
        let deadline = tokio::time::Instant::now() + timeout;
        let mut got_shutdown = false;
        loop {
            match tokio::time::timeout_at(deadline, self.events.recv()).await {
                Ok(Some(evt)) if evt.event == "SHUTDOWN" => {
                    got_shutdown = true;
                    break;
                }
                Ok(Some(_)) => continue, // other event, keep waiting
                Ok(None) => break,        // channel closed
                Err(_) => break,          // timeout
            }
        }

        if got_shutdown {
            // Guest shut down gracefully — wait for process exit + cleanup
            self.qmp.shutdown().await;
            self.process.cleanup().await;
            return;
        }

        // Step 3: Send quit
        let _ = self.qmp.execute(&qapi::commands::Quit {}).await;

        // Step 4: Wait 5s for exit
        let quit_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            match tokio::time::timeout_at(quit_deadline, self.process.exit_watch.changed()).await {
                Ok(Ok(())) => {
                    if self.process.exit_watch.borrow().is_some() {
                        // Process exited
                        self.qmp.shutdown().await;
                        self.process.cleanup().await;
                        return;
                    }
                }
                _ => break, // timeout or channel closed
            }
        }

        // Step 5: Force kill
        self.qmp.shutdown().await;
        self.process.cleanup().await;
    }
}

#[cfg(test)]
mod tests {
    use super::validate_memory;

    #[test]
    fn validate_memory_accepts_bare_integer() {
        assert!(validate_memory("1024").is_ok());
        assert!(validate_memory("0").is_ok());
        assert!(validate_memory("4294967296").is_ok());
    }

    #[test]
    fn validate_memory_accepts_unit_suffix() {
        assert!(validate_memory("512K").is_ok());
        assert!(validate_memory("512M").is_ok());
        assert!(validate_memory("2G").is_ok());
        assert!(validate_memory("1T").is_ok());
    }

    #[test]
    fn validate_memory_accepts_lowercase_unit() {
        assert!(validate_memory("512k").is_ok());
        assert!(validate_memory("512m").is_ok());
        assert!(validate_memory("2g").is_ok());
        assert!(validate_memory("1t").is_ok());
    }

    #[test]
    fn validate_memory_accepts_b_suffix() {
        assert!(validate_memory("512MB").is_ok());
        assert!(validate_memory("2GB").is_ok());
        assert!(validate_memory("1024KB").is_ok());
        assert!(validate_memory("1TB").is_ok());
    }

    #[test]
    fn validate_memory_rejects_empty() {
        assert!(validate_memory("").is_err());
    }

    #[test]
    fn validate_memory_rejects_non_numeric() {
        assert!(validate_memory("abc").is_err());
        assert!(validate_memory("M").is_err());
        assert!(validate_memory("GB").is_err());
    }

    #[test]
    fn validate_memory_rejects_bad_unit() {
        // 'X' is not a valid unit suffix.
        assert!(validate_memory("512X").is_err());
        // 'P' (petabyte) is also not accepted by this validator. QEMU's
        // own parser allows it but no realistic VM uses it today.
        assert!(validate_memory("2P").is_err());
    }

    #[test]
    fn validate_memory_rejects_trailing_garbage() {
        assert!(validate_memory("512MiB").is_err());
        assert!(validate_memory("2Gibi").is_err());
        assert!(validate_memory("512M ").is_err());
        assert!(validate_memory("2GBB").is_err());
    }

    #[test]
    fn validate_memory_rejects_internal_whitespace() {
        assert!(validate_memory("2 G").is_err());
        assert!(validate_memory(" 2G").is_err());
    }

    #[test]
    fn validate_memory_rejects_fractional() {
        // QEMU does accept fractional sizes in some contexts, but the
        // manager's validator deliberately requires integer input to
        // match the type used in `MachineConfig::{Bios,Uefi}`.
        assert!(validate_memory("1.5G").is_err());
    }
}
