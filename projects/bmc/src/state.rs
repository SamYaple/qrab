use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinHandle;

use manager::arch::{Arch, MachineConfig};
use manager::vm::{Vm, VmBuilder};
use qapi_protocol::QmpEvent;

use crate::types::{
    AttestationState, BmcConfig, BmcEvent, BmcPush, BmcSnapshot, CpuInfoResponse, KvmInfoResponse,
    SevCapabilitiesResponse, SevStatusResponse, TpmStatusResponse, VmPhase,
};

const EVENT_HISTORY_CAP: usize = 1000;

/// Interval between server-side liveness frames on the SSE stream.
/// Browsers auto-reconnect an `EventSource` after a few seconds of
/// silence; 15s is comfortably under that while not flooding idle
/// connections.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

static BMC: OnceLock<Arc<BmcState>> = OnceLock::new();

/// Get the global BmcState, initializing with defaults if needed.
pub fn bmc() -> Arc<BmcState> {
    BMC.get_or_init(BmcState::new).clone()
}

/// Internal disk descriptor used by `attach_blockdevs` / `attach_devices`.
///
/// The user-facing `BmcConfig` no longer exposes disk knobs — every VM gets
/// the SNP Ubuntu overlay + cloud-init seed by default. This struct is the
/// internal handoff between the resolver and the QMP attach helpers.
#[derive(Clone)]
struct InternalDisk {
    file: PathBuf,
    format: InternalDiskFormat,
    read_only: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InternalDiskFormat {
    Qcow2,
    Raw,
}

/// Internal VM state. Not serializable — holds owned QEMU handles.
///
/// `Initializing` exists so `power_on` can release the `vm` mutex while
/// doing slow IO (600 MiB SHA-256 hash of the Ubuntu cloud image,
/// cloud-init seed generation, `qemu-img create`, QEMU spawn) without
/// blocking read-only `phase()` / dashboard polls. R15-3.
pub enum VmState {
    Off,
    Initializing,
    Running(Vm),
    ShuttingDown,
}

/// Result of [`BmcState::qga_sock_probe`]. Distinguishes three cases
/// a caller otherwise had to reconstruct by correlating two separate
/// lookups: VM not Running, VM Running but no QGA configured, and VM
/// Running with a live QGA socket path. Single lock, single decision.
#[derive(Debug)]
pub enum QgaSockProbe {
    NotRunning,
    NoQgaSocket,
    Ok(PathBuf),
}

/// RAII guard for the temporary qcow2 overlay created per power cycle.
///
/// `create_temp_overlay` writes a fresh `/tmp/qrab-overlay-<pid>-<ms>.qcow2`
/// on every `power_on`. Without a cleanup hook, each power cycle leaks one
/// file on disk. This guard tracks the path and removes it when dropped.
///
/// Removal is best-effort: if the file is already gone (user cleaned up,
/// tmpfs reboot, etc.) the error is logged and swallowed — we do NOT fail
/// teardown on a missing overlay.
pub(crate) struct OverlayGuard {
    path: PathBuf,
}

impl OverlayGuard {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for OverlayGuard {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.path) {
            Ok(()) => {
                tracing::debug!("removed temp overlay {:?}", self.path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Already gone — fine.
            }
            Err(e) => {
                tracing::warn!(
                    "failed to remove temp overlay {:?}: {e} (leaking)",
                    self.path
                );
            }
        }
    }
}

/// Static info cached once per Running VM session.
#[derive(Clone)]
pub struct CachedStaticInfo {
    pub cpu_info: CpuInfoResponse,
    pub kvm_info: KvmInfoResponse,
    pub sev_status: Option<SevStatusResponse>,
    pub sev_capabilities: Option<SevCapabilitiesResponse>,
    pub tpm_status: Option<TpmStatusResponse>,
}

/// Central BMC state shared across server functions.
///
/// VNC socket and serial log paths are NOT stored here separately. They live
/// inside the `VmState` (via `Vm` accessors). Reading them requires holding
/// the `vm` lock, which makes "is the VM running and does it have a VNC
/// socket?" an atomic question by construction.
pub struct BmcState {
    pub vm: Mutex<VmState>,
    /// Broadcast channel for every server-to-client push (SSE). Replaces
    /// the old `event_tx` which only carried raw `QmpEvent`s — now every
    /// frame is a typed `BmcPush` variant (dashboard slice, queue status,
    /// config change, QMP event, heartbeat). The SSE handler subscribes
    /// here; client-side polling loops are gone.
    pub push_tx: broadcast::Sender<BmcPush>,
    pub event_history: Arc<Mutex<VecDeque<QmpEvent>>>,
    pub config: Mutex<BmcConfig>,
    /// Cached CPU/KVM/security info for the currently running VM. These
    /// don't change for the lifetime of a QEMU process, so the dashboard
    /// polling path queries them once per Running session and reuses the
    /// cached values until the next force_off transition.
    /// `None` while Off / ShuttingDown.
    pub cached_static: Mutex<Option<CachedStaticInfo>>,
    /// Attestation state — populated after power_on when SEV-SNP is enabled.
    /// Mutable: `request_attestation_report` updates it at runtime.
    pub attestation: Mutex<AttestationState>,
    event_forwarder: Mutex<Option<JoinHandle<()>>>,
    /// Tracks the temp qcow2 overlay created by `create_temp_overlay` for
    /// the currently-running VM. `Some(_)` while Running; cleared (and the
    /// file removed via `OverlayGuard::drop`) on transition back to Off.
    overlay: Mutex<Option<OverlayGuard>>,
}

impl BmcState {
    pub fn new() -> Arc<Self> {
        let (push_tx, _) = broadcast::channel(256);

        // Heartbeat task: emit a `BmcPush::Heartbeat` every
        // `HEARTBEAT_INTERVAL` so idle SSE clients can detect dead
        // connections without any server-side state change.
        //
        // The task owns its own `push_tx` clone and runs detached for the
        // lifetime of the process. `send` is infallible-noop when there
        // are no subscribers.
        {
            let tx = push_tx.clone();
            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(HEARTBEAT_INTERVAL);
                // First tick fires immediately; skip it so the Snapshot
                // frame is always the first thing a new SSE sees.
                ticker.tick().await;
                loop {
                    ticker.tick().await;
                    let _ = tx.send(BmcPush::Heartbeat);
                }
            });
        }

