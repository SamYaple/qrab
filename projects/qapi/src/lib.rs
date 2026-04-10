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
    const UNSTABLE: bool;
    const AVAILABLE_SINCE: (u32, u32, u32);
    const REMOVED_IN: Option<(u32, u32, u32)>;
    /// The QEMU version when `allow-preconfig` was added for this command.
    /// `None` if the command never supports preconfig.
    const PRECONFIG_SINCE: Option<(u32, u32, u32)>;
}

/// Marker trait for commands that are safe to execute during QEMU's preconfig
/// state (before `x-exit-preconfig`). Automatically implemented by the `#[qapi]`
/// macro for commands with `allow-preconfig: true` in the QEMU schema.
///
/// This enables compile-time enforcement: `PreconfigVm::execute()` requires
/// `C: QapiCommand + PreconfigSafe`, preventing non-preconfig commands from
/// being issued before the VM is started.
///
/// The `--preconfig` flag was introduced in QEMU 3.0.0.
pub trait PreconfigSafe: QapiCommand {}

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

/// QEMU Guest Agent (QGA) types.
pub mod qga;

static QGA_VERSION: OnceLock<String> = OnceLock::new();

/// Set the QGA version string (from guest-info response).
pub fn set_qga_version(version: String) {
    QGA_VERSION.set(version).ok();
}

/// Get the QGA version. Returns empty string if not yet set.
pub fn qga_version() -> &'static str {
    QGA_VERSION.get().map(|s| s.as_str()).unwrap_or("")
}
