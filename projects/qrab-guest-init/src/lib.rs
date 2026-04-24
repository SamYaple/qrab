//! qrab-guest-init — L5 PID-1 policy gate.
//!
//! Runs inside the confidential guest as PID 1 after qrab-boot pivots
//! the kernel into the UKI rootfs. Cross-checks every L1-L4 invariant
//! against authoritative OS-visible state before handing control to
//! the workload service manager. No shell, no retry, no fallback.
//!
//! Full spec: `~/workspace/llm-rag/docs/security/level-5-init.md`.
//!
//! **Phase 1 (this crate, today).** Steps 1-8 of the boot sequence
//! are implemented with unit tests: SEV status MSR, Secure Boot,
//! Deployed/Setup mode, lockdown, module-signature enforcement, IMA
//! policy, dm-verity on `/`. The binary emits `QRAB_INIT_START` →
//! (fatal token) | `QRAB_INIT_OK` on `/dev/console`.
//!
//! **Phase 2a (landed).** Steps 9-12: TCG 2.0 event-log parse +
//! SHA-256 replay + `/dev/tpmrm0` PCR read + per-PCR compare.
//!
//! **Phase 2b (landed).** Steps 13-18: `/run/qrab/report` parse
//! (`qrab-snp-proto::SnpAttestationReport` zerocopy cast), vmpl=0
//! check, measurement / policy-bit / reported_tcb whitelist checks,
//! and `report_data` reconstruct bound to the PCR-8 uki_sha256 event
//! + `sha256(/run/qrab/secret)`.
//!
//! **Phase 2c (this commit).** Steps 19-22: single-shot consume of
//! `/run/qrab/secret` via memfd + seal + unlink (`secret::consume`),
//! per-PCR workload whitelist (`Policy::check_pcr_whitelist`), and
//! the `execve(next_stage, …, QRAB_SECRET_FD=<n>)` handoff from
//! `main.rs`. `run_gate` now returns the raw memfd FD via
//! [`sequencer::GateOk`] so the binary can thread it through the
//! exec envp.

pub mod efivar;
pub mod eventlog;
pub mod halt;
pub mod ima;
pub mod log;
pub mod lockdown;
pub mod modsig;
pub mod msr;
pub mod pcr;
pub mod policy;
pub mod real_probe;
pub mod report;
pub mod secret;
pub mod sequencer;
pub mod stash;
pub mod tokens;
pub mod verity;

pub use halt::halt;
pub use log::{CaptureSink, ConsoleSink, TokenSink};
pub use real_probe::RealProbe;
pub use sequencer::{GateOk, Probe, run_gate};
pub use tokens::Token;
