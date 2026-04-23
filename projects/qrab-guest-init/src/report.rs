//! Parse the SNP attestation report written by the dracut pre-pivot
//! hook at `/run/qrab/report` (`MAGIC || body`). Produces a
//! [`qrab_snp_proto::SnpAttestationReport`] via zerocopy's `FromBytes`
//! cast. All validation of report fields (vmpl, measurement, policy,
//! tcb, report_data) happens in [`crate::policy`]; this module only
//! parses.
//!
//! Drives L5 step 13 of `docs/security/level-5-init.md`. The dracut
//! hook (contrib/qrab-boot-initrd-module/) writes the 14-byte magic
//! prefix before the raw 1184-byte report body so a truncated /
//! half-written file is caught at parse time rather than trickling
//! into a zerocopy slice that silently misaligns.

use qrab_snp_proto::{SNP_ATTESTATION_REPORT_SIZE, SnpAttestationReport};
use zerocopy::FromBytes;

/// Magic prefix qrab-boot writes before the report body. Mirrors
/// `qrab_boot::uki::QRAB_REPORT_MAGIC`. If that bootloader constant
/// ever bumps, this module MUST bump in lockstep or the parse will
/// start rejecting every real-boot report.
pub const REPORT_MAGIC: &[u8; 14] = b"QRAB_REPORT_V1";

