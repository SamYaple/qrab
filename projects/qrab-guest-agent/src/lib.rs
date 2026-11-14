//! qrab's guest-side agent speaking the QEMU guest agent wire protocol.
//!
//! It serves a fixed allowlist: synchronization, ping/info, OS identity and
//! time, filesystem freeze/thaw/trim, shutdown, and the namespaced
//! `qrab-update-trigger`/`qrab-update-status` commands that run only the fixed
//! [`proto::UPDATE_HOOK`]. There is no command for arbitrary execution, file
//! access or account management.
//!
//! The protocol loop is synchronous and generic over a byte stream:
//!
//! ```
//! use qrab_guest_agent::{Agent, Config, sys::Linux};
//! use std::sync::Arc;
//!
//! let agent = Agent::new(Config::default(), Arc::new(Linux::default()));
//! let mut reply = Vec::new();
//! agent.serve(&b"\xff{\"execute\":\"guest-sync-delimited\",\"arguments\":{\"id\":7}}"[..], &mut reply).unwrap();
//! assert_eq!(reply, b"\xff{\"return\":7}\n");
//! ```

mod agent;
pub mod boot;
mod firmware;
mod framing;
mod network;
pub mod proto;
pub mod secure_boot;
pub mod sys;
mod update;

pub use agent::{Agent, Config, Reply, MAX_REQUEST_BYTES};
pub use update::HookPolicy;
