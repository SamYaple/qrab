#![deny(unsafe_code)]

pub mod pod;

/// Crate-level error type for manager.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Configuration or validation error.
    #[error("{0}")]
    Config(String),
    /// Process spawn or lifecycle error.
    #[error("{0}")]
    Spawn(String),
    /// Live migration failed or was refused.
    #[error("migration: {0}")]
    Migration(String),
    /// The supervisor is stopping the VM and accepts no further requests.
    #[error("VM is shutting down")]
    ShuttingDown,
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
