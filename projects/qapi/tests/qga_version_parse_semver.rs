//! QGA version parser — standard `major.minor.patch` and OnceLock
//! idempotence.
//!
//! Combined into one `#[test]` to keep test-binary ordering
//! deterministic: parallel test harnesses would otherwise race on the
//! single `set_qga_version` slot.

#[test]
fn standard_semver_then_second_set_is_noop() {
    // First set: standard semver parses to (9, 1, 0).
    assert!(qapi::set_qga_version("9.1.0".to_string()).is_ok());
    assert_eq!(qapi::qga_version(), (9, 1, 0));

    // Second set: OnceLock rejects with the already-stored value.
    let err = qapi::set_qga_version("7.2.3".to_string());
    assert_eq!(err, Err((9, 1, 0)));
    assert_eq!(qapi::qga_version(), (9, 1, 0));
}
