//! QGA version parser — single `"9"` (major only).

#[test]
fn single_component_fills_zeroes() {
    assert!(qapi::set_qga_version("9".to_string()).is_ok());
    assert_eq!(qapi::qga_version(), (9, 0, 0));
}
