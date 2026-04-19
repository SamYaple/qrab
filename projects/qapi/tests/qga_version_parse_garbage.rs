//! QGA version parser — non-numeric garbage.
//!
//! Each segment is parsed with `.parse().ok()` and defaults to `0`,
//! so `"x.y.z"` silently becomes `(0, 0, 0)`. No panic, no error.

#[test]
fn non_numeric_segments_default_to_zero() {
    assert_eq!(qapi::set_qga_version("x.y.z".to_string()), Ok(()));
    assert_eq!(qapi::qga_version(), (0, 0, 0));
}