        Arc::new(Self {
            vm: Mutex::new(VmState::Off),
            push_tx,
            event_history: Arc::new(Mutex::new(VecDeque::with_capacity(EVENT_HISTORY_CAP))),
            config: Mutex::new(BmcConfig::default()),
            cached_static: Mutex::new(None),
            attestation: Mutex::new(AttestationState::default()),
            event_forwarder: Mutex::new(None),
            overlay: Mutex::new(None),
        })
    }

    /// Read the VNC socket path while holding the vm lock. Returns Some only
    /// when a display-enabled VM is currently Running.
    pub async fn vnc_sock(&self) -> Option<PathBuf> {
        match &*self.vm.lock().await {
            VmState::Running(vm) => vm.vnc_sock().cloned(),
            _ => None,
        }
    }

    /// Read the serial log path while holding the vm lock.
    pub async fn serial_log(&self) -> Option<PathBuf> {
        match &*self.vm.lock().await {
            VmState::Running(vm) => vm.serial_log().cloned(),
            _ => None,
        }
    }

    /// Read the attestation socket path while holding the vm lock.
    /// Returns Some only when a SEV-SNP-enabled VM is currently Running.
    pub async fn attest_sock(&self) -> Option<PathBuf> {
        match &*self.vm.lock().await {
            VmState::Running(vm) => vm.attest_sock().cloned(),
            _ => None,
        }
    }

    /// Read the QGA (QEMU Guest Agent) socket path while holding the vm lock.
    /// Returns Some only when a QGA-enabled VM is currently Running.
    pub async fn qga_sock(&self) -> Option<PathBuf> {
        match &*self.vm.lock().await {
            VmState::Running(vm) => vm.qga_sock().cloned(),
            _ => None,
        }
    }

    /// Atomic probe that collapses "is the VM Running?" and "does it
    /// have a QGA socket?" into a single `vm` lock acquisition.
    /// Using two sequential lookups (Running check, then a separate
    /// [`Self::qga_sock`] call) opens a race window during shutdown:
    /// the first returns `Running`, the VM stops before the second,
    /// and the caller surfaces a "qga_sock not on VmProcess" failure
    /// even though the VM was fine half a ms earlier. B13-3.
    pub async fn qga_sock_probe(&self) -> QgaSockProbe {
        match &*self.vm.lock().await {
            VmState::Running(vm) => match vm.qga_sock().cloned() {
                Some(p) => QgaSockProbe::Ok(p),
                None => QgaSockProbe::NoQgaSocket,
            },
            _ => QgaSockProbe::NotRunning,
        }
    }

    /// Spawn the hardcoded SEV-SNP + SVSM-vTPM + KVM + UEFI x86_64 VM
    /// with the Ubuntu SNP cloud image and cloud-init seed. Transitions
    /// Off -> Running.
    ///
    /// The only knobs that come from `BmcConfig` are `memory` and `cpus`.
    /// Everything else is fixed: SEV-SNP (mandatory), SVSM vTPM at VMPL0
    /// (via `coconut-qemu-SB.igvm`; no host-side swtpm on the CoCo path),
    /// KVM with EPYC-v4 CPU, UEFI via the IGVM-embedded OVMF-SVSM-SB.fd
    /// (no separate `-machine firmware=` — the IGVM carries it), display
    /// enabled, Ubuntu SNP cloud image as the boot disk, cloud-init seed
    /// as the secondary disk.
    ///
    /// Internally the VM goes through a paused (--preconfig) state where
    /// QMP commands like blockdev-add and device_add are issued, then `cont`
    /// is sent. This intermediate state is never exposed to callers.
    ///
    /// Lock discipline (R15-3): the `vm` mutex is only held for the two
    /// short critical sections that flip state (`Off → Initializing` and
    /// `Initializing → Running`). All slow work in between — 600 MiB
    /// SHA-256 of the Ubuntu cloud image, cloud-init seed gen, qemu-img
    /// overlay creation, QEMU spawn, preconfig device attach, cont +
    /// early-boot probe — runs with the lock released so concurrent
    /// read-only paths (`phase`, dashboard polls, QMP queue status) see
    /// `Initializing` without blocking for seconds.
    pub async fn power_on(self: &Arc<Self>) -> Result<()> {
        // Phase 1: flip Off → Initializing under the lock so concurrent
        // power_on / power_off / shutdown bails cleanly. Hold briefly.
        {
            let mut vm_guard = self.vm.lock().await;
            if !matches!(*vm_guard, VmState::Off) {
                bail!("VM is not off");
            }
            *vm_guard = VmState::Initializing;
        }

        // Phase 2: all the slow work runs with the vm lock released.
        // `power_on_build` owns the Initializing → Running transition on
        // success; on failure we flip back to Off here so state never
        // stays stuck in Initializing after a returned error.
        match self.power_on_build().await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Reacquire lock to clear Initializing. No other writer
                // can race us: power_off / force_off / another power_on
                // all refuse to transition out of Initializing.
                *self.vm.lock().await = VmState::Off;
                Err(e)
            }
        }
    }

    /// Build + start the VM with the `vm` lock RELEASED. Only reacquires it
    /// for the final `Initializing → Running` flip. Split out of
    /// [`Self::power_on`] so the error-path state reset is a single
    /// top-level decision rather than repeated at every bail site. R15-3.
    async fn power_on_build(self: &Arc<Self>) -> Result<()> {
        // Defensive: clear any stale state from a prior power cycle.
        // These are independent locks, not the vm lock.
        self.event_history.lock().await.clear();
        *self.cached_static.lock().await = None;
        *self.attestation.lock().await = AttestationState::default();

        let config = self.config.lock().await.clone();

        // --- Hardcoded prerequisites ---
        // CoCo path: IGVM + IGVM-aware QEMU required. swtpm intentionally
        // absent — host userspace TPM is not acceptable under SEV-SNP.
        let svsm = manager::firmware::find_svsm_artifacts().ok_or_else(|| {
            anyhow::anyhow!(
                "SEV-SNP + SVSM profile requires target/svsm-out/coconut-qemu-SB.igvm \
                 and the coconut-svsm/qemu fork. Run `contrib/build-svsm-igvm.sh` \
                 (see issues/svsm-vtpm-integration.md) or set \
                 QRAB_SVSM_IGVM_PATH + QRAB_SVSM_QEMU_BIN."
            )
        })?;

        // SEV-SNP with Ubuntu needs ≥1G RAM (256M crashes during kernel
        // page conversions, 512M OOMs during cloud-init).
        let memory = {
            let mem = &config.memory;
            let needs_bump = mem.ends_with('M')
                && mem.trim_end_matches('M').parse::<u32>().unwrap_or(0) < 1024;
            if needs_bump {
                tracing::warn!(
                    "SEV-SNP: bumping memory from {} to 1G (minimum for Ubuntu guest)",
                    mem
                );
                "1G".to_string()
            } else {
                config.memory.clone()
            }
        };

        // Resolve disks: SNP overlay + cloud-init seed. Always present —
        // there is no "no disks" code path anymore.
        //
        // R15-3: the 600 MiB SHA-256 hash + qemu-img overlay create +
        // cloud-init seed gen happen here, BEFORE the QEMU spawn and
        // with the vm lock released. Prior to R15-3 the vm lock was held
        // across these awaits and concurrent `phase()` / dashboard polls
        // blocked for seconds waiting on the hash.
        //
        // The overlay is owned by an `OverlayGuard`: if power_on_build
        // returns early (disk attach, start, cont, or boot-validation
        // failure) the guard is dropped here and the temp file is
        // removed. On the success path we hand ownership to
        // `BmcState::overlay` and let the teardown path drop it at
        // power_off/force_off time.
        let base_image = ensure_ubuntu_snp_image().await?;
        let overlay_guard = OverlayGuard::new(create_temp_overlay(&base_image).await?);
        let seed = ensure_cloud_init_seed().await?;
        let disks = vec![
            InternalDisk {
                file: overlay_guard.path().to_path_buf(),
                format: InternalDiskFormat::Qcow2,
                read_only: false,
            },
            InternalDisk {
                file: seed,
                format: InternalDiskFormat::Raw,
                read_only: true,
            },
        ];

        let machine_config = MachineConfig::Uefi { memory };
        // Builder starts in `VmBuilder<NoSnp>`. Optional `.smp()` only
        // applies in that state — once we call `.sev_snp(...)` below
        // the type transitions to `VmBuilder<WithSnp>`, which cannot
        // be assigned back to a `VmBuilder<NoSnp>` slot. Keep the
        // pre-SNP mutation on a separate binding and consume it into
        // the WithSnp chain in one expression.
        let mut pre_snp = VmBuilder::new(Arch::X86_64, machine_config).serial().qga();
        if let Some(cpus) = config.cpus {
            pre_snp = pre_snp.smp(cpus);
        }
        // SEV-SNP requires EPYC CPU model — `-cpu host` exposes
        // CPUID features that fail SNP firmware validation.
        let mut paused_vm = pre_snp
            .accel(manager::vm::Accel::Kvm)
            .cpu("EPYC-v4")
            .display()
            .sev_snp(manager::vm::SevSnpConfig::new())
            .svsm(manager::vm::SvsmConfig {
                igvm_path: svsm.igvm,
                qemu_binary: Some(svsm.qemu),
            })
            .spawn()
            .await?;

        // Add block backends via QMP (preconfig-safe since QEMU 7.1.0).
        if let Err(e) = attach_blockdevs(&paused_vm.qmp, &disks).await {
            paused_vm.shutdown().await;
            return Err(e);
        }

        // SEV-SNP uses --preconfig: device_add must wait until after
        // x-exit-preconfig completes machine init.

        // Extract the real event receiver and replace with a dummy.
        let (dummy_tx, dummy_rx) = mpsc::channel(1);
        drop(dummy_tx);
        let real_rx = std::mem::replace(&mut paused_vm.events, dummy_rx);

        // Spawn event forwarder task. Forwarder wraps each QmpEvent in a
        // BmcPush::QmpEvent before rebroadcasting, and re-pushes dashboard
        // state on life-cycle-significant events.
        let state_clone = self.clone();
        let history = self.event_history.clone();
        let handle = tokio::spawn(async move {
            event_forwarder(real_rx, state_clone, history).await;
        });
        *self.event_forwarder.lock().await = Some(handle);

        // Start: sends x-exit-preconfig.
        tracing::info!("calling PreconfigVm::start()");
        let vm = match paused_vm.start().await {
            Ok(vm) => {
                tracing::info!("PreconfigVm::start() succeeded");
                vm
            }
            Err(e) => {
                // paused_vm was consumed by start(). Tear down forwarder
                // + caches. Outer `power_on` flips state back to Off.
                self.stop_forwarder().await;
                self.event_history.lock().await.clear();
                *self.cached_static.lock().await = None;
                return Err(anyhow::anyhow!(
                    "power_on: PreconfigVm::start failed: {e}"
                ));
            }
        };

        // x-exit-preconfig completed machine init + SNP launch.
        // QEMU is now paused. Attach devices (needs machine init), then cont.
        if let Err(e) = attach_devices(&vm.qmp, &disks).await {
            tracing::error!("device_add failed after x-exit-preconfig: {e}");
            vm.shutdown().await;
            self.stop_forwarder().await;
            self.event_history.lock().await.clear();
            *self.cached_static.lock().await = None;
            return Err(anyhow::anyhow!("device_add failed: {e}"));
        }

        // Always send cont — x-exit-preconfig leaves QEMU paused.
        tracing::info!("SEV-SNP: sending cont after x-exit-preconfig");
        if let Err(e) = vm.cont().await {
            tracing::error!("cont after x-exit-preconfig failed: {e}");
            vm.shutdown().await;
            self.stop_forwarder().await;
            self.event_history.lock().await.clear();
            *self.cached_static.lock().await = None;
            return Err(anyhow::anyhow!("cont failed: {e}"));
        }

        // SEV-SNP: wait for guest to survive early encrypted boot.
        // OVMF needs time for PSP validation, GHCB setup, encrypted
        // memory init. Dashboard polls fire query-sev immediately on
        // transition to Running — validate boot first so early crashes
        // are caught here instead of surprising dashboard polls.
        tracing::info!("SEV-SNP: waiting for guest to survive early boot...");
        tokio::time::sleep(Duration::from_secs(3)).await;
        match vm.qmp.execute(&qapi::commands::QueryStatus {}).await {
            Ok(status) => {
                if !status.running {
                    tracing::error!(
                        "SEV-SNP: guest not running after cont (status: {:?})",
                        status.status
                    );
                    vm.shutdown().await;
                    self.stop_forwarder().await;
                    self.event_history.lock().await.clear();
                    *self.cached_static.lock().await = None;
                    return Err(anyhow::anyhow!(
                        "SEV-SNP guest failed to boot — not running after cont"
                    ));
                }
                tracing::info!(
                    "SEV-SNP: guest survived early boot (status: {:?})",
                    status.status
                );
            }
            Err(e) => {
                tracing::error!("SEV-SNP: query-status failed after cont: {e}");
                tracing::error!(
                    "SEV-SNP: guest likely crashed during early boot — check dmesg for GHCB errors"
                );
                vm.shutdown().await;
                self.stop_forwarder().await;
                self.event_history.lock().await.clear();
                *self.cached_static.lock().await = None;
                return Err(anyhow::anyhow!("SEV-SNP boot failed: {e}"));
            }
        }

        // Final flip: Initializing → Running. We expect to see
        // `Initializing` here — if not, something unrelated mutated
        // state while the lock was released and we must not clobber it.
        {
            let mut vm_guard = self.vm.lock().await;
            if !matches!(*vm_guard, VmState::Initializing) {
                // Shouldn't happen: only power_on sets Initializing, and
                // take_running_vm refuses to transition out of it. Tear
                // down to avoid leaving a stray QEMU running on the host.
                vm.shutdown().await;
                self.stop_forwarder().await;
                self.event_history.lock().await.clear();
                *self.cached_static.lock().await = None;
                return Err(anyhow::anyhow!(
                    "power_on: vm state changed out of Initializing while \
                     building; aborting to avoid stray QEMU"
                ));
            }
            *vm_guard = VmState::Running(vm);
        }
        // Hand overlay ownership to BmcState. Dropped (and file removed)
        // in `final_stop_and_teardown` when the VM transitions back to Off.
        *self.overlay.lock().await = Some(overlay_guard);

        // SEV-SNP attestation queries (query-sev-launch-measure,
        // query-sev-capabilities) removed from power_on path.
        // query-sev-launch-measure is a legacy SEV command not supported
        // by SNP. These queries are available on-demand via the BMC
        // attestation panel server functions instead.

        Ok(())
    }

    /// Pause guest execution.
    pub async fn pause(&self) -> Result<()> {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Running(vm) => {
                vm.stop().await?;
                Ok(())
            }
            _ => bail!("VM is not running"),
        }
    }

    /// Resume guest execution.
    pub async fn resume(&self) -> Result<()> {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Running(vm) => {
                vm.cont().await?;
                Ok(())
            }
            _ => bail!("VM is not running"),
        }
    }

    /// Reset the guest.
    pub async fn reset(&self) -> Result<()> {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Running(vm) => {
                vm.reset().await?;
                Ok(())
            }
            _ => bail!("VM is not running"),
        }
    }

    /// Graceful power-off:
    ///
    /// 1. Send ACPI `system_powerdown` so the guest can quiesce.
    /// 2. Wait up to 5s for a `SHUTDOWN` event (QEMU stays alive thanks to
    ///    the always-on `-no-shutdown` CLI flag).
    /// 3. Final `stop` via QMP, short drain window for trailing events,
    ///    then kill the QEMU process and transition to Off.
    ///
    /// Every power cycle yields a fresh QEMU process — the BMC never
    /// "reuses" a stopped VM; the next `power_on` builds a new one.
    pub async fn power_off(&self) -> Result<()> {
        // Subscribe BEFORE sending powerdown so the SHUTDOWN event is never
        // missed due to a race between sending the command and receiving
        // the event.
        let mut events = self.push_tx.subscribe();

        let vm = match self.take_running_vm().await? {
            Some(vm) => vm,
            None => return Ok(()),
        };

        // ACPI powerdown — non-fatal on QMP failure; we still tear down.
        let _ = vm.powerdown().await;

        // Wait up to 5s for SHUTDOWN. With `-no-shutdown` the guest can
        // quiesce without killing QEMU; once we see SHUTDOWN (or time out)
        // we proceed to the forced stop + kill.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            match tokio::time::timeout_at(deadline, events.recv()).await {
                Ok(Ok(BmcPush::QmpEvent(ev))) if ev.event == "SHUTDOWN" => break,
                Ok(Ok(_)) => continue,
                _ => break,
            }
        }

        self.final_stop_and_teardown(vm).await;
        *self.vm.lock().await = VmState::Off;
        Ok(())
    }

    /// Immediate power-off: skip ACPI and go straight to the final-stop +
    /// teardown sequence.
    pub async fn force_off(&self) -> Result<()> {
        let vm = match self.take_running_vm().await? {
            Some(vm) => vm,
            None => return Ok(()),
        };
        self.final_stop_and_teardown(vm).await;
        *self.vm.lock().await = VmState::Off;
        Ok(())
    }

    /// Atomically take the running `Vm` out of state, transitioning to
    /// `ShuttingDown`. Returns `None` if state was `Off`; errors if state
    /// was already `ShuttingDown` or is mid-Initializing (R15-3).
    async fn take_running_vm(&self) -> Result<Option<Vm>> {
        let mut vm_guard = self.vm.lock().await;
        match std::mem::replace(&mut *vm_guard, VmState::ShuttingDown) {
            VmState::Running(vm) => Ok(Some(vm)),
            VmState::Off => {
                *vm_guard = VmState::Off;
                Ok(None)
            }
            VmState::Initializing => {
                // Restore and refuse — we can't tear down a Vm that isn't
                // built yet, and cancelling the in-flight power_on would
                // strand a spawned QEMU + swtpm. Caller retries once the
                // init task finishes (Running or Off).
                *vm_guard = VmState::Initializing;
                bail!("VM is still initializing; retry once it is running");
            }
            VmState::ShuttingDown => {
                *vm_guard = VmState::ShuttingDown;
                bail!("VM is already shutting down");
            }
        }
    }

    /// Issue QMP `stop` to halt vCPUs, wait briefly for trailing events to
    /// drain through the QMP event channel, then kill the QEMU process and
    /// reset BMC-side state. Does NOT flip the VmState back to Off — the
    /// caller must do so after acquiring the lock.
    async fn final_stop_and_teardown(&self, vm: Vm) {
        // Final stop via QMP — halts vCPUs so the guest can't emit further
        // events past this point. Best-effort; a dead QEMU just returns an
        // error and we continue to the kill.
        let _ = vm.execute(&qapi::commands::Stop {}).await;

        // Drain window: the read-loop is still alive and the event
        // forwarder is still pushing into history — give it a moment to
        // propagate anything queued before we rip the socket down.
        tokio::time::sleep(Duration::from_millis(250)).await;

        // Kill QEMU and tear down the QMP connection.
        vm.shutdown().await;

        // BMC-side cleanup.
        self.stop_forwarder().await;
        self.event_history.lock().await.clear();
        *self.cached_static.lock().await = None;
        *self.attestation.lock().await = AttestationState::default();
        // Drop the overlay guard — removes the temp qcow2 file from disk.
        // Best-effort: OverlayGuard::drop logs and swallows remove errors.
        *self.overlay.lock().await = None;
    }

    /// Get the current VM phase.
    pub async fn phase(&self) -> VmPhase {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Off => VmPhase::Off,
            VmState::Initializing => VmPhase::Initializing,
            VmState::Running(vm) => match vm.execute(&qapi::commands::QueryStatus {}).await {
                Ok(status) if !status.running => VmPhase::Paused,
                _ => VmPhase::Running,
            },
            VmState::ShuttingDown => VmPhase::ShuttingDown,
        }
    }

    /// Wait for the event forwarder task to finish, with a bounded grace
    /// period. The forwarder exits naturally when its mpsc rx closes (which
    /// happens after the QEMU child is killed and the QMP background task
    /// drops its sender). If it doesn't exit within 500ms, abort it as a
    /// safety net.
    async fn stop_forwarder(&self) {
        let handle = self.event_forwarder.lock().await.take();
        if let Some(handle) = handle {
            match tokio::time::timeout(Duration::from_millis(500), handle).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!("forwarder join error: {e}"),
                Err(_) => tracing::warn!("forwarder did not exit within 500ms"),
            }
        }
    }

    // ---------------------------------------------------------------
    // Push helpers
    //
    // Called by server fns and the event forwarder after any state
    // mutation. Broadcast is infallible when there are no subscribers —
    // heartbeat + any active SSE clients keep the channel alive.
    // ---------------------------------------------------------------

    /// Build a full `BmcSnapshot` — the first frame every new SSE client
    /// receives. Encapsulates the three slice builds so the SSE handler
    /// doesn't reach into internals.
    pub async fn snapshot(self: &Arc<Self>) -> BmcSnapshot {
        BmcSnapshot {
            dashboard: crate::server::vm_status::build_dashboard(self).await,
            queue: crate::server::qmp_exec::build_queue(self).await,
            config: self.config.lock().await.clone(),
            events: self.event_history_dto().await,
        }
    }

    /// Snapshot the QMP event history as serializable `BmcEvent` DTOs.
    pub async fn event_history_dto(&self) -> Vec<BmcEvent> {
        let history = self.event_history.lock().await;
        history
            .iter()
            .map(|e| BmcEvent {
                event: e.event.clone(),
                timestamp_secs: e.timestamp.seconds,
                timestamp_usecs: e.timestamp.microseconds,
                data: e.data.clone(),
            })
            .collect()
    }

    /// Rebuild the dashboard slice and broadcast it. Idempotent — safe
    /// to call after any mutation that might flip a field.
    pub async fn push_dashboard(self: &Arc<Self>) {
        let dash = crate::server::vm_status::build_dashboard(self).await;
        let _ = self.push_tx.send(BmcPush::Dashboard(dash));
    }

    /// Rebuild the QMP queue slice and broadcast it.
    pub async fn push_queue(self: &Arc<Self>) {
        let q = crate::server::qmp_exec::build_queue(self).await;
        let _ = self.push_tx.send(BmcPush::Queue(q));
    }

    /// Broadcast the current VM configuration.
    pub async fn push_config(&self) {
        let cfg = self.config.lock().await.clone();
        let _ = self.push_tx.send(BmcPush::Config(cfg));
    }
}

