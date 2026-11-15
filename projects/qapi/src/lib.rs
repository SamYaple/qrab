#![deny(unsafe_code)]

use std::{cell::Cell, sync::OnceLock};

pub use qapi_macros::qapi;
pub use serde;
pub use serde_json;

/// Trait implemented by QAPI command types.
pub trait QapiCommand: serde::Serialize {
    type Returns: serde::de::DeserializeOwned + 'static;
    const NAME: &'static str;
    /// OOB flag in the newest represented schema; use `allows_oob` for a peer.
    const ALLOW_OOB: bool;
    /// Preconfig flag in the newest represented schema.
    const ALLOW_PRECONFIG: bool;
    /// Reply flag in the newest represented schema.
    const SUCCESS_RESPONSE: bool;
    /// Unstable marker in the newest represented schema.
    const UNSTABLE: bool;
    const AVAILABLE_SINCE: (u32, u32, u32);
    const REMOVED_IN: Option<(u32, u32, u32)>;
    /// The QEMU version when `allow-preconfig` was added for this command.
    /// `None` if the command never supports preconfig.
    const PRECONFIG_SINCE: Option<(u32, u32, u32)>;

    /// Whether this command permits out-of-band execution on a peer release.
    fn allows_oob(_version: (u32, u32, u32)) -> bool {
        Self::ALLOW_OOB
    }

    /// Whether this command is safe during preconfig on a peer release.
    fn allows_preconfig(_version: (u32, u32, u32)) -> bool {
        Self::ALLOW_PRECONFIG
    }

    /// Whether a successful command produces a reply on a peer release.
    fn sends_success_response(_version: (u32, u32, u32)) -> bool {
        Self::SUCCESS_RESPONSE
    }

    /// Whether the command is marked unstable in a peer release.
    fn is_unstable(_version: (u32, u32, u32)) -> bool {
        Self::UNSTABLE
    }
}

/// Marker for commands with `allow-preconfig` in at least one schema release.
/// Check `QapiCommand::allows_preconfig` when selecting a peer release.
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

/// Decode a QAPI union branch selected by its discriminator.
/// Flat union payloads are objects; historical simple unions can carry any JSON value.
pub trait QapiFlatUnion: Sized {
    fn deserialize_variant(
        discriminator: &str,
        map: serde_json::Map<String, serde_json::Value>,
    ) -> Result<Self, serde_json::Error>;

    /// Decode a branch payload, including historical scalar and array payloads.
    fn deserialize_variant_value(
        discriminator: &str,
        value: serde_json::Value,
    ) -> Result<Self, serde_json::Error> {
        use serde::de::Error as _;
        match value {
            serde_json::Value::Object(map) => Self::deserialize_variant(discriminator, map),
            _ => Err(serde_json::Error::custom(
                "expected an object for a flat union payload",
            )),
        }
    }
}

impl<T: QapiFlatUnion> QapiFlatUnion for Box<T> {
    fn deserialize_variant(
        discriminator: &str,
        map: serde_json::Map<String, serde_json::Value>,
    ) -> Result<Self, serde_json::Error> {
        T::deserialize_variant(discriminator, map).map(Box::new)
    }

    fn deserialize_variant_value(
        discriminator: &str,
        value: serde_json::Value,
    ) -> Result<Self, serde_json::Error> {
        T::deserialize_variant_value(discriminator, value).map(Box::new)
    }
}

static QEMU_VERSION: OnceLock<(u32, u32, u32)> = OnceLock::new();

thread_local! {
    static SERIALIZATION_VERSION: Cell<Option<(u32, u32, u32)>> = const { Cell::new(None) };
}

/// Scope QAPI serialization and historical wire layouts to a peer's version.
/// QMP and QGA clients use this for requests and replies, so different releases
/// can coexist.
///
/// The override is synchronous, thread-local, nestable and restored on unwind.
/// Perform the work inside the closure; do not return a future to await later.
pub fn with_version<T>(version: (u32, u32, u32), f: impl FnOnce() -> T) -> T {
    struct Restore(Option<(u32, u32, u32)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SERIALIZATION_VERSION.set(self.0);
        }
    }
    let _restore = Restore(SERIALIZATION_VERSION.replace(Some(version)));
    f()
}

