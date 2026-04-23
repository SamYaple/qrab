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
//! **Phase 2 (planned).** Steps 9-20: TCG 2.0 event-log parse +
//! replay, TPM 2.0 PCR read via `/dev/tpmrm0`, SnpAttestationReport
//! parse (reusing `qrab-snp-proto`), report_data reconstruct, secret
//! consume with memfd+execve handoff. Full module paths already
//! listed in the L5 spec §Components table.

pub mod efivar;
pub mod halt;
pub mod ima;
pub mod log;
pub mod lockdown;
pub mod modsig;
pub mod msr;
pub mod real_probe;
pub mod sequencer;
pub mod tokens;
pub mod verity;

pub use halt::halt;
pub use log::{CaptureSink, ConsoleSink, TokenSink};
pub use real_probe::RealProbe;
pub use sequencer::{run_gate, Probe};
pub use tokens::Token;
