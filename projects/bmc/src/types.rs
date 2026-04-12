use serde::{Deserialize, Serialize};

/// VM lifecycle phases visible to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmPhase {
    Off,
    Preconfig,
    Running,
    Paused,
    ShuttingDown,
}

impl std::fmt::Display for VmPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmPhase::Off => write!(f, "Off"),
            VmPhase::Preconfig => write!(f, "Preconfig"),
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

/// VM configuration for spawning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BmcConfig {
    pub arch: String,
    pub machine: String,
    pub memory: String,
    pub display: bool,
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

impl Default for BmcConfig {
    fn default() -> Self {
        Self {
            arch: "x86_64".into(),
            machine: "uefi".into(),
            memory: "256M".into(),
            display: true,
        }
    }
}
