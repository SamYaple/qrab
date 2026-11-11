#![deny(unsafe_code)]
//! Async QMP and QGA connections for current QEMU.
//!
//! Each connection owns its version and lifecycle. See [`Qmp`] and
//! [`qga::Qga`] for cancellation and timeout semantics.
//!
//! ```no_run
//! # async fn example() -> qapi_protocol::Result<()> {
//! let (qmp, _events) = qapi_protocol::Qmp::new(std::path::Path::new("/run/vm/qmp.sock")).await?;
//! let status = qmp.execute(&qapi::commands::QueryStatus {}).await?;
//! println!("running: {}", status.running);
//! qmp.shutdown().await;
//! # Ok(())
//! # }
//! ```

mod framing;
pub mod qga;
mod qmp;
pub mod server;
mod wire;

pub use qmp::{Greeting, QemuVersion, Qmp, VersionInfo, QMP_QUEUE_CAPACITY};
use std::{sync::Arc, time::Duration};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Qmp(#[from] QmpError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Timeout(String),
    #[error("{0}")]
    Protocol(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("connection closed")]
    Closed,
    /// The original connection failure is shared with all pending callers.
    #[error("connection lost: {0}")]
    Disconnected(#[source] Arc<Error>),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Timeouts cover the entire operation, including admission and socket writes.
/// Message size counts JSON bytes, excluding the line terminator.
#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub connect_timeout: Duration,
    pub handshake_timeout: Duration,
    pub command_timeout: Duration,
    /// Accommodates QGA's documented 48 MiB file-read payload after base64.
    pub max_message_bytes: usize,
    /// Large events are dropped instead of filling the event queue with blobs.
    pub max_event_bytes: usize,
    /// QMP events are bounded and never block command responses.
    pub event_capacity: usize,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            handshake_timeout: Duration::from_secs(10),
            command_timeout: Duration::from_secs(30),
            max_message_bytes: 64 * 1024 * 1024 + 1024,
            max_event_bytes: 1024 * 1024,
            event_capacity: 128,
        }
    }
}

impl ClientOptions {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.connect_timeout.is_zero()
            || self.handshake_timeout.is_zero()
            || self.command_timeout.is_zero()
            || self.max_message_bytes == 0
            || self.max_event_bytes == 0
            || self.event_capacity == 0
            || self.event_capacity > tokio::sync::Semaphore::MAX_PERMITS
            || [
                self.connect_timeout,
                self.handshake_timeout,
                self.command_timeout,
            ]
            .iter()
            .any(|duration| tokio::time::Instant::now().checked_add(*duration).is_none())
        {
            return Err(Error::InvalidRequest(
                "limits and timeouts must be nonzero and capacities valid".into(),
            ));
        }
        Ok(())
    }
}

/// Preserves unknown error classes from newer or patched peers verbatim.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
#[error("QMP error {class}: {desc}")]
pub struct QmpError {
    pub class: String,
    pub desc: String,
}

impl QmpError {
    pub fn known_class(&self) -> Option<qapi::QapiErrorClass> {
        serde_json::from_value(serde_json::Value::String(self.class.clone())).ok()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QmpEvent {
    pub event: String,
    pub timestamp: Timestamp,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// QEMU reports both components as -1 when host time is unavailable.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Timestamp {
    pub seconds: i64,
    pub microseconds: i64,
}
