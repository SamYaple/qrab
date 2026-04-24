//! Build-time embedder for the attestation-server SEC1 public key.
//!
//! The SEC1-uncompressed public key of the attestation server
//! (`<workspace>/contrib/attest-keys/server.sec1` or
//! `$QRAB_ATTEST_PUBKEY_SEC1`) is embedded into `qrab-boot.efi` so
//! `attest::verify` can check the ECDSA-P256 signature on every
//! `AttestResponse`. The build fail-closes hard when no key is
//! available — there is **no dev bypass**. Fresh clones run
//! `contrib/gen-attest-keys.sh` (idempotent) once and are good
//! thereafter.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest
        .parent()
        .expect("CARGO_MANIFEST_DIR has no parent")
        .parent()
        .expect("workspace has no parent")
        .to_path_buf();

    // Broadcast every cfg we might emit so `#[cfg(...)]` checks
    // don't produce "unexpected cfg" warnings under a stable-lint
    // toolchain.
    println!("cargo::rustc-check-cfg=cfg(qrab_uki_boot)");
    println!("cargo::rustc-check-cfg=cfg(qrab_allow_non_snp)");
    println!("cargo::rustc-check-cfg=cfg(qrab_require_vtpm)");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=QRAB_ATTEST_PUBKEY_SEC1");
    println!("cargo:rerun-if-env-changed=QRAB_UKI_BOOT");
    println!("cargo:rerun-if-env-changed=QRAB_ALLOW_NON_SNP");
    println!("cargo:rerun-if-env-changed=QRAB_ALLOW_NO_VTPM");

    // Phase 5 StartImage: the terminal `LoadImage` + `StartImage`
    // path in `src/uki.rs` is gated on this cfg. Default builds (TCG
    // smoke, plain `cargo build`) leave it off so the 512-byte stub
    // UKI logs `QRAB_LOADIMAGE_STUB_SKIP` instead of trying to parse
    // a non-PE buffer through `load_image`. Real-UKI integration runs
    // (matrix tests, BMC boots) set `QRAB_UKI_BOOT=1`.
    if env::var_os("QRAB_UKI_BOOT").map(|v| v == "1").unwrap_or(false) {
        println!("cargo:rustc-cfg=qrab_uki_boot");
    }

    // R16-H14: fail-closed on non-SNP by default. `main.rs` hard-
    // halts when `ghcb::detect()` reports no SEV-SNP unless this
    // opt-out cfg is set. Only matrix scenario 9 (SNP-gate proof on
    // TCG) should ever build with `QRAB_ALLOW_NON_SNP=1`; any other
    // use ships a bootloader that'll boot on TCG / plain KVM, which
    // defeats the attest chain's host-isolation guarantee.
    if env::var_os("QRAB_ALLOW_NON_SNP")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        println!("cargo:rustc-cfg=qrab_allow_non_snp");
        println!(
            "cargo:warning=QRAB_ALLOW_NON_SNP=1 set — bootloader will NOT enforce SEV-SNP. \
             Non-prod use only (matrix scenario 9)."
        );
    }

    // R20 Phase 3: fail-closed on missing vTPM by default. `main.rs`
    // calls `measure::require_and_probe_vtpm` after the SNP gate and
    // halts with `QRAB_FAIL_NO_VTPM` when the EFI_TCG2_PROTOCOL is
    // absent, reports no TPM, lacks a SHA-256 bank, or returns an
    // all-zero PCR 0. `QRAB_ALLOW_NO_VTPM=1` compiles the check out
    // so the TCG stub smoke (no OVMF, no SVSM vTPM) still runs — it
    // emits `QRAB_TCG2_SOFT_ABSENT` instead and continues. This cfg
    // is orthogonal to `qrab_uki_boot`: the TCG smoke can opt into
    // real UKIs without having a vTPM, and a live SVSM build can
    // force-halt on missing vTPM even while still using the stub
    // LoadImage path.
    if env::var_os("QRAB_ALLOW_NO_VTPM")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        println!(
            "cargo:warning=QRAB_ALLOW_NO_VTPM=1 set — bootloader will NOT enforce a vTPM. \
             TCG/stub smoke only; production SVSM builds must leave this unset."
        );
    } else {
        println!("cargo:rustc-cfg=qrab_require_vtpm");
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR unset"));

    embed_server_pubkey(&workspace_root, &out);
}

/// Resolve the attestation-server SEC1 public key and write it to
/// `<OUT_DIR>/attest-server-pubkey.sec1`. Fail-closed: no dev bypass,
/// no zero-filled placeholder. A fresh clone must run
/// `contrib/gen-attest-keys.sh` (or set `$QRAB_ATTEST_PUBKEY_SEC1`) to
/// produce a key before `cargo build` will succeed. Previously a
/// `QRAB_DEV_NO_PUBKEY=1` opt-in wrote 65 zero bytes + a `cfg` gate;
/// that hatch was retired 2026-04-24 to match the CoCo-mandatory
/// posture (attestation is not optional on the qrab path).
fn embed_server_pubkey(workspace_root: &Path, out: &Path) {
    let default_path = workspace_root
        .join("contrib")
        .join("attest-keys")
        .join("server.sec1");
    let env_source = env::var_os("QRAB_ATTEST_PUBKEY_SEC1").map(PathBuf::from);

    // Env override wins; otherwise the default path must exist.
    let source = match (&env_source, default_path.exists()) {
        (Some(p), _) => p.clone(),
        (None, true) => default_path.clone(),
        (None, false) => panic!(
            "qrab-boot build.rs: no attestation-server pubkey. Expected one of:\n  \
             1. file at {}\n  \
             2. $QRAB_ATTEST_PUBKEY_SEC1=<path> pointing at a 65-byte SEC1 pubkey\n\
             Dev flow: run `contrib/gen-attest-keys.sh` from the workspace root (idempotent).",
            default_path.display()
        ),
    };
    println!("cargo:rerun-if-changed={}", source.display());
    // Also rerun when the default path appears/disappears, even when
    // an env override is active — we want changes to propagate either
    // way.
    println!("cargo:rerun-if-changed={}", default_path.display());

    match fs::read(&source) {
        Ok(bytes) if bytes.len() == 65 && bytes[0] == 0x04 => {
            fs::write(out.join("attest-server-pubkey.sec1"), &bytes).unwrap();
        }
        Ok(bytes) => panic!(
            "qrab-boot build.rs: pubkey at {} is {} bytes (need exactly 65, prefix 0x04). \
             Regenerate via `contrib/gen-attest-keys.sh` or fix the file at \
             $QRAB_ATTEST_PUBKEY_SEC1.",
            source.display(),
            bytes.len()
        ),
        Err(e) => panic!(
            "qrab-boot build.rs: cannot read attestation-server pubkey at {}: {e}. \
             Run `contrib/gen-attest-keys.sh` from the workspace root or set \
             `$QRAB_ATTEST_PUBKEY_SEC1=<path>` to a 65-byte SEC1 file.",
            source.display()
        ),
    }
}
