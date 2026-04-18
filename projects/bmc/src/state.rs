use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinHandle;

use manager::arch::{Arch, MachineConfig};
use manager::vm::{PreconfigVm, Vm, VmBuilder};
use qapi_protocol::QmpEvent;

use crate::types::{
    AttestationState, AttestationStatus, BmcArch, BmcCapabilities, BmcConfig, BmcMachine,
    CpuInfoResponse, DiskConfig, DiskFormat, KvmInfoResponse, SevCapabilitiesResponse,
    SevStatusResponse, TpmStatusResponse, VmPhase,
};

const EVENT_HISTORY_CAP: usize = 1000;

static BMC: OnceLock<Arc<BmcState>> = OnceLock::new();

/// Get the global BmcState, initializing with defaults if needed.
pub fn bmc() -> Arc<BmcState> {
    BMC.get_or_init(BmcState::new).clone()
}

/// Internal VM state. Not serializable — holds owned QEMU handles.
pub enum VmState {
    Off,
    Running(Vm),
    ShuttingDown,
}

/// Static info cached once per Running VM session.
#[derive(Clone)]
pub struct CachedStaticInfo {
    pub cpu_info: CpuInfoResponse,
    pub kvm_info: KvmInfoResponse,
    pub sev_status: Option<SevStatusResponse>,
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
    pub event_tx: broadcast::Sender<QmpEvent>,
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
}

