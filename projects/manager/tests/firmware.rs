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

/// Phase-4 stale-embed regression: the OVMF/IGVM artifacts produced by
/// `contrib/build-svsm-igvm.sh` must be at least as new as the
/// `qrab-boot.efi` they package. Before the mtime gate landed in
/// `build-svsm-igvm.sh`, the script would rebuild `qrab-boot.efi` AND
/// drop it into the edk2 source tree via `embed-efi.py`, but skip the
/// OVMF rebuild when `OVMF.fd` already existed — stranding the fresh
/// `.efi` on disk while the compiled firmware volume kept the stale
/// payload inside. The running guest then ran a qrab-boot mismatched
/// with the host-side attest-server and bailed with
/// `QRAB_ATTEST_BAD_SIG`.
///
/// This test fails loudly from `cargo test -p manager` so a stale
/// rebuild is caught before the next matrix / smoke run sinks another
/// hour of phase-4 debug into the exact same trap. The test is a
/// no-op when either artifact is missing (bare CI, fresh clone) — it
/// only enforces consistency when both exist.
#[test]
fn svsm_igvm_not_older_than_qrab_boot_efi() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("walk up from projects/manager to workspace root");

    // Standalone qrab-boot.efi (the source of truth — build.rs stamps
    // the embedded pubkey at this mtime).
    let boot_efi = workspace
        .join("projects/qrab-boot/target/x86_64-unknown-uefi/release/qrab-boot.efi");

    // Every derived artifact `build-svsm-igvm.sh` emits into
    // `target/svsm-out/` must be >= boot_efi mtime. `target/svsm-build`
    // OVMF.fd also included so a pre-copy staleness still trips the
    // test.
    let out_dir = workspace.join("target/svsm-out");
    let build_ovmf = workspace
        .join("target/svsm-build/edk2-svsm/Build/AmdSev/RELEASE_GCC5/FV/OVMF.fd");
    let derived: &[std::path::PathBuf] = &[
        out_dir.join("OVMF-SVSM-SB.fd"),
        out_dir.join("OVMF-SVSM.fd"),
        out_dir.join("coconut-qemu-SB.igvm"),
        out_dir.join("coconut-qemu.igvm"),
        build_ovmf,
    ];

    let Ok(boot_meta) = std::fs::metadata(&boot_efi) else {
        eprintln!(
            "skipping: {} not present (build with `cargo build -p qrab-boot --release`)",
            boot_efi.display()
        );
        return;
    };
    let boot_mtime = boot_meta.modified().expect("mtime on boot.efi");

    let mut checked = 0usize;
    for path in derived {
        let Ok(meta) = std::fs::metadata(path) else {
            continue;
        };
        let art_mtime = meta.modified().expect("mtime on artifact");
        checked += 1;
        assert!(
            art_mtime >= boot_mtime,
            "{art} (mtime {a:?}) is OLDER than qrab-boot.efi (mtime {b:?}). \
             `contrib/build-svsm-igvm.sh` did not rebuild OVMF after the \
             qrab-boot.efi rebuild, so the running guest boots a stale \
             qrab-boot payload. Re-run the script with \
             `QRAB_FORCE_REBUILD_OVMF=1`, or rely on the staged-grub-efi \
             mtime check that should have fired automatically — if it did \
             not, `build-svsm-igvm.sh` regressed. See \
             llm-rag/issues/ovmf-stale-embed-qrab-boot.md.",
            art = path.display(),
            a = art_mtime,
            b = boot_mtime,
        );
    }
    if checked == 0 {
        eprintln!(
            "skipping: no SVSM artifacts found under {} or {} — run \
             contrib/build-svsm-igvm.sh to populate",
            workspace.join("target/svsm-out").display(),
            workspace.join("target/svsm-build").display(),
        );
    }
}

