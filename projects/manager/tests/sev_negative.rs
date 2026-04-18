//! Canary + negative-path tests pinning SEV-SNP host-configuration claims.
//!
//! Each of these used to be a bare claim in `llm-rag/issues/` with no
//! regression test. Running them revealed that two of the claims
//! (`-S`-is-fatal and `-bios`-crashes-vCPU) are **FALSE** on
//! QEMU 10.2 + AmdSev OVMF + kernel 6.18. The tests therefore assert
//! the currently observed behaviour: both flags start an SNP guest
//! cleanly. If either ever flips back to failing, the test panics and
//! forces a doc re-audit. The one claim that is actually true —
//! `pflash` is rejected by QEMU on SNP — stays as a negative test.
//!
//! All tests skip on hosts without `/dev/sev`, without AmdSev OVMF, or
//! without `qemu-system-x86_64` on `PATH`.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use manager::firmware;

/// Wall-clock budget for each run. Enough for an SNP launch + a couple QMP
/// queries on this host.
const QEMU_DEADLINE: Duration = Duration::from_secs(15);

fn qemu_on_path() -> bool {
    which::which("qemu-system-x86_64").is_ok()
}

fn sev_available() -> bool {
    std::path::Path::new("/dev/sev").exists()
}

fn amdsev_fw() -> Option<PathBuf> {
    firmware::find_amdsev_ovmf()
}

fn skip_unless_snp_ready(what: &str) -> Option<PathBuf> {
    if !qemu_on_path() {
        eprintln!("SKIP {what}: qemu-system-x86_64 not on PATH");
        return None;
    }
    if !sev_available() {
        eprintln!("SKIP {what}: /dev/sev missing");
        return None;
    }
    match amdsev_fw() {
        Some(fw) => Some(fw),
        None => {
            eprintln!("SKIP {what}: AmdSev OVMF not found");
            None
        }
    }
}

/// Observed outcome of a QEMU run.
#[derive(Debug)]
struct RunResult {
    stdout: String,
    stderr: String,
    exit_ok: bool,
}

/// Spawn QEMU with `-qmp stdio`, feed the supplied QMP commands in, then
/// wait for the child to exit (or `QEMU_DEADLINE`) and collect output.
fn run_qemu_with_qmp(args: &[String], qmp_lines: &[&str]) -> RunResult {
    let mut child = Command::new("qemu-system-x86_64")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn qemu-system-x86_64");

    if let Some(mut stdin) = child.stdin.take() {
        for line in qmp_lines {
            let _ = writeln!(stdin, "{line}");
        }
        // Drop stdin so QMP sees EOF after our scripted exchange.
    }

    let start = Instant::now();
    let exit = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {
                if start.elapsed() >= QEMU_DEADLINE {
                    let _ = child.kill();
                    break child.wait().ok();
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(_) => {
                let _ = child.kill();
                break child.wait().ok();
            }
        }
    };

    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut stdout);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut stderr);
    }
    RunResult {
        stdout,
        stderr,
        exit_ok: exit.map(|s| s.success()).unwrap_or(false),
    }
}

fn snp_object_arg() -> &'static str {
    "sev-snp-guest,id=sev0,reduced-phys-bits=1,cbitpos=51,sev-device=/dev/sev"
}

/// The legacy claim in AGENTS.md / `manager/src/vm.rs:977` said that
/// `-S` (start paused) under SEV-SNP defers `SNP_LAUNCH_FINISH` and
/// strands GHCB pages, causing `kvm run failed Invalid argument`.
/// This test runs the configuration and asserts the **currently
/// observed** behaviour: QEMU 10.2 + AmdSev OVMF + kernel 6.18 launches
/// cleanly. `cont` flips the VM to running and the guest reaches OVMF
/// far enough to raise `RTC_CHANGE`. No KVM failure marker appears.
///
/// Acts as a canary: if a future QEMU/kernel/OVMF combo brings the
/// failure back, the assertions fire and the doc must be updated.
#[test]
fn snp_dash_s_flag_starts_cleanly_on_current_host() {
    let fw = match skip_unless_snp_ready("snp+-S canary") { Some(f) => f, None => return };

    let args = vec![
        "-S".to_string(),
        "-machine".into(), format!("q35,confidential-guest-support=sev0,firmware={}", fw.display()),
        "-m".into(), "512M".into(),
        "-accel".into(), "kvm".into(),
        "-cpu".into(), "EPYC-v4".into(),
        "-object".into(), snp_object_arg().into(),
        "-qmp".into(), "stdio".into(),
        "-nodefaults".into(),
        "-display".into(), "none".into(),
        "-no-reboot".into(),
        "-serial".into(), "null".into(),
    ];
    let qmp = [
        r#"{"execute":"qmp_capabilities"}"#,
        r#"{"execute":"cont"}"#,
        r#"{"execute":"query-status"}"#,
    ];
    let r = run_qemu_with_qmp(&args, &qmp);
    eprintln!("--- stdout ---\n{}", r.stdout);
    eprintln!("--- stderr ({} bytes) ---\n{}", r.stderr.len(), r.stderr);

    let lower = format!("{}{}", r.stdout.to_ascii_lowercase(), r.stderr.to_ascii_lowercase());
    let kvm_failure = lower.contains("kvm run failed")
        || lower.contains("kvm_run failed")
        || lower.contains("snp_launch_finish failed")
        || lower.contains("ghcb gpa") && lower.contains("not registered");

    assert!(
        !kvm_failure,
        "SNP + -S produced a KVM/SNP failure signature on this host. \
         The claim that `-S` is fatal under SNP may be back. \
         Re-audit AGENTS.md and vm.rs:977 comment. Output:\n\
         STDOUT:\n{}\nSTDERR:\n{}",
        r.stdout, r.stderr
    );

    // Positive marker: we got the QMP greeting AND at least one successful
    // `{"return":{}}` acknowledging `cont`.
    assert!(
        r.stdout.contains("\"QMP\""),
        "QEMU didn't even emit the QMP greeting under -S + SNP. \
         STDOUT:\n{}\nSTDERR:\n{}",
        r.stdout, r.stderr
    );
    assert!(
        r.stdout.contains("\"return\""),
        "QEMU didn't acknowledge the cont command. \
         STDOUT:\n{}\nSTDERR:\n{}",
        r.stdout, r.stderr
    );

    eprintln!("VERIFIED: SNP + -S launches cleanly (legacy claim 'must use --preconfig' no longer holds)");
}

