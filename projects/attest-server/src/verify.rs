//! Post-report verification plumbing (F2).
//!
//! Decodes the guest's [`AttestPostReport`] frame, extracts the
//! SNP attestation report + cert chain, and hands the chain off to
//! `qrab_attest_verifier::verify_chain` for the ARK→ASK→VCEK walk +
//! PSP signature check.
//!
//! This module is the integration seam between attest-server (vsock
//! binary) and qrab-attest-verifier (host-side chain walker). The
//! chain-walk logic itself, including PSS param parsing and AMD TCB
//! extension cross-check, lives in the verifier crate — this module
//! only wires inputs, routes errors to observable log tokens, and
//! provides overrides for hermetic tests.

use qrab_attest_verifier::{ChainError, VerifiedAttestation, verify_chain};
use qrab_boot_proto::{
    AttestPostReport, ProtoError, SNP_ATTESTATION_REPORT_LEN, decode_framed,
    validate_post_report,
};
use qrab_snp_proto::SnpAttestationReport;
use qrab_snp_proto::cert_table::{CertKind, parse_cert_table};
use zerocopy::FromBytes;

/// Built-in development ARK cert, loaded at compile time. **NOT**
/// AMD's real ARK — see `contrib/snp-cert-pins/README.md`. Real
/// deployments must override via [`ENV_ARK_PATH`].
pub const BUNDLED_ARK_PEM: &str =
    include_str!("../../../contrib/snp-cert-pins/ARK-Genoa.pem");

/// Environment variable an operator sets to point at a real AMD
/// ARK PEM file (override of [`BUNDLED_ARK_PEM`]).
pub const ENV_ARK_PATH: &str = "QRAB_ATTEST_VERIFIER_ARK_PEM";

/// Environment variable an operator sets to a hex-encoded 48-byte
/// `expected_measurement` that a verified report must match. When
/// unset, chain + sig + TCB + chipID are still enforced; only the
/// measurement compare is skipped.
pub const ENV_EXPECTED_MEASUREMENT_HEX: &str =
    "QRAB_ATTEST_VERIFIER_MEASUREMENT_HEX";

/// Errors emitted by [`handle_post_report_bytes`]. Each one maps to
/// a distinct reason suffix on `QRAB_SNP_POST_VERIFY_FAIL` tokens so
/// log readers can triage without digging through cargo output.
#[derive(Debug)]
pub enum PostReportError {
    /// Framing layer rejected the bytes (short, oversize, wrong
    /// wire version, etc).
    Proto(ProtoError),
    /// Cert-table parser rejected the host-supplied blob.
    CertTable(String),
    /// `ARK`-side setup failed: PEM couldn't be decoded or the
    /// env override pointed at a missing path.
    ArkLoad(String),
    /// Cert blob lacked an ASK entry (host did not populate
    /// `-object sev-snp-guest,certs-path=`) and fallback KDS fetch
    /// is not yet wired in from this path.
    AskMissing,
    /// Cert blob lacked a VCEK (and a VLEK fallback) so the chain
    /// walk has no leaf.
    VcekMissing,
    /// `SnpAttestationReport` failed the `FromBytes` cast — the
    /// 1184-byte payload did not align with the struct layout.
    ReportCastFailed,
    /// Expected-measurement env var was set but could not be
    /// parsed as 48 hex bytes.
    MeasurementHex(String),
    /// Chain walker rejected the report / chain.
    Chain(ChainError),
}

impl core::fmt::Display for PostReportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Proto(e) => write!(f, "proto: {e}"),
            Self::CertTable(s) => write!(f, "cert_table: {s}"),
            Self::ArkLoad(s) => write!(f, "ark_load: {s}"),
            Self::AskMissing => f.write_str("ASK missing from cert_blob"),
            Self::VcekMissing => f.write_str("VCEK missing from cert_blob"),
            Self::ReportCastFailed => f.write_str("report_bytes cast failed"),
            Self::MeasurementHex(s) => write!(f, "measurement_hex: {s}"),
            Self::Chain(e) => write!(f, "chain: {e}"),
        }
    }
}

impl std::error::Error for PostReportError {}

