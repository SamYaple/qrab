//! Firmware discovery tests.

use manager::firmware;
use std::path::Path;

#[test]
fn find_ovmf_standard_exists() {
    let fw = firmware::find_ovmf(false);
    assert!(
        fw.is_some(),
        "Standard OVMF not found — install sys-firmware/edk2-ovmf"
    );
    let fw = fw.unwrap();
    assert!(fw.code.is_file());
    assert!(fw.vars_template.is_file());
}

#[test]
fn find_ovmf_secure_exists() {
    let fw = firmware::find_ovmf(true);
    assert!(
        fw.is_some(),
        "Secure OVMF not found — install sys-firmware/edk2-ovmf"
    );
    let fw = fw.unwrap();
    assert!(fw.code.is_file());
    assert!(fw.vars_template.is_file());
}

#[test]
fn prepare_vars_copy() {
    let fw = firmware::find_ovmf(false).expect("OVMF required");
    let tmpdir = tempfile::TempDir::new().unwrap();
    let copy = firmware::prepare_vars_copy(&fw.vars_template, tmpdir.path()).unwrap();
    assert!(copy.exists());
    assert!(copy.is_file());
    // Verify it's a distinct copy, not the original
    assert_ne!(copy, fw.vars_template);
}

#[test]
fn prepare_vars_copy_content_matches() {
    let fw = firmware::find_ovmf(false).expect("OVMF required");
    let tmpdir = tempfile::TempDir::new().unwrap();
    let copy = firmware::prepare_vars_copy(&fw.vars_template, tmpdir.path()).unwrap();
    let original = std::fs::read(&fw.vars_template).unwrap();
    let copied = std::fs::read(&copy).unwrap();
    assert_eq!(
        original.len(),
        copied.len(),
        "vars copy should have same size"
    );
    assert_eq!(original, copied, "vars copy content should match original");
}

#[test]
fn prepare_vars_copy_fails_missing_template() {
    let tmpdir = tempfile::TempDir::new().unwrap();
    let result = firmware::prepare_vars_copy(Path::new("/nonexistent/OVMF_VARS.fd"), tmpdir.path());
    assert!(result.is_err());
}

#[test]
fn ovmf_firmware_paths_are_absolute() {
    if let Some(fw) = firmware::find_ovmf(false) {
        assert!(fw.code.is_absolute(), "OVMF code path should be absolute");
        assert!(
            fw.vars_template.is_absolute(),
            "OVMF vars path should be absolute"
        );
    }
    if let Some(fw) = firmware::find_ovmf(true) {
        assert!(
            fw.code.is_absolute(),
            "secure OVMF code path should be absolute"
        );
        assert!(
            fw.vars_template.is_absolute(),
            "secure OVMF vars path should be absolute"
        );
    }
}
