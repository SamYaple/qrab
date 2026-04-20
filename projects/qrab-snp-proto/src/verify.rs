//! ECDSA-P384 signature verification for SEV-SNP attestation reports.
//!
//! Per AMD SEV-SNP ABI 1.55 §8.26.2 Table 137 ("Attestation Report
//! Format") the report is signed in place: the first 0x2A0 bytes
//! are the "signed data" and the next 512 bytes are the signature:
//!
//!   signature[0..48]   = R, little-endian
//!   signature[48..96]  = S, little-endian
//!   signature[96..512] = reserved, must be zero
//!
//! Hash = SHA-384, curve = secp384r1 (aka P-384). The signing key
//! is the VCEK (or VLEK) ECDSA keypair the PSP holds for the chip.
//! A fully-deployed verifier chains that up through an ASK that
//! itself was signed by the AMD root (ARK) — see A03 in
//! `~/workspace/llm-rag/issues/snp-attest-loop-incomplete.md`. This
//! module only handles the last step (leaf VCEK verifies the
//! report).
//!
//! The public key is expected in SEC1 uncompressed form:
//!
//!   pubkey[0]      = 0x04 (uncompressed tag)
//!   pubkey[1..49]  = X, big-endian
//!   pubkey[49..97] = Y, big-endian
//!
//! which is exactly what AMD's VCEK X.509 certificate embeds in
//! its `SubjectPublicKeyInfo` BIT STRING.

use p384::ecdsa::{Signature, VerifyingKey, signature::hazmat::PrehashVerifier};
use p384::elliptic_curve::generic_array::GenericArray;
use sha2::{Digest, Sha384};
use zerocopy::IntoBytes;

use crate::SnpAttestationReport;

/// Length of the signed prefix of an `SnpAttestationReport`. Every
/// byte from 0..SIGNED_PREFIX_LEN is authenticated; the remaining
/// 512 bytes after it are the signature itself.
pub const SIGNED_PREFIX_LEN: usize = 0x2A0;

/// Expected length of an uncompressed SEC1 P-384 pubkey (0x04 || X || Y).
pub const VCEK_SEC1_LEN: usize = 1 + 48 + 48;

/// Errors from [`verify_report`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// Public-key input was not 97 bytes or did not start with 0x04.
    /// AMD VCEK certs are always uncompressed SEC1 on secp384r1, so
    /// 97 bytes is the only well-formed length.
    BadPubkeyFormat,
    /// Public-key bytes parsed as SEC1 but do not describe a point
    /// on secp384r1. Usually means the wrong curve or a corrupted
    /// cert blob.
    PubkeyNotOnCurve,
    /// The last 512 - 96 = 416 bytes of `report.signature` must be
    /// zero per AMD ABI. Non-zero padding is a spec violation and a
    /// likely downgrade / tamper signal — fail closed rather than
    /// silently ignore the extra bytes.
    SignaturePaddingNonZero,
    /// `report.signature_algo` was not `1` (ECDSA-P384/SHA-384).
    /// VLEK uses a different `signature_algo` value in some ABI
    /// revisions; callers who want to accept VLEK should check
    /// the algo themselves before calling this function.
    UnsupportedSignatureAlgo(u32),
    /// ECDSA verification (the actual math) rejected the signature.
    /// Means the PSP did not sign these exact bytes with this
    /// public key. Could be: tampered report, wrong VCEK for the
    /// chip, wrong leaf in a cert chain, or a genuinely malicious
    /// host.
    SignatureMismatch,
}

impl core::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadPubkeyFormat => f.write_str("vcek pubkey not 97 B of SEC1 uncompressed"),
            Self::PubkeyNotOnCurve => f.write_str("vcek pubkey does not lie on secp384r1"),
            Self::SignaturePaddingNonZero => {
                f.write_str("attestation report signature padding is non-zero")
            }
            Self::UnsupportedSignatureAlgo(v) => {
                write!(f, "attestation report signature_algo = {v}, expected 1 (ECDSA-P384/SHA-384)")
            }
            Self::SignatureMismatch => f.write_str("attestation report signature does not verify"),
        }
    }
}

impl std::error::Error for VerifyError {}

