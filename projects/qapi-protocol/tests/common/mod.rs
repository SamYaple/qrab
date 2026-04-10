pub mod fixtures;
pub mod serial;

// Re-export from manager for test convenience
pub use manager::arch::detect_version;
pub use manager::{Arch, MachineConfig, Vm, VmBuilder};

use qapi_protocol::QmpEvent;
use std::time::Duration;
use tokio::sync::mpsc;

// ── Event helpers ────────────────────────────────────────────────────────────

/// Wait for a QMP event with a specific name.
pub async fn wait_for_event(
    events: &mut mpsc::Receiver<QmpEvent>,
    event_name: &str,
    timeout: Duration,
) -> Option<QmpEvent> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, events.recv()).await {
            Ok(Some(evt)) if evt.event == event_name => return Some(evt),
            Ok(Some(_)) => continue,
            _ => return Option::None,
        }
    }
}

/// Wait for a typed QMP event, deserializing the data field.
#[allow(dead_code)]
pub async fn wait_for_typed_event<T: qapi::QapiEvent + serde::de::DeserializeOwned>(
    events: &mut mpsc::Receiver<QmpEvent>,
    timeout: Duration,
) -> Option<T> {
    let evt = wait_for_event(events, T::NAME, timeout).await?;
    let data = if evt.data.is_null() {
        serde_json::Value::Object(serde_json::Map::new())
    } else {
        evt.data
    };
    serde_json::from_value(data).ok()
}

/// Event/operation timeout from env or default 30s.
pub fn event_timeout() -> Duration {
    let secs: u64 = std::env::var("QEMU_TEST_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

/// Boot timeout from env or default 30s.
pub fn boot_timeout() -> Duration {
    let secs: u64 = std::env::var("QEMU_TEST_BOOT_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

/// Set QEMU version to 99.0.0 for non-QEMU serde tests.
pub fn ensure_version_set() {
    qapi::set_qemu_version(99, 0, 0);
}

// ── Test generation macros ───────────────────────────────────────────────────

/// Generate a test function for each specified architecture.
#[macro_export]
macro_rules! arch_tests {
    ($name:ident, [$($arch:ident),+ $(,)?], $body:expr) => {
        $(
            paste::paste! {
                #[tokio::test]
                async fn [<$name _ $arch:lower>]() {
                    use common::*;
                    let arch = Arch::$arch;
                    if !arch.is_available() {
                        eprintln!("SKIP: {} not available ({})", arch, arch.binary().display());
                        return;
                    }
                    ($body)(arch).await;
                }
            }
        )+
    };
}

/// Like arch_tests! but also checks QEMU version.
#[macro_export]
macro_rules! arch_version_tests {
    ($name:ident, [$($arch:ident),+ $(,)?], >= ($maj:expr, $min:expr), $body:expr) => {
        $(
            paste::paste! {
                #[tokio::test]
                async fn [<$name _ $arch:lower>]() {
                    use common::*;
                    let arch = Arch::$arch;
                    if !arch.is_available() {
                        eprintln!("SKIP: {} not available ({})", arch, arch.binary().display());
                        return;
                    }
                    if let Some(v) = detect_version(arch) {
                        if v < ($maj, $min, 0) {
                            eprintln!("SKIP: {} is v{}.{}.{}, need >= {}.{}", arch, v.0, v.1, v.2, $maj, $min);
                            return;
                        }
                    }
                    ($body)(arch).await;
                }
            }
        )+
    };
}
