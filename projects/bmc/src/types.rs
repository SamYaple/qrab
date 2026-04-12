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
}

impl Default for BmcConfig {
    fn default() -> Self {
        Self {
            arch: "x86_64".into(),
            machine: "none".into(),
            memory: "256M".into(),
        }
    }
}
