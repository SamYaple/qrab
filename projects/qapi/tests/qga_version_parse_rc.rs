//! QGA version parser — `9.1.0-rc0` patch-suffix stripping.
//!
//! The patch segment is parsed up to the first non-ASCII-digit, so
//! `"9.1.0-rc0"` must yield `(9, 1, 0)`, not panic.

#[test]
fn rc_suffix_on_patch_is_stripped() {
    assert!(qapi::set_qga_version("9.1.0-rc0".to_string()).is_ok());
    assert_eq!(qapi::qga_version(), (9, 1, 0));
}
