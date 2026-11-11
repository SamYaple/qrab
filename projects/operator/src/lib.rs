pub mod api;
pub mod controller;
pub mod provision;
pub mod resources;
pub mod storage;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid VM configuration: {0}")]
    Invalid(String),
    #[error("resource ownership conflict: {0}")]
    Ownership(String),
    #[error(transparent)]
    Kubernetes(#[from] kube::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Storage(#[from] storage::StorageError),
}
pub type Result<T> = std::result::Result<T, Error>;
