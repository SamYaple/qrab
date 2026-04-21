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
//! or `$QRAB_ATTEST_PUBKEY_SEC1`). R16-H15: the build now *requires*
//! one of two things:
//!   * a real key file on disk (at the default path or at
//!     `$QRAB_ATTEST_PUBKEY_SEC1`); OR
//!   * an explicit developer opt-in `QRAB_DEV_NO_PUBKEY=1`, which
//!     writes 65 zero bytes + sets `qrab_no_server_pubkey` + emits a
//!     loud `cargo:warning=` so CI logs flag the insecure build.
//! Anything else panics with instructions — fresh clones no longer
//! produce a silently-insecure bootloader.

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
    println!("cargo::rustc-check-cfg=cfg(qrab_uki_boot)");
    println!("cargo::rustc-check-cfg=cfg(qrab_require_snp)");

    println!("cargo:rerun-if-changed=build.rs");
    for name in FIXTURES {
        let path = fixtures_dir.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-env-changed=QRAB_ATTEST_PUBKEY_SEC1");
    println!("cargo:rerun-if-env-changed=QRAB_DEV_NO_PUBKEY");
    println!("cargo:rerun-if-env-changed=QRAB_UKI_BOOT");
    println!("cargo:rerun-if-env-changed=QRAB_REQUIRE_SNP");

    // Phase 5 StartImage: the terminal `LoadImage` + `StartImage`
    // path in `src/uki.rs` is gated on this cfg. Default builds (TCG
    // smoke, plain `cargo build`) leave it off so the 512-byte stub
    // UKI logs `QRAB_LOADIMAGE_STUB_SKIP` instead of trying to parse
    // a non-PE buffer through `load_image`. Real-UKI integration runs
    // (matrix tests, BMC boots) set `QRAB_UKI_BOOT=1`.
    if env::var_os("QRAB_UKI_BOOT").map(|v| v == "1").unwrap_or(false) {
        println!("cargo:rustc-cfg=qrab_uki_boot");
    }

    // F6 scenario 9: a build with `QRAB_REQUIRE_SNP=1` emits a cfg
    // that makes `main.rs` hard-fail when `ghcb::detect()` reports
    // no SEV-SNP. Lets the matrix prove an SNP-locked bootloader
    // refuses to run on a non-SNP (TCG / plain-KVM) host.
    if env::var_os("QRAB_REQUIRE_SNP")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        println!("cargo:rustc-cfg=qrab_require_snp");
    }

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
/// `<OUT_DIR>/attest-server-pubkey.sec1`. R16-H15: fail-closed. The
/// only way the build succeeds without a real key is the explicit
/// `QRAB_DEV_NO_PUBKEY=1` opt-in, which also sets the
/// `qrab_no_server_pubkey` cfg so `main.rs` refuses any response.
fn embed_server_pubkey(workspace_root: &Path, out: &Path) {
    let default_path = workspace_root
        .join("contrib")
        .join("attest-keys")
        .join("server.sec1");
    let env_source = env::var_os("QRAB_ATTEST_PUBKEY_SEC1").map(PathBuf::from);
    let dev_no_pubkey = env::var_os("QRAB_DEV_NO_PUBKEY")
        .map(|v| v == "1")
        .unwrap_or(false);

    // Path we actually read from. Env override wins; otherwise try the
    // default path iff it exists (so `QRAB_DEV_NO_PUBKEY=1` on a fresh
    // clone without any file doesn't trip the "missing file" panic).
    let source = match (&env_source, default_path.exists()) {
        (Some(p), _) => Some(p.clone()),
        (None, true) => Some(default_path.clone()),
        (None, false) => None,
    };
    if let Some(ref p) = source {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    println!("cargo:rerun-if-changed={}", default_path.display());

    match (source, dev_no_pubkey) {
        (Some(ref path), _) => match fs::read(path) {
            Ok(bytes) if bytes.len() == 65 && bytes[0] == 0x04 => {
                fs::write(out.join("attest-server-pubkey.sec1"), &bytes).unwrap();
            }
            Ok(bytes) => panic!(
                "qrab-boot build.rs: pubkey at {} is {} bytes (need exactly 65, prefix 0x04). \
                 Regenerate via `contrib/gen-attest-keys.sh` or fix the file at $QRAB_ATTEST_PUBKEY_SEC1.",
                path.display(),
                bytes.len()
            ),
            Err(e) => panic!(
                "qrab-boot build.rs: cannot read attestation-server pubkey at {}: {e}. \
                 Either drop a real key there (run `contrib/gen-attest-keys.sh`), set \
                 `$QRAB_ATTEST_PUBKEY_SEC1=<path>` to a 65-byte SEC1 file, or set \
                 `$QRAB_DEV_NO_PUBKEY=1` for a dev build that refuses responses at runtime.",
                path.display()
            ),
        },
        (None, true) => {
            // Dev opt-in: explicit, loud, cfg-gated.
            println!(
                "cargo:warning=QRAB_DEV_NO_PUBKEY=1 — qrab-boot.efi built without a server pubkey; attestation responses will be rejected at runtime. Do NOT ship this binary."
            );
            println!("cargo:rustc-cfg=qrab_no_server_pubkey");
            fs::write(out.join("attest-server-pubkey.sec1"), [0u8; 65]).unwrap();
        }
        (None, false) => panic!(
            "qrab-boot build.rs: no attestation-server pubkey. Expected one of:\n  \
             1. file at {}\n  \
             2. $QRAB_ATTEST_PUBKEY_SEC1=<path> pointing at a 65-byte SEC1 pubkey\n  \
             3. $QRAB_DEV_NO_PUBKEY=1 to opt into a dev build that rejects responses\n\
             Dev flow: run `contrib/gen-attest-keys.sh` from the workspace root.",
            default_path.display()
        ),
    }
}

fn copy_or_die(src: &Path, dst: &Path) {
    fs::copy(src, dst).unwrap_or_else(|e| {
        panic!("build.rs: copy {} -> {}: {e}", src.display(), dst.display())
    });
}
