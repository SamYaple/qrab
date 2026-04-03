use std::sync::OnceLock;

pub use qapi_macros::qapi;
pub use serde;
pub use serde_json;

/// Trait implemented by QAPI command types.
pub trait QapiCommand: serde::Serialize {
    type Returns: serde::de::DeserializeOwned;
    const NAME: &'static str;
    const ALLOW_OOB: bool;
    const ALLOW_PRECONFIG: bool;
    const SUCCESS_RESPONSE: bool;
}

/// Trait implemented by QAPI event types.
pub trait QapiEvent: serde::de::DeserializeOwned {
    const NAME: &'static str;
}

static QEMU_VERSION: OnceLock<(u32, u32, u32)> = OnceLock::new();

/// Set the QEMU version for runtime version gating.
/// Should be called once after connecting to QEMU and querying its version.
pub fn set_qemu_version(major: u32, minor: u32, patch: u32) {
    QEMU_VERSION.set((major, minor, patch)).ok();
}

/// Get the QEMU version. Returns (0, 0, 0) if not yet set.
pub fn qemu_version() -> (u32, u32, u32) {
    *QEMU_VERSION.get().unwrap_or(&(0, 0, 0))
}

mod generated;
pub use generated::*;
