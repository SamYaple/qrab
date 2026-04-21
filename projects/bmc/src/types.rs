use serde::{Deserialize, Serialize};

/// VM lifecycle phases visible to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmPhase {
    Off,
    Initializing,
    Running,
    Paused,
    ShuttingDown,
}

impl std::fmt::Display for VmPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmPhase::Off => write!(f, "Off"),
            VmPhase::Initializing => write!(f, "Initializing"),
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub sev_capabilities: Option<SevCapabilitiesResponse>,
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
            sev_capabilities: None,
            tpm_status: None,
            attestation: None,
        }
    }
}

/// SEV status from a running VM.
///
/// Covers the `query-sev` QMP return plus its flat-union branches
/// (`sev` and `sev-snp`). Fields may be absent when the QMP peer is
/// older or the field was removed between versions (e.g. `handle` and
/// `policy` are removed in QEMU 9.1+).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SevStatusResponse {
    pub enabled: bool,
    pub sev_type: Option<String>,
    pub state: Option<String>,
    /// Convenience `api_major.api_minor` rendering for the hero badge.
    pub api_version: Option<String>,
    pub api_major: Option<u8>,
    pub api_minor: Option<u8>,
    pub build_id: Option<u8>,
    /// Legacy SEV firmware handle (removed in 9.1).
    pub handle: Option<u32>,
    /// Legacy SEV policy (removed in 9.1) — rendered as hex string.
    pub policy: Option<String>,
    /// SEV-SNP policy (from the `sev-snp` branch) — rendered as hex.
    pub snp_policy: Option<String>,
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

/// Parsed snapshot of the guest's `QrabReport` EFI variable.
///
/// The variable is published by qrab-boot in the UKI stub
/// (`projects/qrab-boot/src/uki.rs`) with attrs `BS|RT` (volatile,
/// per-boot) and GUID `6e6f1cba-b5ba-4e54-a3b3-2a7a8f3d9b00`.
///
/// efivarfs layout is a 4-byte LE `u32` attribute prefix followed by the
/// variable body. The body starts with the 14-byte ASCII magic
/// `QRAB_REPORT_V1`. Today the post-magic payload is the 32-byte
/// `report_data` hash; the plan allows growing it to the full 1184-byte
/// signed SNP attestation report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QrabReportSnapshot {
    /// Hex-encoded EFI variable attribute flags (4 bytes LE).
    pub efi_attrs_hex: String,
    /// True when the 14-byte `QRAB_REPORT_V1` magic matched.
    pub magic_matched: bool,
    /// Post-magic payload length in bytes. 32 = raw `report_data`,
    /// 1184 = full signed `SnpAttestationReport`.
    pub payload_len: usize,
    /// First 32 bytes of the post-magic payload, hex-encoded. For the
    /// current `report_data` shape this is the full payload.
    pub payload_prefix_hex: String,
    /// UTC fetch timestamp (RFC3339-ish seconds-since-epoch).
    pub fetched_at: String,
}

/// Lifecycle status of the `QrabReport` EFI-variable probe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum QrabReportStatus {
    /// Guest has not been polled yet this session.
    Pending,
    /// QGA path attempted, efivar absent (guest pre-qrab-boot or the
    /// variable was wiped before QGA polled).
    Absent,
    /// Fetched + parsed successfully.
    Ok(QrabReportSnapshot),
    /// QGA or parse error; `reason` is a short diagnostic.
    Failed { reason: String },
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
    /// Guest-side `QrabReport` EFI variable state (per-boot). `Pending`
    /// until the UI triggers an on-demand fetch via QGA.
    pub qrab_report: QrabReportStatus,
}

impl Default for AttestationState {
    fn default() -> Self {
        Self {
            launch_measure: None,
            sev_capabilities: None,
            last_attestation_report: None,
            status: AttestationStatus::NotAvailable,
            qrab_report: QrabReportStatus::Pending,
        }
    }
}

/// Initial snapshot sent as the first SSE frame on connect. Replaces the
/// old multi-fetch seed pattern — one frame carries everything the
/// dashboard, QMP console, config panel and event log need to render.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BmcSnapshot {
    pub dashboard: DashboardState,
    pub queue: QueueStatus,
    pub config: BmcConfig,
    pub events: Vec<BmcEvent>,
}

/// Server -> client push over `/api/events` SSE. Replaces per-component
/// polling: the client keeps a single SSE connection open and updates its
/// central store as frames arrive.
///
/// `kind` tags the variant; `data` carries the variant payload. Heartbeat
/// frames have no payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum BmcPush {
    /// Full initial state. First frame after connect.
    Snapshot(BmcSnapshot),
    /// Dashboard slice changed — power-state / SEV / status flip.
    Dashboard(DashboardState),
    /// QMP queue depth / OOB availability changed.
    Queue(QueueStatus),
    /// VM configuration changed (memory / cpus).
    Config(BmcConfig),
    /// Single QMP event emitted by QEMU.
    QmpEvent(BmcEvent),
    /// Liveness heartbeat every 15s. No payload.
    Heartbeat,
}
