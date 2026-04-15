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
    BmcArch, BmcConfig, BmcMachine, CpuInfoResponse, DiskConfig, DiskFormat, KvmInfoResponse,
    VmPhase,
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
    Preconfig(PreconfigVm),
    Running(Vm),
    ShuttingDown,
}

/// Central BMC state shared across server functions.
///
/// VNC socket and serial log paths are NOT stored here separately. They live
/// inside the `VmState` (via `PreconfigVm` / `Vm` accessors). Reading them
/// requires holding the `vm` lock, which makes "is the VM running and does
/// it have a VNC socket?" an atomic question by construction.
pub struct BmcState {
    pub vm: Mutex<VmState>,
    pub event_tx: broadcast::Sender<QmpEvent>,
    pub event_history: Arc<Mutex<VecDeque<QmpEvent>>>,
    pub config: Mutex<BmcConfig>,
    /// Cached CPU/KVM info for the currently running VM. CPU topology and
    /// KVM module status don't change for the lifetime of a QEMU process,
    /// so the dashboard polling path queries them once per Running session
    /// and reuses the cached values until the next force_off transition.
    /// `None` while Off / Preconfig / ShuttingDown.
    pub cached_static: Mutex<Option<(CpuInfoResponse, KvmInfoResponse)>>,
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
            event_forwarder: Mutex::new(None),
        })
    }

    /// Read the VNC socket path while holding the vm lock. Returns Some only
    /// when a display-enabled VM is currently in Preconfig or Running.
    pub async fn vnc_sock(&self) -> Option<PathBuf> {
        match &*self.vm.lock().await {
            VmState::Preconfig(pvm) => pvm.vnc_sock().cloned(),
            VmState::Running(vm) => vm.vnc_sock().cloned(),
            _ => None,
        }
    }

    /// Read the serial log path while holding the vm lock.
    pub async fn serial_log(&self) -> Option<PathBuf> {
        match &*self.vm.lock().await {
            VmState::Preconfig(pvm) => pvm.serial_log().cloned(),
            VmState::Running(vm) => vm.serial_log().cloned(),
            _ => None,
        }
    }

    /// Spawn a VM with the current config. Transitions Off -> Preconfig.
    ///
    /// Note: holds the `vm` lock for the entire spawn (~hundreds of ms).
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

        let config = self.config.lock().await.clone();
        let arch = arch_to_manager(config.arch);
        let machine_config = machine_to_manager(config.machine, config.memory.clone());

        let mut builder = VmBuilder::new(arch, machine_config).serial();
        if config.kvm {
            builder = builder.accel(manager::vm::Accel::Kvm);
        }
        if config.display {
            builder = builder.display();
        }

        if matches!(config.machine, BmcMachine::Uefi) {
            if let Some(fw) = find_ovmf() {
                builder = builder.firmware(fw);
            }
        }

        let mut preconfig = builder.spawn().await?;

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

        // Add disks via QMP. blockdev-add is preconfig-safe (since 7.1.0);
        // device_add is not, so it requires the paused-mode (-S) path used
        // when display is enabled. With machine=none there's no PCI bus, so
        // skip disk attachment entirely.
        if !matches!(config.machine, BmcMachine::None) {
            if let Err(e) = attach_disks(&preconfig, &disks).await {
                preconfig.shutdown().await;
                return Err(e);
            }
        }

        // Extract the real event receiver and replace with a dummy.
        let (dummy_tx, dummy_rx) = mpsc::channel(1);
        drop(dummy_tx);
        let real_rx = std::mem::replace(&mut preconfig.events, dummy_rx);

        // Spawn event forwarder task.
        let tx = self.event_tx.clone();
        let history = self.event_history.clone();
        let handle = tokio::spawn(async move {
            event_forwarder(real_rx, tx, history).await;
        });

        let mut fwd = self.event_forwarder.lock().await;
        *fwd = Some(handle);

        *vm_guard = VmState::Preconfig(preconfig);
        Ok(())
    }

    /// Exit preconfig and start guest execution. Transitions Preconfig -> Running.
    ///
    /// On error, the VM is forced off (forwarder stopped, vnc/serial cleared,
    /// state transitioned to `Off`). Caller must `power_on` again to retry.
    pub async fn start_vm(self: &Arc<Self>) -> Result<()> {
        let mut vm_guard = self.vm.lock().await;
        let state = std::mem::replace(&mut *vm_guard, VmState::ShuttingDown);
        let pvm = match state {
            VmState::Preconfig(pvm) => pvm,
            other => {
                *vm_guard = other;
                bail!("VM is not in preconfig state");
            }
        };
        match pvm.start().await {
            Ok(vm) => {
                *vm_guard = VmState::Running(vm);
                Ok(())
            }
            Err(e) => {
                // pvm was consumed by start(). Tear down everything else and
                // force the state to Off. Drop the vm guard first since
                // stop_forwarder takes its own lock.
                drop(vm_guard);
                self.stop_forwarder().await;
                self.event_history.lock().await.clear();
                *self.cached_static.lock().await = None;
                *self.vm.lock().await = VmState::Off;
                Err(e).context("start_vm: PreconfigVm::start failed; transitioned to Off")
            }
        }
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
            VmState::Preconfig(pvm) => {
                pvm.shutdown().await;
            }
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
        *vm_guard = VmState::Off;
        Ok(())
    }

    /// Get the current VM phase.
    pub async fn phase(&self) -> VmPhase {
        let vm_guard = self.vm.lock().await;
        match &*vm_guard {
            VmState::Off => VmPhase::Off,
            VmState::Preconfig(_) => VmPhase::Preconfig,
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

fn machine_to_manager(machine: BmcMachine, memory: String) -> MachineConfig {
    match machine {
        BmcMachine::None => MachineConfig::None,
        BmcMachine::Bios => MachineConfig::Bios { memory },
        BmcMachine::Uefi => MachineConfig::Uefi { memory },
    }
}

/// Attach a list of disks to a paused (or preconfig) VM via QMP.
///
/// For each disk: blockdev-add (file backend) → blockdev-add (format layer)
/// → device_add (virtio-blk-pci). The device_add command requires the VM
/// to be in -S paused mode (machine initialized but CPU paused), not in
/// `--preconfig` mode, since device_add is not preconfig-safe.
async fn attach_disks(preconfig: &PreconfigVm, disks: &[DiskConfig]) -> Result<()> {
    use qapi::commands::{BlockdevAdd, DeviceAdd};
    use qapi::{
        BlockdevDriver, BlockdevOptionsBranch, BlockdevOptionsFile, BlockdevOptionsQcow2,
        BlockdevRef,
    };
    use std::collections::HashMap;

    for (idx, disk) in disks.iter().enumerate() {
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
                    filename: disk.file.display().to_string(),
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
            .context("blockdev-add file backend")?;

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
            .context("blockdev-add format layer")?;

        // 3. Wire to a virtio-blk-pci device. Requires paused (-S) mode;
        //    device_add is not preconfig-safe.
        //    Sets boot-index so the firmware boots disks in declaration order.
        preconfig
            .qmp
            .execute(&DeviceAdd {
                driver: "virtio-blk-pci".into(),
                id: Some(device_id),
                bus: None,
                arguments: Some(HashMap::from([
                    ("drive".to_string(), serde_json::Value::String(fmt_node)),
                    (
                        "bootindex".to_string(),
                        serde_json::Value::Number((idx as i64).into()),
                    ),
                ])),
            })
            .await
            .context("device_add virtio-blk-pci")?;
    }

    Ok(())
}

/// Search common system paths for OVMF firmware.
fn find_ovmf() -> Option<PathBuf> {
    let candidates = [
        "/usr/share/edk2-ovmf/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
        "/usr/share/qemu/edk2-x86_64-code.fd",
    ];
    candidates.iter().map(Path::new).find(|p| p.exists()).map(PathBuf::from)
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