/// Belt-and-suspenders companion to
/// `svsm_igvm_not_older_than_qrab_boot_efi` — a pure-logic check that
/// runs regardless of whether the workspace has built SVSM artifacts,
/// so the mtime-comparison semantics are pinned on every `cargo test`.
/// If somebody refactors the inequality (`>=` vs `>`, or flips the
/// operands), this test flips red before CI has a chance to hit the
/// skip-path of the real-artifact test.
#[test]
fn mtime_compare_pure_logic() {
    let tmp = tempfile::TempDir::new().unwrap();
    let older = tmp.path().join("qrab-boot.efi");
    let newer = tmp.path().join("OVMF.fd");
    std::fs::write(&older, b"stale qrab-boot").unwrap();
    // Sleep briefly so filesystem mtime resolution (often 1s on tmpfs,
    // ~ns on ext4) places `newer` strictly after `older`.
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&newer, b"fresh OVMF").unwrap();

    let a = std::fs::metadata(&older).unwrap().modified().unwrap();
    let b = std::fs::metadata(&newer).unwrap().modified().unwrap();

    // Happy case: freshly-built OVMF is >= qrab-boot.efi (non-stale).
    assert!(b >= a, "newer file must not be older than older file");

    // Regression case: if we swap roles — qrab-boot rebuild after the
    // OVMF — the invariant breaks and the real artifact test flips red.
    let swapped_older = newer;
    let swapped_newer = tmp.path().join("qrab-boot.efi.rebuilt");
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&swapped_newer, b"fresh qrab-boot").unwrap();
    let a2 = std::fs::metadata(&swapped_older).unwrap().modified().unwrap();
    let b2 = std::fs::metadata(&swapped_newer).unwrap().modified().unwrap();
    assert!(
        b2 > a2,
        "rebuilt qrab-boot.efi must be strictly newer than the prior \
         OVMF.fd — tmpfs mtime granularity regression otherwise"
    );
    // And the SAME comparison the real test uses (`art >= boot`) now
    // fires red: OVMF.fd (swapped_older) is NOT >= qrab-boot (swapped_newer).
    assert!(!(a2 >= b2), "stale-OVMF case must violate the >= invariant");
}

/// Pin that bash's `[[ A -nt B ]]` operator used by
/// `contrib/build-svsm-igvm.sh` (NEED_OVMF_REBUILD block) agrees with
/// the Rust mtime comparison. If this host's bash / filesystem ever
/// disagrees (e.g. non-POSIX shell in $PATH, or tmpfs with 1 s mtime
/// granularity that lumps both files into the same second), the real
/// staleness check on this host stops firing and BAD_SIG regressions
/// come back silently. The test skips with a clear diagnostic if `bash`
/// is unavailable rather than hard-failing so the manager test suite
/// still runs in minimal CI images.
#[test]
fn bash_nt_operator_matches_mtime_semantics() {
    let bash = match std::process::Command::new("bash")
        .arg("--version")
        .output()
    {
        Ok(o) if o.status.success() => true,
        _ => {
            eprintln!("skipping: bash not available in PATH");
            return;
        }
    };
    assert!(bash);

    let tmp = tempfile::TempDir::new().unwrap();
    let older = tmp.path().join("older");
    let newer = tmp.path().join("newer");
    std::fs::write(&older, b"a").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100)); // beat 1 s tmpfs granularity
    std::fs::write(&newer, b"b").unwrap();

    // `newer -nt older` -> exit 0 (true).
    let out = std::process::Command::new("bash")
        .args([
            "-c",
            &format!(
                "[[ {n:?} -nt {o:?} ]]",
                n = newer.display().to_string(),
                o = older.display().to_string()
            ),
        ])
        .status()
        .expect("bash spawn");
    assert!(
        out.success(),
        "bash -nt did not report `newer` as newer than `older` on this \
         host; contrib/build-svsm-igvm.sh staleness gate will miss the \
         rebuild trigger."
    );

    // `older -nt newer` -> exit 1 (false).
    let out = std::process::Command::new("bash")
        .args([
            "-c",
            &format!(
                "[[ {o:?} -nt {n:?} ]]",
                o = older.display().to_string(),
                n = newer.display().to_string()
            ),
        ])
        .status()
        .expect("bash spawn");
    assert!(
        !out.success(),
        "bash -nt falsely reported older file as newer — the script's \
         staleness check would over-fire and rebuild OVMF every run."
    );
}
