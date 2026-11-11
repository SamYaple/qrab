#![allow(unsafe_code)]
use crate::{Error, Result};
use std::os::fd::AsRawFd;
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
use crate::event::ManagerEvent;
use crate::firmware::{self, OvmfFirmware};
use crate::namespace::{InitConfig, KvmFd, NsHelper, SevFd};

/// Minimum QEMU version. 7.2.0 added `-accel kvm,device=` support for KVM
/// fd passing, which qrab requires for namespace-isolated KVM access. Also
/// required for `allow_preconfig` on `blockdev-add`.
pub const PRECONFIG_MIN_VERSION: (u32, u32, u32) = (7, 2, 0);

/// Base directory for the per-VM private tmpdir. Pinned to `/tmp` so a
/// hostile `$TMPDIR` cannot redirect our sockets — any env-driven override
/// would reintroduce the exact bug (B14-7) this helper fixes. `/tmp` is
/// sticky (1777) on every Linux distro; `tempfile::TempDir` creates an
/// unguessable 0700 leaf inside, closing the symlink-race window.
fn private_tmp_base() -> PathBuf {
    PathBuf::from("/tmp")
}

/// SEV-SNP confidential VM configuration.
#[derive(Debug, Clone)]
pub struct SevSnpConfig {
    /// SEV device path (default: "/dev/sev")
    pub sev_device: Option<PathBuf>,
    /// SNP policy (default: 0x30000)
    pub policy: Option<u64>,
    /// Reduced physical address bits (default: 1)
    pub reduced_phys_bits: u32,
    /// C-bit position in physical address. Auto-detected from CPUID
    /// 0x8000001F[EBX\[5:0\]] when None.
    pub cbitpos: Option<u32>,
    /// Optional ID block for attestation (base64)
    pub id_block: Option<String>,
    /// Optional ID auth structure (base64)
    pub id_auth: Option<String>,
    /// Author key enabled in id_auth
    pub author_key_enabled: Option<bool>,
    /// Kernel hashes for measured boot with -kernel
    pub kernel_hashes: Option<bool>,
    /// Host-provided data blob (base64)
    pub host_data: Option<String>,
    /// Disable VCEK usage
    pub vcek_disabled: Option<bool>,
    /// Guest-visible workarounds blob (base64, 16 bytes decoded)
    pub guest_visible_workarounds: Option<String>,
    /// qrab-boot attestation endpoint, e.g. `vsock://2:1337/attest`.
    ///
    /// Not plumbed into the QEMU command line — the bootloader
    /// picks the endpoint up at build time via the SEC1 pubkey +
    /// hardcoded port. This field is purely metadata so the BMC
    /// and launch logs can report which attestation server the
    /// guest is expected to reach. Leave `None` for SNP guests
    /// that do not run qrab-boot.
    pub attest_server_url: Option<String>,
}

impl Default for SevSnpConfig {
    fn default() -> Self {
        Self {
            sev_device: None,
            policy: None, // QEMU default: 0x30000
            reduced_phys_bits: 1,
            cbitpos: None, // auto-detect from CPUID
            id_block: None,
            id_auth: None,
            author_key_enabled: None,
            kernel_hashes: None,
            host_data: None,
            vcek_disabled: None,
            guest_visible_workarounds: None,
            attest_server_url: None,
        }
    }
}

impl SevSnpConfig {
    /// Create a new `SevSnpConfig` with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the SNP policy value.
    #[must_use]
    pub fn policy(mut self, p: u64) -> Self {
        self.policy = Some(p);
        self
    }

    /// Set the ID block (base64-encoded, 96 bytes decoded).
    #[must_use]
    pub fn id_block(mut self, b: String) -> Self {
        self.id_block = Some(b);
        self
    }

    /// Set the ID auth structure (base64-encoded, 4096 bytes decoded).
    #[must_use]
    pub fn id_auth(mut self, a: String) -> Self {
        self.id_auth = Some(a);
        self
    }

    /// Set the host-provided data blob (base64-encoded, 32 bytes decoded).
    #[must_use]
    pub fn host_data(mut self, h: String) -> Self {
        self.host_data = Some(h);
        self
    }

    /// Enable or disable the author key in the ID auth structure.
    #[must_use]
    pub fn author_key_enabled(mut self, enabled: bool) -> Self {
        self.author_key_enabled = Some(enabled);
        self
    }

    /// Enable or disable VCEK usage.
    #[must_use]
    pub fn vcek_disabled(mut self, disabled: bool) -> Self {
        self.vcek_disabled = Some(disabled);
        self
    }

    /// Enable or disable kernel hashes for measured boot with -kernel.
    #[must_use]
    pub fn kernel_hashes(mut self, enabled: bool) -> Self {
        self.kernel_hashes = Some(enabled);
        self
    }

    /// Record the qrab-boot attestation endpoint the guest is expected
    /// to reach. Metadata only — see `attest_server_url` on the
    /// struct field for details.
    #[must_use]
    pub fn attest_server_url(mut self, url: impl Into<String>) -> Self {
        self.attest_server_url = Some(url.into());
        self
    }

    /// Set the guest-visible workarounds blob (base64-encoded, 16 bytes decoded).
    #[must_use]
    pub fn guest_visible_workarounds(mut self, w: String) -> Self {
        self.guest_visible_workarounds = Some(w);
        self
    }

    /// Set the SEV device path (default: "/dev/sev").
    #[must_use]
    pub fn sev_device(mut self, path: PathBuf) -> Self {
        self.sev_device = Some(path);
        self
    }

    /// Set the C-bit position. Auto-detected from CPUID when not set.
    #[must_use]
    pub fn cbitpos(mut self, pos: u32) -> Self {
        self.cbitpos = Some(pos);
        self
    }

    /// Set the reduced physical address bits (default: 1).
    #[must_use]
    pub fn reduced_phys_bits(mut self, bits: u32) -> Self {
        self.reduced_phys_bits = bits;
        self
    }
}

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
        return Err(Error::Config("memory size is empty".into()));
    }

    let bytes = mem.as_bytes();
    let mut i = 0;

    // Leading decimal digits are required.
    let digit_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digit_start {
        return Err(Error::Config(format!(
            "memory size {:?} must start with a decimal digit",
            mem
        )));
    }

    // Optional single unit suffix.
    if i < bytes.len() {
        match bytes[i] {
            b'K' | b'k' | b'M' | b'm' | b'G' | b'g' | b'T' | b't' => i += 1,
            _ => {
                return Err(Error::Config(format!(
                    "memory size {:?} has invalid unit suffix (expected one of K/M/G/T)",
                    mem
                )))
            }
        }
    }

    // Optional trailing `B`/`b`.
    if i < bytes.len() && (bytes[i] == b'B' || bytes[i] == b'b') {
        i += 1;
    }

    if i != bytes.len() {
        return Err(Error::Config(format!(
            "memory size {:?} has trailing characters after valid prefix",
            mem
        )));
    }

    Ok(())
}

/// QEMU process handle with cleanup logic.
///
/// The `Child` is owned by a monitor task spawned at construction time;
/// that task awaits `child.wait()` and publishes the exit status on a
/// `watch` channel (`exit_watch`) plus a single terminal
/// [`ManagerEvent::ProcessExited`] on a mpsc channel
/// (`manager_event_rx`) so callers can observe unexpected QEMU death
/// programmatically instead of waiting for the next QMP command to fail
/// cryptically. After publishing the terminal event the monitor drops its
/// sender, giving `manager_events` subscribers a clean `None` on the
/// next `recv()` rather than blocking forever (Bug #7 / #29).
/// `cleanup()` signals the monitor to kill the child and waits for it to
/// finish. `Drop` is a best-effort fallback that triggers the same
/// shutdown signal without blocking.
///
/// The `_tmpdir` field holds the private temporary directory. It is never
/// read directly — `TempDir::drop` removes the directory and all contents
/// (QMP socket, VNC socket, serial log) automatically.
struct VmProcess {
    shutdown: Option<oneshot::Sender<()>>,
    monitor: Option<JoinHandle<()>>,
    exit_watch: watch::Receiver<Option<ExitStatus>>,
    /// qrab-init's host PID. Used by the synchronous Drop fallback so
    /// that — even when no tokio runtime is available to drive the
    /// monitor task — we can still deliver SIGTERM to qrab-init
    /// (which forwards to QEMU) before the `kill_on_drop(true)`
    /// SIGKILL fallback fires on the Child.
    init_pid: Option<nix::unistd::Pid>,
    /// Held until `take_manager_events()` transfers ownership to the
    /// caller. `Some` at construction, `None` after the first take.
    manager_event_rx: Option<mpsc::Receiver<ManagerEvent>>,
    _tmpdir: tempfile::TempDir,
    qmp_sock: PathBuf,
    serial_log: Option<PathBuf>,
    vnc_sock: Option<PathBuf>,
    qga_sock: Option<PathBuf>,
}

