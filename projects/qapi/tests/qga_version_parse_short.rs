//! QGA version parser — short `"9.1"` (no patch).
//!
//! Missing patch segment must silently default to `0`, matching the
//! codegen's `parse_version` helper.

#[test]
fn two_component_version_fills_patch_zero() {
    assert!(qapi::set_qga_version("9.1".to_string()).is_ok());
    assert_eq!(qapi::qga_version(), (9, 1, 0));
}
