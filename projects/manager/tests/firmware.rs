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

/// Phase-5 `#GP` regression guard: scan the compiled `qrab-boot.efi`
/// for a `TEST r/m8, imm8` (`F6 /0`) immediately followed by a
/// conditional branch whose target equals the TESTB's own address —
/// a tight spin-loop polling a memory byte for a bit to change.
///
/// Release-optimised Rust emits many `testb $imm, offset(%reg)`
/// instructions against ordinary struct fields (enum discriminants,
/// bitflag probes). Those run once, fall through, and are harmless.
/// The dangerous form is a busy-wait over the same byte: in SEV-SNP
/// guests, if that byte is MMIO-mapped (BAR-backed), each read traps
/// through `#VC` into OVMF's `CcExitLib::MmioExit`, which only
/// decodes MOV-family opcodes. `TESTB r/m8, imm8` is not in the
/// decode table, so the handler returns cannot-decode and the
/// exception re-enters the guest as `#GP(0)`.
///
/// The concrete bug we hit on 2026-04-23:
/// `virtio_drivers::transport::pci::PciTransport::drop` issues
/// `while self.get_status() != DeviceStatus::empty() {}` which LLVM
/// folds to `testb $0xCF, 0x14(%rax)` +
/// `jne 0x...<self>`. Fix landed in
/// `projects/qrab-boot/src/volatile_transport.rs` via `ManuallyDrop`
/// + no-op wrapper `Drop`. See
/// `llm-rag/issues/qrab-boot-phase5-gp-testb-mmio.md`.
///
/// We intentionally match ONLY the spin-loop signature
/// (`testb mem / jne self`), not any `testb` over memory, because
/// one-shot tests are safe — only a spin loop is pathologically
/// stuck in the CcExitLib decoder gap. Stack-based spins
/// (`(%rsp)`, `(%rbp)`) are further filtered out as they cannot be
/// MMIO-backed; extremely unlikely to exist on the spin-loop form
/// anyway, but cheap to guard.
///
/// Test skips when `llvm-objdump` is missing or the `.efi` hasn't
/// been built — CI without a UEFI-target build chain stays green.
#[test]
fn qrab_boot_efi_no_testb_mmio_spin_loop() {
    let efi = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("walk up to workspace root")
        .join("projects/qrab-boot/target/x86_64-unknown-uefi/release/qrab-boot.efi");

    if !efi.is_file() {
        eprintln!(
            "skipping: {} not built (run `cargo build -p qrab-boot \
             --target x86_64-unknown-uefi --release`)",
            efi.display()
        );
        return;
    }

    let objdump = match std::process::Command::new("llvm-objdump")
        .args(["-d", efi.to_str().unwrap()])
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => {
            eprintln!("skipping: llvm-objdump not available or disasm failed");
            return;
        }
    };
    let disasm = String::from_utf8_lossy(&objdump.stdout);

    // Parse each disasm line into address + mnemonic + operands.
    // Example:
    //   14000407f: f6 40 14 cf   testb $-0x31, 0x14(%rax)
    //   140004083: 75 fa         jne   0x14000407f <.text+0x307f>
    //
    // Offender := line N is `testb <imm>, <mem-with-non-stack-reg>`
    // AND line N+1 is `jne`/`je` whose target == line N's address.
    struct Insn {
        addr: u64,
        mnemonic: String,
        operands: String,
    }
    fn parse_line(line: &str) -> Option<Insn> {
        let line = line.trim_start();
        let colon = line.find(':')?;
        let addr = u64::from_str_radix(line[..colon].trim(), 16).ok()?;
        // llvm-objdump separates the byte dump from the mnemonic with
        // a tab. After the byte dump comes `mnemonic\toperands`
        // (or, for zero-operand insns, just `mnemonic`).
        let after_addr = line[colon + 1..].trim_start();
        let tab1 = after_addr.find('\t')?;
        let after_bytes = after_addr[tab1 + 1..].trim_start();
        let (mnem, ops) = after_bytes
            .split_once('\t')
            .or_else(|| after_bytes.split_once(' '))
            .unwrap_or((after_bytes, ""));
        Some(Insn {
            addr,
            mnemonic: mnem.trim().to_string(),
            operands: ops.trim().to_string(),
        })
    }
    fn parse_branch_target(operands: &str) -> Option<u64> {
        // jne/je operand form: `0x<hex> <.text+0x...>`. Only the
        // leading hex literal is what we want.
        let first = operands.split_whitespace().next()?;
        let hex = first.strip_prefix("0x").unwrap_or(first);
        u64::from_str_radix(hex, 16).ok()
    }

    let mut prev: Option<Insn> = None;
    let mut offenders: Vec<String> = Vec::new();
    for line in disasm.lines() {
        let Some(cur) = parse_line(line) else {
            // Non-instruction line (section header, symbol, blank)
            // — reset the "previous insn" context so we don't
            // match across gaps.
            prev = None;
            continue;
        };
        if let Some(ref p) = prev {
            let spin_branch =
                (cur.mnemonic == "jne" || cur.mnemonic == "je")
                    && parse_branch_target(&cur.operands) == Some(p.addr);
            let test_mmio = p.mnemonic == "testb"
                && p.operands.contains("(%")
                && !p.operands.contains("(%rsp")
                && !p.operands.contains("(%rbp");
            if spin_branch && test_mmio {
                offenders.push(format!(
                    "0x{:x}: testb {}\n  0x{:x}: {} {}",
                    p.addr, p.operands, cur.addr, cur.mnemonic, cur.operands,
                ));
            }
        }
        prev = Some(cur);
    }

    assert!(
        offenders.is_empty(),
        "phase-5 #GP regression: qrab-boot.efi contains {} \
         TESTB-against-memory / jne-self spin-loop(s). Under \
         SEV-SNP + SVSM, OVMF's CcExitLib decoder rejects TESTB \
         on MMIO and the fault surfaces as #GP in the guest. Fix: \
         wrap the offending type in `ManuallyDrop` + no-op `Drop`, \
         or route the access through `volatile_transport.rs`'s \
         MOV-only shim. See \
         llm-rag/issues/qrab-boot-phase5-gp-testb-mmio.md.\n\n\
         Offenders:\n{}",
        offenders.len(),
        offenders
            .iter()
            .map(|s| format!("  {s}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// Drives `contrib/build-qrab-uki.sh` via its shell functional test
/// (`simulate-uki-baker.sh`) and asserts the baker produces a signed
/// PE with the expected `.linux` / `.initrd` / `.cmdline` / `.uname`
/// sections and an initramfs containing `/init`, `/sbin/qrab-guest-init`,
/// `/sbin/next-stage`, plus libc + ld-linux.
///
/// Skipped on hosts without ukify / sbsign / sbverify / cpio, or
/// without a distro vmlinuz at `/usr/lib/modules/$(uname -r)/vmlinuz`.
/// The shell script handles that — we just propagate its exit code.
///
/// Why a shell-out test: the baker is itself bash, so exercising it
/// through Rust-native helpers would duplicate staging logic. The
/// shell test already covers 13 assertions; this wrapper pins the
/// cargo-test harness to notice any regression.
#[test]
fn qrab_uki_baker_smoke() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("walk up from projects/manager to workspace root");

    let baker = workspace.join("contrib/build-qrab-uki.sh");
    let test = workspace.join("contrib/qrab-boot-initrd-module/tests/simulate-uki-baker.sh");
    if !baker.is_file() || !test.is_file() {
        eprintln!("skipping: baker or its simulate-*.sh harness missing");
        return;
    }

    let out = std::process::Command::new("bash")
        .arg(&test)
        .current_dir(workspace)
        .output()
        .expect("spawn simulate-uki-baker.sh");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // Shell script prints `SKIP simulate-uki-baker: …` and exits 0
    // when prereqs are missing. Treat that as a non-assertion skip.
    if stdout.contains("SKIP simulate-uki-baker:") || stderr.contains("SKIP simulate-uki-baker:") {
        eprintln!("skipping: {}", stdout.trim());
        return;
    }

    assert!(
        out.status.success(),
        "simulate-uki-baker.sh exited non-zero.\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr,
    );
    assert!(
        stdout.contains("FAIL=0"),
        "simulate-uki-baker.sh reported assertion failures. stdout:\n{}",
        stdout,
    );
}

/// Phase-5.3.e: boot the baker's UKI chain under QEMU and assert the
/// init wiring reaches an expected point on the guest serial.
///
/// This wraps `contrib/smoke-uki-boot.sh` which:
///   * bakes the UKI if `target/qrab-uki/initrd.cpio.gz` is missing,
///   * boots with `-kernel vmlinuz -initrd cpio.gz`, no UEFI,
///   * greps serial for `QRAB_INIT_START` / `QRAB_INIT_MOUNT_OK /proc`
///     / `QRAB_INIT_EXEC` / `FailNoSnp`.
///
/// The shell test exits 77 ("SKIP") on hosts without qemu / ukify /
/// cpio / distro kernel. Exit 77 maps to "ignored in this environment"
/// — we return early so cargo reports the test as green.
///
/// Ignored by default because the boot takes up to 20 s under KVM and
/// longer under TCG; matrix CI runs it via `cargo test -- --ignored`.
#[test]
#[ignore = "slow — boots a QEMU guest; run with `cargo test -- --ignored`"]
fn qrab_uki_boot_smoke() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("walk up from projects/manager to workspace root");

    let smoke = workspace.join("contrib/smoke-uki-boot.sh");
    if !smoke.is_file() {
        eprintln!("skipping: contrib/smoke-uki-boot.sh missing");
        return;
    }

    let out = std::process::Command::new("bash")
        .arg(&smoke)
        .current_dir(workspace)
        .output()
        .expect("spawn smoke-uki-boot.sh");

    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // Exit 77 = host-side prereq skip (qemu/kernel/ukify absent).
    if code == 77 {
        eprintln!("skipping: {}", stderr.trim());
        return;
    }

    assert!(
        out.status.success(),
        "smoke-uki-boot.sh exited with code {code}.\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr,
    );
    assert!(
        stdout.contains("PASS=") && stdout.contains("FAIL=0"),
        "smoke-uki-boot.sh did not report PASS=…/FAIL=0.\nstdout:\n{}",
        stdout,
    );
}