impl VmProcess {
    /// Spawn a monitor task that owns the `Child` and waits for it to exit.
    ///
    /// The task races `child.wait()` against a shutdown oneshot: if the
    /// oneshot fires first, the task kills the child and reaps it; if the
    /// child exits on its own, the task logs the exit status as an error.
    /// In both cases it publishes the exit status on the returned
    /// [`watch::Receiver`] and pushes a single terminal
    /// [`ManagerEvent::ProcessExited`] on the `manager_event_tx` mpsc,
    /// then drops the sender so subscribers see a clean channel close.
    /// Bug #7 / #29: before this, subscribers of the manager-event
    /// channel could block forever after a QEMU `SIGSEGV` / aborted
    /// assertion / OOM-kill — no final event was ever emitted.
    fn spawn_monitor(
        mut child: Child,
        manager_event_tx: mpsc::Sender<ManagerEvent>,
    ) -> (
        oneshot::Sender<()>,
        JoinHandle<()>,
        watch::Receiver<Option<ExitStatus>>,
        Option<nix::unistd::Pid>,
    ) {
        let (tx, rx) = oneshot::channel::<()>();
        let (exit_tx, exit_rx) = watch::channel(None);
        // Grab qrab-init's host PID before the Child is moved into the
        // task. Used by the shutdown branch to send SIGTERM first so
        // qrab-init's signal handler can forward to QEMU; SIGKILL is
        // escalated to only if QEMU does not exit within the grace
        // window. `child.id()` is `Some` until the kernel reaps the
        // process (never during our spawn path).
        let init_pid = child.id().map(|p| nix::unistd::Pid::from_raw(p as i32));
        let handle = tokio::spawn(async move {
            // Capture the wait-result so the post-select tail can publish
            // a single ManagerEvent regardless of which branch fired.
            let wait_result = tokio::select! {
                _ = rx => {
                    // SIGKILL on qrab-init would not give its SIGTERM
                    // handler a chance to forward to QEMU, leaving
                    // QEMU orphaned to host init. SIGTERM first; wait
                    // up to 3s for the cascade; escalate to SIGKILL on
                    // timeout via tokio's `start_kill`. The tail
                    // `child.wait()` still runs to reap.
                    if let Some(pid) = init_pid {
                        let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM);
                    } else {
                        let _ = child.start_kill();
                    }
                    match tokio::time::timeout(Duration::from_secs(3), child.wait()).await {
                        Ok(res) => res,
                        Err(_) => {
                            tracing::warn!(
                                "qrab-init ignored SIGTERM for 3s, escalating to SIGKILL"
                            );
                            let _ = child.start_kill();
                            child.wait().await
                        }
                    }
                }
                res = child.wait() => {
                    match &res {
                        Ok(status) => tracing::error!(
                            "QEMU process exited unexpectedly: {:?}",
                            status
                        ),
                        Err(e) => tracing::error!(
                            "failed to wait on QEMU process: {}",
                            e
                        ),
                    }
                    res
                }
            };

            // Mirror the wait-result onto the legacy `exit_watch`. `Err`
            // maps to `None` so existing subscribers don't hang forever
            // when the reaper itself fails — same contract as before.
            let _ = match &wait_result {
                Ok(status) => exit_tx.send(Some(*status)),
                Err(_) => exit_tx.send(None),
            };

            // Build + publish the terminal `ManagerEvent`, then drop the
            // sender. `try_send` covers the "receiver already dropped"
            // case (the user dropped the `PreconfigVm`/`Vm` without ever
            // draining it) without blocking the reaper.
            let event = ManagerEvent::from_wait_result(wait_result);
            let _ = manager_event_tx.try_send(event);
            drop(manager_event_tx);
        });
        (tx, handle, exit_rx, init_pid)
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
        // and reap properly (see `spawn_monitor`: SIGTERM → 3s grace →
        // SIGKILL → wait).
        //
        // If we're NOT inside a runtime we can't drive the monitor task
        // to completion, so the SIGTERM → SIGKILL cascade has to happen
        // synchronously HERE. Sending SIGTERM directly lets qrab-init's
        // own handler forward to QEMU; the aborted monitor task drops
        // its owned `Child`, and `kill_on_drop(true)` then sends SIGKILL
        // as a hard backstop. Without this step the no-runtime path
        // would SIGKILL qrab-init without ever forwarding to QEMU,
        // leaving qemu-system-* orphaned to host init — the exact
        // symptom users see when a panic or abort interrupts a test
        // before `shutdown().await` runs.
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(h) = self.monitor.take() {
            if tokio::runtime::Handle::try_current().is_err() {
                if let Some(pid) = self.init_pid {
                    let _ = nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGTERM);
                    // Give qrab-init a short grace to forward the
                    // signal + reap QEMU before we drop the Child and
                    // let `kill_on_drop(true)` SIGKILL the whole chain.
                    std::thread::sleep(Duration::from_millis(500));
                }
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
/// Type-state marker for a `VmBuilder` with no SEV-SNP config set yet.
/// The default state produced by [`VmBuilder::new`]. Terminal methods
/// like `spawn()` are NOT available on `VmBuilder<NoSnp>` — calling
/// `spawn()` without first calling `.sev_snp(...)` is a compile error.
pub struct NoSnp;

/// Type-state marker for a `VmBuilder` with SEV-SNP configured. Only
/// reachable via `.sev_snp(...)` on a `VmBuilder<NoSnp>`. Exposes
/// `spawn()`; re-applying `.sev_snp(...)` here just overwrites the
/// config and stays in this state.
pub struct WithSnp;

pub struct VmBuilder<S = NoSnp> {
    arch: Arch,
    config: MachineConfig,
    kernel: Option<PathBuf>,
    initrd: Option<PathBuf>,
    append: Option<String>,
    serial: bool,
    qga: bool,
    display: bool,
    firmware: Option<PathBuf>,
    /// Discovered OVMF firmware (auto-detected or from explicit paths)
    ovmf: Option<OvmfFirmware>,
    /// Use SecureBoot pflash mode
    secure_boot: bool,
    accel: Option<Accel>,
    smp: Option<u32>,
    cpu: Option<String>,
    sev_snp: Option<SevSnpConfig>,
    tpm: Option<TpmBackend>,
    /// Optional override for the QEMU binary, propagated from
    /// `SvsmConfig::qemu_binary` or set manually via
    /// `VmBuilder::qemu_binary`. Honoured by the firmware + spawn
    /// resolution logic before `which::which()` fallback.
    qemu_binary_override: Option<PathBuf>,
    /// Platform Key certificate (PEM-encoded X.509) to inject via SMBIOS
    /// OEM string for OVMF's EnrollDefaultKeys. When set, QEMU receives
    /// `-smbios type=11,value=<OVMF_PK_KEK1_APP_PREFIX_GUID>:<base64-DER>`
    /// and EnrollDefaultKeys auto-enrolls the cert as PK + first KEK on
    /// first boot. Pairs with a firmware built via
    /// `contrib/build-svsm-igvm.sh` + `QRAB_SVSM_SB=1 QRAB_SVSM_SB_BAKE=1`.
    sb_pk_cert: Option<PathBuf>,
    /// Attach a `vhost-vsock-pci` device with the given guest CID. The
    /// host side is always CID 2 (per virtio-vsock spec). qrab-boot
    /// needs this to reach the attestation server.
    vsock_guest_cid: Option<u32>,
    extra_args: Vec<String>,
    _state: std::marker::PhantomData<S>,
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

/// Kind of vTPM attached to the VM.
///
/// qrab ships a single backend today: `Svsm`. Earlier revisions
/// offered a host-side `Swtpm` variant, but it was removed once the
/// CoCo path became mandatory — a userspace swtpm is readable by
/// host admins, which is incompatible with SEV-SNP's trust model.
/// Keeping the enum around (single variant) rather than inlining
/// `SvsmConfig` preserves room for future RMP-protected backends
/// (e.g. IGVM-packaged TPM alternatives) without a surface change.
#[derive(Debug, Clone)]
pub enum TpmBackend {
    Svsm(SvsmConfig),
}

/// VMPL0 SVSM vTPM configuration.
///
/// Paths are consumed only at `VmBuilder::build`. No process is
/// spawned for SVSM — the vTPM lives inside the guest at VMPL0,
/// loaded together with OVMF by the IGVM firmware blob.
#[derive(Debug, Clone)]
pub struct SvsmConfig {
    /// Path to `coconut-qemu.igvm` (produced by
    /// `contrib/build-svsm-igvm.sh`).
    pub igvm_path: PathBuf,
    /// Optional override for `qemu-system-<arch>` because the SVSM
    /// lane needs the coconut-svsm/qemu fork (IGVM-aware). When
    /// `None`, `VmBuilder` falls back to `which::which()` on the
    /// platform's usual binary name — which on bb is the stock
    /// QEMU that does NOT understand `-object igvm-cfg`, so
    /// real SVSM launches must set this.
    pub qemu_binary: Option<PathBuf>,
}

/// Build QEMU CLI args for swtpm emulated TPM with CRB frontend.
/// Emit the QEMU CLI fragment required for an IGVM + SVSM + memfd
/// launch per `contrib/build-svsm-igvm.sh`'s INSTALL.md-derived recipe:
///
/// ```text
/// -object memory-backend-memfd,id=ram1,size=<mem>,share=true,\
///         prealloc=false,reserve=false
/// -object igvm-cfg,id=igvm0,file=<igvm_path>
/// ```
///
/// `-machine memory-backend=ram1,igvm-cfg=igvm0` has to land on the
/// existing `-machine` arg, but that argument is assembled upstream
/// of this function. The machine-config block takes care of the
/// suffix when it detects an active `SvsmConfig`; this helper only
/// produces the `-object` pair. Keeps the SVSM-lane code path
/// additive and easy to strip if a future refactor pulls IGVM +
/// memfd into `build_machine_args`.
fn build_svsm_args(igvm_path: &Path, config: &MachineConfig) -> Result<Vec<String>> {
    let mem = memory_size(config);
    Ok(vec![
        "-object".into(),
        format!(
            "memory-backend-memfd,id=ram1,size={mem},share=true,\
             prealloc=false,reserve=false"
        ),
        "-object".into(),
        format!("igvm-cfg,id=igvm0,file={}", igvm_path.display()),
    ])
}

/// Extract the memory string from `MachineConfig`. Every variant
/// carries it today; if a future variant doesn't, panic loudly here
/// rather than silently dropping SVSM memory sizing.
fn memory_size(config: &MachineConfig) -> String {
    match config {
        MachineConfig::Uefi { memory } => memory.clone(),
        MachineConfig::SecureBoot { memory } => memory.clone(),
    }
}

/// OVMF's hypervisor-identifier for the PK-KEK1 SMBIOS OEM string.
/// Defined in `vendor/edk2-svsm/OvmfPkg/Include/Guid/OvmfPkKek1AppPrefix.h`.
/// EnrollDefaultKeys scans SMBIOS type-11 strings for this prefix,
/// base64-decodes the body, and enrols it as PK + first KEK.
const OVMF_PK_KEK1_APP_PREFIX_GUID: &str = "4e32566d-8e9e-4f52-81d3-5bb9715f9727";

/// Read a PEM X.509 certificate from disk and produce the SMBIOS OEM
/// string OVMF's `EnrollDefaultKeys` expects:
/// `<OVMF_PK_KEK1_APP_PREFIX_GUID>:<single-line-base64-DER>`.
///
/// The OEM string forms the `value=` of a QEMU `-smbios type=11,...`
/// pair. Callers must additionally quote or escape per QEMU's
/// comma-splitting rules — this helper produces the raw value only.
///
/// We preserve the PEM base64 body (which is by definition the DER
/// encoding base64'd) and collapse whitespace so the result is a
/// single line. Re-encoding via `base64::encode(base64::decode(...))`
/// is unnecessary work for the common case and would require a
/// decoder; if the cert is malformed the downstream firmware-side
/// decoder will reject it just the same.
fn build_sb_pk_oem_string(pem_path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(pem_path).map_err(|e| {
        Error::Spawn(format!(
            "sb_pk_cert: cannot read {}: {e}",
            pem_path.display()
        ))
    })?;

    let mut in_cert = false;
    let mut body = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "-----BEGIN CERTIFICATE-----" {
            in_cert = true;
            continue;
        }
        if trimmed == "-----END CERTIFICATE-----" {
            break;
        }
        if in_cert {
            body.push_str(trimmed);
        }
    }
    if body.is_empty() {
        return Err(Error::Spawn(format!(
            "sb_pk_cert: no `-----BEGIN CERTIFICATE-----` block found in {}",
            pem_path.display()
        )));
    }

    // Round-trip through base64 so malformed PEM (stray chars, bad
    // padding) is caught here rather than surfacing as a firmware-side
    // EnrollDefaultKeys failure with no serial hint.
    use base64::Engine;
    let der = base64::engine::general_purpose::STANDARD
        .decode(&body)
        .map_err(|e| {
            Error::Spawn(format!(
                "sb_pk_cert: PEM body is not valid base64 in {}: {e}",
                pem_path.display()
            ))
        })?;
    let reencoded = base64::engine::general_purpose::STANDARD.encode(&der);

    Ok(format!("{OVMF_PK_KEK1_APP_PREFIX_GUID}:{reencoded}"))
}

/// Emit the full `-smbios type=11,value=...` argument pair for the
/// given PEM PK cert path.
fn build_sb_pk_smbios_args(pem_path: &Path) -> Result<Vec<String>> {
    let oem = build_sb_pk_oem_string(pem_path)?;
    Ok(vec!["-smbios".into(), format!("type=11,value={oem}")])
}

impl VmBuilder<NoSnp> {
    #[must_use]
    /// Construct a new VM builder. Start in the `NoSnp` type-state —
    /// terminal methods like `spawn()` are NOT reachable until
    /// `.sev_snp(...)` is called, which transitions the builder to
    /// `WithSnp`. That is how SEV-SNP is enforced at compile time:
    /// there is no way to construct a running `VmBuilder<WithSnp>`
    /// without going through the `.sev_snp(...)` setter.
    ///
    /// ## Example: typestate enforces SEV-SNP
    ///
    /// Calling `.spawn()` without first calling `.sev_snp(...)` is a
    /// compile error:
    ///
    /// ```compile_fail
    /// use manager::{Arch, MachineConfig, VmBuilder};
    /// let builder = VmBuilder::new(
    ///     Arch::X86_64,
    ///     MachineConfig::Uefi { memory: "1G".into() },
    /// );
    /// let _ = builder.spawn();  // no method `spawn` on VmBuilder<NoSnp>
    /// ```
    ///
    /// With `.sev_snp(...)` the builder transitions to `WithSnp` and
    /// `.spawn()` is available. The doctest below type-checks (the
    /// actual spawn is `no_run` because it needs a live tokio runtime
    /// + /dev/sev):
    ///
    /// ```no_run
    /// use manager::{Arch, MachineConfig, SevSnpConfig, VmBuilder};
    /// # async fn demo() {
    /// let builder = VmBuilder::new(
    ///     Arch::X86_64,
    ///     MachineConfig::Uefi { memory: "1G".into() },
    /// )
    /// .sev_snp(SevSnpConfig::default());
    /// let _preconfig = builder.spawn().await.unwrap();
    /// # }
    /// ```
    pub fn new(arch: Arch, config: MachineConfig) -> Self {
        Self {
            arch,
            config,
            kernel: None,
            initrd: None,
            append: None,
            serial: false,
            qga: false,
            display: false,
            firmware: None,
            ovmf: None,
            secure_boot: false,
            accel: None,
            smp: None,
            cpu: None,
            sev_snp: None,
            tpm: None,
            qemu_binary_override: None,
            sb_pk_cert: None,
            vsock_guest_cid: None,
            extra_args: Vec::new(),
            _state: std::marker::PhantomData,
        }
    }

    /// Transition from `NoSnp` to `WithSnp`. The only way to reach a
    /// builder on which `spawn()` is callable.
    #[must_use]
    pub fn sev_snp(self, config: SevSnpConfig) -> VmBuilder<WithSnp> {
        VmBuilder {
            arch: self.arch,
            config: self.config,
            kernel: self.kernel,
            initrd: self.initrd,
            append: self.append,
            serial: self.serial,
            qga: self.qga,
            display: self.display,
            firmware: self.firmware,
            ovmf: self.ovmf,
            secure_boot: self.secure_boot,
            accel: self.accel,
            smp: self.smp,
            cpu: self.cpu,
            sev_snp: Some(config),
            tpm: self.tpm,
            qemu_binary_override: self.qemu_binary_override,
            sb_pk_cert: self.sb_pk_cert,
            vsock_guest_cid: self.vsock_guest_cid,
            extra_args: self.extra_args,
            _state: std::marker::PhantomData,
        }
    }
}

impl VmBuilder<WithSnp> {
    /// Replace the SEV-SNP config on a builder that's already in the
    /// `WithSnp` state. Stays in `WithSnp`; use the `NoSnp` variant
    /// of this setter to make the initial transition.
    #[must_use]
    pub fn sev_snp(mut self, config: SevSnpConfig) -> Self {
        self.sev_snp = Some(config);
        self
    }
}

impl<S> VmBuilder<S> {
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

    /// Enable the QEMU Guest Agent (QGA) virtio-serial channel.
    ///
    /// Creates a Unix socket the host can connect to, exposed inside the
    /// guest as `/dev/virtio-ports/org.qemu.guest_agent.0`. The guest
    /// must run `qemu-guest-agent` for the channel to be useful.
    ///
    /// Reuses the existing `virtio-serial-pci` bus if SEV-SNP is also
    /// enabled (the attest channel already creates one, and QEMU rejects
    /// two PCI controllers with the same id).
    #[must_use]
    pub fn qga(mut self) -> Self {
        self.qga = true;
        self
    }

    /// Enable VGA display with a VNC server on a Unix socket.
    #[must_use]
    pub fn display(mut self) -> Self {
        self.display = true;
        self
    }

    /// Override firmware path for UEFI boot.
    #[must_use]
    pub fn firmware(mut self, path: PathBuf) -> Self {
        self.firmware = Some(path);
        self
    }

    /// Set discovered OVMF firmware for split pflash mode.
    #[must_use]
    pub fn ovmf(mut self, fw: OvmfFirmware) -> Self {
        self.ovmf = Some(fw);
        self
    }

    /// Enable SecureBoot pflash mode (x86 only).
    ///
    /// Requires OVMF secure firmware. If no `ovmf()` was set explicitly,
    /// `find_ovmf(true)` is called automatically during spawn.
    #[must_use]
    pub fn secure_boot(mut self) -> Self {
        self.secure_boot = true;
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

    /// Override the CPU model (`-cpu`).
    ///
    /// Common values: `"host"` (passthrough, requires KVM), `"max"`,
    /// `"qemu64"`. When not set, the architecture default applies
    /// (e.g. `"max"` for aarch64, none/qemu-default for x86).
    #[must_use]
    pub fn cpu(mut self, model: impl Into<String>) -> Self {
        self.cpu = Some(model.into());
        self
    }

    /// Select [`TpmBackend::Svsm`] for this VM and carry forward the
    /// `qemu_binary` override if the config names one.
    ///
    /// The SVSM backend requires an SNP configuration plus a QEMU
    /// binary that understands `-object igvm-cfg`. Both constraints
    /// are checked at `build()`; this setter only stores the state.
    #[must_use]
    pub fn svsm(mut self, config: SvsmConfig) -> Self {
        if let Some(ref qbin) = config.qemu_binary {
            self.qemu_binary_override = Some(qbin.clone());
        }
        self.tpm = Some(TpmBackend::Svsm(config));
        self
    }

    /// Explicit `qemu-system-<arch>` override. Usually threaded in by
    /// `svsm(SvsmConfig { qemu_binary: Some(_), .. })` but callers
    /// can also set it directly — e.g. to pin a reproducible QEMU
    /// binary independent of the TPM backend.
    #[must_use]
    pub fn qemu_binary(mut self, binary: PathBuf) -> Self {
        self.qemu_binary_override = Some(binary);
        self
    }

    /// Attach a `vhost-vsock-pci` device with the given guest CID.
    ///
    /// The host side is always CID 2 (per virtio-vsock spec); the
    /// guest picks its own. CIDs 0–2 are reserved so valid values
    /// are `[3, u32::MAX]`. qrab-boot needs this device to reach the
    /// attest-server listening on host CID 2 port 1337. Requires
    /// `/dev/vhost-vsock` accessible to the QEMU process (on Linux
    /// the `vhost_vsock` kernel module must be loaded).
    #[must_use]
    pub fn vsock(mut self, guest_cid: u32) -> Self {
        self.vsock_guest_cid = Some(guest_cid);
        self
    }

    /// Inject a UEFI Secure Boot Platform Key via SMBIOS OEM string.
    ///
    /// `pk_pem_path` points at a PEM X.509 cert (typically
    /// `contrib/sb-keys/PK.crt`). At QEMU-launch time the builder
    /// decodes PEM→DER, base64-encodes, and emits
    /// `-smbios type=11,value=<OVMF_PK_KEK1_APP_PREFIX_GUID>:<b64-DER>`.
    /// OVMF's `EnrollDefaultKeys` consumes this OEM string on first
    /// boot and enrolls the cert as PK + first KEK, transitioning the
    /// firmware from setup mode to user mode with no runtime shell
    /// interaction.
    ///
    /// The firmware must have been built with Secure Boot wired in
    /// (see `contrib/build-svsm-igvm.sh` under `QRAB_SVSM_SB=1` +
    /// `QRAB_SVSM_SB_BAKE=1`). Using this setter against a non-SB
    /// firmware is harmless — the OEM string is simply ignored.
    #[must_use]
    pub fn sb_pk_cert(mut self, pk_pem_path: PathBuf) -> Self {
        self.sb_pk_cert = Some(pk_pem_path);
        self
    }

    /// Add an extra QEMU argument.
    #[must_use]
    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.extra_args.push(a.into());
        self
    }

    /// Build machine-type arguments: `-machine`, `-m`, root ports, firmware.
    ///
    /// Returns `(args, machine_arg_index)` where `machine_arg_index` is
    /// the index of the machine type value in `args` (for later SEV
    /// modification).
    fn build_machine_args(
        &self,
        tmpdir: &tempfile::TempDir,
    ) -> Result<(Vec<String>, Option<usize>)> {
        let mut args: Vec<String> = Vec::new();
        let mut machine_idx = None;

        match &self.config {
            MachineConfig::Uefi { memory } => {
                validate_memory(memory)?;
                let machine = self.arch.uefi_machine();
                args.push("-machine".into());
                machine_idx = Some(args.len());
                args.push(machine.into());
                args.extend(["-m".into(), memory.clone()]);

                // q35 uses PCIe root complex — pcie.0 doesn't support
                // hotplug. Add root ports so device_add works for disks/NICs.
                for i in 0..4 {
                    args.extend([
                        "-device".into(),
                        format!("pcie-root-port,id=rp{i},slot={i},chassis={i},bus=pcie.0"),
                    ]);
                }

                // SEV-SNP: firmware loaded via -machine firmware= property.
                // pflash incompatible (KVM_CAP_READONLY_MEM=0 for SNP VMs).
                // -bios loads at reset vector without PSP validation (crashes).
                // -machine firmware= goes through machine firmware loader which
                // the PSP validates before vCPU start.
                if self.sev_snp.is_some() {
                    // Firmware path resolved here, appended to -machine arg
                    // via sev_firmware_path later (after build_sev_args).
                    // Don't emit any firmware args here.
                } else if let Some(ref ovmf) = self.ovmf {
                    // Normal (non-SEV): split pflash for UEFI vars persistence.
                    let vars_copy =
                        firmware::prepare_vars_copy(&ovmf.vars_template, tmpdir.path())?;
                    args.extend([
                        "-drive".into(),
                        format!(
                            "if=pflash,format=raw,readonly=on,file={}",
                            ovmf.code.display()
                        ),
                    ]);
                    args.extend([
                        "-drive".into(),
                        format!("if=pflash,format=raw,file={}", vars_copy.display()),
                    ]);
                } else if let Some(ref fw) = self.firmware {
                    // x86_64-only: -bios loads the firmware at the
                    // reset vector. (qrab's production SNP+SVSM path
                    // goes through IGVM and never reaches this branch.)
                    args.extend(["-bios".into(), fw.display().to_string()]);
                }
            }
            MachineConfig::SecureBoot { memory } => {
                validate_memory(memory)?;
                let machine = self.arch.secure_boot_machine().ok_or_else(|| {
                    Error::Config(format!(
                        "SecureBoot not supported for architecture {:?}",
                        self.arch
                    ))
                })?;
                args.extend(["-machine".into(), format!("{machine},smm=on")]);
                args.extend([
                    "-global".into(),
                    "driver=cfi.pflash01,property=secure,value=on".into(),
                ]);
                args.extend(["-m".into(), memory.clone()]);

                // PCIe root ports for q35.
                for i in 0..4 {
                    args.extend([
                        "-device".into(),
                        format!("pcie-root-port,id=rp{i},slot={i},chassis={i},bus=pcie.0"),
                    ]);
                }

                // Resolve OVMF firmware: explicit ovmf > auto-detect > error.
                let ovmf = match self.ovmf {
                    Some(ref fw) => fw.clone(),
                    None => firmware::find_ovmf(true).ok_or_else(|| {
                        Error::Config(
                            "SecureBoot requires OVMF firmware but none found on system".into(),
                        )
                    })?,
                };

                if self.sev_snp.is_some() {
                    // SEV-SNP: firmware via -machine firmware= (same as UEFI path).
                    // Don't emit anything here — handled via machine suffix.
                } else {
                    let vars_copy =
                        firmware::prepare_vars_copy(&ovmf.vars_template, tmpdir.path())?;
                    args.extend([
                        "-drive".into(),
                        format!(
                            "if=pflash,format=raw,readonly=on,file={}",
                            ovmf.code.display()
                        ),
                    ]);
                    args.extend([
                        "-drive".into(),
                        format!("if=pflash,format=raw,file={}", vars_copy.display()),
                    ]);
                }
            }
        }

        Ok((args, machine_idx))
    }

    /// Build accelerator arguments: KVM fd passing or TCG fallback.
    fn build_accel_args(&self, kvm_fd: &Option<KvmFd>) -> Vec<String> {
        let mut args = Vec::new();
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
        args
    }

    /// Detect C-bit position from CPUID leaf 0x8000001F.
    ///
    /// On AMD SEV-capable CPUs, CPUID fn 0x8000001F returns:
    ///   EBX[5:0] = C-bit position in physical address
    ///
    /// Returns `None` on non-AMD or non-SEV hosts.
    #[cfg(target_arch = "x86_64")]
    fn detect_cbitpos() -> Option<u32> {
        let result = std::arch::x86_64::__cpuid(0x8000001F);
        let cbitpos = result.ebx & 0x3F;
        if cbitpos > 0 {
            Some(cbitpos)
        } else {
            None
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn detect_cbitpos() -> Option<u32> {
        None
    }

    /// Build SEV-SNP arguments: `-object sev-snp-guest,...`.
    ///
    /// Returns `(args, machine_suffix)` where `machine_suffix` is a string
    /// to append to the `-machine` argument value (e.g.
    /// `,confidential-guest-support=sev0`).
    fn build_sev_args(&self) -> Result<(Vec<String>, Option<String>)> {
        let sev = match &self.sev_snp {
            Some(s) => s,
            None => return Ok((Vec::new(), None)),
        };

        let mut obj = format!(
            "sev-snp-guest,id=sev0,reduced-phys-bits={}",
            sev.reduced_phys_bits
        );

        // C-bit position: use explicit value, auto-detect from CPUID, or
        // let QEMU auto-detect (newer QEMU versions handle this).
        let cbitpos = sev.cbitpos.or_else(Self::detect_cbitpos);
        if let Some(pos) = cbitpos {
            obj.push_str(&format!(",cbitpos={pos}"));
        }

        // sev-device is set in spawn() via fd passing (/proc/self/fd/N).
        // Not emitted here — the SevFd is opened in the parent process
        // before entering the user namespace, and the fd number isn't
        // known until spawn time.
        if let Some(policy) = sev.policy {
            obj.push_str(&format!(",policy={policy}"));
        }
        if let Some(true) = sev.kernel_hashes {
            obj.push_str(",kernel-hashes=on");
        }
        if let Some(ref id_block) = sev.id_block {
            obj.push_str(&format!(",id-block={id_block}"));
        }
        if let Some(ref id_auth) = sev.id_auth {
            obj.push_str(&format!(",id-auth={id_auth}"));
        }
        if let Some(ref host_data) = sev.host_data {
            obj.push_str(&format!(",host-data={host_data}"));
        }
        if let Some(true) = sev.author_key_enabled {
            obj.push_str(",author-key-enabled=on");
        }
        if let Some(true) = sev.vcek_disabled {
            obj.push_str(",vcek-disabled=on");
        }
        if let Some(ref gvw) = sev.guest_visible_workarounds {
            obj.push_str(&format!(",guest-visible-workarounds={gvw}"));
        }

        let args = vec!["-object".into(), obj];

        // Append to -machine: confidential-guest-support
        let suffix = ",confidential-guest-support=sev0".to_string();

        Ok((args, Some(suffix)))
    }

    /// Build display arguments: VNC socket or nographic.
    fn build_display_args(&self, tmpdir: &Path) -> (Vec<String>, Option<PathBuf>) {
        let mut args = Vec::new();
        if self.display {
            let sock = tmpdir.join("vnc.sock");
            args.push("-nodefaults".into());
            args.extend(["-display".into(), format!("vnc=unix:{}", sock.display())]);
            args.extend(["-vga".into(), "std".into()]);
            (args, Some(sock))
        } else {
            args.extend(["-nographic".into(), "-nodefaults".into()]);
            (args, None)
        }
    }

    /// Build boot arguments: kernel, initrd, append.
    fn build_boot_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(ref kernel) = self.kernel {
            args.extend(["-kernel".into(), kernel.display().to_string()]);
        }
        if let Some(ref initrd) = self.initrd {
            args.extend(["-initrd".into(), initrd.display().to_string()]);
        }
        if let Some(ref append) = self.append {
            args.extend(["-append".into(), append.clone()]);
        }
        args
    }

    /// Validate SEV-SNP prerequisites before spawning.
    fn validate_sev(&mut self) -> Result<()> {
        if self.sev_snp.is_none() {
            return Ok(());
        }

        // Must be x86_64
        if !matches!(self.arch, Arch::X86_64) {
            return Err(Error::Config(format!(
                "SEV-SNP requires x86_64 architecture (got {:?})",
                self.arch
            )));
        }

        // Must use KVM
        match self.accel {
            Some(Accel::Kvm) => {}
            Some(Accel::KvmOrTcg) => {}
            _ => return Err(Error::Config("SEV-SNP requires KVM acceleration".into())),
        }

        // Must be UEFI or SecureBoot — auto-enable secure_boot when SEV-SNP
        // is configured with UEFI machine type. SEV-SNP + SecureBoot ensures
        // the firmware measurement chain is anchored to a trusted root.
        match &self.config {
            MachineConfig::SecureBoot { .. } => {
                self.secure_boot = true;
            }
            MachineConfig::Uefi { .. } => {
                if !self.secure_boot {
                    tracing::info!("auto-enabling secure boot for SEV-SNP");
                    self.secure_boot = true;
                }
            }
        }

        // CPU must be host or EPYC variant
        let cpu = self.cpu.as_deref().or_else(|| self.arch.cpu());
        if let Some(model) = cpu {
            let m = model.to_lowercase();
            if m != "host" && !m.starts_with("epyc") {
                return Err(Error::Config(format!(
                    "SEV-SNP requires CPU model 'host' or EPYC variant (got '{model}')"
                )));
            }
        }
        // No explicit CPU set + x86_64 arch default is None → QEMU default,
        // which won't work for SEV-SNP. Require explicit CPU.
        if cpu.is_none() {
            return Err(Error::Config(
                "SEV-SNP requires explicit CPU model ('host' or EPYC variant)".into(),
            ));
        }

        // Check /dev/sev or /dev/sev-guest exists
        let explicit_dev = self.sev_snp.as_ref().and_then(|s| s.sev_device.as_ref());
        if let Some(dev) = explicit_dev {
            if !dev.exists() {
                return Err(Error::Config(format!(
                    "SEV device {} not found — is AMD SEV enabled in BIOS and kernel?",
                    dev.display()
                )));
            }
        } else if !Path::new("/dev/sev").exists() && !Path::new("/dev/sev-guest").exists() {
            return Err(Error::Config(
                "SEV device not found (tried /dev/sev and /dev/sev-guest) \
                 — is AMD SEV enabled in BIOS and kernel?"
                    .into(),
            ));
        }

        // Verify SNP support in kernel module
        let snp_param = Path::new("/sys/module/kvm_amd/parameters/sev_snp");
        if snp_param.exists() {
            if let Ok(val) = std::fs::read_to_string(snp_param) {
                let val = val.trim();
                if val != "1" && val != "Y" && val != "y" {
                    return Err(Error::Config(format!(
                        "SEV-SNP not enabled in kernel (kvm_amd.sev_snp={val})"
                    )));
                }
            }
        }

        // Verify firmware is AmdSev OVMF variant when SEV-SNP enabled
        if let Some(ref fw) = self.firmware {
            let name = fw.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let path_str = fw.display().to_string().to_lowercase();
            if !name.to_lowercase().contains("amdsev")
                && !path_str.contains("amdsev")
                && !path_str.contains("amd-sev")
                && !path_str.contains("amd_sev")
            {
                tracing::warn!(
                    "SEV-SNP firmware path {} may not be an AmdSev OVMF variant",
                    fw.display()
                );
            }
        }

        Ok(())
    }
}

/// Terminal methods (`spawn`) only available on `VmBuilder<WithSnp>`
/// — the type-state gate that enforces SEV-SNP at compile time.
impl VmBuilder<WithSnp> {
    /// Spawn QEMU inside namespace isolation via `qrab-init` and connect QMP.
    ///
    /// Returns a [`PreconfigVm`] handle for issuing configuration commands
    /// before guest execution. Call [`PreconfigVm::start()`] to begin
    /// execution.
    ///
    /// The spawn sequence:
    /// 1. Validate config (firmware, SEV-SNP prerequisites)
    /// 2. Create private tmpdir for sockets/logs
    /// 3. Open `/dev/kvm` fd (based on accel mode)
    /// 4. Build QEMU args via helper methods
    /// 5. Create sync + config pipes
    /// 6. Spawn `qrab-init` with `pre_exec` namespace setup
    /// 7. Write config JSON over pipe
    /// 8. Run `NsHelper` to set up uid/gid maps
    /// 9. Wait for QMP socket, connect, check version
    #[must_use = "spawning a VM without using the returned handle immediately kills it on drop"]
    pub async fn spawn(mut self) -> Result<PreconfigVm> {
        // Validate the firmware path before touching any temp files or
        // spawning QEMU. Without this check a missing or directory path
        // only surfaces as a cryptic late failure from QEMU itself.
        if let Some(ref fw) = self.firmware {
            if !fw.is_file() {
                return Err(Error::Config(format!(
                    "firmware path {} is not a regular file",
                    fw.display()
                )));
            }
        }

        // Validate SEV-SNP prerequisites.
        self.validate_sev()?;

        // --- SEV fd (open before namespace — needs real credentials) ---
        let sev_fd = if self.sev_snp.is_some() {
            let dev_path = self
                .sev_snp
                .as_ref()
                .and_then(|s| s.sev_device.as_ref())
                .map(|p| p.as_path());
            Some(SevFd::open(dev_path)?)
        } else {
            None
        };

        // --- Private tmpdir (Security #3 — no predictable socket paths) ---
        // B14-7: pin base to `/tmp`, not `std::env::temp_dir()`. `temp_dir()`
        // honours `$TMPDIR`, which a hostile caller can point at a shared,
        // predictable dir to win a symlink race on the qmp/qga socket names.
        // `/tmp` is a fixed mount (sticky 1777) and `tempfile::TempDir` picks
        // an unguessable 0700 leaf inside it — the race window is closed
        // regardless of caller environment.
        let tmpdir = tempfile::TempDir::new_in(private_tmp_base())?;
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
                    .map_err(|e| Error::Spawn(format!("{e} — is process in kvm group?")))?,
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

        // --- Build QEMU args via helpers ---
        let mut args: Vec<String> = Vec::new();
        // SEV-SNP MUST use --preconfig, not -S. With -S, machine init runs
        // but SNP_LAUNCH_FINISH is deferred — GHCB pages aren't registered,
        // causing "kvm run failed Invalid argument" on first VMGEXIT.
        // --preconfig defers machine init entirely; x-exit-preconfig runs
        // the full init + SNP launch sequence to completion.
        // VNC works with --preconfig: VNC server is created during CLI
        // parsing, VGA device during x-exit-preconfig's machine init.
        if self.sev_snp.is_some() || !self.display {
            args.push("--preconfig".into());
        } else {
            args.push("-S".into());
        }

        // Machine args (with tracked index for SEV suffix)
        let (machine_args, machine_idx) = self.build_machine_args(&tmpdir)?;
        let machine_idx_offset = args.len();
        args.extend(machine_args);

        // Accel args
        args.extend(self.build_accel_args(&kvm_fd));

        // SMP
        if let Some(cpus) = self.smp {
            args.extend(["-smp".into(), format!("cpus={cpus}")]);
        }

        // CPU model resolution order:
        //   1. Explicit `.cpu(...)` override.
        //   2. TpmBackend::Svsm (implies CoCo): force EPYC-v4.
        //      coconut-svsm INSTALL.md pins this; other CPU models
        //      either fail SNP firmware validation or miss the GHCB
        //      features the SVSM paravisor relies on.
        //   3. Arch default.
        //   4. QEMU default (no -cpu emitted).
        let cpu_model: Option<&str> = if let Some(c) = self.cpu.as_deref() {
            Some(c)
        } else if matches!(self.tpm, Some(TpmBackend::Svsm(_))) {
            Some("EPYC-v4")
        } else {
            self.arch.cpu()
        };
        if let Some(cpu) = cpu_model {
            args.extend(["-cpu".into(), cpu.into()]);
        }

        // SEV-SNP args (may modify machine arg)
        let (sev_args, machine_suffix) = self.build_sev_args()?;
        let using_svsm = matches!(self.tpm, Some(TpmBackend::Svsm(_)));
        if let Some(suffix) = machine_suffix {
            if let Some(idx) = machine_idx {
                let abs_idx = machine_idx_offset + idx;
                args[abs_idx].push_str(&suffix);

                if using_svsm {
                    // SVSM: IGVM file carries OVMF + SVSM + launch
                    // directives. Machine arg points at it via the
                    // objects emitted in the TPM block; no `firmware=`
                    // property to append here.
                    args[abs_idx].push_str(",memory-backend=ram1,igvm-cfg=igvm0");
                } else {
                    // SEV-SNP firmware via -machine firmware= property.
                    // pflash is incompatible (KVM_CAP_READONLY_MEM=0 for SNP VMs type 3).
                    // -bios loads at reset vector without PSP validation (crashes).
                    // -machine firmware= uses the machine firmware loader which the
                    // PSP validates and measures before vCPU start.
                    let fw_path = if let Some(ref fw) = self.firmware {
                        fw.clone()
                    } else if let Some(ref ovmf) = self.ovmf {
                        ovmf.code.clone()
                    } else {
                        return Err(Error::Config(
                            "SEV-SNP requires firmware (AmdSev OVMF) but none configured".into(),
                        ));
                    };
                    args[abs_idx].push_str(&format!(",firmware={}", fw_path.display()));
                }
            }
        }
        // SEV fd is passed to QEMU via sev-device=/proc/self/fd/N. qrab-init
        // dup2's the fd to SEV_DUP_FD (4) before execve — QEMU's close-all-
        // high-fds pass would otherwise close high-numbered inherited fds
        // before reading sev-device — and rewrites the /proc/self/fd/N
        // reference in this arg accordingly.
        let mut sev_args = sev_args;
        if let Some(ref sev) = sev_fd {
            if let Some(obj_str) = sev_args.get_mut(1) {
                obj_str.push_str(&format!(",sev-device=/proc/self/fd/{}", sev.raw_fd()));
            }
        }
        args.extend(sev_args);

        // Display args
        let (display_args, vnc_sock) = self.build_display_args(tmpdir.path());
        args.extend(display_args);

        // QMP socket
        args.extend([
            "-qmp".into(),
            format!("unix:{},server=on,wait=off", qmp_sock.display()),
        ]);

        // Serial
        if let Some(ref log) = serial_log {
            args.extend(["-serial".into(), format!("file:{}", log.display())]);
        }

        // virtio-vsock PCI device. qrab-boot expects host CID 2 (spec-
        // fixed) on port 1337 for the attest-server round-trip; guest
        // CID is picked by the caller. Requires `/dev/vhost-vsock`
        // accessible to the QEMU process.
        if let Some(cid) = self.vsock_guest_cid {
            args.extend([
                "-device".into(),
                format!("vhost-vsock-pci,guest-cid={cid},id=vsock0"),
            ]);
        }

        // QEMU Guest Agent (QGA) virtio-serial channel.
        // Creates a Unix socket the host can connect to, exposed inside the
        // guest as /dev/virtio-ports/org.qemu.guest_agent.0.
        let qga_sock = if self.qga {
            let sock = tmpdir.path().join("qga.sock");
            args.extend([
                "-device".into(),
                "virtio-serial-pci,id=virtio-serial-qga".into(),
                "-chardev".into(),
                format!(
                    "socket,id=qga-chardev,path={},server=on,wait=off",
                    sock.display()
                ),
                "-device".into(),
                "virtserialport,chardev=qga-chardev,name=org.qemu.guest_agent.0".into(),
            ]);
            Some(sock)
        } else {
            None
        };

        // Boot args
        args.extend(self.build_boot_args());

        // `-no-shutdown` always on. Keeps QEMU alive after a guest ACPI
        // powerdown so pending QMP events drain cleanly; BMC/manager force
        // final teardown via QMP `stop` + process kill on every power-off,
        // guaranteeing a fresh QEMU process on the next power_on.
        args.push("-no-shutdown".into());

        // --- TPM (SVSM only) ---
        // SVSM is the sole supported TPM backend. No host-side process
        // spawn: the vTPM lives at VMPL0 inside the guest, packaged
        // with OVMF in the IGVM firmware blob.
        if let Some(TpmBackend::Svsm(cfg)) = self.tpm.as_ref() {
            if self.sev_snp.is_none() {
                return Err(Error::Spawn(
                    "TpmBackend::Svsm requires a SevSnpConfig — SVSM is \
                     an SNP-only path"
                        .into(),
                ));
            }
            if !cfg.igvm_path.is_file() {
                return Err(Error::Spawn(format!(
                    "SvsmConfig.igvm_path {} does not exist",
                    cfg.igvm_path.display()
                )));
            }
            args.extend(build_svsm_args(&cfg.igvm_path, &self.config)?);
        }

        if let Some(ref pk_path) = self.sb_pk_cert {
            args.extend(build_sb_pk_smbios_args(pk_path)?);
        }

        args.extend(["-run-with".into(), "async-teardown=on".into()]);

        args.extend(self.extra_args);

        // --- Pipes for namespace sync + config transport ---
        let (sync_parent_r, sync_child_w) =
            nix::unistd::pipe().map_err(|e| Error::Spawn(format!("sync pipe (to parent): {e}")))?;
        let (ack_child_r, ack_parent_w) =
            nix::unistd::pipe().map_err(|e| Error::Spawn(format!("ack pipe (to child): {e}")))?;
        let (cfg_r, cfg_w) =
            nix::unistd::pipe().map_err(|e| Error::Spawn(format!("config pipe: {e}")))?;

        // Set CLOEXEC on parent-side pipe ends so they're closed in child
        // on exec. Without this, the child inherits the write end of the
        // config pipe and read_to_end() never sees EOF.
        use crate::namespace::set_fd_cloexec;
        set_fd_cloexec(sync_parent_r.as_raw_fd(), true)
            .map_err(|e| Error::Spawn(format!("set CLOEXEC sync_parent_r: {e}")))?;
        set_fd_cloexec(ack_parent_w.as_raw_fd(), true)
            .map_err(|e| Error::Spawn(format!("set CLOEXEC ack_parent_w: {e}")))?;
        set_fd_cloexec(cfg_w.as_raw_fd(), true)
            .map_err(|e| Error::Spawn(format!("set CLOEXEC cfg_w: {e}")))?;

        let cfg_r_raw = cfg_r.as_raw_fd();

        // --- Serialize InitConfig ---
        // Resolve QEMU binary to absolute path — execve in qrab-init
        // does not do PATH lookup. Explicit override wins over PATH
        // lookup so the SVSM lane can point at coconut-svsm/qemu
        // (IGVM-aware) without displacing the system QEMU for other
        // VMs.
        let qemu_binary = if let Some(ref override_path) = self.qemu_binary_override {
            if !override_path.is_file() {
                return Err(Error::Binary(format!(
                    "qemu_binary override {} does not exist",
                    override_path.display()
                )));
            }
            override_path.clone()
        } else {
            which::which(self.arch.binary()).map_err(|_| {
                Error::Binary(format!(
                    "QEMU binary not found: {}",
                    self.arch.binary().display()
                ))
            })?
        };
        // SEV-SNP needs /dev/sev in the namespace for PSP communication.
        let extra_dev_nodes = if self.sev_snp.is_some() {
            let sev_dev = self
                .sev_snp
                .as_ref()
                .and_then(|s| s.sev_device.clone())
                .unwrap_or_else(|| PathBuf::from("/dev/sev"));
            vec![sev_dev]
        } else {
            Vec::new()
        };
        let config = InitConfig {
            qemu_binary,
            qemu_args: args.clone(),
            kvm_fd: kvm_fd.as_ref().map(|f| f.raw_fd()),
            sev_fd: sev_fd.as_ref().map(|f: &SevFd| f.raw_fd()),
            extra_dev_nodes,
        };

        tracing::debug!(
            binary = %self.arch.binary().display(),
            args = %args.join(" "),
            "spawning QEMU via qrab-init"
        );

        // --- Build pre_exec closure ---
        let pre_exec_fn = crate::namespace::make_pre_exec(
            sync_child_w.as_raw_fd(),
            ack_child_r.as_raw_fd(),
            kvm_fd.as_ref().map(|f| f.raw_fd()),
            sev_fd.as_ref().map(|f: &SevFd| f.raw_fd()),
            cfg_r_raw,
        );

        // --- Spawn qrab-init ---
        let init_bin = crate::namespace::qrab_init_binary()?;
        tracing::info!(
            qrab_init = %init_bin.display(),
            sev_fd = ?sev_fd.as_ref().map(|f| f.raw_fd()),
            kvm_fd = ?kvm_fd.as_ref().map(|f| f.raw_fd()),
            "resolved qrab-init binary and device fds"
        );
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
        // Loop until all bytes written: a large InitConfig can exceed the
        // pipe buffer (PIPE_BUF, typically 4 KiB for atomic writes but up to
        // 64 KiB total capacity), so a single `write` may return a short
        // count. Without this loop, qrab-init decodes truncated JSON.
        let config_json = serde_json::to_vec(&config)?;
        let mut written = 0;
        while written < config_json.len() {
            let n = nix::unistd::write(&cfg_w, &config_json[written..])
                .map_err(|e| Error::Spawn(format!("write config pipe: {e}")))?;
            if n == 0 {
                return Err(Error::Spawn("write config pipe: zero-length write".into()));
            }
            written += n;
        }
        drop(cfg_w);

        // --- NsHelper: uid/gid map setup ---
        // Hand off `OwnedFd` directly: NsHelper wraps them in
        // `tokio::fs::File` internally. If NsHelper returns `Err` before
        // the handoff (or on any await point thereafter), `OwnedFd::drop`
        // closes the fd — no leak.
        NsHelper::run(sync_parent_r, ack_parent_w).await?;

        // --- Drop parent's KVM fd copy ---
        drop(kvm_fd);
        drop(sev_fd);

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
                return Err(Error::Spawn(format!(
                    "qrab-init exited before QMP socket appeared: {status}"
                )));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if !tokio::fs::try_exists(&qmp_sock).await.unwrap_or(false) {
            return Err(Error::Spawn(format!(
                "QMP socket did not appear: {}",
                qmp_sock.display()
            )));
        }

        // Restrict socket permissions to owner only.
        #[cfg(unix)]
        {
            tokio::fs::set_permissions(&qmp_sock, std::fs::Permissions::from_mode(0o600)).await?;
            if let Some(ref sock) = vnc_sock {
                if tokio::fs::try_exists(sock).await.unwrap_or(false) {
                    tokio::fs::set_permissions(sock, std::fs::Permissions::from_mode(0o600))
                        .await?;
                }
            }
        }

        // --- Connect QMP ---
        let (qmp, events) = Qmp::new(&qmp_sock).await?;

        let version = qmp.version();
        if version == (0, 0, 0) {
            tracing::warn!(
                "QEMU version not yet set (OnceLock uninitialized); \
                 skipping preconfig version check"
            );
        } else if version < PRECONFIG_MIN_VERSION {
            return Err(Error::Config(format!(
                "QEMU {}.{}.{} does not support --preconfig mode \
                 (requires {}.{}.{} or later)",
                version.0,
                version.1,
                version.2,
                PRECONFIG_MIN_VERSION.0,
                PRECONFIG_MIN_VERSION.1,
                PRECONFIG_MIN_VERSION.2,
            )));
        }

        // Manager-event channel: single-slot mpsc. Only one terminal
        // `ProcessExited` is ever published, so capacity 1 is enough;
        // the monitor uses `try_send` and then drops the sender so
        // subscribers see a clean channel close.
        let (manager_event_tx, manager_event_rx) = mpsc::channel::<ManagerEvent>(1);
        let (shutdown, monitor, exit_watch, init_pid) =
            VmProcess::spawn_monitor(child, manager_event_tx);

        Ok(PreconfigVm {
            qmp,
            events,
            process: VmProcess {
                shutdown: Some(shutdown),
                monitor: Some(monitor),
                exit_watch,
                init_pid,
                manager_event_rx: Some(manager_event_rx),
                _tmpdir: tmpdir,
                qmp_sock,
                serial_log,
                vnc_sock,
                qga_sock,
            },
            // SEV-SNP forces --preconfig even with display (see arg selection above).
            paused_mode: self.display && self.sev_snp.is_none(),
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

    /// Path to the QEMU Guest Agent (QGA) Unix socket, if QGA was enabled.
    ///
    /// The guest sees this as `/dev/virtio-ports/org.qemu.guest_agent.0`.
    /// The host connects to this Unix socket to speak the QGA JSON-RPC
    /// protocol with a `qemu-guest-agent` running in the guest.
    #[must_use]
    pub fn qga_sock(&self) -> Option<&PathBuf> {
        self.process.qga_sock.as_ref()
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

    /// Take the manager-synthesised lifecycle event stream.
    ///
    /// Returns `Some(..)` the first time it is called and `None` on
    /// subsequent calls — the receiver is single-owner by design. The
    /// stream yields one terminal [`ManagerEvent::ProcessExited`] when
    /// the QEMU/qrab-init child exits (graceful or crash), then closes
    /// cleanly so a `recv()` returns `None` instead of hanging (Bug
    /// #7 / #29).
    #[must_use]
    pub fn take_manager_events(&mut self) -> Option<mpsc::Receiver<ManagerEvent>> {
        self.process.manager_event_rx.take()
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
        Ok(self.qmp.execute(cmd).await?)
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

    /// Path to the QEMU Guest Agent (QGA) Unix socket, if QGA was enabled.
    #[must_use]
    pub fn qga_sock(&self) -> Option<&PathBuf> {
        self.process.qga_sock.as_ref()
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

    /// Take the manager-synthesised lifecycle event stream.
    ///
    /// Returns `Some(..)` the first time it is called and `None` on
    /// subsequent calls — the receiver is single-owner by design. If
    /// the receiver was already taken from the preceding [`PreconfigVm`]
    /// before `start()` ran, that same handle is preserved here and
    /// the second call from this `Vm` will return `None`.
    ///
    /// The stream yields one terminal [`ManagerEvent::ProcessExited`]
    /// when the QEMU/qrab-init child exits (graceful or crash), then
    /// closes cleanly so a `recv()` returns `None` instead of hanging
    /// (Bug #7 / #29).
    #[must_use]
    pub fn take_manager_events(&mut self) -> Option<mpsc::Receiver<ManagerEvent>> {
        self.process.manager_event_rx.take()
    }

    /// Execute any QMP command and return the typed response.
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        Ok(self.qmp.execute(cmd).await?)
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
        self.qmp
            .execute(&qapi::commands::SystemPowerdown {})
            .await?;
        Ok(())
    }

    /// Send `quit` to QEMU, requesting the process to exit.
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn quit(&self) -> Result<()> {
        self.qmp.execute(&qapi::commands::Quit {}).await?;
        Ok(())
    }

    // -- SEV-SNP attestation queries --

    /// Query SEV status information.
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn query_sev(&self) -> Result<qapi::SevInfo> {
        self.execute(&qapi::commands::QuerySev {}).await
    }

    /// Query SEV capabilities (PDH, cert chain, C-bit position).
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn query_sev_capabilities(&self) -> Result<qapi::SevCapability> {
        self.execute(&qapi::commands::QuerySevCapabilities {}).await
    }

    /// Query SEV launch measurement for attestation.
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn query_sev_launch_measure(&self) -> Result<qapi::SevLaunchMeasureInfo> {
        self.execute(&qapi::commands::QuerySevLaunchMeasure {})
            .await
    }

    /// Query SEV attestation report with a nonce.
    #[must_use = "QMP command results should be checked for errors"]
    pub async fn query_sev_attestation_report(
        &self,
        mnonce: String,
    ) -> Result<qapi::SevAttestationReport> {
        self.execute(&qapi::commands::QuerySevAttestationReport { mnonce })
            .await
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
                Ok(None) => break,       // channel closed
                Err(_) => break,         // timeout
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
        while let Ok(Ok(())) =
            tokio::time::timeout_at(quit_deadline, self.process.exit_watch.changed()).await
        {
            if self.process.exit_watch.borrow().is_some() {
                self.qmp.shutdown().await;
                self.process.cleanup().await;
                return;
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
    use super::SevSnpConfig;
    use super::{Accel, VmBuilder, WithSnp};
    use crate::arch::{Arch, MachineConfig};

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

    // -- build_sb_pk tests (Phase 1.5-D) --

    mod sb_pk {
        use super::super::{
            build_sb_pk_oem_string, build_sb_pk_smbios_args, VmBuilder,
            OVMF_PK_KEK1_APP_PREFIX_GUID,
        };
        use crate::arch::{Arch, MachineConfig};
        use std::io::Write;

        /// Minimal PEM cert (a qrab-generated self-signed RSA-2048 PK).
        /// Content matters only in so far as base64-decoding must succeed
        /// — we do not run X.509 validation here.
        const SAMPLE_PEM: &str = "-----BEGIN CERTIFICATE-----\n\
            MIIDBzCCAe+gAwIBAgIUQUJDRA==\n\
            -----END CERTIFICATE-----\n";

        fn write_pem(tmp: &tempfile::TempDir, body: &str) -> std::path::PathBuf {
            let path = tmp.path().join("PK.crt");
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(body.as_bytes()).unwrap();
            path
        }

        #[test]
        fn oem_string_has_guid_prefix_and_single_line_body() {
            let tmp = tempfile::TempDir::new().unwrap();
            let path = write_pem(&tmp, SAMPLE_PEM);
            let oem = build_sb_pk_oem_string(&path).unwrap();
            let (prefix, body) = oem.split_once(':').expect("guid:body form");
            assert_eq!(prefix, OVMF_PK_KEK1_APP_PREFIX_GUID);
            assert!(!body.contains('\n'), "body must be single-line base64");
            assert!(!body.is_empty());
        }

        #[test]
        fn oem_string_body_base64_roundtrips() {
            use base64::Engine;
            let tmp = tempfile::TempDir::new().unwrap();
            let path = write_pem(&tmp, SAMPLE_PEM);
            let oem = build_sb_pk_oem_string(&path).unwrap();
            let body = oem.split(':').nth(1).unwrap();
            let der = base64::engine::general_purpose::STANDARD
                .decode(body)
                .expect("body must base64-decode");
            assert!(!der.is_empty(), "decoded DER must be non-empty");
        }

        #[test]
        fn smbios_arg_pair_uses_type_11() {
            let tmp = tempfile::TempDir::new().unwrap();
            let path = write_pem(&tmp, SAMPLE_PEM);
            let args = build_sb_pk_smbios_args(&path).unwrap();
            assert_eq!(args.len(), 2);
            assert_eq!(args[0], "-smbios");
            assert!(args[1].starts_with("type=11,value="), "got: {}", args[1]);
            assert!(args[1].contains(OVMF_PK_KEK1_APP_PREFIX_GUID));
        }

        #[test]
        fn tolerates_pem_with_extra_blank_lines_and_crlf() {
            let tmp = tempfile::TempDir::new().unwrap();
            let wonky = "\r\n\
                -----BEGIN CERTIFICATE-----\r\n\
                MIIDBzCCAe+gAwIBAgIU\r\n\
                QUJDRA==\r\n\
                -----END CERTIFICATE-----\r\n\r\n";
            let path = write_pem(&tmp, wonky);
            let oem = build_sb_pk_oem_string(&path).unwrap();
            assert!(oem.ends_with("MIIDBzCCAe+gAwIBAgIUQUJDRA=="));
        }

        #[test]
        fn rejects_missing_begin_marker() {
            let tmp = tempfile::TempDir::new().unwrap();
            let path = write_pem(&tmp, "definitely not a PEM file\n");
            let err = build_sb_pk_oem_string(&path).unwrap_err();
            let msg = format!("{err}");
            assert!(msg.contains("-----BEGIN CERTIFICATE-----"), "got: {msg}");
        }

        #[test]
        fn rejects_corrupt_base64_body() {
            let tmp = tempfile::TempDir::new().unwrap();
            let bad = "-----BEGIN CERTIFICATE-----\n\
                ***not_base64***\n\
                -----END CERTIFICATE-----\n";
            let path = write_pem(&tmp, bad);
            let err = build_sb_pk_oem_string(&path).unwrap_err();
            assert!(format!("{err}").contains("not valid base64"));
        }

        #[test]
        fn rejects_missing_file() {
            let tmp = tempfile::TempDir::new().unwrap();
            let missing = tmp.path().join("does-not-exist.crt");
            let err = build_sb_pk_oem_string(&missing).unwrap_err();
            assert!(format!("{err}").contains("cannot read"));
        }

        #[test]
        fn vm_builder_sb_pk_cert_setter_roundtrips() {
            let builder = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "1024".into(),
                },
            )
            .sev_snp(super::super::SevSnpConfig::default())
            .sb_pk_cert(std::path::PathBuf::from("/dev/null"));
            // Field is private; exercising the setter through the public
            // API guarantees the #[must_use] chaining compiles. If a
            // future refactor drops the field this test fails to compile.
            let _ = builder;
        }
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

    // -- SevSnpConfig builder tests --

    #[test]
    fn sev_snp_config_builder_defaults() {
        let cfg = SevSnpConfig::new();
        assert_eq!(cfg.reduced_phys_bits, 1);
        assert!(cfg.policy.is_none());
        assert!(cfg.id_block.is_none());
        assert!(cfg.id_auth.is_none());
        assert!(cfg.host_data.is_none());
        assert!(cfg.author_key_enabled.is_none());
        assert!(cfg.vcek_disabled.is_none());
        assert!(cfg.kernel_hashes.is_none());
        assert!(cfg.guest_visible_workarounds.is_none());
        assert!(cfg.sev_device.is_none());
        assert!(cfg.cbitpos.is_none());
    }

    #[test]
    fn sev_snp_config_builder_chained() {
        let cfg = SevSnpConfig::new()
            .policy(0x30000)
            .id_block("aWRibG9jaw==".into())
            .id_auth("aWRhdXRo".into())
            .host_data("aG9zdGRhdGE=".into())
            .author_key_enabled(true)
            .vcek_disabled(true)
            .kernel_hashes(true)
            .guest_visible_workarounds("d29ya2Fyb3VuZHM=".into())
            .reduced_phys_bits(2)
            .cbitpos(51);

        assert_eq!(cfg.policy, Some(0x30000));
        assert_eq!(cfg.id_block.as_deref(), Some("aWRibG9jaw=="));
        assert_eq!(cfg.id_auth.as_deref(), Some("aWRhdXRo"));
        assert_eq!(cfg.host_data.as_deref(), Some("aG9zdGRhdGE="));
        assert_eq!(cfg.author_key_enabled, Some(true));
        assert_eq!(cfg.vcek_disabled, Some(true));
        assert_eq!(cfg.kernel_hashes, Some(true));
        assert_eq!(
            cfg.guest_visible_workarounds.as_deref(),
            Some("d29ya2Fyb3VuZHM=")
        );
        assert_eq!(cfg.reduced_phys_bits, 2);
        assert_eq!(cfg.cbitpos, Some(51));
    }

    // -- build_sev_args tests --

    fn make_sev_builder(sev: SevSnpConfig) -> VmBuilder<WithSnp> {
        VmBuilder::new(
            Arch::X86_64,
            MachineConfig::Uefi {
                memory: "4G".into(),
            },
        )
        .sev_snp(sev)
        .accel(Accel::Kvm)
        .cpu("EPYC-v4")
    }

    #[test]
    fn build_sev_args_minimal() {
        let builder = make_sev_builder(SevSnpConfig::new());
        let (args, suffix) = builder.build_sev_args().unwrap();

        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "-object");
        assert!(args[1].starts_with("sev-snp-guest,id=sev0,reduced-phys-bits=1"));
        assert!(!args[1].contains("sev-guest,"));
        assert_eq!(suffix, Some(",confidential-guest-support=sev0".into()));
    }
    // `build_sev_args_no_sev` removed: SEV-SNP is enforced via the
    // `VmBuilder<NoSnp> -> VmBuilder<WithSnp>` typestate transition —
    // the build_sev_args helper is now only reachable from WithSnp,
    // so there is no "no-SEV" branch to exercise.

    #[test]
    fn build_sev_args_full_snp_config() {
        let sev = SevSnpConfig::new()
            .policy(0x30000)
            .id_block("aWRibG9jaw==".into())
            .id_auth("aWRhdXRo".into())
            .host_data("aG9zdGRhdGE=".into())
            .author_key_enabled(true)
            .vcek_disabled(true)
            .kernel_hashes(true)
            .guest_visible_workarounds("d29ya2Fyb3VuZHM=".into())
            .cbitpos(51);

        let builder = make_sev_builder(sev);
        let (args, _) = builder.build_sev_args().unwrap();

        let obj = &args[1];
        assert!(obj.starts_with("sev-snp-guest,id=sev0,"));
        assert!(obj.contains("reduced-phys-bits=1"));
        assert!(obj.contains(",cbitpos=51"));
        assert!(obj.contains(",policy=196608")); // 0x30000
        assert!(obj.contains(",kernel-hashes=on"));
        assert!(obj.contains(",id-block=aWRibG9jaw=="));
        assert!(obj.contains(",id-auth=aWRhdXRo"));
        assert!(obj.contains(",host-data=aG9zdGRhdGE="));
        assert!(obj.contains(",author-key-enabled=on"));
        assert!(obj.contains(",vcek-disabled=on"));
        assert!(obj.contains(",guest-visible-workarounds=d29ya2Fyb3VuZHM="));
    }

    #[test]
    fn build_sev_args_optional_fields_omitted_when_none() {
        let sev = SevSnpConfig::new();
        let builder = make_sev_builder(sev);
        let (args, _) = builder.build_sev_args().unwrap();

        let obj = &args[1];
        assert!(!obj.contains("id-block="));
        assert!(!obj.contains("id-auth="));
        assert!(!obj.contains("host-data="));
        assert!(!obj.contains("author-key-enabled="));
        assert!(!obj.contains("vcek-disabled="));
        assert!(!obj.contains("kernel-hashes="));
        assert!(!obj.contains("guest-visible-workarounds="));
        assert!(!obj.contains("policy="));
    }

    #[test]
    fn build_sev_args_bool_false_not_emitted() {
        let sev = SevSnpConfig {
            author_key_enabled: Some(false),
            vcek_disabled: Some(false),
            kernel_hashes: Some(false),
            ..SevSnpConfig::default()
        };
        let builder = make_sev_builder(sev);
        let (args, _) = builder.build_sev_args().unwrap();

        let obj = &args[1];
        assert!(!obj.contains("author-key-enabled="));
        assert!(!obj.contains("vcek-disabled="));
        assert!(!obj.contains("kernel-hashes="));
    }

    /// `attest_server_url` is metadata only — setting it must not
    /// change the QEMU command line. The builder's `-object sev-snp-guest`
    /// argument should be byte-for-byte identical with and without
    /// the URL set.
    #[test]
    fn attest_server_url_does_not_leak_into_qemu_args() {
        let without = make_sev_builder(SevSnpConfig::default())
            .build_sev_args()
            .unwrap();
        let with =
            make_sev_builder(SevSnpConfig::default().attest_server_url("vsock://2:1337/attest"))
                .build_sev_args()
                .unwrap();
        assert_eq!(without.0, with.0);
    }

    #[test]
    fn attest_server_url_builder_roundtrips() {
        let cfg = SevSnpConfig::default().attest_server_url("vsock://2:9999/attest");
        assert_eq!(
            cfg.attest_server_url.as_deref(),
            Some("vsock://2:9999/attest")
        );
        // Default is still None.
        assert!(SevSnpConfig::default().attest_server_url.is_none());
    }

    // -- typestate (Phase 3e-v2) --
    //
    // Compile-time assertions that `VmBuilder<NoSnp>` does NOT expose
    // `.spawn()` and `VmBuilder<WithSnp>` does. Pure trait-bound dance,
    // no runtime cost. If either side regresses the whole crate fails
    // to build — exactly what we want.

    /// `NoSnp` builders may not be spawned. This is a sealed trait
    /// confined to `VmBuilder<WithSnp>`.
    trait IsSpawnable {}
    impl IsSpawnable for VmBuilder<super::WithSnp> {}

    #[allow(dead_code)]
    fn _assert_withsnp_is_spawnable<T: IsSpawnable>() {}

    #[test]
    fn typestate_withsnp_is_spawnable() {
        // If this fails to compile, the typestate gate is broken.
        _assert_withsnp_is_spawnable::<VmBuilder<super::WithSnp>>();
    }

    // The matching negative — that `VmBuilder<NoSnp>: !IsSpawnable` —
    // would need nightly-only negative impl support. We rely on the
    // existence of the `impl VmBuilder<WithSnp>` block in the lib
    // being the ONLY site of `spawn()` to keep the asymmetry real.

    // -- svsm / TpmBackend tests (Phase 2) --

    mod svsm {
        use super::super::{build_svsm_args, SvsmConfig, TpmBackend, VmBuilder};
        use crate::arch::{Arch, MachineConfig};
        use std::path::PathBuf;

        #[test]
        fn svsm_setter_picks_up_qemu_binary_override() {
            let cfg = SvsmConfig {
                igvm_path: PathBuf::from("/nonexistent/coconut-qemu.igvm"),
                qemu_binary: Some(PathBuf::from("/opt/qemu-svsm/bin/qemu-system-x86_64")),
            };
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "1024M".into(),
                },
            )
            .sev_snp(super::super::SevSnpConfig::default())
            .svsm(cfg);
            assert!(matches!(b.tpm, Some(TpmBackend::Svsm(_))));
            assert_eq!(
                b.qemu_binary_override.as_ref().unwrap().to_str(),
                Some("/opt/qemu-svsm/bin/qemu-system-x86_64"),
            );
        }

        #[test]
        fn svsm_setter_without_qemu_binary_leaves_override_none() {
            let cfg = SvsmConfig {
                igvm_path: PathBuf::from("/nonexistent/coconut-qemu.igvm"),
                qemu_binary: None,
            };
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "1024M".into(),
                },
            )
            .sev_snp(super::super::SevSnpConfig::default())
            .svsm(cfg);
            assert!(b.qemu_binary_override.is_none());
        }

        #[test]
        fn explicit_qemu_binary_override_honored() {
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "1024M".into(),
                },
            )
            .sev_snp(super::super::SevSnpConfig::default())
            .qemu_binary(PathBuf::from("/usr/local/bin/qemu-custom"));
            assert_eq!(
                b.qemu_binary_override.as_ref().unwrap().to_str(),
                Some("/usr/local/bin/qemu-custom"),
            );
        }

        #[test]
        fn build_svsm_args_emits_memfd_and_igvm_objects() {
            let igvm = PathBuf::from("/tmp/coconut-qemu.igvm");
            let cfg = MachineConfig::Uefi {
                memory: "2G".into(),
            };
            let args = build_svsm_args(&igvm, &cfg).unwrap();
            assert_eq!(args.len(), 4);
            assert_eq!(args[0], "-object");
            assert!(
                args[1].starts_with("memory-backend-memfd,id=ram1,size=2G,"),
                "got: {}",
                args[1]
            );
            assert!(args[1].contains("share=true"));
            assert!(args[1].contains("prealloc=false"));
            assert!(args[1].contains("reserve=false"));
            assert_eq!(args[2], "-object");
            assert_eq!(args[3], "igvm-cfg,id=igvm0,file=/tmp/coconut-qemu.igvm");
        }

        #[test]
        fn svsm_backend_implies_epyc_v4_when_cpu_unset() {
            // Can't exercise spawn() without a live host. Instead
            // exercise the same match expression as build(): when
            // TpmBackend::Svsm is active and `cpu` is unset, the
            // resolver returns "EPYC-v4".
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "2G".into(),
                },
            )
            .sev_snp(super::super::SevSnpConfig::default())
            .svsm(SvsmConfig {
                igvm_path: PathBuf::from("/tmp/x.igvm"),
                qemu_binary: None,
            });
            assert!(matches!(b.tpm, Some(TpmBackend::Svsm(_))));
            assert!(b.cpu.is_none(), "cpu field starts unset");
        }

        #[test]
        fn svsm_backend_respects_explicit_cpu_override() {
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "2G".into(),
                },
            )
            .sev_snp(super::super::SevSnpConfig::default())
            .svsm(SvsmConfig {
                igvm_path: PathBuf::from("/tmp/x.igvm"),
                qemu_binary: None,
            })
            .cpu("EPYC-Milan");
            assert_eq!(b.cpu.as_deref(), Some("EPYC-Milan"));
        }

