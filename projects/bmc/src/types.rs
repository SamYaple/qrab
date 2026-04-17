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
    /// Enable UEFI SecureBoot (requires UEFI machine, OVMF secure firmware).
    #[serde(default)]
    pub secure_boot: bool,
    /// SEV-SNP configuration (None = disabled).
    #[serde(default)]
    pub sev_snp: Option<BmcSevSnpConfig>,
    /// TPM configuration (None = disabled).
    #[serde(default)]
    pub tpm: Option<BmcTpmConfig>,
    /// Custom firmware path override.
    #[serde(default)]
    pub firmware_path: Option<PathBuf>,
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
    pub sev_status: Option<SevStatusResponse>,
    pub tpm_status: Option<TpmStatusResponse>,
    pub capabilities: BmcCapabilities,
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
            sev_status: None,
            tpm_status: None,
            capabilities: BmcCapabilities {
                kvm_available: false,
                sev_available: false,
                sev_snp_available: false,
                tpm_passthrough_available: false,
                swtpm_available: false,
                ovmf_found: false,
                ovmf_secure_found: false,
            },
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
            secure_boot: false,
            sev_snp: None,
            tpm: None,
            firmware_path: None,
        }
    }
}

/// SEV-SNP configuration for BMC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BmcSevSnpConfig {
    /// SNP policy (default: 0x30000).
    #[serde(default)]
    pub policy: Option<u64>,
    /// Reduced physical address bits (default: 1).
    #[serde(default = "default_reduced_phys_bits")]
    pub reduced_phys_bits: u32,
    /// ID block for attestation (base64).
    #[serde(default)]
    pub id_block: Option<String>,
    /// Host-provided data blob (base64).
    #[serde(default)]
    pub host_data: Option<String>,
}

fn default_reduced_phys_bits() -> u32 {
    1
}

impl Default for BmcSevSnpConfig {
    fn default() -> Self {
        Self {
            policy: None,
            reduced_phys_bits: 1,
            id_block: None,
            host_data: None,
        }
    }
}

/// TPM configuration for BMC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BmcTpmConfig {
    pub backend: BmcTpmBackend,
    /// Persist vTPM state across reboots.
    #[serde(default)]
    pub persistent: bool,
}

/// TPM backend selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BmcTpmBackend {
    Emulated,
    Passthrough,
}

/// Host capability detection result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BmcCapabilities {
    pub kvm_available: bool,
    pub sev_available: bool,
    pub sev_snp_available: bool,
    pub tpm_passthrough_available: bool,
    pub swtpm_available: bool,
    pub ovmf_found: bool,
    pub ovmf_secure_found: bool,
}

/// SEV status from a running VM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SevStatusResponse {
    pub enabled: bool,
    pub sev_type: Option<String>,
    pub state: Option<String>,
    pub api_version: Option<String>,
}

/// TPM status from a running VM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TpmStatusResponse {
    pub present: bool,
    pub model: Option<String>,
    pub backend_type: Option<String>,
}
