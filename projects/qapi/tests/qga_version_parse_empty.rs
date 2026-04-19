//! QGA version parser — empty string.
//!
//! The parser must not panic on `""`; all three components default to
//! `0`, and the OnceLock stores `(0, 0, 0)`. Note that `qga_version()`
//! also returns `(0, 0, 0)` when unset, so the `Ok` return is the only
//! distinguishing signal.

#[test]
fn empty_string_parses_as_zero_triple() {
    assert_eq!(qapi::set_qga_version(String::new()), Ok(()));
    assert_eq!(qapi::qga_version(), (0, 0, 0));
}
