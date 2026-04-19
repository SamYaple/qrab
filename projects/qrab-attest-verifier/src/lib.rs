//! Host-side SEV-SNP attestation chain verifier.
//!
//! Composes the no_std primitives already in `qrab-snp-proto`
//! (ECDSA-P384 report verify, measurement compare, cert-table
//! parse, VLEK detection) with a full X.509 chain walk over the
//! AMD Root Key (ARK) -> AMD SEV Signing Key (ASK) -> Versioned
//! Chip Endorsement Key (VCEK) hierarchy that signs the
//! attestation report.
//!
//! A verifier has two inputs that must agree:
//!
//! * A caller-trusted ARK (DER) — AMD ships a per-product root
//!   (Milan / Genoa / Bergamo / Turin). Callers pin this out of
//!   band (checksum published by AMD; one-time check after
//!   downloading).
//! * The VCEK + ASK the hypervisor handed back via the extended
//!   guest-request VMGEXIT, or fetched from AMD's KDS.
//!
//! Chain walk (in order):
//!
//! 1. Verify ARK is self-signed.
//! 2. Verify ASK is signed by ARK's public key.
//! 3. Verify VCEK is signed by ASK's public key.
//! 4. Extract VCEK SEC1 pubkey, feed into
//!    `qrab_snp_proto::verify::verify_report` to check the PSP
//!    signature over the attestation report.
//! 5. (Optional) `verify_measurement` against an expected
//!    golden OVMF digest.
//!
//! Signature schemes used across the AMD generations:
//!
//! | Link      | Algorithm                 | RFC / Spec                       |
//! |-----------|---------------------------|----------------------------------|
//! | ARK->ARK  | RSA-4096 PSS SHA-384      | AMD "SEV Secure Nested Paging"   |
//! | ARK->ASK  | RSA-4096 PSS SHA-384      | AMD KDS cert dumps               |
//! | ASK->VCEK | RSA-4096 PSS SHA-384      | AMD KDS cert dumps               |
//! | VCEK->SR  | ECDSA-P384 SHA-384        | `qrab_snp_proto::verify`         |
//!
//! (SR = signed-report-prefix — first 0x2A0 bytes of
//!  [`qrab_snp_proto::SnpAttestationReport`].)
//!
//! Older Milan chips occasionally emit `sha1WithRSAEncryption`
//! for the ARK self-signature; we accept it when the OID on the
//! certificate says so.
//!
//! ## Out of scope (follow-ups)
//!
//! * Full TCB extension OID extraction (AMD OID
//!   `1.3.6.1.4.1.3704.1.*`). The current implementation does not
//!   enforce that `VCEK.tcb == report.reported_tcb`; the KDS URL
//!   builder accepts TCB parts from the caller. Tracked as a
//!   follow-up in the A03 gap entry.
//! * Certificate revocation (CRL / OCSP). AMD publishes a CRL
//!   under KDS; wiring it in is an operational concern, not a
//!   bootloader concern.

use std::time::Duration;

use qrab_snp_proto::SnpAttestationReport;
use thiserror::Error;

pub mod chain;
pub mod kds;

pub use chain::{VerifiedAttestation, verify_chain};
pub use kds::{KdsError, KdsProduct, fetch_vcek_from_kds};

/// Top-level error from [`verify_chain`]. See the sub-modules for
/// the underlying variants they wrap.
#[derive(Debug, Error)]
pub enum ChainError {
    /// Failed to parse one of the caller-supplied DERs as an X.509
    /// certificate.
    #[error("X.509 parse: {0}")]
    X509Parse(String),
    /// Failed to extract a public key from a parsed certificate.
    #[error("public key extract: {0}")]
    PubKeyExtract(String),
    /// `ASK.issuer != ARK.subject` or `VCEK.issuer != ASK.subject`
    /// — the caller handed us a chain that cannot possibly link.
    /// We refuse to run signature verify on a mis-linked chain
    /// because a successful sig check would otherwise prove nothing
    /// about the intended trust anchor.
    #[error("chain link mismatch: {link} (expected {expected:?}, got {actual:?})")]
    ChainLink {
        link: &'static str,
        expected: Vec<u8>,
        actual: Vec<u8>,
    },
    /// ARK self-signature did not verify with the ARK's own pubkey.
    #[error("ARK self-signature failed to verify")]
    ArkSelfSig,
    /// ASK signature did not verify against the ARK's pubkey.
    #[error("ASK signature failed to verify against ARK")]
    AskSig,
    /// VCEK signature did not verify against the ASK's pubkey.
    #[error("VCEK signature failed to verify against ASK")]
    VcekSig,
    /// `qrab_snp_proto::verify::verify_report` rejected the PSP
    /// signature on the attestation report.
    #[error("report signature failed to verify against VCEK: {0}")]
    ReportSig(String),
    /// Golden-measurement comparison failed.
    #[error("measurement mismatch: report != expected")]
    MeasurementMismatch,
    /// Unsupported signature algorithm OID encountered inside the
    /// cert chain. Kept distinct from `X509Parse` so the caller
    /// can decide whether to fetch a fresher ARK.
    #[error("unsupported signature algorithm: {oid}")]
    UnsupportedAlgorithm { oid: String },
    /// VCEK SEC1 pubkey could not be extracted / was malformed.
    #[error("VCEK SEC1 pubkey: {0}")]
    VcekPubkey(String),
}

/// Default blocking timeout for [`fetch_vcek_from_kds`] — 15 s
/// covers AMD KDS's worst-case response on observed network
/// conditions while still failing fast when the endpoint is
/// unreachable.
pub const KDS_DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// Type alias for a raw SEC1-uncompressed ECDSA-P384 public key
/// (0x04 || X(48) || Y(48) = 97 bytes). The
/// [`qrab_snp_proto::verify::verify_report`] consumer takes this
/// shape, and chain-walk extracts it from the VCEK subjectPublicKey.
pub type VcekSec1 = Vec<u8>;

// Pin [`SnpAttestationReport`] is reachable from this level even
// when callers depend on `qrab-attest-verifier` without also
// pulling `qrab-snp-proto` directly.
pub use qrab_snp_proto::SnpAttestationReport as _Report;
#[allow(dead_code)]
fn _report_type_pin() -> Option<SnpAttestationReport> { None }

#[cfg(test)]
mod tests {
    #[test]
    fn crate_surface_compiles() {
        // Pin that the top-level re-exports resolve. The heavy
        // lifting lives in the sub-modules' own test blocks.
        let _: super::KdsError =
            super::KdsError::Transport("probe".into());
    }
}
