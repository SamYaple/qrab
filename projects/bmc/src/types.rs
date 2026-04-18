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

/// Live QMP queue status for the schema-driven QMP console.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueStatus {
    /// Current in-flight command count (includes OOB).
    pub depth: usize,
    /// Maximum concurrent commands the client negotiated with QEMU.
    pub capacity: usize,
    /// Whether the mock / QEMU peer advertised OOB in its greeting and the
    /// handshake negotiated it successfully.
    pub oob_supported: bool,
}

impl Default for QueueStatus {
    fn default() -> Self {
        Self {
            depth: 0,
            capacity: 8,
            oob_supported: false,
        }
    }
}

/// Mirror of `qapi::metadata::TypeKind`. Carried in `TypeDto` so the UI can
/// dispatch field inputs without peeking at the other fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeKindDto {
    Primitive,
    Enum,
    Struct,
    List,
}

/// Mirror of `qapi::metadata::Prim`. Kept as a string on the wire so the
/// client doesn't have to carry an exhaustive enum the server might grow
/// between deploys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimDto {
    Str,
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F64,
    Null,
    Any,
}

/// Inlined type information. `List` recurses via `inner`; `Enum` / `Struct`
/// carry the owning type's name for the UI to look up on demand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeDto {
    pub kind: TypeKindDto,
    pub name: String,
    pub prim: Option<PrimDto>,
    pub inner: Option<Box<TypeDto>>,
}

/// Mirror of `qapi::metadata::EnumKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnumKindDto {
    Unit,
    Alternate,
    FlatUnionBranch,
}

/// Mirror of `qapi::metadata::FieldMeta`, owned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDto {
    pub name: String,
    pub wire_name: String,
    pub ty: TypeDto,
    pub optional: bool,
    pub is_discriminator: bool,
    pub recursive: bool,
    pub since: (u32, u32, u32),
    pub until: Option<(u32, u32, u32)>,
    pub deprecated: bool,
    pub unstable: bool,
    pub doc: String,
}

/// Mirror of `qapi::metadata::EnumVariantMeta`, owned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumVariantDto {
    pub name: String,
    pub wire_name: String,
    pub payload_struct: Option<String>,
    pub since: (u32, u32, u32),
    pub until: Option<(u32, u32, u32)>,
    pub deprecated: bool,
    pub unstable: bool,
    pub doc: String,
}

/// Mirror of `qapi::metadata::EnumMeta`, owned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumDto {
    pub name: String,
    pub doc: String,
    pub variants: Vec<EnumVariantDto>,
    pub kind: EnumKindDto,
    pub since: (u32, u32, u32),
    pub until: Option<(u32, u32, u32)>,
}

/// Mirror of `qapi::metadata::StructMeta`, owned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructDto {
    pub name: String,
    pub doc: String,
    pub fields: Vec<FieldDto>,
    pub is_union: bool,
    pub discriminator_field: Option<String>,
    pub branch_enum: Option<String>,
    pub free_form: bool,
    pub since: (u32, u32, u32),
    pub until: Option<(u32, u32, u32)>,
}

/// Mirror of `qapi::metadata::CommandMeta`, owned. `args` inlines the
/// argument struct so one round-trip surfaces everything the picker needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandDto {
    pub wire_name: String,
    pub rust_name: String,
    pub args: StructDto,
    pub allow_oob: bool,
    pub allow_preconfig: bool,
    pub success_response: bool,
    pub since: (u32, u32, u32),
    pub until: Option<(u32, u32, u32)>,
    pub deprecated: bool,
    pub unstable: bool,
    pub doc: String,
    pub category: String,
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
