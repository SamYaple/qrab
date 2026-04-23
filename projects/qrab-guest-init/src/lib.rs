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
//! **Phase 2b (this commit).** Steps 13-18: `/run/qrab/report`
//! parse (`qrab-snp-proto::SnpAttestationReport` zerocopy cast),
//! vmpl=0 check, measurement / policy-bit / reported_tcb whitelist
//! checks, and `report_data` reconstruct bound to the PCR-8
//! uki_sha256 event + `sha256(/run/qrab/secret)`.
//!
//! **Phase 2c (planned).** Step 19 — secret consume (unlink +
//! memfd handoff) + step 20 PCR whitelist. The probe's
//! `read_secret()` method added in Phase 2b is read-only; Phase 2c
//! will add a distinct `consume_secret` method that does the
//! single-shot read + unlink.

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
pub mod sequencer;
pub mod tokens;
pub mod verity;

pub use halt::halt;
pub use log::{CaptureSink, ConsoleSink, TokenSink};
pub use real_probe::RealProbe;
pub use sequencer::{run_gate, Probe};
pub use tokens::Token;