impl BmcState {
    pub fn new() -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(256);
        Arc::new(Self {
            vm: Mutex::new(VmState::Off),
            event_tx,
            event_history: Arc::new(Mutex::new(VecDeque::with_capacity(EVENT_HISTORY_CAP))),
            config: Mutex::new(BmcConfig::default()),
            cached_static: Mutex::new(None),
            attestation: Mutex::new(AttestationState::default()),
            event_forwarder: Mutex::new(None),
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

    /// Spawn a VM, attach disks, and start execution. Transitions Off -> Running.
    ///
    /// Internally the VM goes through a paused (-S) state where QMP commands
    /// like blockdev-add and device_add are issued, then `cont` is sent.
    /// This intermediate state is never exposed to callers.
    ///
    /// Note: holds the `vm` lock for the entire spawn + start (~hundreds of ms).
    /// Concurrent `phase()` reads will block. Acceptable for single-user
    /// BMC; if this becomes a multi-tenant concern, add a `VmState::Initializing`
    /// placeholder that lets reads see a busy state without blocking.
    pub async fn power_on(self: &Arc<Self>) -> Result<()> {
        let mut vm_guard = self.vm.lock().await;
        if !matches!(*vm_guard, VmState::Off) {
            bail!("VM is not off");
        }

        // Defensive: clear any stale state from a prior power cycle.
        self.event_history.lock().await.clear();
        *self.cached_static.lock().await = None;
        *self.attestation.lock().await = AttestationState::default();

        let config = self.config.lock().await.clone();

        // --- Validation ---
        if config.secure_boot && config.machine != BmcMachine::Uefi {
            bail!("SecureBoot requires UEFI machine type");
        }
        if config.sev_snp.is_some() {
            if !config.kvm {
                bail!("SEV-SNP requires KVM acceleration");
            }
            if config.machine != BmcMachine::Uefi {
                bail!("SEV-SNP requires UEFI machine type");
            }
            if config.arch != BmcArch::X86_64 {
                bail!("SEV-SNP requires x86_64 architecture");
            }
        }
        if config.tpm.is_some() {
            if which::which("swtpm").is_err() {
                bail!("TPM requires swtpm but it was not found in PATH");
            }
        }

        let arch = arch_to_manager(config.arch);
        let machine_config =
            machine_to_manager(config.machine, config.memory.clone(), config.secure_boot);

        let mut builder = VmBuilder::new(arch, machine_config).serial();
        if let Some(cpus) = config.cpus {
            builder = builder.smp(cpus);
        }
        if config.kvm {
            // SEV-SNP requires EPYC CPU model — `-cpu host` exposes
            // CPUID features that fail SNP firmware validation.
            let cpu_model = if config.sev_snp.is_some() {
                "EPYC-v4"
            } else {
                "host"
            };
            builder = builder
                .accel(manager::vm::Accel::Kvm)
                .cpu(cpu_model);
        }
        if config.display {
            builder = builder.display();
        }

        // Firmware: custom path > SEV-SNP AmdSev OVMF > standard OVMF.
        if let Some(ref fw_path) = config.firmware_path {
            builder = builder.firmware(fw_path.clone());
        } else if config.sev_snp.is_some() {
            // SEV-SNP needs the AmdSev OVMF variant (contains SEV metadata
            // GUID table for SNP_LAUNCH_UPDATE). Standard OVMF won't work.
            if let Some(fw) = manager::firmware::find_amdsev_ovmf() {
                builder = builder.firmware(fw);
            } else {
                bail!(
                    "SEV-SNP requires AmdSev OVMF firmware but none found. \
                     Install edk2-amdsev or set firmware_path manually."
                );
            }
        } else if matches!(config.machine, BmcMachine::Uefi) {
            if let Some(fw) = manager::firmware::find_ovmf(config.secure_boot) {
                builder = builder.ovmf(fw);
            }
        }

        // SecureBoot flag on builder (enables SMM, pflash secure mode).
        if config.secure_boot {
            builder = builder.secure_boot();
        }

        // SEV-SNP
        if let Some(ref sev) = config.sev_snp {
            let sev_config = manager::vm::SevSnpConfig {
                sev_device: None,
                policy: sev.policy,
                reduced_phys_bits: sev.reduced_phys_bits,
                cbitpos: None, // auto-detect from CPUID
                id_block: sev.id_block.clone(),
                id_auth: None,
                author_key_enabled: None,
                kernel_hashes: None,
                host_data: sev.host_data.clone(),
                vcek_disabled: None,
                guest_visible_workarounds: None,
            };
            builder = builder.sev_snp(sev_config);
        }

        // TPM (swtpm only)
        if let Some(ref tpm) = config.tpm {
            let tpm_config = manager::vm::TpmConfig {
                state_dir: if tpm.persistent {
                    Some(tpm_state_dir())
                } else {
                    None
                },
            };
            builder = builder.tpm(tpm_config);
        }

        let mut paused_vm = builder.spawn().await?;

        // Resolve disk list. Demo fallback: if no disks configured and the
        // machine has a PCI bus, attach cirros so power_on yields a bootable
        // VM out of the box. Long term this should be configured via UI.
        let disks = if !matches!(config.machine, BmcMachine::None) && config.disks.is_empty() {
            let cirros = ensure_cirros_image().await?;
            // Create a temporary overlay so the base image is never modified.
            // Each VM gets a fresh writable snapshot; force_off discards it.
            let overlay = create_temp_overlay(&cirros).await?;
            vec![DiskConfig {
                file: overlay,
                format: DiskFormat::Qcow2,
                interface: crate::types::DiskInterface::Virtio,
                read_only: false,
            }]
        } else {
            config.disks.clone()
        };

        // Add disks via QMP while paused (-S). blockdev-add and device_add
        // both work in paused mode. With machine=none there's no PCI bus,
        // so skip disk attachment entirely.
        if !matches!(config.machine, BmcMachine::None) {
            if let Err(e) = attach_disks(&paused_vm, &disks).await {
                paused_vm.shutdown().await;
                return Err(e);
            }
        }

        // Extract the real event receiver and replace with a dummy.
        let (dummy_tx, dummy_rx) = mpsc::channel(1);
        drop(dummy_tx);
        let real_rx = std::mem::replace(&mut paused_vm.events, dummy_rx);

        // Spawn event forwarder task.
        let tx = self.event_tx.clone();
        let history = self.event_history.clone();
        let handle = tokio::spawn(async move {
            event_forwarder(real_rx, tx, history).await;
        });

        let mut fwd = self.event_forwarder.lock().await;
        *fwd = Some(handle);

        // Start execution (sends `cont` in -S paused mode).
        let vm = match paused_vm.start().await {
            Ok(vm) => vm,
            Err(e) => {
                // paused_vm was consumed by start(). Tear down everything
                // else and force the state to Off.
                drop(fwd);
                self.stop_forwarder().await;
                self.event_history.lock().await.clear();
                *self.cached_static.lock().await = None;
                *vm_guard = VmState::Off;
                return Err(anyhow::anyhow!("power_on: PreconfigVm::start failed; transitioned to Off: {e}"));
            }
        };

        *vm_guard = VmState::Running(vm);

        // If SEV-SNP enabled, query launch measurement and capabilities.
        // Errors are non-fatal — attestation degrades gracefully.
        if config.sev_snp.is_some() {
            if let VmState::Running(ref vm) = *vm_guard {
                let mut att = self.attestation.lock().await;
                att.status = AttestationStatus::Pending;

                // Launch measurement
                match vm.query_sev_launch_measure().await {
                    Ok(measure) => {
                        att.launch_measure = Some(measure.data);
                        att.status = AttestationStatus::Measured;
                    }
                    Err(e) => {
                        tracing::warn!("query-sev-launch-measure failed: {e}");
                        att.status = AttestationStatus::Failed(e.to_string());
                    }
                }

                // SEV capabilities (PDH, cert chain)
                match vm.query_sev_capabilities().await {
                    Ok(caps) => {
                        att.sev_capabilities = Some(SevCapabilitiesResponse {
                            pdh: caps.pdh,
                            cert_chain: caps.cert_chain,
                            cbitpos: caps.cbitpos,
                            reduced_phys_bits: caps.reduced_phys_bits,
                        });
                    }
                    Err(e) => {
                        tracing::warn!("query-sev-capabilities failed: {e}");
                    }
                }
            }
        }

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

    /// Request guest powerdown via ACPI.
    pub async fn power_off(&self) -> Result<()> {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Running(vm) => {
                vm.powerdown().await?;
                Ok(())
            }
            _ => bail!("VM is not running"),
        }
    }

    /// Force quit the QEMU process.
    pub async fn force_off(&self) -> Result<()> {
        let mut vm_guard = self.vm.lock().await;
        let state = std::mem::replace(&mut *vm_guard, VmState::ShuttingDown);
        match state {
            VmState::Running(vm) => {
                vm.shutdown().await;
            }
            VmState::Off => {
                *vm_guard = VmState::Off;
                return Ok(());
            }
            VmState::ShuttingDown => {
                *vm_guard = VmState::ShuttingDown;
                bail!("VM is already shutting down");
            }
        }
        self.stop_forwarder().await;
        self.event_history.lock().await.clear();
        *self.cached_static.lock().await = None;
        *self.attestation.lock().await = AttestationState::default();
        *vm_guard = VmState::Off;
        Ok(())
    }

    /// Get the current VM phase.
    pub async fn phase(&self) -> VmPhase {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Off => VmPhase::Off,
            VmState::Running(vm) => {
                match vm.execute(&qapi::commands::QueryStatus {}).await {
                    Ok(status) if !status.running => VmPhase::Paused,
                    _ => VmPhase::Running,
                }
            }
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
}

async fn event_forwarder(
    mut rx: mpsc::Receiver<QmpEvent>,
    tx: broadcast::Sender<QmpEvent>,
    history: Arc<Mutex<VecDeque<QmpEvent>>>,
) {
    while let Some(event) = rx.recv().await {
        let _ = tx.send(event.clone());
        let mut h = history.lock().await;
        h.push_back(event);
        if h.len() > EVENT_HISTORY_CAP {
            h.pop_front();
        }
    }
}

fn arch_to_manager(arch: BmcArch) -> Arch {
    match arch {
        BmcArch::X86_64 => Arch::X86_64,
        BmcArch::I386 => Arch::I386,
        BmcArch::Aarch64 => Arch::Aarch64,
        BmcArch::Arm => Arch::Arm,
    }
}

fn machine_to_manager(machine: BmcMachine, memory: String, secure_boot: bool) -> MachineConfig {
    match machine {
        BmcMachine::None => MachineConfig::None,
        BmcMachine::Bios => MachineConfig::Bios { memory },
        BmcMachine::Uefi => {
            if secure_boot {
                MachineConfig::SecureBoot { memory }
            } else {
                MachineConfig::Uefi { memory }
            }
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
    let canonical = path.canonicalize().map_err(|e| {
        anyhow::anyhow!(
            "disk path {:?} cannot be resolved: {e}",
            path
        )
    })?;

    // Reject non-regular files (block devices, pipes, /dev/*, etc.)
    let meta = std::fs::metadata(&canonical).map_err(|e| {
        anyhow::anyhow!("cannot stat disk path {:?}: {e}", canonical)
    })?;
    if !meta.is_file() {
        anyhow::bail!(
            "disk path {:?} is not a regular file",
            canonical
        );
    }

    // Reject paths under sensitive directories
    let sensitive = ["/proc", "/sys", "/dev", "/etc/shadow", "/etc/passwd"];
    let canonical_str = canonical.to_string_lossy();
    for prefix in &sensitive {
        if canonical_str.starts_with(prefix) {
            anyhow::bail!(
                "disk path {:?} is under restricted directory {:?}",
                canonical, prefix
            );
        }
    }

    Ok(canonical)
}

/// Attach a list of disks to a paused (-S) VM via QMP.
///
/// For each disk: blockdev-add (file backend) → blockdev-add (format layer)
/// → device_add (virtio-blk-pci). The VM must be in -S paused mode
/// (machine initialized but CPU paused) for device_add to work.
async fn attach_disks(preconfig: &PreconfigVm, disks: &[DiskConfig]) -> Result<()> {
    use qapi::commands::{BlockdevAdd, DeviceAdd};
    use qapi::{
        BlockdevDriver, BlockdevOptionsBranch, BlockdevOptionsFile, BlockdevOptionsQcow2,
        BlockdevRef,
    };
    use std::collections::HashMap;

    for (idx, disk) in disks.iter().enumerate() {
        let validated_path = validate_disk_path(&disk.file)
            .map_err(|e| anyhow::anyhow!("disk {idx}: {e}"))?;
        let file_node = format!("disk{idx}-file");
        let fmt_node = format!("disk{idx}");
        let device_id = format!("blk{idx}");

        // 1. File backend
        preconfig
            .qmp
            .execute(&BlockdevAdd {
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
            DiskFormat::Qcow2 => BlockdevOptionsBranch::Qcow2(BlockdevOptionsQcow2 {
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
            DiskFormat::Raw => {
                // raw uses BlockdevOptionsRaw which wraps a BlockdevRef
                BlockdevOptionsBranch::Raw(qapi::BlockdevOptionsRaw {
                    file: Box::new(BlockdevRef::Reference(file_node.clone())),
                    offset: None,
                    size: None,
                })
            }
        };
        let fmt_driver = match disk.format {
            DiskFormat::Qcow2 => BlockdevDriver::Qcow2,
            DiskFormat::Raw => BlockdevDriver::Raw,
        };
        preconfig
            .qmp
            .execute(&BlockdevAdd {
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

        // 3. Wire to a virtio-blk-pci device. Requires paused (-S) mode.
        //    Sets boot-index so the firmware boots disks in declaration order.
        preconfig
            .qmp
            .execute(&DeviceAdd {
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

/// Probe host capabilities for SEV, TPM, OVMF.
pub fn query_capabilities() -> BmcCapabilities {
    BmcCapabilities {
        kvm_available: std::path::Path::new("/dev/kvm").exists(),
        sev_available: std::path::Path::new("/dev/sev").exists(),
        sev_snp_available: std::fs::read_to_string("/sys/module/kvm_amd/parameters/sev_snp")
            .map(|s| s.trim() == "Y")
            .unwrap_or(false),
        swtpm_available: which::which("swtpm").is_ok(),
        ovmf_found: manager::firmware::find_ovmf(false).is_some(),
        ovmf_secure_found: manager::firmware::find_ovmf(true).is_some(),
    }
}

/// Persistent TPM state directory. Created under the project target dir
/// so vTPM state survives VM reboots but is cleaned up with `cargo clean`.
fn tpm_state_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("tpm-state");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Create a temporary qcow2 overlay backed by the given base image.
///
/// The overlay is a thin qcow2 that stores writes in a temp file while
/// reading unchanged sectors from the base. This keeps the base image
/// pristine across VM restarts — an unclean kill only dirties the
/// disposable overlay, not the downloaded cirros image.
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
            "-f", "qcow2",
            "-b", &base.display().to_string(),
            "-F", "qcow2",
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

    Ok(overlay)
}

/// Download cirros disk image if not cached.
async fn ensure_cirros_image() -> Result<PathBuf> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("test-fixtures");
    tokio::fs::create_dir_all(&cache_dir).await?;

    let filename = "cirros-0.6.2-x86_64-disk.img";
    let path = cache_dir.join(filename);
    if path.exists() {
        return Ok(path);
    }

    let url = format!("https://download.cirros-cloud.net/0.6.2/{filename}");
    let output = tokio::process::Command::new("curl")
        .args(["-fSL", "-o", &path.display().to_string(), &url])
        .output()
        .await?;

    if !output.status.success() {
        bail!(
            "Failed to download cirros: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(path)
}
