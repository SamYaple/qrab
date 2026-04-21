//! Host-side invariants on the `qrab-real-uki-demo` PE fixture.
//!
//! Covered invariants (B13-6):
//!
//! 1. **Byte-size budget** — the final PE must fit under
//!    `qrab_boot_proto::MAX_FRAMED_PAYLOAD` (128 KiB). Mirrors the
//!    `MAX_BYTES` assertion in `contrib/build-test-uki.sh`. A refactor
//!    that doubled the binary size (e.g. by accidentally pulling in
//!    a larger `uefi` feature-set) now flips red at `cargo test -p
//!    qrab-real-uki-demo-tests` time, not at the next `matrix-smoke`
//!    run.
//! 2. **PE/COFF magic** — the file begins with the `MZ` DOS stub and
//!    has a valid `e_lfanew` pointing to a `PE\0\0` signature. Catches
//!    a link script regression that would emit a non-PE blob.
//! 3. **Marker string present** — the bytes `QRAB_REAL_UKI_BOOT_OK`
//!    (the runtime token the smoke harness greps for) appear in the
//!    binary. Ensures `strip = "symbols"` + LTO never strip the
//!    constant out from under the I/O loop in `efi_main`.
//!
//! ## Skip semantics
//!
//! `qrab-real-uki-demo` is a `x86_64-unknown-uefi` crate excluded from
//! the main workspace, so `cargo test -p qrab-real-uki-demo-tests`
//! cannot build it as a side effect. These tests instead locate the
//! pre-built artefact on disk and skip (not fail) when it is absent,
//! so a bare `cargo test --workspace` on a fresh checkout does not
//! spuriously fail before `contrib/build-test-uki.sh` has been run.
//! Set `QRAB_REAL_UKI_TESTS_REQUIRE=1` to turn the skip into a hard
//! fail (CI opt-in).
//!
//! See also `projects/qrab-real-uki-demo/` and
//! `contrib/build-test-uki.sh`.

use std::path::PathBuf;

/// Mirrors `qrab_boot_proto::MAX_FRAMED_PAYLOAD = 128 * 1024`. Duplicated
/// as a literal because this crate deliberately has no Cargo dependency
/// on the wire-proto crate to keep the tests dependency-free; the sibling
/// build script and constant will trip this on drift.
const MAX_BYTES: u64 = 128 * 1024;

/// Runtime marker the scenario-1r smoke harness greps for on the guest
/// serial log. Must appear verbatim in the fixture binary — if the
/// linker strips the constant, scenario 1r silently regresses.
const MARKER: &[u8] = b"QRAB_REAL_UKI_BOOT_OK";

/// Locate the repository root by walking up from `CARGO_MANIFEST_DIR`
/// until we find a `Cargo.toml` that declares a `[workspace]` table.
/// Same pattern as `qrab-boot-verify/tests/snp_secrets_pa_matches_fdf.rs`.
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
            panic!(
                "could not find workspace root from {}",
                env!("CARGO_MANIFEST_DIR")
            );
        }
    }
}

/// Resolve the fixture path. Checked in order:
///
/// 1. `$QRAB_REAL_UKI_FIXTURE` — explicit override for CI or unusual
///    out-of-tree builds.
/// 2. `projects/qrab-boot-tests/fixtures/uki-real.efi` — the staged
///    location `contrib/build-test-uki.sh` installs to.
/// 3. `projects/qrab-real-uki-demo/target/x86_64-unknown-uefi/release/qrab-real-uki-demo.efi`
///    — the raw `cargo build --release` output from the sibling crate.
///
/// Returns `None` if no artefact is found at any of those paths; tests
/// then skip cleanly.
fn fixture_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("QRAB_REAL_UKI_FIXTURE") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    let root = repo_root();
    let staged = root.join("projects/qrab-boot-tests/fixtures/uki-real.efi");
    if staged.is_file() {
        return Some(staged);
    }
    let raw = root.join(
        "projects/qrab-real-uki-demo/target/x86_64-unknown-uefi/release/qrab-real-uki-demo.efi",
    );
    if raw.is_file() {
        return Some(raw);
    }
    None
}

/// Emit a skip reason and return `None` so the test body can early-
/// return. Honours `QRAB_REAL_UKI_TESTS_REQUIRE=1` — flips the skip
/// into a hard panic so CI jobs that run `contrib/build-test-uki.sh`
/// beforehand can enforce fixture availability.
fn fixture_or_skip(test_name: &str) -> Option<(PathBuf, Vec<u8>)> {
    let Some(path) = fixture_path() else {
        let msg = format!(
            "{test_name}: skipped — no qrab-real-uki-demo fixture found. Run \
             `contrib/build-test-uki.sh` or set `QRAB_REAL_UKI_FIXTURE=<path>` \
             to enable this test. Set `QRAB_REAL_UKI_TESTS_REQUIRE=1` to turn \
             this skip into a hard fail."
        );
        if std::env::var_os("QRAB_REAL_UKI_TESTS_REQUIRE").is_some() {
            panic!("{msg}");
        }
        eprintln!("{msg}");
        return None;
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            if std::env::var_os("QRAB_REAL_UKI_TESTS_REQUIRE").is_some() {
                panic!("{test_name}: failed to read {}: {e}", path.display());
            }
            eprintln!("{test_name}: skipped — read({}) failed: {e}", path.display());
            return None;
        }
    };
    Some((path, bytes))
}

