use serde::{Deserialize, Serialize};

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

/// VM configuration for spawning. The hardcoded BMC profile only exposes
/// memory and cpu count; arch / machine / SEV-SNP / TPM / KVM / display /
/// firmware are all implicit (SEV-SNP + TPM + KVM + UEFI + x86_64 + display).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BmcConfig {
    pub memory: String,
    #[serde(default)]
    pub cpus: Option<u32>,
}

impl Default for BmcConfig {
    fn default() -> Self {
        Self {
            memory: "1G".into(),
            cpus: None,
        }
    }
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
    pub attestation: Option<AttestationState>,
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
            attestation: None,
        }
    }
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

/// Attestation lifecycle status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AttestationStatus {
    /// No SEV configured or available.
    NotAvailable,
    /// SEV active, not yet measured.
    Pending,
    /// Launch measurement captured.
    Measured,
    /// Attestation report verified.
    Verified,
    /// Verification failed.
    Failed(String),
}

/// SEV capabilities response DTO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SevCapabilitiesResponse {
    /// Platform Diffie-Hellman key (base64).
    pub pdh: String,
    /// Certificate chain (base64).
    pub cert_chain: String,
    /// C-bit position in page table entry.
    pub cbitpos: i64,
    /// Number of physical address bits reduced when SEV enabled.
    pub reduced_phys_bits: i64,
}

/// Attestation report response DTO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttestationReportResponse {
    /// Base64-encoded attestation report data.
    pub data: String,
    /// Nonce used for this report.
    pub mnonce: String,
    /// ISO 8601 timestamp when report was captured.
    pub timestamp: String,
}

/// QR code generation response DTO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttestationQrResponse {
    /// ASCII art or SVG content.
    pub qr_data: String,
    /// "ascii" or "svg".
    pub format: String,
    /// Raw JSON payload encoded in QR.
    pub payload: String,
    /// ISO 8601 timestamp.
    pub timestamp: String,
}

/// Combined attestation state for the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttestationState {
    /// Base64-encoded launch measurement.
    pub launch_measure: Option<String>,
    /// SEV capabilities (PDH, cert chain, etc.).
    pub sev_capabilities: Option<SevCapabilitiesResponse>,
    /// Most recent attestation report.
    pub last_attestation_report: Option<AttestationReportResponse>,
    /// Current attestation lifecycle status.
    pub status: AttestationStatus,
}

impl Default for AttestationState {
    fn default() -> Self {
        Self {
            launch_measure: None,
            sev_capabilities: None,
            last_attestation_report: None,
            status: AttestationStatus::NotAvailable,
        }
    }
}