/// Set the legacy process default for direct QAPI serialization.
///
/// Protocol clients scope serialization to their own version. Direct users
/// should prefer [`with_version`] when handling more than one peer.
/// Returns `Ok(())` on the first call, or `Err` if a version was already set.
/// If the new version differs, the existing process default is retained.
pub fn set_qemu_version(major: u32, minor: u32, patch: u32) -> Result<(), (u32, u32, u32)> {
    let new = (major, minor, patch);
    match QEMU_VERSION.set(new) {
        Ok(()) => Ok(()),
        Err(_) => {
            let existing = *QEMU_VERSION.get().unwrap();
            if existing != new {
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

/// Serialize a QAPI value for a particular peer version.
///
/// Generated types gate fields and whole types on the peer's QEMU version,
/// so serializing outside a version context fails. Protocol clients scope
/// this for requests; use this helper for values built elsewhere.
///
/// ```
/// let props: qapi::CpuInstanceProperties =
///     serde_json::from_value(serde_json::json!({"core-id": 1})).unwrap();
/// let error = serde_json::to_value(&props).unwrap_err();
/// assert!(error.to_string().contains("qapi::with_version"));
/// let value = qapi::to_value((11, 1, 3), &props).unwrap();
/// assert_eq!(value["core-id"], 1);
/// ```
pub fn to_value<T: serde::Serialize + ?Sized>(
    version: (u32, u32, u32),
    value: &T,
) -> serde_json::Result<serde_json::Value> {
    with_version(version, || serde_json::to_value(value))
}

/// Error text for a version-gated type serialized outside its range. Without
/// any version context it names the fix rather than blaming the peer.
#[doc(hidden)]
pub fn unavailable_type_message(type_name: &str) -> String {
    match qemu_version() {
        (0, 0, 0) => format!(
            "Type {type_name} cannot be serialized without a QEMU version; \
             wrap serialization in qapi::with_version(peer_version, ..) or use qapi::to_value"
        ),
        (major, minor, patch) => {
            format!("Type {type_name} not available in QEMU {major}.{minor}.{patch}")
        }
    }
}

/// Get the explicit peer override, excluding the legacy process default.
pub fn scoped_qemu_version() -> Option<(u32, u32, u32)> {
    SERIALIZATION_VERSION.get()
}

/// Get the scoped peer version, or the legacy process default.
/// Returns (0, 0, 0) if neither has been set.
pub fn qemu_version() -> (u32, u32, u32) {
    SERIALIZATION_VERSION
        .get()
        .unwrap_or_else(|| *QEMU_VERSION.get().unwrap_or(&(0, 0, 0)))
}

#[rustfmt::skip]
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
#[rustfmt::skip]
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
    let new = parse_qga_version(&version);
    match QGA_VERSION.set(new) {
        Ok(()) => Ok(()),
        Err(_) => {
            let existing = *QGA_VERSION.get().unwrap();
            if existing != new {
                eprintln!(
                    "qapi: QGA version already set to {}.{}.{}, ignoring {}.{}.{}",
                    existing.0, existing.1, existing.2, new.0, new.1, new.2,
                );
            }
            Err(existing)
        }
    }
}

/// Parse a guest-info version, accepting a patch suffix such as `11.1.0rc1`.
/// Missing or nonnumeric components default to zero.
pub fn parse_qga_version(version: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = version.split('.').collect();
    let major = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts
        .get(2)
        .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (major, minor, patch)
}

/// Get the QGA version. Returns `(0, 0, 0)` if not yet set.
pub fn qga_version() -> (u32, u32, u32) {
    *QGA_VERSION.get().unwrap_or(&(0, 0, 0))
}
