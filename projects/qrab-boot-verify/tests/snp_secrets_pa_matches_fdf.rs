//! Invariant-guard test: the `SNP_SECRETS_PA` constant compiled into
//! `qrab-boot` MUST match the Secrets-page physical address that
//! `vendor/edk2`'s AmdSev OVMF `.fdf` build descriptor actually emits.
//!
//! Why this lives here instead of in `qrab-boot`: `qrab-boot` is an
//! `x86_64-unknown-uefi` crate and cannot be compiled (or `cargo test`-ed)
//! on the host toolchain. `qrab-boot-verify` is a host-compiled support
//! crate sitting in the same workspace, so the test grep-matches the
//! relevant literals from the source files directly — a pure text-level
//! invariant check, no cross-target link.
//!
//! Failure mode we are guarding against: an edk2 submodule bump that
//! silently moves `MEMFD_BASE_ADDRESS` or `PcdOvmfSnpSecretsBase`
//! without touching `projects/qrab-boot/src/snp_secrets.rs`. That drift
//! would make `qrab-boot` read random memory for VMPCK0 and silently
//! break the SNP attestation round-trip (the first visible symptom
//! would be `SecretsValidateError::VersionZero` or AEAD failures far
//! downstream — hard to trace back to an OVMF layout change).
//!
//! Skip (do not fail) if `vendor/edk2/` is not populated in this
//! worktree — agents frequently operate on checkouts without the
//! submodule and hard-failing would turn a legitimate offline setup
//! into a spurious test failure. The CI run that *does* build OVMF
//! always has the submodule and will catch drift there.

use std::path::PathBuf;

/// Locate the repository root by walking up from `CARGO_MANIFEST_DIR`
/// until we find a `Cargo.toml` that declares a `[workspace]` table.
fn repo_root() -> PathBuf {
    let mut cur = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let cargo = cur.join("Cargo.toml");
        if cargo.exists() {
            let text = std::fs::read_to_string(&cargo).unwrap_or_default();
            if text.contains("[workspace]") {
                return cur;
            }
        }
        if !cur.pop() {
            panic!("could not find workspace root from {}", env!("CARGO_MANIFEST_DIR"));
        }
    }
}

/// Parse `SNP_SECRETS_PA` value out of `projects/qrab-boot/src/snp_secrets.rs`.
///
/// Looks for a line matching `pub const SNP_SECRETS_PA: u64 = <expr>;`,
/// strips underscores, and evaluates the `a + b` form the current source
/// uses (`0x0080_0000 + 0x0D000`).
fn parse_rust_constant(src: &str) -> Option<u64> {
    for line in src.lines() {
        let line = line.trim();
        let prefix = "pub const SNP_SECRETS_PA: u64 =";
        if let Some(rest) = line.strip_prefix(prefix) {
            let expr = rest.trim_end_matches(';').trim().replace('_', "");
            let mut sum: u64 = 0;
            for term in expr.split('+') {
                let t = term.trim();
                let v = if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    u64::from_str_radix(hex, 16).ok()?
                } else {
                    t.parse::<u64>().ok()?
                };
                sum = sum.checked_add(v)?;
            }
            return Some(sum);
        }
    }
    None
}

/// Parse `DEFINE MEMFD_BASE_ADDRESS = 0x800000` out of
/// `OvmfPkg/Include/Fdf/OvmfPkgDefines.fdf.inc`.
fn parse_memfd_base(src: &str) -> Option<u64> {
    for line in src.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("DEFINE MEMFD_BASE_ADDRESS") {
            let after_eq = rest.split('=').nth(1)?.trim();
            let hex = after_eq.strip_prefix("0x").or_else(|| after_eq.strip_prefix("0X"))?;
            return u64::from_str_radix(hex.trim(), 16).ok();
        }
    }
    None
}

/// Parse the offset (first field of `<off>|<size>`) whose following
/// line declares `PcdOvmfSnpSecretsBase`, from
/// `OvmfPkg/Include/Fdf/MemFd.fdf.inc`.
fn parse_snp_secrets_offset(src: &str) -> Option<u64> {
    let lines: Vec<&str> = src.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.contains("PcdOvmfSnpSecretsBase") {
            // Walk backward for the nearest `0xNNNN|0xNNNN` line.
            for j in (0..i).rev() {
                let prev = lines[j].trim();
                if prev.is_empty() || prev.starts_with('#') {
                    continue;
                }
                let off = prev.split('|').next()?.trim();
                let hex = off.strip_prefix("0x").or_else(|| off.strip_prefix("0X"))?;
                return u64::from_str_radix(hex, 16).ok();
            }
        }
    }
    None
}

#[test]
fn snp_secrets_pa_matches_ovmf_fdf_layout() {
    let root = repo_root();

    let rust_path = root.join("projects/qrab-boot/src/snp_secrets.rs");
    let defines_path = root.join("vendor/edk2/OvmfPkg/Include/Fdf/OvmfPkgDefines.fdf.inc");
    let memfd_path = root.join("vendor/edk2/OvmfPkg/Include/Fdf/MemFd.fdf.inc");

    // qrab-boot must exist — it is a workspace member. This is a hard
    // assertion rather than a skip because absence indicates a broken
    // checkout, not an unpopulated submodule.
    assert!(
        rust_path.exists(),
        "{} missing — qrab-boot source not in this checkout",
        rust_path.display(),
    );

    if !defines_path.exists() || !memfd_path.exists() {
        eprintln!(
            "SKIP: vendor/edk2 submodule not populated; run \
             `git submodule update --init vendor/edk2` to enable the \
             SNP_SECRETS_PA invariant check"
        );
        return;
    }

    let rust_src = std::fs::read_to_string(&rust_path)
        .expect("read qrab-boot/src/snp_secrets.rs");
    let defines_src = std::fs::read_to_string(&defines_path)
        .expect("read OvmfPkgDefines.fdf.inc");
    let memfd_src = std::fs::read_to_string(&memfd_path)
        .expect("read MemFd.fdf.inc");

    let rust_pa = parse_rust_constant(&rust_src)
        .expect("parse SNP_SECRETS_PA from qrab-boot source");
    let memfd_base = parse_memfd_base(&defines_src)
        .expect("parse MEMFD_BASE_ADDRESS from OvmfPkgDefines.fdf.inc");
    let secrets_offset = parse_snp_secrets_offset(&memfd_src)
        .expect("parse PcdOvmfSnpSecretsBase offset from MemFd.fdf.inc");
    let fdf_pa = memfd_base + secrets_offset;

    assert_eq!(
        rust_pa, fdf_pa,
        "SNP_SECRETS_PA drift detected!\n  \
         qrab-boot SNP_SECRETS_PA = {rust_pa:#x}\n  \
         MEMFD_BASE_ADDRESS       = {memfd_base:#x}\n  \
         PcdOvmfSnpSecretsBase    = {secrets_offset:#x}\n  \
         computed OVMF .fdf PA    = {fdf_pa:#x}\n\n\
         Either edk2 moved the Secrets page or the Rust constant is \
         stale. Update projects/qrab-boot/src/snp_secrets.rs and the \
         upstream plan in llm-rag/reference/snp-secrets-upstream-plan.md."
    );

    // Sanity: the value the source-of-truth build system produces today.
    // Hard-code here so that a future edit that "fixes" the test by
    // copying a new bad value into both places still trips this check.
    assert_eq!(
        rust_pa, 0x0080_D000,
        "SNP_SECRETS_PA must equal 0x80D000 for the current AmdSev \
         OVMF layout; got {rust_pa:#x}"
    );
}
