#![deny(unsafe_code)]

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

/// Trait for discriminator-based flat union deserialization.
///
/// QAPI flat unions embed the discriminator and branch-specific fields in the
/// same JSON object. The `#[qapi]` proc macro generates an impl for each union
/// branch enum so that the containing struct's deserializer can select the
/// correct variant by discriminator value instead of brute-force trial
/// deserialization.
pub trait QapiFlatUnion: Sized {
    fn deserialize_variant(
        discriminator: &str,
        map: serde_json::Map<String, serde_json::Value>,
    ) -> Result<Self, serde_json::Error>;
}

impl<T: QapiFlatUnion> QapiFlatUnion for Box<T> {
    fn deserialize_variant(
        discriminator: &str,
        map: serde_json::Map<String, serde_json::Value>,
    ) -> Result<Self, serde_json::Error> {
        T::deserialize_variant(discriminator, map).map(Box::new)
    }
}

static QEMU_VERSION: OnceLock<(u32, u32, u32)> = OnceLock::new();

/// Set the QEMU version for runtime version gating.
///
/// Should be called once after connecting to QEMU and querying its version.
/// Returns `Ok(())` on the first call, or `Err` if a version was already set.
/// If the new version differs from the existing one, a warning is logged —
/// this catches the multi-VM case where a second QEMU instance with a
/// different version silently gets the wrong version gating.
pub fn set_qemu_version(major: u32, minor: u32, patch: u32) -> Result<(), (u32, u32, u32)> {
    let new = (major, minor, patch);
    match QEMU_VERSION.set(new) {
        Ok(()) => Ok(()),
        Err(_) => {
            let existing = *QEMU_VERSION.get().unwrap();
            if existing != new {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    existing = ?existing,
                    attempted = ?new,
                    "QEMU version already set to {}.{}.{}, ignoring {}.{}.{} — \
                     OnceLock only stores the first value. Multi-VM processes with \
                     different QEMU versions need per-connection version tracking.",
                    existing.0, existing.1, existing.2,
                    new.0, new.1, new.2,
                );
                #[cfg(not(feature = "tracing"))]
                eprintln!(
                    "qapi: QEMU version already set to {}.{}.{}, ignoring {}.{}.{} — \
                     multi-VM version mismatch",
                    existing.0, existing.1, existing.2, new.0, new.1, new.2,
                );
            }
            Err(existing)
        }
    }
}

/// Get the QEMU version. Returns (0, 0, 0) if not yet set.
pub fn qemu_version() -> (u32, u32, u32) {
    *QEMU_VERSION.get().unwrap_or(&(0, 0, 0))
}

mod generated;
/// QMP data types (structs, enums, unions) re-exported at the crate root.
pub use generated::*;

/// QMP command types — access via `qapi::commands::QueryStatus`, etc.
pub use generated::commands;

/// QMP event types — access via `qapi::events::ShutdownEvent`, etc.
pub use generated::events;

/// Compile-time schema metadata — access via `qapi::metadata::COMMANDS`,
/// `qapi::metadata::STRUCTS`, etc. Used by tooling that needs to render
/// QAPI schema information at runtime (e.g. the BMC web UI).
pub use generated::metadata;

/// QEMU Guest Agent (QGA) types.
pub mod qga;

static QGA_VERSION: OnceLock<(u32, u32, u32)> = OnceLock::new();

/// Set the QGA version from the `guest-info` response string.
///
/// The schema declares `GuestAgentInfo.version` as an opaque `String`, but in
/// every released `qemu-ga` it is formatted as `"major.minor.patch"`. We parse
/// that eagerly so callers see the same `(u32, u32, u32)` shape as
/// [`qemu_version`], making the two APIs interchangeable. Non-numeric segments
/// silently become `0`, matching the behaviour of the codegen's own
/// `parse_version` helper.
///
/// Returns `Err` with the existing version if already set.
pub fn set_qga_version(version: String) -> Result<(), (u32, u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();
    let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts
        .get(2)
        .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let new = (major, minor, patch);
    match QGA_VERSION.set(new) {
        Ok(()) => Ok(()),
        Err(_) => {
            let existing = *QGA_VERSION.get().unwrap();
            if existing != new {
                #[cfg(feature = "tracing")]
                tracing::warn!(
                    existing = ?existing,
                    attempted = ?new,
                    "QGA version already set to {}.{}.{}, ignoring {}.{}.{}",
                    existing.0, existing.1, existing.2,
                    new.0, new.1, new.2,
                );
                #[cfg(not(feature = "tracing"))]
                eprintln!(
                    "qapi: QGA version already set to {}.{}.{}, ignoring {}.{}.{}",
                    existing.0, existing.1, existing.2, new.0, new.1, new.2,
                );
            }
            Err(existing)
        }
    }
}

/// Get the QGA version. Returns `(0, 0, 0)` if not yet set.
pub fn qga_version() -> (u32, u32, u32) {
    *QGA_VERSION.get().unwrap_or(&(0, 0, 0))
}
