#![deny(unsafe_code)]

#[cfg(feature = "legacy-host")]
pub mod arch;
#[cfg(feature = "legacy-host")]
pub mod event;
#[cfg(feature = "legacy-host")]
pub mod firmware;
#[cfg(feature = "legacy-host")]
pub mod namespace;
pub mod pod;
#[cfg(feature = "legacy-host")]
pub mod vm;

#[cfg(feature = "legacy-host")]
pub use arch::{Arch, MachineConfig};
#[cfg(feature = "legacy-host")]
pub use event::ManagerEvent;
#[cfg(feature = "legacy-host")]
pub use firmware::OvmfFirmware;
#[cfg(feature = "legacy-host")]
pub use namespace::{InitConfig, KvmFd, SevFd};
#[cfg(feature = "legacy-host")]
pub use vm::{
    Accel, NoSnp, PreconfigVm, SevSnpConfig, SvsmConfig, TpmBackend, Vm, VmBuilder, WithSnp,
    PRECONFIG_MIN_VERSION,
};

/// Crate-level error type for manager.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Configuration or validation error.
    #[error("{0}")]
    Config(String),
    /// Process spawn or lifecycle error.
    #[error("{0}")]
    Spawn(String),
    #[cfg(feature = "legacy-host")]
    /// Required binary not found.
    #[error("{0}")]
    Binary(String),
    #[cfg(feature = "legacy-host")]
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
