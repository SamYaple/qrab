//! Host-side integration tests for `qrab-boot-verify` driven by the
//! fixtures that `contrib/build-test-ukis.sh` writes to
//! `projects/qrab-boot-tests/fixtures/`.
//!
//! Running:
//!
//!     # One-time (or whenever sources change):
//!     contrib/sb-enroll-keys.sh
//!     contrib/build-test-ukis.sh
//!
//!     # Then:
//!     cargo test -p qrab-boot-verify
//!
//! If the fixtures are missing the tests are marked skipped (rather
//! than failing) so a fresh clone without SB keys does not trip CI.

use std::path::PathBuf;

use qrab_boot_verify::{VerifyError, verify_detached};

fn fixtures_dir() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // projects/qrab-boot-verify/ → projects/qrab-boot-tests/fixtures/
    manifest
        .parent()
        .expect("CARGO_MANIFEST_DIR has no parent")
        .join("qrab-boot-tests")
        .join("fixtures")
}

fn load(name: &str) -> Option<Vec<u8>> {
    let path = fixtures_dir().join(name);
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("reading {}: {e}", path.display()),
    }
}

fn require(name: &str) -> Vec<u8> {
    match load(name) {
        Some(b) => b,
        None => panic!(
            "fixture {name} missing under {} — run contrib/build-test-ukis.sh",
            fixtures_dir().display()
        ),
    }
}

fn fixtures_available() -> bool {
    ["uki-valid.efi", "uki-valid.efi.sig", "db.pubkey.der"]
        .iter()
        .all(|f| fixtures_dir().join(f).exists())
}

#[test]
fn valid_signature_passes() {
    if !fixtures_available() {
        eprintln!("skipping: run contrib/build-test-ukis.sh to populate fixtures");
        return;
    }
    let payload = require("uki-valid.efi");
    let sig = require("uki-valid.efi.sig");
    let pubkey = require("db.pubkey.der");
    verify_detached(&payload, &sig, &pubkey).expect("valid fixture must verify");
}

#[test]
fn tampered_payload_rejected() {
    if !fixtures_available() {
        eprintln!("skipping: run contrib/build-test-ukis.sh to populate fixtures");
        return;
    }
    // The tampered fixture carries a valid Authenticode sig (sbsign
    // wrapping) but the raw bytes drifted by one byte after signing,
    // so the detached RSA-SHA256 over the modified bytes must fail.
    let payload = require("uki-tampered.efi");
    let sig = require("uki-valid.efi.sig");
    let pubkey = require("db.pubkey.der");
    match verify_detached(&payload, &sig, &pubkey) {
        Err(VerifyError::BadSignature) => {}
        other => panic!("expected BadSignature, got {other:?}"),
    }
}

#[test]
fn truncated_signature_rejected() {
    if !fixtures_available() {
        eprintln!("skipping: run contrib/build-test-ukis.sh to populate fixtures");
        return;
    }
    let payload = require("uki-valid.efi");
    let mut sig = require("uki-valid.efi.sig");
    sig.truncate(4);
    let pubkey = require("db.pubkey.der");
    // The rsa crate accepts short-byte inputs at decode time and
    // rejects at verify time; both `SignatureDecode` and `BadSignature`
    // are acceptable rejections here. The only unacceptable outcome is
    // `Ok`.
    match verify_detached(&payload, &sig, &pubkey) {
        Err(VerifyError::SignatureDecode | VerifyError::BadSignature) => {}
        other => panic!("expected SignatureDecode or BadSignature, got {other:?}"),
    }
}

#[test]
fn garbage_pubkey_rejected() {
    if !fixtures_available() {
        eprintln!("skipping: run contrib/build-test-ukis.sh to populate fixtures");
        return;
    }
    let payload = require("uki-valid.efi");
    let sig = require("uki-valid.efi.sig");
    let mut pubkey = require("db.pubkey.der");
    pubkey[0] ^= 0xff;
    match verify_detached(&payload, &sig, &pubkey) {
        Err(VerifyError::PubkeyParse) => {}
        // Some byte flips may still land in a parsable-but-wrong key,
        // in which case the sig simply fails to verify — both are
        // acceptable rejections.
        Err(VerifyError::BadSignature) => {}
        other => panic!("expected PubkeyParse or BadSignature, got {other:?}"),
    }
}