/// Forwarder: drains QMP events from the manager's mpsc, records them in
/// history, and republishes each as a `BmcPush::QmpEvent` so the SSE
/// stream stays push-only. Life-cycle-significant events (`STOP`,
/// `RESUME`, `RESET`, `SHUTDOWN`) also trigger a dashboard re-push so
/// the status badge flips without a poll.
async fn event_forwarder(
    mut rx: mpsc::Receiver<QmpEvent>,
    state: Arc<BmcState>,
    history: Arc<Mutex<VecDeque<QmpEvent>>>,
) {
    while let Some(event) = rx.recv().await {
        // Record in history first so anything reading it sees the event
        // even if the broadcast has no live subscribers.
        {
            let mut h = history.lock().await;
            h.push_back(event.clone());
            if h.len() > EVENT_HISTORY_CAP {
                h.pop_front();
            }
        }

        let dto = BmcEvent {
            event: event.event.clone(),
            timestamp_secs: event.timestamp.seconds,
            timestamp_usecs: event.timestamp.microseconds,
            data: event.data.clone(),
        };
        let _ = state.push_tx.send(BmcPush::QmpEvent(dto));

        // STOP/RESUME/RESET/SHUTDOWN flip the VM phase or clear guest
        // state the dashboard surfaces. Re-broadcast dashboard so the UI
        // flips immediately instead of on the next poll.
        if matches!(
            event.event.as_str(),
            "STOP" | "RESUME" | "RESET" | "SHUTDOWN" | "POWERDOWN"
        ) {
            state.push_dashboard().await;
        }
    }
}