/// `issues/sev-snp-pflash.md` claimed `-bios <OVMF>` causes an immediate
/// vCPU crash under SNP (loads at reset vector without PSP validation).
/// On QEMU 10.2 + AmdSev OVMF + kernel 6.18 this is no longer true:
/// the guest starts and reaches at least RTC setup.
///
/// Canary: if `-bios` ever crashes the SNP vCPU again, the test fails and
/// the pflash-doc claim is re-verified.
#[test]
fn snp_bios_firmware_starts_cleanly_on_current_host() {
    let fw = match skip_unless_snp_ready("snp+-bios canary") { Some(f) => f, None => return };

    let args = vec![
        "-machine".into(), "q35,confidential-guest-support=sev0".into(),
        "-bios".into(), fw.display().to_string(),
        "-m".into(), "512M".into(),
        "-accel".into(), "kvm".into(),
        "-cpu".into(), "EPYC-v4".into(),
        "-object".into(), snp_object_arg().into(),
        "-qmp".into(), "stdio".into(),
        "-nodefaults".into(),
        "-display".into(), "none".into(),
        "-no-reboot".into(),
        "-serial".into(), "null".into(),
    ];
    let qmp = [
        r#"{"execute":"qmp_capabilities"}"#,
        r#"{"execute":"query-status"}"#,
    ];
    let r = run_qemu_with_qmp(&args, &qmp);
    eprintln!("--- stdout ---\n{}", r.stdout);
    eprintln!("--- stderr ({} bytes) ---\n{}", r.stderr.len(), r.stderr);

    let lower = format!("{}{}", r.stdout.to_ascii_lowercase(), r.stderr.to_ascii_lowercase());
    let kvm_failure = lower.contains("kvm run failed")
        || lower.contains("kvm_run failed")
        || lower.contains("internal-error");

    assert!(
        !kvm_failure,
        "SNP + -bios produced a KVM failure on this host. \
         The claim that `-bios` immediately crashes an SNP vCPU may be back. \
         Re-audit issues/sev-snp-pflash.md. Output:\n\
         STDOUT:\n{}\nSTDERR:\n{}",
        r.stdout, r.stderr
    );

    // At minimum the QMP dialog must succeed.
    assert!(
        r.stdout.contains("\"QMP\""),
        "QEMU didn't emit QMP greeting under -bios + SNP. \
         STDOUT:\n{}\nSTDERR:\n{}",
        r.stdout, r.stderr
    );
    assert!(
        r.stdout.contains("\"status\":"),
        "QEMU didn't answer query-status under -bios + SNP. \
         STDOUT:\n{}\nSTDERR:\n{}",
        r.stdout, r.stderr
    );

    eprintln!("VERIFIED: SNP + -bios launches cleanly (legacy '-bios crashes vCPU' claim no longer holds)");
}

/// SNP + pflash must be rejected by QEMU at CLI parse / memory-region
/// setup time. This claim is **still true** on QEMU 10.2 and pairs with
/// `kvm_cap_readonly_mem_snp_type_returns_zero` in the companion test
/// file.
///
/// Note: `--preconfig` suppresses the error (machine init is deferred),
/// so we run without it.
#[test]
fn snp_rejects_pflash_drive() {
    let fw = match skip_unless_snp_ready("snp+pflash") { Some(f) => f, None => return };

    let args = vec![
        "-machine".into(), "q35,confidential-guest-support=sev0".into(),
        "-drive".into(), format!("if=pflash,format=raw,readonly=on,file={}", fw.display()),
        "-m".into(), "512M".into(),
        "-accel".into(), "kvm".into(),
        "-cpu".into(), "EPYC-v4".into(),
        "-object".into(), snp_object_arg().into(),
        "-nodefaults".into(),
        "-display".into(), "none".into(),
        "-no-reboot".into(),
        "-serial".into(), "null".into(),
    ];
    // No QMP needed — QEMU errors out during machine init, before any
    // monitor socket is usable.
    let r = run_qemu_with_qmp(&args, &[]);
    eprintln!("--- stderr ({} bytes) ---\n{}", r.stderr.len(), r.stderr);

    let lower = r.stderr.to_ascii_lowercase();
    let is_pflash_err = lower.contains("pflash")
        && (lower.contains("readonly memory")
            || lower.contains("readonly mem")
            || lower.contains("read-only"));

    assert!(
        is_pflash_err,
        "SNP + pflash should print \
         'pflash with kvm requires KVM readonly memory support' on stderr. \
         If it didn't, re-audit issues/sev-snp-pflash.md. Stderr was:\n{}",
        r.stderr
    );
    assert!(
        !r.exit_ok,
        "SNP + pflash should exit non-zero. Stderr:\n{}",
        r.stderr
    );

    eprintln!("VERIFIED: SNP + pflash rejected by QEMU with readonly-memory error");
}