#[test]
fn fixture_within_framed_payload_budget() {
    let Some((path, bytes)) = fixture_or_skip("fixture_within_framed_payload_budget") else {
        return;
    };
    let size = bytes.len() as u64;
    assert!(
        size <= MAX_BYTES,
        "fixture {} is {size} B, exceeds MAX_FRAMED_PAYLOAD budget {MAX_BYTES} B. \
         A qrab-real-uki-demo refactor (or uefi crate bump) has grown the PE past \
         the vsock frame cap. Shrink the binary or raise MAX_FRAMED_PAYLOAD in \
         qrab-boot-proto — but raising the cap is a protocol change.",
        path.display(),
    );
    assert!(
        size > 0,
        "fixture {} is zero bytes — aborted build?",
        path.display(),
    );
}

#[test]
fn fixture_is_valid_pe() {
    let Some((path, bytes)) = fixture_or_skip("fixture_is_valid_pe") else {
        return;
    };

    // DOS MZ header: first two bytes must be "MZ".
    assert!(
        bytes.len() >= 64,
        "fixture {} is only {} B — smaller than the DOS header",
        path.display(),
        bytes.len(),
    );
    assert_eq!(
        &bytes[0..2],
        b"MZ",
        "fixture {} does not start with the MZ DOS stub; first two bytes are {:02x?}",
        path.display(),
        &bytes[0..2],
    );

    // `e_lfanew` lives at DOS header offset 0x3c, 4 bytes little-endian,
    // and points to the PE signature.
    let e_lfanew = u32::from_le_bytes(
        bytes[0x3c..0x40]
            .try_into()
            .expect("bytes[0x3c..0x40] is 4 bytes"),
    ) as usize;
    assert!(
        e_lfanew + 4 <= bytes.len(),
        "fixture {} e_lfanew={e_lfanew:#x} points past EOF (size {} B)",
        path.display(),
        bytes.len(),
    );

    // PE signature: exactly b"PE\0\0".
    let pe_sig = &bytes[e_lfanew..e_lfanew + 4];
    assert_eq!(
        pe_sig,
        b"PE\0\0",
        "fixture {} has no PE signature at e_lfanew={e_lfanew:#x}; found {pe_sig:02x?}",
        path.display(),
    );

    // COFF Machine at PE sig + 4 must be 0x8664 (x86_64).
    let machine_off = e_lfanew + 4;
    assert!(
        machine_off + 2 <= bytes.len(),
        "fixture {} COFF header truncated",
        path.display(),
    );
    let machine = u16::from_le_bytes([bytes[machine_off], bytes[machine_off + 1]]);
    assert_eq!(
        machine, 0x8664,
        "fixture {} COFF Machine is {machine:#06x}, expected 0x8664 (x86_64). \
         A cross-target link regression may have emitted a non-x86_64 PE.",
        path.display(),
    );
}

#[test]
fn fixture_contains_runtime_marker() {
    let Some((path, bytes)) = fixture_or_skip("fixture_contains_runtime_marker") else {
        return;
    };
    assert!(
        contains_subslice(&bytes, MARKER),
        "fixture {} does not contain the runtime marker `{}` — \
         the release profile's strip/LTO has elided the constant. \
         scenario 1r's grep on serial.log will silently never match.",
        path.display(),
        std::str::from_utf8(MARKER).expect("MARKER is ASCII"),
    );
}

/// Byte-level `memmem`-style lookup. No external crate — std does not
/// expose `slice::contains_slice` on stable yet, and pulling `memchr`
/// for two tests would add a dependency the harness does not need.
fn contains_subslice(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if hay.len() < needle.len() {
        return false;
    }
    hay.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod self_tests {
    //! The skip/fail machinery above is not itself covered by the
    //! fixture-dependent tests. These self-tests exercise the local
    //! helpers so a regression in the path-resolution logic is caught
    //! regardless of whether the fixture is present.

    use super::{MARKER, MAX_BYTES, contains_subslice, repo_root};

    #[test]
    fn contains_subslice_basic() {
        assert!(contains_subslice(b"abcdef", b"cd"));
        assert!(contains_subslice(b"abcdef", b"abcdef"));
        assert!(contains_subslice(b"abc", b""));
        assert!(!contains_subslice(b"abc", b"xyz"));
        assert!(!contains_subslice(b"", b"x"));
        assert!(!contains_subslice(b"abc", b"abcd"));
    }

    #[test]
    fn marker_is_ascii_printable() {
        assert!(MARKER.iter().all(|&b| b.is_ascii_graphic()));
    }

    #[test]
    fn budget_matches_max_framed_payload_literal() {
        // Sanity: the documented budget is exactly 128 KiB.
        assert_eq!(MAX_BYTES, 128 * 1024);
    }

    #[test]
    fn repo_root_is_workspace() {
        let root = repo_root();
        let cargo = std::fs::read_to_string(root.join("Cargo.toml"))
            .expect("workspace root has a Cargo.toml");
        assert!(cargo.contains("[workspace]"));
        assert!(root.join("projects").is_dir(), "workspace has projects/");
    }
}