/// Validate that a disk path is safe to pass to QEMU.
///
/// Canonicalizes the path and rejects traversal attempts (symlinks that
/// escape, `..` components, non-regular files). This prevents CWE-22
/// path traversal where an attacker could read arbitrary host files via
/// the QEMU block layer.
fn validate_disk_path(path: &Path) -> Result<PathBuf> {
    // Canonicalize resolves symlinks and `..` components.
    let canonical = path
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("disk path {:?} cannot be resolved: {e}", path))?;

    // Reject non-regular files (block devices, pipes, /dev/*, etc.)
    let meta = std::fs::metadata(&canonical)
        .map_err(|e| anyhow::anyhow!("cannot stat disk path {:?}: {e}", canonical))?;
    if !meta.is_file() {
        anyhow::bail!("disk path {:?} is not a regular file", canonical);
    }

    // Reject paths under sensitive directories
    let sensitive = ["/proc", "/sys", "/dev", "/etc/shadow", "/etc/passwd"];
    let canonical_str = canonical.to_string_lossy();
    for prefix in &sensitive {
        if canonical_str.starts_with(prefix) {
            anyhow::bail!(
                "disk path {:?} is under restricted directory {:?}",
                canonical,
                prefix
            );
        }
    }

    Ok(canonical)
}

