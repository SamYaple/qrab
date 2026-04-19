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
pub const SIG_ALGO_ECDSA_P384_SHA384: u32 = 1;

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
    for &b in &sig_bytes[96..] {
        if b != 0 {
            return Err(VerifyError::SignaturePaddingNonZero);
        }
    }
    let mut r_be = [0u8; 48];
    let mut s_be = [0u8; 48];
    for i in 0..48 {
        r_be[i] = sig_bytes[47 - i];
        s_be[i] = sig_bytes[48 + 47 - i];
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
            buf[0x2A0 + i] = r_be[47 - i];
            buf[0x2A0 + 48 + i] = s_be[47 - i];
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
        bytes[0x2A0 + 96] = 0x01; // first reserved byte non-zero
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
}
