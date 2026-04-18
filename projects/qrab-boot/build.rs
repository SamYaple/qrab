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
    let fixtures_dir = manifest
        .parent()
        .expect("CARGO_MANIFEST_DIR has no parent")
        .join("qrab-boot-tests")
        .join("fixtures");

    // Every `include_bytes!` consumer benefits from re-running this
    // script whenever any fixture on disk changes.
    println!("cargo:rerun-if-changed=build.rs");
    for name in FIXTURES {
        let path = fixtures_dir.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let all_present = FIXTURES.iter().all(|n| fixtures_dir.join(n).exists());
    if !all_present {
        // Broadcast the cfg so the runtime code path can skip the demo.
        // The key point is that the *crate still builds* — a fresh
        // clone without SB keys must not fail `cargo build`.
        println!("cargo::rustc-check-cfg=cfg(qrab_no_fixtures)");
        println!("cargo:rustc-cfg=qrab_no_fixtures");
        // Still emit empty placeholder files so `include_bytes!` calls
        // compile without conditional imports sprinkled through main.rs.
        let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR unset"));
        for name in FIXTURES {
            fs::write(out.join(name), b"").unwrap();
        }
        return;
    }

    // Copy each fixture into OUT_DIR so we can include them with stable
    // paths (`include_bytes!(concat!(env!("OUT_DIR"), "/..."))`).
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR unset"));
    for name in FIXTURES {
        let src = fixtures_dir.join(name);
        let dst = out.join(name);
        copy_or_die(&src, &dst);
    }
    println!("cargo::rustc-check-cfg=cfg(qrab_no_fixtures)");
}

fn copy_or_die(src: &Path, dst: &Path) {
    fs::copy(src, dst).unwrap_or_else(|e| {
        panic!("build.rs: copy {} -> {}: {e}", src.display(), dst.display())
    });
}