/// AMD SEV-SNP `signature_algo` value that means ECDSA-P384 with
/// SHA-384. Any other value is rejected by this function; callers
/// that want to accept future algorithms (e.g. VLEK RSA, if ever
/// added) should extend the match themselves.
///
/// VLEK and VCEK reports *both* land here — AMD uses the same
/// ECDSA-P384/SHA-384 scheme for each, only the signing key
/// differs. See [`ReportSigner`] / [`signing_key_of`] for the
/// VCEK-vs-VLEK flag.
pub const SIG_ALGO_ECDSA_P384_SHA384: u32 = 1;

/// Which of the two AMD SEV-SNP key hierarchies the PSP used to
/// sign the report. Decoded from `AttestationReport.flags` bits
/// 2..4 per AMD SEV-SNP ABI 1.55 Table 23.
///
/// Named `ReportSigner` rather than `SigningKey` to avoid colliding
/// with `p384::ecdsa::SigningKey` in tests + downstream crates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportSigner {
    /// Versioned Chip Endorsement Key — derived from the chip's
    /// root key plus the reported TCB version. Fetched from AMD
    /// KDS by `(chip_id, reported_tcb)`. The default for
    /// production deployments.
    Vcek,
    /// Versioned Loaded Endorsement Key — supplied by the
    /// hypervisor and used for hosts that want to rotate the
    /// leaf key without a chip reboot. Cert is loaded by the
    /// hypervisor rather than fetched from KDS.
    Vlek,
    /// Any other 3-bit value the firmware may emit. Newer ABIs
    /// could introduce additional signing-key hierarchies.
    Other(u8),
}

/// Decode the signing-key hierarchy out of `report.flags`.
/// A04 concern: a verifier that only recognises VCEK would
/// silently accept VLEK-signed reports if it skipped this check,
/// allowing a hypervisor that holds a valid VLEK to impersonate a
/// VCEK-signed chip.
pub fn signing_key_of(report: &SnpAttestationReport) -> ReportSigner {
    let flags = { report.flags };
    match ((flags >> 2) & 0x7) as u8 {
        0 => ReportSigner::Vcek,
        1 => ReportSigner::Vlek,
        other => ReportSigner::Other(other),
    }
}

/// Length of an SEV-SNP launch measurement in bytes (SHA-384).
pub const MEASUREMENT_LEN: usize = 48;

/// `report.measurement` did not match the expected golden digest.
///
/// A08 is the check that binds the signed report back to a
/// known-good firmware + IGVM / kernel + cmdline configuration.
/// Without it, a valid PSP signature only proves \"this ran on *some*
/// SEV-SNP AMD chip\" — it does not prove the guest is actually the
/// build the relying party expected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementMismatch {
    pub expected: [u8; MEASUREMENT_LEN],
    pub actual: [u8; MEASUREMENT_LEN],
}

impl core::fmt::Display for MeasurementMismatch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Only log the first 8 bytes of each side to keep full
        // digests out of casual serial logs — the caller can format
        // the whole thing if it wants.
        write!(
            f,
            "report.measurement mismatch: expected {:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}… actual {:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}…",
            self.expected[0], self.expected[1], self.expected[2], self.expected[3],
            self.expected[4], self.expected[5], self.expected[6], self.expected[7],
            self.actual[0], self.actual[1], self.actual[2], self.actual[3],
            self.actual[4], self.actual[5], self.actual[6], self.actual[7],
        )
    }
}

impl std::error::Error for MeasurementMismatch {}

/// Constant-time-ish byte comparison — the caller already has both
/// sides in memory so the primary goal is \"no leak by timing\" only
/// in the weak sense; `subtle::ConstantTimeEq` would be stronger
/// but brings in another crate. For a 48-byte post-boot check the
/// XOR-accumulate fold below is indistinguishable from it in
/// practice.
fn ct_eq_48(a: &[u8; MEASUREMENT_LEN], b: &[u8; MEASUREMENT_LEN]) -> bool {
    let mut acc = 0u8;
    for i in 0..MEASUREMENT_LEN {
        acc |= a[i] ^ b[i];
    }
    acc == 0
}

