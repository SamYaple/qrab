//! Build-time fixture embedder.
//!
//! For Phase 2b we want the running `qrab-boot.efi` to exercise
//! `qrab-boot-verify` with real data — both a positive and a negative
//! case — so a QEMU boot is also an end-to-end integration test of the
//! verifier on the UEFI target (not just the host `cargo test`).
//!
//! The fixtures live at
//! `projects/qrab-boot-tests/fixtures/`, populated by
//! `contrib/build-test-ukis.sh`. They are intentionally not committed,
//! so this script must tolerate their absence: when any fixture is
//! missing, we leave the bytes empty and set a `qrab_no_fixtures` cfg
//! which `main.rs` uses to skip the boot-time verify demo.
//!
//! Phase 4 adds a second input: the SEC1-uncompressed public key of
//! the attestation server (`<workspace>/contrib/attest-keys/server.sec1`
//! or `$QRAB_ATTEST_PUBKEY_SEC1`). When absent we emit a 65-byte
//! zero placeholder and a `qrab_no_server_pubkey` cfg so `main.rs`
//! can fall back to "no attestation verification" cleanly on a fresh
//! clone.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const FIXTURES: &[&str] = &[
    "db.pubkey.der",
    "uki-valid.efi",
    "uki-valid.efi.sig",
    "uki-tampered.efi",
];

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // projects/qrab-boot/ → projects/qrab-boot-tests/fixtures/
    let workspace_root = manifest
        .parent()
        .expect("CARGO_MANIFEST_DIR has no parent")
        .parent()
        .expect("workspace has no parent")
        .to_path_buf();
    let fixtures_dir = manifest
        .parent()
        .expect("CARGO_MANIFEST_DIR has no parent")
        .join("qrab-boot-tests")
        .join("fixtures");

    // Broadcast every cfg we might emit so `#[cfg(...)]` checks
    // don't produce "unexpected cfg" warnings under a stable-lint
    // toolchain.
    println!("cargo::rustc-check-cfg=cfg(qrab_no_fixtures)");
    println!("cargo::rustc-check-cfg=cfg(qrab_no_server_pubkey)");

    println!("cargo:rerun-if-changed=build.rs");
    for name in FIXTURES {
        let path = fixtures_dir.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-env-changed=QRAB_ATTEST_PUBKEY_SEC1");

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR unset"));

    let all_fixtures_present = FIXTURES.iter().all(|n| fixtures_dir.join(n).exists());
    if !all_fixtures_present {
        println!("cargo:rustc-cfg=qrab_no_fixtures");
        for name in FIXTURES {
            fs::write(out.join(name), b"").unwrap();
        }
    } else {
        for name in FIXTURES {
            let src = fixtures_dir.join(name);
            let dst = out.join(name);
            copy_or_die(&src, &dst);
        }
    }

    embed_server_pubkey(&workspace_root, &out);
}

/// Resolve the attestation-server SEC1 public key and write it to
/// `<OUT_DIR>/attest-server-pubkey.sec1`. Sets `qrab_no_server_pubkey`
/// when no real key is found.
fn embed_server_pubkey(workspace_root: &Path, out: &Path) {
    let default_path = workspace_root
        .join("contrib")
        .join("attest-keys")
        .join("server.sec1");
    let source = env::var_os("QRAB_ATTEST_PUBKEY_SEC1")
        .map(PathBuf::from)
        .unwrap_or(default_path);

    println!("cargo:rerun-if-changed={}", source.display());

    match fs::read(&source) {
        Ok(bytes) if bytes.len() == 65 && bytes[0] == 0x04 => {
            fs::write(out.join("attest-server-pubkey.sec1"), &bytes).unwrap();
        }
        Ok(bytes) => {
            println!(
                "cargo:warning=QRAB_ATTEST_PUBKEY_SEC1 at {} is {} bytes (need 65, prefix 0x04); using placeholder",
                source.display(),
                bytes.len()
            );
            write_placeholder_pubkey(out);
        }
        Err(e) => {
            println!(
                "cargo:warning=no server pubkey at {} ({e}); bootloader will not verify attestation responses",
                source.display()
            );
            write_placeholder_pubkey(out);
        }
    }
}

fn write_placeholder_pubkey(out: &Path) {
    println!("cargo:rustc-cfg=qrab_no_server_pubkey");
    fs::write(out.join("attest-server-pubkey.sec1"), [0u8; 65]).unwrap();
}

fn copy_or_die(src: &Path, dst: &Path) {
    fs::copy(src, dst).unwrap_or_else(|e| {
        panic!("build.rs: copy {} -> {}: {e}", src.display(), dst.display())
    });
}
