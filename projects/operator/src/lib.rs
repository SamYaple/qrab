pub mod api;
pub mod controller;
pub mod resources;

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
}
pub type Result<T> = std::result::Result<T, Error>;