/// Failure modes returned by [`parse`]. Plain enum + `Display`
/// because the L5 musl build wants a narrow dep graph — no
/// `thiserror`, no `anyhow`.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Fewer than `REPORT_MAGIC.len() + SNP_ATTESTATION_REPORT_SIZE`
    /// bytes in the file — either the hook wrote a truncated blob or
    /// we were handed something that isn't a qrab-boot report at all.
    TooShort,
    /// First 14 bytes did not equal [`REPORT_MAGIC`]. Guards against
    /// a consumer wired to a completely different file format
    /// (e.g. an older bootloader, a stray debug log).
    WrongMagic,
    /// The body after the magic was not exactly
    /// [`SNP_ATTESTATION_REPORT_SIZE`] bytes.
    BodyLengthMismatch {
        expected: usize,
        actual: usize,
    },
    /// `SnpAttestationReport::read_from_bytes` refused the cast. The
    /// struct is `FromBytes`-derived so this should only surface if
    /// the buffer is truncated or misaligned in a way the length
    /// check above missed — the handler treats it as a parse failure
    /// identical to truncation.
    StructCast,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort => f.write_str("/run/qrab/report shorter than MAGIC + report"),
            Self::WrongMagic => f.write_str("/run/qrab/report missing QRAB_REPORT_V1 prefix"),
            Self::BodyLengthMismatch { expected, actual } => write!(
                f,
                "report body length {actual} != expected {expected}",
            ),
            Self::StructCast => {
                f.write_str("SnpAttestationReport::read_from_bytes rejected the report body")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a `MAGIC || SnpAttestationReport` blob and hand back the
/// cast struct. The returned report is an owned copy; zerocopy gives
/// us a reference into `raw` and we dereference it once so the
/// caller isn't tied to the input buffer's lifetime.
pub fn parse(raw: &[u8]) -> Result<SnpAttestationReport, ParseError> {
    const MAGIC_LEN: usize = REPORT_MAGIC.len();
    const EXPECTED_TOTAL: usize = MAGIC_LEN + SNP_ATTESTATION_REPORT_SIZE;

    // Ladder in spec order: TooShort → WrongMagic → BodyLengthMismatch
    // → StructCast. Any buffer smaller than MAGIC + REPORT is TooShort
    // regardless of prefix bytes so a truncation attack can't be
    // misclassified as "wrong magic" by chopping off the tail.
    if raw.len() < EXPECTED_TOTAL {
        return Err(ParseError::TooShort);
    }

    if &raw[..MAGIC_LEN] != REPORT_MAGIC {
        return Err(ParseError::WrongMagic);
    }

    let body = &raw[MAGIC_LEN..];
    if body.len() != SNP_ATTESTATION_REPORT_SIZE {
        return Err(ParseError::BodyLengthMismatch {
            expected: SNP_ATTESTATION_REPORT_SIZE,
            actual: body.len(),
        });
    }

    // zerocopy v0.8 returns a `Result<&T, _>`; we copy out of the
    // borrow so the caller owns the struct and the input slice's
    // lifetime is free to drop.
    let cast =
        SnpAttestationReport::read_from_bytes(body).map_err(|_| ParseError::StructCast)?;
    Ok(cast)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qrab_snp_proto::SnpAttestationReport;
    use zerocopy::IntoBytes;

    /// Build a fully-zeroed `SnpAttestationReport`. `FromBytes` +
    /// `IntoBytes` together mean we can round-trip through raw
    /// bytes — which is exactly how the dracut hook produces the
    /// file on disk.
    fn zero_report() -> SnpAttestationReport {
        // SAFETY: `SnpAttestationReport: FromBytes` lets a zero-byte
        // pattern inhabit the struct soundly. We construct from a
        // literal 1184-byte array + a `read_from_bytes` cast instead
        // of `core::mem::zeroed` so the test exercises zerocopy on
        // the happy path too.
        let buf = [0u8; SNP_ATTESTATION_REPORT_SIZE];
        SnpAttestationReport::read_from_bytes(&buf).expect("zeroed report casts ok")
    }

    fn make_blob(report: &SnpAttestationReport) -> Vec<u8> {
        let mut blob = Vec::with_capacity(REPORT_MAGIC.len() + SNP_ATTESTATION_REPORT_SIZE);
        blob.extend_from_slice(REPORT_MAGIC);
        blob.extend_from_slice(report.as_bytes());
        blob
    }

    #[test]
    fn parse_happy() {
        let report = zero_report();
        let blob = make_blob(&report);
        let back = parse(&blob).expect("parse happy");
        // `SnpAttestationReport` is `Copy` but `PartialEq` was
        // intentionally NOT derived (packed-struct gotcha). Compare
        // via the byte-level view — that's what every production
        // consumer cares about anyway.
        assert_eq!(back.as_bytes(), report.as_bytes());
    }

    /// Helper: assert `parse(raw)` errored with the expected
    /// `ParseError`. `SnpAttestationReport` intentionally omits
    /// `PartialEq` (packed-struct gotcha), so we can't use the
    /// blanket `assert_eq!` on `Result<SnpAttestationReport, _>`.
    #[track_caller]
    fn assert_parse_err(raw: &[u8], expected: ParseError) {
        match parse(raw) {
            Err(e) => assert_eq!(e, expected),
            Ok(_) => panic!("expected Err({expected:?}), got Ok"),
        }
    }

    #[test]
    fn parse_too_short_when_only_magic() {
        // Only the 14-byte magic present — no report body at all.
        let blob = REPORT_MAGIC.to_vec();
        assert_parse_err(&blob, ParseError::TooShort);
    }

    #[test]
    fn parse_too_short_when_empty() {
        assert_parse_err(&[], ParseError::TooShort);
    }

    #[test]
    fn parse_wrong_magic() {
        // Right total length, wrong magic bytes. Reading an older
        // blob format (or a random file that happened to be 1198 B)
        // must land on `WrongMagic`, not a corrupt cast.
        let mut blob = vec![0u8; REPORT_MAGIC.len() + SNP_ATTESTATION_REPORT_SIZE];
        blob[..REPORT_MAGIC.len()].copy_from_slice(b"QRAB_REPORT_V2");
        assert_parse_err(&blob, ParseError::WrongMagic);
    }

    #[test]
    fn parse_length_mismatch_when_body_truncated() {
        // Correct magic, body truncated to 100 bytes instead of 1184.
        let mut blob = REPORT_MAGIC.to_vec();
        blob.extend_from_slice(&[0u8; 100]);
        // 14 + 100 = 114 < 14 + 1184 → TooShort short-circuits before
        // we hit the "body length mismatch" branch. Spec-wise that's
        // indistinguishable from the attacker's perspective: the
        // report body is wrong-sized, bail.
        assert_parse_err(&blob, ParseError::TooShort);
    }

    #[test]
    fn parse_length_mismatch_when_body_has_trailing_bytes() {
        // Correct magic + correct body length + trailing extras. The
        // dracut hook writes the report atomically from a 1184-byte
        // buffer, so any trailing bytes mean somebody reopened the
        // file and appended. Reject.
        let report = zero_report();
        let mut blob = make_blob(&report);
        blob.extend_from_slice(&[0xAB; 16]);
        match parse(&blob) {
            Err(ParseError::BodyLengthMismatch { expected, actual }) => {
                assert_eq!(expected, SNP_ATTESTATION_REPORT_SIZE);
                assert_eq!(actual, SNP_ATTESTATION_REPORT_SIZE + 16);
            }
            other => panic!("expected BodyLengthMismatch, got {other:?}"),
        }
    }

    #[test]
    fn parse_length_mismatch_when_body_one_byte_short() {
        // Dead-centre around the length boundary: magic + body - 1
        // byte. Below the `MAGIC + SNP_ATTESTATION_REPORT_SIZE`
        // threshold so the caller still lands on `TooShort`.
        let mut blob = REPORT_MAGIC.to_vec();
        blob.extend_from_slice(&[0u8; SNP_ATTESTATION_REPORT_SIZE - 1]);
        assert_parse_err(&blob, ParseError::TooShort);
    }

    #[test]
    fn parse_wrong_magic_when_total_len_matches_exactly() {
        // Buffer length is exactly MAGIC_LEN + REPORT_SIZE but the
        // prefix bytes are the wrong magic. Parse must distinguish
        // "wrong format" from "truncated" — the ladder in `parse`
        // checks total length first, so we need the input long
        // enough that the early TooShort branch doesn't fire.
        let mut blob = vec![0u8; REPORT_MAGIC.len() + SNP_ATTESTATION_REPORT_SIZE];
        blob[..REPORT_MAGIC.len()].copy_from_slice(b"QRAB_XYZ__V1_Z");
        assert_parse_err(&blob, ParseError::WrongMagic);
    }

    #[test]
    fn display_for_parse_error_is_non_empty() {
        for e in [
            ParseError::TooShort,
            ParseError::WrongMagic,
            ParseError::BodyLengthMismatch {
                expected: 1184,
                actual: 42,
            },
            ParseError::StructCast,
        ] {
            assert!(!format!("{e}").is_empty());
        }
    }
}