/// Machine-readable reason suffix for observable tokens. Callers log
/// `QRAB_SNP_POST_VERIFY_FAIL reason=<reason()>`.
pub fn reason(err: &PostReportError) -> &'static str {
    match err {
        PostReportError::Proto(_) => "proto",
        PostReportError::CertTable(_) => "cert_table",
        PostReportError::ArkLoad(_) => "ark_load",
        PostReportError::AskMissing => "ask_missing",
        PostReportError::VcekMissing => "vcek_missing",
        PostReportError::ReportCastFailed => "report_cast",
        PostReportError::MeasurementHex(_) => "measurement_hex",
        PostReportError::Chain(ChainError::ArkSelfSig) => "chain_ark_sig",
        PostReportError::Chain(ChainError::AskSig) => "chain_ask_sig",
        PostReportError::Chain(ChainError::VcekSig) => "chain_vcek_sig",
        PostReportError::Chain(ChainError::ReportSig(_)) => "chain_report_sig",
        PostReportError::Chain(ChainError::MeasurementMismatch) => "measurement",
        PostReportError::Chain(ChainError::TcbMismatch { .. }) => "tcb",
        PostReportError::Chain(ChainError::ChipIdMismatch) => "chip_id",
        PostReportError::Chain(_) => "chain_other",
    }
}

/// Resolve the ARK DER the server will use as the pinned trust
/// anchor, honouring the [`ENV_ARK_PATH`] override. Logs which
/// source was chosen to stderr so operators can see bundle vs env
/// at a glance.
pub fn load_ark_der() -> Result<Vec<u8>, PostReportError> {
    let (source, pem) = match std::env::var(ENV_ARK_PATH) {
        Ok(path) => {
            let pem = std::fs::read_to_string(&path).map_err(|e| {
                PostReportError::ArkLoad(format!("read {path}: {e}"))
            })?;
            ("env", pem)
        }
        Err(_) => ("bundle", BUNDLED_ARK_PEM.to_string()),
    };
    eprintln!("attest-server: QRAB_ATTEST_ARK_SOURCE={source}");
    pem_to_der(&pem)
}

/// PEM → DER converter. Rejects anything without a CERTIFICATE
/// armor or with a base64 decode error.
pub fn pem_to_der(pem: &str) -> Result<Vec<u8>, PostReportError> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    let begin = pem.find(BEGIN).ok_or_else(|| {
        PostReportError::ArkLoad("PEM missing BEGIN CERTIFICATE armor".into())
    })?;
    let after_begin = &pem[begin + BEGIN.len()..];
    let end = after_begin.find(END).ok_or_else(|| {
        PostReportError::ArkLoad("PEM missing END CERTIFICATE armor".into())
    })?;
    let body = &after_begin[..end];
    let compact: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(&compact)
        .map_err(|e| PostReportError::ArkLoad(format!("base64: {e}")))
}

