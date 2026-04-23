//! Thin binary wrapper around [`qrab_guest_init::run_gate`].
//!
//! Two modes:
//!
//! - **Live** (`QRAB_GUEST_INIT_LIVE=1`): emits tokens to
//!   `/dev/console` and, on success, `execve`s into the next-stage
//!   workload binary with the sealed-secret memfd threaded through
//!   envp as `QRAB_SECRET_FD=<n>`. On failure, emits the fatal token
//!   and calls [`qrab_guest_init::halt`] to permanently halt the VM
//!   with no console shell. This is the mode the UKI image installs
//!   as `/sbin/init`.
//!
//! - **Dry run** (default): emits tokens to stderr, logs the memfd
//!   fd and next-stage path on success, and exits with code 0.
//!   `QRAB_GUEST_INIT_LIVE` unset MUST NOT execve — a stray
//!   `cargo run` on a dev host replacing PID 1 with
//!   `/sbin/next-stage` would brick the workstation.
//!
//! The gate itself is identical in both modes — only the sink and the
//! success/failure tail differ.
//!
//! **Phase 2b policy.** The workload policy (`allowed_measurements`,
//! `required_policy_mask`, `forbidden_policy_mask`, `min_tcb`,
//! `allowed_pcrs`) is built into the binary below. The Phase 2c
//! scaffold carries a deliberately-empty measurement whitelist AND
//! empty PCR whitelist — production builds MUST replace
//! [`workload_policy`] with a UKI-baked policy before shipping. An
//! empty measurement whitelist trips [`Token::FailMeasurement`] on
//! every boot, which keeps the dev binary honest (no accidental
//! trust-everyone regression) while the policy-source pipeline is
//! finalised.

use std::ffi::CString;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::ExitCode;

use qrab_guest_init::log::{ConsoleSink, TokenSink};
use qrab_guest_init::policy::Policy;
use qrab_guest_init::{GateOk, Token, halt, real_probe::RealProbe, run_gate};

/// Phase 2c placeholder workload policy. Returns an empty whitelist
/// so every boot fails `FailMeasurement` until production wires in a
/// real policy source. See module doc comment.
fn workload_policy() -> Policy {
    Policy {
        allowed_measurements: Vec::new(),
        required_policy_mask: 0,
        forbidden_policy_mask: 0,
        min_tcb: 0,
        allowed_pcrs: Vec::new(),
    }
}

/// Path to the workload binary the gate hands off to on success.
/// Must be a statically-linked binary inside the IMA-appraised
/// rootfs (e.g., systemd's `/sbin/init` or a lean workload
/// supervisor). Configurable via `QRAB_GUEST_INIT_NEXT_STAGE` for
/// smoke tests.
fn next_stage_path() -> PathBuf {
    std::env::var_os("QRAB_GUEST_INIT_NEXT_STAGE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/sbin/next-stage"))
}

fn main() -> ExitCode {
    let live = std::env::var_os("QRAB_GUEST_INIT_LIVE").is_some();
    let probe = RealProbe::live_defaults();
    let policy = workload_policy();

    // Run the gate under the live or dry-run sink. On success we
    // get back a GateOk carrying the sealed-secret memfd fd; on
    // failure we get the fatal token.
    let gate_result: Result<GateOk, Token> = if live {
        match open_console_sink() {
            Ok(mut sink) => run_gate(&probe, &policy, &mut sink),
            Err(e) => {
                eprintln!(
                    "qrab-guest-init: cannot open /dev/console ({e}); halting"
                );
                halt();
            }
        }
    } else {
        let mut sink = ConsoleSink::new(io::stderr());
        run_gate(&probe, &policy, &mut sink)
    };

    match gate_result {
        Ok(GateOk { memfd_fd }) => {
            if live {
                // Step 22: execve into the next stage with the
                // sealed-secret memfd threaded through envp. If this
                // returns, the workload failed to come up — emit a
                // distinct failure token (FailSecretConsumed — the
                // memfd is already consumed from the gate's point of
                // view; there's no way back to a pre-consume state)
                // and halt.
                let next = next_stage_path();
                let code = execve_next_stage(&next, memfd_fd);
                // execve_next_stage only returns on failure (success
                // never returns — the image is replaced).
                if let Ok(mut sink) = open_console_sink() {
                    let _ = sink.emit(Token::FailSecretConsumed);
                } else {
                    eprintln!(
                        "qrab-guest-init: execve({}) failed with {code:?}; halting",
                        next.display(),
                    );
                }
                halt();
            } else {
                // Dry run: DO NOT execve. Log the fd + next-stage
                // path + close the fd so we don't leak. A stray
                // `cargo run` on a dev host lands here.
                eprintln!(
                    "qrab-guest-init: dry-run OK; would execve {} with QRAB_SECRET_FD={memfd_fd}",
                    next_stage_path().display(),
                );
                // SAFETY: `memfd_fd` was just returned by the gate
                // in a GateOk; no other owner exists.
                let _ = unsafe { OwnedFd::from_raw_fd(memfd_fd) };
                ExitCode::SUCCESS
            }
        }
        Err(t) => {
            if live {
                if let Ok(mut sink) = open_console_sink() {
                    let _ = sink.emit(t);
                } else {
                    eprintln!("qrab-guest-init: final token {}", t.as_str());
                }
                halt();
            } else {
                eprintln!("qrab-guest-init: would halt on {}", t.as_str());
                ExitCode::FAILURE
            }
        }
    }
}

fn open_console_sink() -> io::Result<ConsoleSink<std::fs::File>> {
    let f = qrab_guest_init::log::open_console()?;
    Ok(ConsoleSink::new(f))
}

/// Replace the current process image with the next-stage workload
/// binary. Passes `QRAB_SECRET_FD=<fd>` as the single envp entry so
/// the workload can `read(fd)` to retrieve the sealed secret.
///
/// Returns only on failure — a successful `execve` never returns
/// (the process image is replaced). The caller interprets any
/// return as a fatal failure and halts.
///
/// argv is a single-element `[next_stage_path]`. Phase 2c does not
/// parametrise the argv/envp further — a richer shim would likely
/// pass through a small subset of the bootloader-supplied env, but
/// that's out of scope for this phase.
fn execve_next_stage(next: &std::path::Path, memfd_fd: RawFd) -> nix::Error {
    // argv[0] = next-stage path. Interior-NUL-free bytes guaranteed
    // by `Path::as_os_str().as_bytes()` (no /? valid path ever has
    // a NUL).
    let argv0 = match CString::new(next.as_os_str().as_bytes()) {
        Ok(c) => c,
        Err(_) => return nix::Error::EINVAL,
    };
    let argv = [argv0.clone()];
    let envp = match CString::new(format!("QRAB_SECRET_FD={memfd_fd}")) {
        Ok(c) => c,
        Err(_) => return nix::Error::EINVAL,
    };
    let envp_slice = [envp];
    match nix::unistd::execve(&argv[0], &argv, &envp_slice) {
        Ok(inf) => match inf {}, // Infallible
        Err(e) => e,
    }
}
