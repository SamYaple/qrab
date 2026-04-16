use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// Note: BmcArch / BmcMachine -> manager type conversions live in state.rs
// because the manager crate is server-only and types.rs is shared with WASM.

/// VM lifecycle phases visible to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmPhase {
    Off,
    Running,
    Paused,
    ShuttingDown,
}

impl std::fmt::Display for VmPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmPhase::Off => write!(f, "Off"),
            VmPhase::Running => write!(f, "Running"),
            VmPhase::Paused => write!(f, "Paused"),
            VmPhase::ShuttingDown => write!(f, "Shutting Down"),
        }
    }
}

/// VM status response DTO for the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VmStatusResponse {
    pub phase: VmPhase,
    pub running: Option<bool>,
    pub status: Option<String>,
    pub qemu_version: Option<String>,
}

/// QEMU target architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BmcArch {
    X86_64,
    I386,
    Aarch64,
    Arm,
}

/// Machine type / firmware mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BmcMachine {
    /// `-machine none`. No PCI bus, no devices.
    None,
    /// SeaBIOS legacy boot (`-machine pc`).
    Bios,
    /// UEFI boot via OVMF (`-machine q35`).
    Uefi,
}

/// Disk image format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiskFormat {
    Qcow2,
    Raw,
}

/// Disk attachment interface (currently virtio-blk only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiskInterface {
    Virtio,
}

/// A single virtual disk attached to the VM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskConfig {
    pub file: PathBuf,
    pub format: DiskFormat,
    pub interface: DiskInterface,
    #[serde(default)]
    pub read_only: bool,
}

/// VM configuration for spawning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BmcConfig {
    pub arch: BmcArch,
    pub machine: BmcMachine,
    pub memory: String,
    pub display: bool,
    /// Enable KVM hardware acceleration. When true, requires host KVM
    /// support and process membership in the `kvm` group. When false,
    /// QEMU uses TCG software emulation.
    #[serde(default)]
    pub kvm: bool,
    #[serde(default)]
    pub disks: Vec<DiskConfig>,
}

/// CPU information response DTO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuInfoResponse {
    pub guest_cpus: Vec<GuestCpu>,
    pub host_threads: usize,
}

/// A single guest vCPU entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuestCpu {
    pub index: i64,
    pub thread_id: i64,
    pub qom_path: String,
}

/// KVM status response DTO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KvmInfoResponse {
    pub enabled: bool,
    pub present: bool,
}

/// QMP event DTO for the frontend (serializable, owned).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BmcEvent {
    pub event: String,
    pub timestamp_secs: i64,
    pub timestamp_usecs: u32,
    pub data: serde_json::Value,
}

/// Combined dashboard state DTO. One server round-trip gathers every slice
/// the dashboard needs, under a single `state.vm` lock acquisition, so the
/// rendered snapshot is internally consistent. Events are NOT included —
/// they flow through the `/api/events` SSE endpoint directly to the
/// EventLog component and are not part of the polled dashboard state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DashboardState {
    pub status: VmStatusResponse,
    pub cpu_info: CpuInfoResponse,
    pub kvm_info: KvmInfoResponse,
    pub console_available: bool,
    pub serial_tail: String,
}

impl Default for DashboardState {
    /// The "Off VM, no data" placeholder used by the frontend before the
    /// first fetch resolves and on transient fetch errors.
    fn default() -> Self {
        Self {
            status: VmStatusResponse {
                phase: VmPhase::Off,
                running: None,
                status: None,
                qemu_version: None,
            },
            cpu_info: CpuInfoResponse {
                guest_cpus: Vec::new(),
                host_threads: 0,
            },
            kvm_info: KvmInfoResponse {
                enabled: false,
                present: false,
            },
            console_available: false,
            serial_tail: String::new(),
        }
    }
}

impl Default for BmcConfig {
    fn default() -> Self {
        Self {
            arch: BmcArch::X86_64,
            machine: BmcMachine::Uefi,
            memory: "256M".into(),
            display: true,
            kvm: true,
            disks: Vec::new(),
        }
    }
}