/// Resolve the 48-byte expected measurement the verifier will
/// compare against. `None` when env unset.
pub fn expected_measurement_from_env() -> Result<Option<[u8; 48]>, PostReportError> {
    let hex = match std::env::var(ENV_EXPECTED_MEASUREMENT_HEX) {
        Err(_) => return Ok(None),
        Ok(s) => s,
    };
    let bytes = hex_decode(&hex)
        .map_err(|e| PostReportError::MeasurementHex(e))?;
    if bytes.len() != 48 {
        return Err(PostReportError::MeasurementHex(format!(
            "expected 48 bytes, got {}", bytes.len()
        )));
    }
    let mut out = [0u8; 48];
    out.copy_from_slice(&bytes);
    Ok(Some(out))
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    let clean: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 2 != 0 {
        return Err(format!("odd hex length {}", clean.len()));
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    let bytes = clean.as_bytes();
    for chunk in bytes.chunks(2) {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(10 + c - b'a'),
        b'A'..=b'F' => Ok(10 + c - b'A'),
        _ => Err(format!("non-hex byte {:#x}", c)),
    }
}

/// Decode a framed `AttestPostReport` and walk the chain. Returns
/// a `VerifiedAttestation` on full green, a `PostReportError`
/// otherwise. Caller is responsible for logging the outcome — this
/// function is pure so hermetic tests can assert on the return
/// without side-channel stdout parsing.
///
/// `ark_der_override` lets a test pass its own ARK DER without
/// touching the env; production callers pass `None` and rely on
/// [`load_ark_der`].
pub fn handle_post_report_bytes(
    bytes: &[u8],
    ark_der_override: Option<&[u8]>,
    expected_measurement_override: Option<[u8; 48]>,
) -> Result<VerifiedAttestation, PostReportError> {
    let post: AttestPostReport =
        decode_framed(bytes).map_err(PostReportError::Proto)?;
    validate_post_report(&post).map_err(PostReportError::Proto)?;
    debug_assert_eq!(post.report_bytes.len(), SNP_ATTESTATION_REPORT_LEN);

    let report = SnpAttestationReport::read_from_bytes(&post.report_bytes)
        .map_err(|_| PostReportError::ReportCastFailed)?;

    let entries = parse_cert_table(&post.cert_blob)
        .map_err(|e| PostReportError::CertTable(format!("{e}")))?;
    let mut ask: Option<Vec<u8>> = None;
    let mut vcek: Option<Vec<u8>> = None;
    for e in &entries {
        match e.kind {
            CertKind::Ask => ask = Some(e.der.to_vec()),
            CertKind::Vcek | CertKind::Vlek => {
                if vcek.is_none() {
                    vcek = Some(e.der.to_vec());
                }
            }
            _ => {}
        }
    }
    let ask_der = ask.ok_or(PostReportError::AskMissing)?;
    let vcek_der = vcek.ok_or(PostReportError::VcekMissing)?;

    let ark_der = match ark_der_override {
        Some(bytes) => bytes.to_vec(),
        None => load_ark_der()?,
    };

    let expected = match expected_measurement_override {
        Some(m) => Some(m),
        None => expected_measurement_from_env()?,
    };

    verify_chain(&ark_der, &ask_der, &vcek_der, &report, expected.as_ref())
        .map_err(PostReportError::Chain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_ark_parses_as_pem() {
        let der = pem_to_der(BUNDLED_ARK_PEM).expect("bundled ARK PEM decodes");
        assert!(der.len() > 100, "DER suspiciously short: {}", der.len());
        // DER of an X.509 cert starts with SEQUENCE (0x30).
        assert_eq!(der[0], 0x30);
    }

    #[test]
    fn load_ark_der_from_bundle_when_env_unset() {
        unsafe { std::env::remove_var(ENV_ARK_PATH) };
        let der = load_ark_der().expect("bundle loads");
        assert_eq!(der[0], 0x30);
    }

    #[test]
    fn expected_measurement_unset_returns_none() {
        unsafe { std::env::remove_var(ENV_EXPECTED_MEASUREMENT_HEX) };
        assert_eq!(expected_measurement_from_env().unwrap(), None);
    }

    #[test]
    fn hex_decode_basic() {
        assert_eq!(hex_decode("0001020a0b").unwrap(), vec![0x00, 0x01, 0x02, 0x0a, 0x0b]);
        assert_eq!(hex_decode("DEADBEEF").unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(hex_decode("  de ad  ").unwrap(), vec![0xDE, 0xAD]);
        assert!(hex_decode("ZZ").is_err());
        assert!(hex_decode("abc").is_err());
    }

    #[test]
    fn handle_post_report_rejects_bad_frame() {
        let err = handle_post_report_bytes(&[0u8; 3], None, None).unwrap_err();
        assert_eq!(reason(&err), "proto");
    }

    #[test]
    fn handle_post_report_rejects_empty_cert_blob() {
        // Legitimate report-length payload but no ASK/VCEK in the
        // cert_blob → AskMissing / VcekMissing surface before the
        // chain walker sees anything.
        let post = qrab_boot_proto::new_post_report(
            vec![0u8; SNP_ATTESTATION_REPORT_LEN],
            // A valid but empty cert table: just the 24-byte all-zero terminator.
            vec![0u8; 24],
        );
        let frame = qrab_boot_proto::encode_framed(&post).unwrap();
        let err = handle_post_report_bytes(&frame, None, None).unwrap_err();
        assert_eq!(reason(&err), "ask_missing");
    }

    #[test]
    fn reason_covers_all_chain_variants() {
        // Sentinel: if ChainError grows new variants this test
        // keeps us honest by forcing a decision on its reason
        // tag.
        use qrab_attest_verifier::ChainError;
        let sample = ChainError::ArkSelfSig;
        let _ = reason(&PostReportError::Chain(sample));
    }
}