/// Attach a list of disks to a paused (--preconfig) VM via QMP.
///
/// Add block backends for each disk: blockdev-add (file) + blockdev-add (format).
/// Preconfig-safe (blockdev-add has allow_preconfig since QEMU 7.1.0).
async fn attach_blockdevs(qmp: &qapi_protocol::Qmp, disks: &[InternalDisk]) -> Result<()> {
    use qapi::commands::BlockdevAdd;
    use qapi::{
        BlockdevDriver, BlockdevOptionsBranch, BlockdevOptionsFile, BlockdevOptionsQcow2,
        BlockdevRef,
    };

    for (idx, disk) in disks.iter().enumerate() {
        let validated_path =
            validate_disk_path(&disk.file).map_err(|e| anyhow::anyhow!("disk {idx}: {e}"))?;
        let file_node = format!("disk{idx}-file");
        let fmt_node = format!("disk{idx}");

        // 1. File backend
        qmp.execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::File),
            node_name: Some(file_node.clone()),
            read_only: Some(disk.read_only),
            u: Some(BlockdevOptionsBranch::File(BlockdevOptionsFile {
                filename: validated_path.display().to_string(),
                aio: None,
                aio_max_batch: None,
                drop_cache: None,
                locking: None,
                pr_manager: None,
                x_check_cache_dropped: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .map_err(|e| anyhow::anyhow!("blockdev-add file backend (disk{idx}): {e:#}"))?;

        // 2. Format layer
        let fmt_branch = match disk.format {
            InternalDiskFormat::Qcow2 => BlockdevOptionsBranch::Qcow2(BlockdevOptionsQcow2 {
                file: Box::new(BlockdevRef::Reference(file_node.clone())),
                backing: None,
                cache_clean_interval: None,
                cache_size: None,
                data_file: None,
                discard_no_unref: None,
                encrypt: None,
                l2_cache_entry_size: None,
                l2_cache_size: None,
                lazy_refcounts: None,
                overlap_check: None,
                pass_discard_other: None,
                pass_discard_request: None,
                pass_discard_snapshot: None,
                refcount_cache_size: None,
            }),
            InternalDiskFormat::Raw => BlockdevOptionsBranch::Raw(qapi::BlockdevOptionsRaw {
                file: Box::new(BlockdevRef::Reference(file_node.clone())),
                offset: None,
                size: None,
            }),
        };
        let fmt_driver = match disk.format {
            InternalDiskFormat::Qcow2 => BlockdevDriver::Qcow2,
            InternalDiskFormat::Raw => BlockdevDriver::Raw,
        };
        qmp.execute(&BlockdevAdd {
            driver: Some(fmt_driver),
            node_name: Some(fmt_node.clone()),
            read_only: Some(disk.read_only),
            u: Some(fmt_branch),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .map_err(|e| anyhow::anyhow!("blockdev-add format layer (disk{idx}): {e:#}"))?;
    }

    Ok(())
}

/// Attach virtio-blk-pci devices for previously-added blockdevs.
/// Requires machine to be initialized (NOT preconfig-safe).
/// Call after x-exit-preconfig or in -S paused mode.
async fn attach_devices(qmp: &qapi_protocol::Qmp, disks: &[InternalDisk]) -> Result<()> {
    use qapi::commands::DeviceAdd;
    use std::collections::HashMap;

    for (idx, _disk) in disks.iter().enumerate() {
        let fmt_node = format!("disk{idx}");
        let device_id = format!("blk{idx}");

        qmp.execute(&DeviceAdd {
            driver: "virtio-blk-pci".into(),
            id: Some(device_id.clone()),
            bus: Some(format!("rp{idx}")),
            arguments: Some(HashMap::from([
                ("drive".to_string(), serde_json::Value::String(fmt_node)),
                (
                    "bootindex".to_string(),
                    serde_json::Value::Number((idx as i64).into()),
                ),
            ])),
        })
        .await
        .map_err(|e| anyhow::anyhow!("device_add {device_id} virtio-blk-pci: {e:#}"))?;
    }

    Ok(())
}

/// Create a temporary qcow2 overlay backed by the given base image.
///
/// The overlay is a thin qcow2 that stores writes in a temp file while
/// reading unchanged sectors from the base. This keeps the base image
/// pristine across VM restarts — an unclean kill only dirties the
/// disposable overlay, not the downloaded SNP image.
async fn create_temp_overlay(base: &Path) -> Result<PathBuf> {
    let overlay = std::env::temp_dir().join(format!(
        "qrab-overlay-{}-{}.qcow2",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));

    let output = tokio::process::Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            "-b",
            &base.display().to_string(),
            "-F",
            "qcow2",
            &overlay.display().to_string(),
        ])
        .output()
        .await
        .context("failed to run qemu-img for overlay creation")?;

    if !output.status.success() {
        bail!(
            "qemu-img create overlay failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // R14-A7: resize the overlay (not the base). Resizing mutates
    // qcow2 metadata and would invalidate the base image's pinned
    // SHA-256 on every run. Default Ubuntu cloud image is ~2.2 G
    // virtual; 4 G leaves headroom for cloud-init + package installs
    // without touching the verified base.
    let resize = tokio::process::Command::new("qemu-img")
        .args(["resize", &overlay.display().to_string(), "4G"])
        .output()
        .await?;
    if !resize.status.success() {
        tracing::warn!(
            "qemu-img resize overlay failed (non-fatal): {}",
            String::from_utf8_lossy(&resize.stderr)
        );
    }

    Ok(overlay)
}

/// Environment override for the expected SHA-256 of the Ubuntu
/// cloud image. When set to 64 hex chars, `ensure_ubuntu_snp_image`
/// fails closed on any mismatch (including a stale cached file).
/// When unset we emit a loud warning per run and the actual hash
/// so an operator can pin it back via this variable. R14-A7.
pub const ENV_UBUNTU_IMAGE_SHA256: &str = "QRAB_UBUNTU_SNP_IMAGE_SHA256_HEX";

/// Download Ubuntu 24.04 cloud image for SEV-SNP VMs.
///
/// CirrosOS cannot be used for SEV-SNP because its kernel lacks
/// CONFIG_AMD_MEM_ENCRYPT and crashes during page state transitions.
/// Ubuntu 24.04 ships kernel 6.8+ with full SEV-SNP guest support.
///
/// Login: ubuntu / (per-boot random pwd — see
/// [`ensure_cloud_init_seed`]).
///
/// Supply-chain: the image bytes drive every BMC-spawned SNP guest.
/// R14-A7: we pin SHA-256 via the [`ENV_UBUNTU_IMAGE_SHA256`] env
/// var. When set, a mismatch (corrupt cache, MITM download, bucket
/// drift) fails closed. When unset we log the hash of whatever we
/// downloaded and keep booting — dev ergonomics — but the operator
/// running this in a real BMC deployment is expected to set the
/// pin. SNP measurement chain only covers guest launch, not the
/// pre-boot image filesystem, so a swapped image == compromised
/// guest kernel == full bypass of the attestation chain downstream.
///
/// The fetched file is NOT resized here: resizing rewrites qcow2
/// metadata and invalidates the pinned SHA-256. The overlay path in
/// [`create_temp_overlay`] does the 4G resize so the base stays
/// verifiable across reboots.
async fn ensure_ubuntu_snp_image() -> Result<PathBuf> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("test-fixtures");
    tokio::fs::create_dir_all(&cache_dir).await?;

    let path = cache_dir.join("noble-server-cloudimg-amd64.img");
    let pinned = expected_ubuntu_image_sha256()?;

    if path.exists() {
        let digest = sha256_hex_of_file(&path).await?;
        match &pinned {
            Some(expected) if expected != &digest => {
                tracing::warn!(
                    path = %path.display(),
                    expected = %expected,
                    actual = %digest,
                    "ubuntu cloud image SHA-256 mismatch; deleting cache + redownloading"
                );
                tokio::fs::remove_file(&path).await.ok();
            }
            Some(_) => {
                tracing::info!(
                    path = %path.display(),
                    sha256 = %digest,
                    "ubuntu cloud image: cached copy matches pinned SHA-256"
                );
                return Ok(path);
            }
            None => {
                tracing::warn!(
                    path = %path.display(),
                    sha256 = %digest,
                    env = ENV_UBUNTU_IMAGE_SHA256,
                    "ubuntu cloud image: using cached copy without a pinned \
                     SHA-256 (set {env} to harden against supply-chain drift)",
                    env = ENV_UBUNTU_IMAGE_SHA256,
                );
                return Ok(path);
            }
        }
    }

    tracing::info!("Downloading Ubuntu 24.04 cloud image (~600MB)...");
    let url = "https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img";
    let output = tokio::process::Command::new("curl")
        .args(["-fSL", "-o", &path.display().to_string(), url])
        .output()
        .await?;

    if !output.status.success() {
        bail!(
            "Failed to download Ubuntu cloud image: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let digest = sha256_hex_of_file(&path).await?;
    match &pinned {
        Some(expected) if expected != &digest => {
            // Delete the unpinned download so a retry doesn't
            // succeed just by hitting the cache path above.
            let _ = tokio::fs::remove_file(&path).await;
            bail!(
                "ubuntu cloud image SHA-256 mismatch after download: \
                 expected {expected}, got {digest}. Supply-chain check \
                 failed; file removed."
            );
        }
        Some(_) => {
            tracing::info!(
                sha256 = %digest,
                "ubuntu cloud image downloaded + pinned SHA-256 verified"
            );
        }
        None => {
            tracing::warn!(
                sha256 = %digest,
                env = ENV_UBUNTU_IMAGE_SHA256,
                "ubuntu cloud image downloaded without a pinned SHA-256 \
                 (set {env}={hash} for future runs to harden)",
                env = ENV_UBUNTU_IMAGE_SHA256,
                hash = digest.as_str(),
            );
        }
    }

    Ok(path)
}

/// Parse a 64-hex-char SHA-256 pin from [`ENV_UBUNTU_IMAGE_SHA256`].
/// Returns `Ok(None)` when unset (ergonomic dev default), `Ok(Some)`
/// when a valid pin is provided, or `Err` when the value is set but
/// malformed — we refuse to run with an unparseable pin rather than
/// silently skip the check.
fn expected_ubuntu_image_sha256() -> Result<Option<String>> {
    match std::env::var(ENV_UBUNTU_IMAGE_SHA256) {
        Ok(v) => parse_ubuntu_image_sha256(&v).map(Some),
        Err(_) => Ok(None),
    }
}

/// Pure parser exercised by unit tests. Strips ASCII whitespace,
/// requires exactly 64 hex chars, lowercases on return.
fn parse_ubuntu_image_sha256(raw: &str) -> Result<String> {
    let clean: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() != 64 || !clean.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!(
            "{ENV_UBUNTU_IMAGE_SHA256} must be 64 hex chars, got {} chars",
            clean.len()
        );
    }
    Ok(clean.to_ascii_lowercase())
}

/// Compute the SHA-256 hex digest of a file. Streams in 1 MiB
/// chunks so a 600 MiB cloud image does not need to sit in RAM.
async fn sha256_hex_of_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("opening {} for SHA-256", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .await
            .with_context(|| format!("reading {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest.iter() {
        use core::fmt::Write as _;
        write!(&mut out, "{b:02x}").unwrap();
    }
    Ok(out)
}

/// Atomically create `path` with mode `0o600` and write `pwd` + `\n`.
///
/// R15-1 / R15-7: the previous implementation did
/// `tokio::fs::write(&path, ...)` followed by `set_permissions(0o600)`.
/// That opens a microsecond-scale TOCTOU window: the file exists on
/// disk at the umask-default mode (typically 0644) before the chmod
/// lands, so any local reader of the parent dir can race the open
/// and steal the per-boot guest credential.
///
/// Fix: unlink any stale file first (best-effort — `NotFound` is
/// tolerated), then `O_CREAT | O_EXCL | O_WRONLY` with `mode=0o600`.
/// `tokio::fs::OpenOptions::mode()` forwards to
/// `std::os::unix::fs::OpenOptionsExt::mode`, so no std interop
/// fallback is needed on tokio 1.x. `create_new` guarantees the
/// file did not pre-exist under an attacker-controlled mode at the
/// instant our fd is obtained — the kernel bakes 0o600 into the
/// inode atomically. If a racing attacker re-creates the file
/// between our `remove_file` and `open`, `create_new` fails loudly
/// rather than silently inheriting their permissive mode.
async fn write_pwd_file_0600(path: &Path, pwd: &str) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    // Best-effort unlink of any previous pwd file. `NotFound` is
    // expected on a cold cache; any other error means the parent
    // dir is unwritable and `create_new` will fail anyway — bubble
    // that up with context.
    match tokio::fs::remove_file(path).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(e).with_context(|| {
                format!("removing stale pwd file at {}", path.display())
            });
        }
    }

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .await
        .with_context(|| format!("opening {} for pwd write", path.display()))?;
    file.write_all(pwd.as_bytes())
        .await
        .with_context(|| format!("writing pwd to {}", path.display()))?;
    file.write_all(b"\n")
        .await
        .with_context(|| format!("writing pwd trailing newline to {}", path.display()))?;
    file.flush()
        .await
        .with_context(|| format!("flushing pwd file {}", path.display()))?;
    Ok(())
}

/// Create cloud-init NoCloud seed disk (FAT image labeled "cidata").
///
/// R14-A6: generates a fresh random 32-hex-char password on every
/// call. Previously the user-data YAML hard-coded `plain_text_passwd:
/// snp` for both `ubuntu` and `root` on a cached seed reused across
/// every BMC-spawned SNP VM on every qrab host — a known-bad default
/// that moved from "dev-only" to production surface the moment
/// `power_on` became a public BMC path. The new password is written
/// to `$cache_dir/cloud-init-seed-qga.pwd` (0600) for the operator
/// to read; the seed cache is never reused so every `power_on`
/// brings up a guest with a fresh credential.
///
/// R15-2: the scratch build dir is a per-call `tempfile::TempDir`
/// rooted at `/tmp` (not `$TMPDIR`) so a hostile local user cannot
/// pre-create a `user-data` symlink in the old predictable
/// `$tmpdir/qrab-cloud-init-build/` path and redirect our YAML write
/// (containing the per-boot pwd) onto an attacker-chosen target.
/// Same guarantee as B14-7 on the manager socket dir: unguessable
/// 0700 leaf, sticky-bit-rooted base, auto-remove on drop.
///
/// R15-6: the password is written in exactly one YAML slot —
/// `users[0].plain_text_passwd` for the `ubuntu` user. The legacy
/// top-level `password:` and the redundant `root` entry have been
/// dropped; sudo NOPASSWD on `ubuntu` covers every operator path.
async fn ensure_cloud_init_seed() -> Result<PathBuf> {
    use tokio::io::AsyncReadExt;

    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("test-fixtures");
    tokio::fs::create_dir_all(&cache_dir).await?;
    let path = cache_dir.join("cloud-init-seed-qga.img");
    let pwd_path = cache_dir.join("cloud-init-seed-qga.pwd");

    // 16 bytes from /dev/urandom → 32-hex-char password. Crypto-
    // grade entropy without pulling in a dedicated rand crate.
    let mut pwd_bytes = [0u8; 16];
    tokio::fs::File::open("/dev/urandom")
        .await
        .context("opening /dev/urandom")?
        .read_exact(&mut pwd_bytes)
        .await
        .context("reading /dev/urandom")?;
    let pwd = pwd_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();

    // Store the password for the operator. Overwrite any previous
    // pwd file — we never reuse a prior boot's credential.
    write_pwd_file_0600(&pwd_path, &pwd).await?;
    tracing::info!(
        path = %pwd_path.display(),
        "cloud-init seed: wrote per-boot guest password (0600); \
         previous seed invalidated"
    );

    // R15-2: unguessable 0700 scratch dir under `/tmp` (not
    // `$TMPDIR`), auto-removed on drop. The only persistent output
    // is the final `.img` copied to `cache_dir` below.
    let tmpdir = tempfile::TempDir::new_in("/tmp")
        .context("creating cloud-init build tempdir under /tmp")?;
    let build = tmpdir.path();

    // user-data — single pwd slot (R15-6).
    let user_data = build.join("user-data");
    tokio::fs::write(
        &user_data,
        format!(r#"#cloud-config
chpasswd:
  expire: false
ssh_pwauth: true
users:
  - name: ubuntu
    plain_text_passwd: {pwd}
    lock_passwd: false
    shell: /bin/bash
    sudo: ALL=(ALL) NOPASSWD:ALL
packages:
  - qemu-guest-agent
runcmd:
  - echo "===== SNP BOOT OK =====" > /dev/ttyS0
  - echo "===== SNP BOOT OK =====" > /dev/tty0 || true
  - systemctl enable --now qemu-guest-agent
"#),
    )
    .await?;

    // meta-data
    let meta_data = build.join("meta-data");
    tokio::fs::write(
        &meta_data,
        "instance-id: snp-test-1\nlocal-hostname: snp-guest\n",
    )
    .await?;

    // Build FAT image with label "cidata"
    let img = build.join("seed.img");
    tokio::process::Command::new("truncate")
        .args(["-s", "1M", &img.display().to_string()])
        .output()
        .await?;
    tokio::process::Command::new("mkfs.fat")
        .args(["-n", "cidata", &img.display().to_string()])
        .output()
        .await?;
    tokio::process::Command::new("mcopy")
        .args([
            "-i",
            &img.display().to_string(),
            &user_data.display().to_string(),
            "::user-data",
        ])
        .output()
        .await?;
    tokio::process::Command::new("mcopy")
        .args([
            "-i",
            &img.display().to_string(),
            &meta_data.display().to_string(),
            "::meta-data",
        ])
        .output()
        .await?;

    tokio::fs::copy(&img, &path).await?;
    // `tmpdir` drops here — removes `user-data`, `meta-data`, and
    // the staging `seed.img`. The cached `path` is the only survivor.
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-8 B8-S2 regression: dropping an `OverlayGuard` must remove
    /// the tracked file from disk. Without this the temp qcow2 overlay
    /// created per power cycle leaks one file on every `power_on`.
    #[test]
    fn overlay_guard_removes_file_on_drop() {
        let path = std::env::temp_dir().join(format!(
            "qrab-overlay-test-{}-{}.qcow2",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"fake qcow2 contents").unwrap();
        assert!(path.exists(), "precondition: test file created");

        {
            let guard = OverlayGuard::new(path.clone());
            assert_eq!(guard.path(), path.as_path());
            // guard dropped at end of this scope
        }

        assert!(
            !path.exists(),
            "OverlayGuard::drop must remove the tracked file"
        );
    }

    /// Missing file on drop is non-fatal: `OverlayGuard::drop` must not
    /// panic if the file is already gone (external cleanup, tmpfs reboot,
    /// etc.). Teardown cannot be allowed to fail on a missing overlay.
    #[test]
    fn overlay_guard_drop_tolerates_missing_file() {
        let path = std::env::temp_dir().join(format!(
            "qrab-overlay-test-missing-{}-{}.qcow2",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // Intentionally do NOT create the file.
        assert!(!path.exists(), "precondition: file does not exist");

        let guard = OverlayGuard::new(path.clone());
        drop(guard); // must not panic
    }

    // R14-A7: exercise the pin parser directly via the pure
    // helper. The env-reading wrapper is a thin glue layer we
    // avoid touching here because Rust 2024 made
    // `std::env::set_var` unsafe and the bmc crate forbids unsafe
    // blocks — the pure fn gives the same coverage.
    #[test]
    fn pin_valid_hex_parses_lowercase() {
        assert_eq!(
            parse_ubuntu_image_sha256(&"A".repeat(64)).unwrap(),
            "a".repeat(64)
        );
    }

    #[test]
    fn pin_strips_whitespace() {
        let raw = format!("  {}\n", "0".repeat(64));
        assert_eq!(parse_ubuntu_image_sha256(&raw).unwrap(), "0".repeat(64));
    }

    #[test]
    fn pin_wrong_length_errors() {
        let err = parse_ubuntu_image_sha256("deadbeef").unwrap_err();
        assert!(format!("{err}").contains("64 hex chars"));
    }

    #[test]
    fn pin_non_hex_errors() {
        let err = parse_ubuntu_image_sha256(&"z".repeat(64)).unwrap_err();
        assert!(format!("{err}").contains("64 hex chars"));
    }

    #[tokio::test]
    async fn sha256_hex_of_file_matches_known_vector() {
        // SHA-256 of the empty string.
        let path = std::env::temp_dir().join(format!(
            "qrab-sha256-empty-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"").unwrap();
        let got = sha256_hex_of_file(&path).await.unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            got,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// R15-3 regression: `phase()` must not block while a concurrent task
    /// holds the VM in `Initializing`. The pre-R15-3 `power_on` held the
    /// `vm` lock across ~600 MiB of SHA-256, gating every dashboard poll.
    /// After the fix the lock is released once state is flipped to
    /// `Initializing`, so `phase()` returns `Initializing` promptly.
    ///
    /// The "slow work" is simulated with `tokio::time::sleep` because
    /// constructing a real Vm requires QEMU + SNP hardware. The lock
    /// discipline exercised here is exactly the shape of the production
    /// `power_on` / `power_on_build` split.
    #[tokio::test]
    async fn phase_unblocked_while_state_is_initializing() {
        use std::sync::Arc;
        use tokio::time::{timeout, Duration, Instant};

        let state = BmcState::new();
        let worker_state = Arc::clone(&state);

        // Worker: flip Off → Initializing, release the lock, do slow
        // work, flip Initializing → Off. The real `power_on` does the
        // same dance but with VM build work instead of a sleep.
        let slow_work = Duration::from_millis(500);
        let worker = tokio::spawn(async move {
            {
                let mut guard = worker_state.vm.lock().await;
                assert!(matches!(*guard, VmState::Off));
                *guard = VmState::Initializing;
            }
            // Simulate the slow SHA-256 + qemu-img + spawn work.
            tokio::time::sleep(slow_work).await;
            *worker_state.vm.lock().await = VmState::Off;
        });

        // Yield so the worker can claim the lock + flip to Initializing.
        tokio::task::yield_now().await;
        // Busy-spin a few ticks until we observe Initializing — the
        // scheduler may not have run the worker yet.
        let observed = timeout(Duration::from_millis(100), async {
            loop {
                if matches!(*state.vm.lock().await, VmState::Initializing) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await;
        assert!(observed.is_ok(), "worker should have entered Initializing");

        // The critical assertion: phase() returns while the worker is
        // still sleeping. Budget is 10x smaller than the worker's sleep.
        let phase_budget = Duration::from_millis(50);
        let start = Instant::now();
        let p = timeout(phase_budget, state.phase())
            .await
            .expect("phase() blocked past budget; vm lock is being held across slow work");
        assert_eq!(p, VmPhase::Initializing);
        assert!(
            start.elapsed() < phase_budget,
            "phase() took {:?}, longer than {:?}",
            start.elapsed(),
            phase_budget
        );

        worker.await.unwrap();
    }

    /// R15-3: `take_running_vm` must refuse to tear down a VM that is
    /// still initializing. Otherwise a `power_off` invoked mid-build
    /// would flip state to `ShuttingDown` while the in-flight `power_on`
    /// task is still spawning QEMU + swtpm — stranding both processes.
    #[tokio::test]
    async fn take_running_vm_refuses_while_initializing() {
        let state = BmcState::new();
        *state.vm.lock().await = VmState::Initializing;
        // `Vm` is not Debug so can't use `expect_err`; match manually.
        let msg = match state.take_running_vm().await {
            Ok(_) => panic!("take_running_vm must reject Initializing"),
            Err(e) => format!("{e}"),
        };
        assert!(msg.contains("initializing"), "error was {msg}");
        // Lock must still hold Initializing — no partial transition.
        assert!(matches!(*state.vm.lock().await, VmState::Initializing));
    }

    /// R15-1 / R15-7 regression: `write_pwd_file_0600` must create the
    /// file at mode 0o600 from `open(2)` time, with no umask-default
    /// (0644) window between `write` and a follow-up `chmod`. The test
    /// writes into a fresh temp path, then reads the mode bits back
    /// from the filesystem and asserts the low 9 bits are exactly
    /// rw- --- ---.
    #[tokio::test]
    async fn write_pwd_file_creates_0600_atomically() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "qrab-pwd-mode-test-{}-{}.pwd",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // Precondition: path does not exist.
        let _ = std::fs::remove_file(&path);

        write_pwd_file_0600(&path, "deadbeefcafebabe")
            .await
            .expect("write_pwd_file_0600 must succeed");

        let meta = tokio::fs::metadata(&path)
            .await
            .expect("pwd file must exist after write");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "pwd file mode is {mode:o}, expected 600 — \
             TOCTOU window would reopen if this regressed"
        );

        let body = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(body, "deadbeefcafebabe\n");

        let _ = std::fs::remove_file(&path);
    }

    /// R15-1 / R15-7: overwriting an existing pwd file must still
    /// land at 0o600 — even if an operator or prior process left
    /// the old file at 0o644. The production caller rotates the
    /// credential on every `power_on`; without the unlink step,
    /// `O_CREAT | O_TRUNC` would preserve the permissive mode and
    /// the new credential would leak through the stale ACL.
    #[tokio::test]
    async fn write_pwd_file_overwrite_resets_to_0600() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "qrab-pwd-mode-overwrite-{}-{}.pwd",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // Plant a permissive existing file.
        std::fs::write(&path, b"old-credential\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644,
            "precondition: file is 0644"
        );

        write_pwd_file_0600(&path, "new-credential")
            .await
            .expect("overwrite must succeed");

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "pwd file mode is {mode:o} after overwrite, expected 600 — \
             stale 0644 must not survive the rotation"
        );
        let body = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(body, "new-credential\n");

        let _ = std::fs::remove_file(&path);
    }
}
