#![deny(unsafe_code)]

pub mod arch;
pub mod firmware;
pub mod namespace;
pub mod swtpm;
pub mod vm;

pub use arch::{Arch, MachineConfig};
pub use firmware::OvmfFirmware;
pub use namespace::{InitConfig, KvmFd, SevFd};
pub use vm::{Accel, PreconfigVm, SevSnpConfig, TpmConfig, Vm, VmBuilder, PRECONFIG_MIN_VERSION};

/// Crate-level error type for manager.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Configuration or validation error.
    #[error("{0}")]
    Config(String),
    /// QEMU/swtpm process spawn or lifecycle error.
    #[error("{0}")]
    Spawn(String),
    /// Required binary not found.
    #[error("{0}")]
    Binary(String),
    /// Namespace setup error.
    #[error("{0}")]
    Namespace(String),
    /// QMP/QGA protocol error.
    #[error(transparent)]
    Protocol(#[from] qapi_protocol::Error),
    /// I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON error.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Crate-level Result alias.
pub type Result<T> = std::result::Result<T, Error>;