/// Verify that `report.measurement` equals `expected`.
///
/// The expected digest is the SHA-384 of the guest's initial
/// memory image (OVMF, kernel blob, cmdline, etc.) as launched by
/// `SNP_LAUNCH_UPDATE`; it must be computed out-of-band, typically
/// with AMD's `sev-snp-measure` tool
/// (<https://github.com/virtee/sev-snp-measure>) or an IGVM-aware
/// equivalent, and pinned into the relying party's configuration.
/// Callers must *not* read the expected value out of an untrusted
/// source on the same path that serves the guest.
pub fn verify_measurement(
    report: &SnpAttestationReport,
    expected: &[u8; MEASUREMENT_LEN],
) -> Result<(), MeasurementMismatch> {
    let actual = { report.measurement };
    if ct_eq_48(&actual, expected) {
        Ok(())
    } else {
        Err(MeasurementMismatch {
            expected: *expected,
            actual,
        })
    }
}

/// Verify that `report` was signed by the owner of the VCEK (or
/// VLEK) public key `vcek_sec1`.
///
/// `vcek_sec1` must be 97 bytes of SEC1 uncompressed (`0x04 || X || Y`)
/// — the same layout AMD's X.509 VCEK cert carries in its
/// `SubjectPublicKeyInfo` BIT STRING. Call this after you have
/// independently validated the cert chain (AMD root ARK → ASK →
/// VCEK) — signature verification here is only the last hop and
/// will happily verify a report signed by *any* P-384 key if you
/// hand the wrong one in.
pub fn verify_report(
    report: &SnpAttestationReport,
    vcek_sec1: &[u8],
) -> Result<(), VerifyError> {
    // --- 1. Algorithm gate --------------------------------------------------
    //
    // Copy `signature_algo` out of the packed report first to avoid
    // the 2024-edition `unaligned_references` lint. (`SnpAttestationReport`
    // is `#[repr(C, packed)]` per the ABI layout.)
    let algo = { report.signature_algo };
    if algo != SIG_ALGO_ECDSA_P384_SHA384 {
        return Err(VerifyError::UnsupportedSignatureAlgo(algo));
    }

    // --- 2. Parse the public key into a VerifyingKey ------------------------
    if vcek_sec1.len() != VCEK_SEC1_LEN || vcek_sec1[0] != 0x04 {
        return Err(VerifyError::BadPubkeyFormat);
    }
    let verifying_key =
        VerifyingKey::from_sec1_bytes(vcek_sec1).map_err(|_| VerifyError::PubkeyNotOnCurve)?;

    // --- 3. Parse the (r, s) pair out of report.signature -------------------
    //
    // AMD stores R and S as little-endian 48-byte fields; the p384
    // crate expects them big-endian inside `Signature::from_scalars`.
    // The remaining 416 bytes of `report.signature[96..]` must be
    // zero per ABI Table 137.
    let sig_bytes: [u8; 512] = report.signature;
    // AMD SEV-SNP ABI Table 138 `SIG_ALGO_ECDSA_P384_SHA384`
    // signature layout is three 72-byte slots with 368-byte tail:
    //
    //   bytes 0..72    R (scalar little-endian, first 48 used, top 24 zero)
    //   bytes 72..144  S (same)
    //   bytes 144..512 reserved (368 bytes)
    //
    // Earlier code collapsed R/S into back-to-back 48-byte slots
    // at [0..48] + [48..96], which worked against our synthetic
    // test signatures but mismatched real AMD firmware output —
    // live EPYC 9354 surfaced this 2026-04-20 via F2 live validation.
    // Reference: virtee/sev crate `certs/snp/ecdsa/mod.rs` Signature
    // struct = `r: [u8; 72], s: [u8; 72]`.
    const R_OFF: usize = 0;
    const S_OFF: usize = 72;
    const RESERVED_OFF: usize = 144;
    // Top 24 bytes of each scalar slot plus the 368-byte tail must
    // be zero. Relax via QRAB_SNP_ALLOW_SIG_PADDING=1 for diagnostic
    // runs against AMD firmware that leaks non-zero state there.
    let allow_padding = std::env::var("QRAB_SNP_ALLOW_SIG_PADDING")
        .map(|v| v == "1")
        .unwrap_or(false);
    let mut padding_nonzero = false;
    for &b in &sig_bytes[R_OFF + 48..R_OFF + 72] {
        if b != 0 { padding_nonzero = true; break; }
    }
    if !padding_nonzero {
        for &b in &sig_bytes[S_OFF + 48..S_OFF + 72] {
            if b != 0 { padding_nonzero = true; break; }
        }
    }
    if !padding_nonzero {
        for &b in &sig_bytes[RESERVED_OFF..] {
            if b != 0 { padding_nonzero = true; break; }
        }
    }
    if padding_nonzero && !allow_padding {
        return Err(VerifyError::SignaturePaddingNonZero);
    }
    let mut r_be = [0u8; 48];
    let mut s_be = [0u8; 48];
    for i in 0..48 {
        r_be[i] = sig_bytes[R_OFF + 47 - i];
        s_be[i] = sig_bytes[S_OFF + 47 - i];
    }
    let signature = Signature::from_scalars(
        GenericArray::clone_from_slice(&r_be),
        GenericArray::clone_from_slice(&s_be),
    )
    .map_err(|_| VerifyError::SignatureMismatch)?;

    // --- 4. Hash the signed prefix + verify ---------------------------------
    //
    // The signed portion is the first 0x2A0 bytes of the report
    // struct, i.e. everything up to (but not including) the
    // `signature` field.
    let report_bytes = <SnpAttestationReport as IntoBytes>::as_bytes(report);
    let signed_prefix = &report_bytes[..SIGNED_PREFIX_LEN];
    let digest = Sha384::digest(signed_prefix);

    verifying_key
        .verify_prehash(&digest, &signature)
        .map_err(|_| VerifyError::SignatureMismatch)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SnpAttestationReport;
    use p384::ecdsa::{Signature, SigningKey, signature::hazmat::PrehashSigner};
    use p384::elliptic_curve::generic_array::GenericArray;
    use zerocopy::FromBytes;

    /// Deterministic P-384 secret scalar used throughout these tests.
    /// Picked arbitrarily — 48 bytes of `0xA5 ^ i` so each byte is
    /// distinct but none overlaps with a pattern likely to collide
    /// with cipher output during debugging.
    fn test_signing_key(tweak: u8) -> SigningKey {
        let mut bytes = [0u8; 48];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = 0xA5u8 ^ (i as u8) ^ tweak;
        }
        // `from_bytes` rejects the all-zero scalar and values >= n.
        // The constant we build here is well away from both bounds.
        SigningKey::from_bytes(&GenericArray::clone_from_slice(&bytes))
            .expect("test scalar is a valid P-384 private key")
    }

    /// Build an all-zero report, then overwrite the signed prefix with
    /// arbitrary bytes and fill `signature` with an ECDSA-P384 sig over
    /// those bytes. Returns (report, vcek_sec1).
    fn round_trip_fixture(signing_key: &SigningKey, signed_prefix_seed: u8) -> (SnpAttestationReport, [u8; VCEK_SEC1_LEN]) {
        let mut buf = [0u8; core::mem::size_of::<SnpAttestationReport>()];
        // Deterministic signed prefix.
        for (i, b) in buf[..SIGNED_PREFIX_LEN].iter_mut().enumerate() {
            *b = signed_prefix_seed.wrapping_add(i as u8);
        }
        // signature_algo must be 1 for verify to accept.
        buf[0x34..0x38].copy_from_slice(&SIG_ALGO_ECDSA_P384_SHA384.to_le_bytes());
        // Sign.
        let digest = Sha384::digest(&buf[..SIGNED_PREFIX_LEN]);
        let sig: Signature = signing_key.sign_prehash(&digest).expect("sign");
        // Encode R, S LE per AMD ABI.
        let (r, s) = sig.split_scalars();
        let r_be = r.to_bytes();
        let s_be = s.to_bytes();
        for i in 0..48 {
            // AMD SIG layout: 72-byte slots for R + S (ABI Table 138).
            // First 48 bytes of each slot hold the P-384 scalar LE;
            // trailing 24 bytes remain zero.
            buf[0x2A0 + i] = r_be[47 - i];
            buf[0x2A0 + 72 + i] = s_be[47 - i];
        }
        let report = SnpAttestationReport::read_from_bytes(&buf).expect("decode");
        let mut vcek = [0u8; VCEK_SEC1_LEN];
        let enc = signing_key.verifying_key().to_encoded_point(false);
        vcek.copy_from_slice(enc.as_bytes());
        (report, vcek)
    }

    #[test]
    fn verify_accepts_fresh_signature() {
        let key = test_signing_key(0x00);
        let (report, vcek) = round_trip_fixture(&key, 0x11);
        assert_eq!(verify_report(&report, &vcek), Ok(()));
    }

    #[test]
    fn verify_rejects_tampered_signed_prefix() {
        let key = test_signing_key(0x01);
        let (mut report, vcek) = round_trip_fixture(&key, 0x22);
        // Flip a byte inside the signed prefix.
        let bytes = <SnpAttestationReport as IntoBytes>::as_mut_bytes(&mut report);
        bytes[0x100] ^= 0x01;
        assert_eq!(verify_report(&report, &vcek), Err(VerifyError::SignatureMismatch));
    }

    #[test]
    fn verify_rejects_tampered_signature() {
        let key = test_signing_key(0x02);
        let (mut report, vcek) = round_trip_fixture(&key, 0x33);
        let bytes = <SnpAttestationReport as IntoBytes>::as_mut_bytes(&mut report);
        bytes[0x2A0] ^= 0x01; // flip first byte of R
        assert_eq!(verify_report(&report, &vcek), Err(VerifyError::SignatureMismatch));
    }

    #[test]
    fn verify_rejects_wrong_key() {
        let key = test_signing_key(0x03);
        let other = test_signing_key(0x04);
        let (report, _) = round_trip_fixture(&key, 0x44);
        let mut other_vcek = [0u8; VCEK_SEC1_LEN];
        let enc = other.verifying_key().to_encoded_point(false);
        other_vcek.copy_from_slice(enc.as_bytes());
        assert_eq!(verify_report(&report, &other_vcek), Err(VerifyError::SignatureMismatch));
    }

    #[test]
    fn verify_rejects_non_zero_signature_padding() {
        let key = test_signing_key(0x05);
        let (mut report, vcek) = round_trip_fixture(&key, 0x55);
        let bytes = <SnpAttestationReport as IntoBytes>::as_mut_bytes(&mut report);
        // 72-byte R + 72-byte S + reserved tail. First reserved byte
        // sits at signature offset 144. Setting the old `96`
        // collides with live S-scalar data under the new layout and
        // would mean a different test (SignatureMismatch).
        bytes[0x2A0 + 144] = 0x01;
        assert_eq!(
            verify_report(&report, &vcek),
            Err(VerifyError::SignaturePaddingNonZero)
        );
    }

    #[test]
    fn verify_rejects_wrong_algo() {
        let key = test_signing_key(0x06);
        let (mut report, vcek) = round_trip_fixture(&key, 0x66);
        let bytes = <SnpAttestationReport as IntoBytes>::as_mut_bytes(&mut report);
        bytes[0x34..0x38].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        assert_eq!(
            verify_report(&report, &vcek),
            Err(VerifyError::UnsupportedSignatureAlgo(0xDEAD_BEEF))
        );
    }

    #[test]
    fn verify_rejects_short_pubkey() {
        let key = test_signing_key(0x07);
        let (report, _) = round_trip_fixture(&key, 0x77);
        let short = [0x04u8; VCEK_SEC1_LEN - 1];
        assert_eq!(verify_report(&report, &short), Err(VerifyError::BadPubkeyFormat));
    }

    #[test]
    fn verify_rejects_compressed_pubkey() {
        let key = test_signing_key(0x08);
        let (report, _) = round_trip_fixture(&key, 0x88);
        let mut compressed = [0u8; VCEK_SEC1_LEN];
        compressed[0] = 0x02; // uncompressed must be 0x04
        assert_eq!(verify_report(&report, &compressed), Err(VerifyError::BadPubkeyFormat));
    }

    #[test]
    fn signature_algo_constant() {
        // AMD SEV-SNP ABI 1.55 Table 21 (AttestationReport.signature_algo).
        assert_eq!(SIG_ALGO_ECDSA_P384_SHA384, 1);
    }

    #[test]
    fn signed_prefix_len_constant() {
        // SIGNED_PREFIX_LEN must equal `offset_of!(..., signature)` so
        // every byte *before* the signature gets hashed.
        assert_eq!(SIGNED_PREFIX_LEN, 0x2A0);
        assert_eq!(
            core::mem::offset_of!(SnpAttestationReport, signature),
            SIGNED_PREFIX_LEN
        );
    }

    #[test]
    fn vcek_sec1_len_constant() {
        // 0x04 tag byte + 48-byte X + 48-byte Y.
        assert_eq!(VCEK_SEC1_LEN, 97);
    }

    #[test]
    fn signing_key_of_vcek() {
        // flags = 0 -> SigningKey = 0 = VCEK.
        let mut buf = [0u8; core::mem::size_of::<SnpAttestationReport>()];
        buf[0x34..0x38].copy_from_slice(&SIG_ALGO_ECDSA_P384_SHA384.to_le_bytes());
        // report.flags lives at offset 0x3C (u32 after signature_algo
        // (0x34) + current_tcb (0x38) — actually after current_tcb u64
        // comes platform_info u64 then flags u32). Use offset_of.
        let flags_off = core::mem::offset_of!(SnpAttestationReport, flags);
        buf[flags_off..flags_off + 4].copy_from_slice(&0u32.to_le_bytes());
        let report = SnpAttestationReport::read_from_bytes(&buf).unwrap();
        assert_eq!(signing_key_of(&report), ReportSigner::Vcek);
    }

    #[test]
    fn signing_key_of_vlek() {
        // flags = 0b0100 (bit 2 set) = signing_key field = 1 = VLEK.
        let mut buf = [0u8; core::mem::size_of::<SnpAttestationReport>()];
        let flags_off = core::mem::offset_of!(SnpAttestationReport, flags);
        buf[flags_off..flags_off + 4].copy_from_slice(&0b0100u32.to_le_bytes());
        let report = SnpAttestationReport::read_from_bytes(&buf).unwrap();
        assert_eq!(signing_key_of(&report), ReportSigner::Vlek);
    }

    #[test]
    fn signing_key_of_other_value() {
        // flags = 0b1000 (bits 2..4 = 0b010 = 2) -> Other(2).
        let mut buf = [0u8; core::mem::size_of::<SnpAttestationReport>()];
        let flags_off = core::mem::offset_of!(SnpAttestationReport, flags);
        buf[flags_off..flags_off + 4].copy_from_slice(&0b1000u32.to_le_bytes());
        let report = SnpAttestationReport::read_from_bytes(&buf).unwrap();
        assert_eq!(signing_key_of(&report), ReportSigner::Other(2));
    }

    #[test]
    fn verify_measurement_accepts_match() {
        let mut buf = [0u8; core::mem::size_of::<SnpAttestationReport>()];
        let off = core::mem::offset_of!(SnpAttestationReport, measurement);
        let expected: [u8; MEASUREMENT_LEN] = core::array::from_fn(|i| 0x10 + i as u8);
        buf[off..off + MEASUREMENT_LEN].copy_from_slice(&expected);
        let report = SnpAttestationReport::read_from_bytes(&buf).unwrap();
        assert!(verify_measurement(&report, &expected).is_ok());
    }

    #[test]
    fn verify_measurement_rejects_one_bit_flip() {
        let mut buf = [0u8; core::mem::size_of::<SnpAttestationReport>()];
        let off = core::mem::offset_of!(SnpAttestationReport, measurement);
        let mut expected: [u8; MEASUREMENT_LEN] = core::array::from_fn(|i| 0x20 + i as u8);
        buf[off..off + MEASUREMENT_LEN].copy_from_slice(&expected);
        expected[17] ^= 0x01;
        let report = SnpAttestationReport::read_from_bytes(&buf).unwrap();
        let err = verify_measurement(&report, &expected).unwrap_err();
        assert_eq!(err.expected, expected);
        assert_ne!(err.actual, err.expected);
    }

    #[test]
    fn verify_measurement_rejects_wrong_length_not_exposed() {
        // The public API forces a fixed-size array so the bad-length
        // case cannot exist at the type level — document by the
        // type that the comparison is always 48 B wide.
        assert_eq!(MEASUREMENT_LEN, 48);
    }

    #[test]
    fn signing_key_of_ignores_other_flag_bits() {
        // Unrelated bits (AuthorKeyEn bit 0, MaskChipKey bit 1) set,
        // SigningKey bits stay 0 -> VCEK.
        let mut buf = [0u8; core::mem::size_of::<SnpAttestationReport>()];
        let flags_off = core::mem::offset_of!(SnpAttestationReport, flags);
        buf[flags_off..flags_off + 4].copy_from_slice(&0b11u32.to_le_bytes());
        let report = SnpAttestationReport::read_from_bytes(&buf).unwrap();
        assert_eq!(signing_key_of(&report), ReportSigner::Vcek);
    }
}