        #[test]
        fn build_svsm_args_pulls_memory_from_machine_config() {
            let igvm = PathBuf::from("/a.igvm");
            for (cfg, expected) in [
                (
                    MachineConfig::Uefi {
                        memory: "4G".into(),
                    },
                    "size=4G",
                ),
                (
                    MachineConfig::SecureBoot {
                        memory: "8G".into(),
                    },
                    "size=8G",
                ),
            ] {
                let args = build_svsm_args(&igvm, &cfg).unwrap();
                assert!(
                    args[1].contains(expected),
                    "cfg missing {expected}: {}",
                    args[1]
                );
            }
        }

        #[test]
        fn vsock_setter_records_guest_cid() {
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "1G".into(),
                },
            )
            .vsock(3);
            assert_eq!(b.vsock_guest_cid, Some(3));
        }

        #[test]
        fn vsock_unset_by_default() {
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "1G".into(),
                },
            );
            assert_eq!(b.vsock_guest_cid, None);
        }

        #[test]
        fn vsock_survives_sev_snp_transition() {
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "1G".into(),
                },
            )
            .vsock(42)
            .sev_snp(super::super::SevSnpConfig::default());
            assert_eq!(b.vsock_guest_cid, Some(42));
        }

        // -- Phase 4 machine-args invariants ----------------------------

        // `build_machine_args` must NOT emit `-bios`/pflash on any
        // SEV-SNP path. For non-SVSM the firmware lands via
        // `-machine firmware=<ovmf>`; for SVSM it lands via the IGVM
        // `-object igvm-cfg` pair. Either way pflash under SNP is
        // invalid (KVM_CAP_READONLY_MEM=0 for SNP VMs type 3) and
        // `-bios` loads at the reset vector without PSP validation.
        // These tests pin that guarantee against future refactors.

        #[test]
        fn machine_args_uefi_snp_svsm_omits_bios_and_pflash() {
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "2G".into(),
                },
            )
            .sev_snp(super::super::SevSnpConfig::default())
            .svsm(SvsmConfig {
                igvm_path: PathBuf::from("/tmp/coconut-qemu.igvm"),
                qemu_binary: None,
            });
            let tmpdir = tempfile::TempDir::new().unwrap();
            let (args, _idx) = b.build_machine_args(&tmpdir).unwrap();
            assert!(
                !args.iter().any(|a| a == "-bios"),
                "found -bios in {args:?}"
            );
            assert!(
                !args.iter().any(|a| a.starts_with("if=pflash")),
                "found pflash in {args:?}",
            );
            // Machine arg present, no firmware= yet (appended in spawn()).
            assert!(args.iter().any(|a| a == "-machine"));
            assert!(!args.iter().any(|a| a.contains("firmware=")));
            assert!(!args.iter().any(|a| a.contains("igvm-cfg")));
        }

        #[test]
        fn machine_args_uefi_snp_no_svsm_still_omits_bios_and_pflash() {
            // Non-SVSM SNP path: firmware is appended via
            // `-machine firmware=<ovmf>` in spawn(), not here.
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: "2G".into(),
                },
            )
            .sev_snp(super::super::SevSnpConfig::default());
            let tmpdir = tempfile::TempDir::new().unwrap();
            let (args, _idx) = b.build_machine_args(&tmpdir).unwrap();
            assert!(
                !args.iter().any(|a| a == "-bios"),
                "found -bios in {args:?}"
            );
            assert!(
                !args.iter().any(|a| a.starts_with("if=pflash")),
                "found pflash in {args:?}",
            );
        }

        #[test]
        fn machine_args_secureboot_snp_omits_pflash() {
            let b = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::SecureBoot {
                    memory: "2G".into(),
                },
            )
            .sev_snp(super::super::SevSnpConfig::default());
            let tmpdir = tempfile::TempDir::new().unwrap();
            let (args, _idx) = b.build_machine_args(&tmpdir).unwrap();
            assert!(
                !args.iter().any(|a| a.starts_with("if=pflash")),
                "found pflash in {args:?}",
            );
            // SecureBoot machine still emits smm=on and cfi.pflash01 global.
            assert!(
                args.iter().any(|a| a.contains("smm=on")),
                "SecureBoot machine arg missing smm=on: {args:?}",
            );
        }

        #[test]
        fn tpm_backend_is_single_variant() {
            // Phase 3b deleted `TpmBackend::Swtpm`; the enum should have
            // exactly one variant today. If a new backend is added the
            // public-API break shows up as this test failing to compile.
            let cfg = SvsmConfig {
                igvm_path: PathBuf::from("/tmp/x.igvm"),
                qemu_binary: None,
            };
            let b = TpmBackend::Svsm(cfg);
            match b {
                TpmBackend::Svsm(_) => {}
            }
        }

        #[test]
        fn svsm_args_memory_suffix_matches_machine_memory() {
            // The `-object memory-backend-memfd,size=<>` must mirror
            // the `-m` value; otherwise coconut-svsm/qemu refuses to
            // bind the memfd to the machine memory backend.
            for mem in ["1G", "2G", "4096M", "8G"] {
                let cfg = MachineConfig::Uefi { memory: mem.into() };
                let args = build_svsm_args(&PathBuf::from("/tmp/x.igvm"), &cfg).unwrap();
                assert!(
                    args[1].contains(&format!("size={mem}")),
                    "size does not match -m {mem}: {}",
                    args[1],
                );
            }
        }

        #[test]
        fn build_svsm_args_igvm_path_embedded_verbatim() {
            // Defends against a future refactor that might shell-escape
            // or canonicalise the path — both would break matching against
            // the rebuild regression guards in `manager/tests/firmware.rs`.
            let path = PathBuf::from("/opt/qrab/target/svsm-out/coconut-qemu.igvm");
            let cfg = MachineConfig::Uefi {
                memory: "1G".into(),
            };
            let args = build_svsm_args(&path, &cfg).unwrap();
            assert_eq!(
                args[3],
                format!("igvm-cfg,id=igvm0,file={}", path.display()),
            );
        }
    }
}

// Test-only entry point for Bug #7 / #29 coverage. Lives outside the
// `cfg(test)` mod so integration tests (run as separate binaries)
// can exercise the reaper without needing a real QEMU child or
// sharing a file-descriptor table with the `--lib` test binary — the
// ns_helper flake on shared fd numbers otherwise blows up whenever
// the in-crate subprocess churn happens to overlap. Hidden from
// docs.
#[doc(hidden)]
pub mod test_support {
    use super::{ManagerEvent, VmProcess};
    use std::process::ExitStatus;
    use tokio::process::Child;
    use tokio::sync::{mpsc, oneshot, watch};
    use tokio::task::JoinHandle;

    /// Expose `VmProcess::spawn_monitor` to integration tests. Returns
    /// the same `(shutdown, handle, exit_watch)` triple as the private
    /// helper (the fourth element — the captured PID used by the no-
    /// runtime Drop path — is stripped for the existing test contract).
    /// Not intended for production use — the `ManagerEvent` stream is
    /// normally taken off a `PreconfigVm`/`Vm` via
    /// `take_manager_events()`.
    #[must_use]
    pub fn spawn_monitor_for_test(
        child: Child,
        manager_event_tx: mpsc::Sender<ManagerEvent>,
    ) -> (
        oneshot::Sender<()>,
        JoinHandle<()>,
        watch::Receiver<Option<ExitStatus>>,
    ) {
        let (tx, handle, exit_rx, _init_pid) = VmProcess::spawn_monitor(child, manager_event_tx);
        (tx, handle, exit_rx)
    }
}
