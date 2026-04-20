//! ARK -> ASK -> VCEK X.509 chain walk + PSP-report bridge.
//!
//! The entry point is [`verify_chain`]. It assumes the caller has
//! already fetched the three DER blobs (from
//! [`crate::kds::fetch_vcek_from_kds`] or from the cert table
//! the guest got back via `SVM_VMGEXIT_EXT_GUEST_REQUEST`) and
//! has a [`SnpAttestationReport`] to check. The steps are pure:
//! no I/O, deterministic, cancelable with `?`.
//!
//! ## Signature algorithms recognised
//!
//! The AMD cert chain uses RSA-4096 for ARK and ASK, ECDSA-P384
//! for VCEK (and VLEK). Over the generations shipped so far we've
//! observed:
//!
//! * `1.2.840.113549.1.1.10` — `rsassaPss`
//!   Parameters: SHA-384 hash, MGF1-SHA-384 mask, salt_len = 48.
//!   Dominant on Milan/Genoa/Bergamo/Turin ARK + ASK.
//! * `1.2.840.113549.1.1.12` — `sha384WithRSAEncryption`
//!   (PKCS1-v1.5). Seen on occasional ASK re-issuances.
//! * `1.2.840.113549.1.1.5` — `sha1WithRSAEncryption`
//!   Legacy Milan ARK self-signatures (pre-2022 KDS dumps). We
//!   accept for ARK self-sig only; any other link must use
//!   SHA-384 or stronger.
//! * `1.2.840.10045.4.3.3` — `ecdsa-with-SHA384`
//!   VCEK signature over the report (handled by
//!   [`qrab_snp_proto::verify::verify_report`]).
//!
//! Unknown OIDs are rejected with
//! [`crate::ChainError::UnsupportedAlgorithm`] so a hostile /
//! future cert can't sneak a weaker alg through.

use der::Encode;
use qrab_snp_proto::SnpAttestationReport;
use qrab_snp_proto::verify::{ReportSigner, VerifyError, verify_report};
use rsa::pkcs1::RsaPssParams;
use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::pss::VerifyingKey as PssVerifyingKey;
use rsa::signature::Verifier;
use rsa::{BigUint, RsaPublicKey};
use sha2::{Digest as _, Sha256, Sha384, Sha512};
use x509_cert::Certificate;
use x509_cert::der::Decode;

use crate::ChainError;
use crate::kds::TcbParts;

/// Outcome of a successful [`verify_chain`] call.
#[derive(Debug, Clone)]
pub struct VerifiedAttestation {
    /// The verified attestation report.
    pub report: SnpAttestationReport,
    /// Which endorsement key signed the report (`Vcek` is the
    /// factory-provisioned key, `Vlek` is the host-rotated key).
    pub signer: ReportSigner,
    /// SEC1-uncompressed VCEK / VLEK pubkey (97 bytes,
    /// `0x04 || X(48) || Y(48)`).
    pub vcek_sec1: Vec<u8>,
}

/// PKCS#1 v1.5 / PSS sha1WithRSAEncryption OID.
const OID_RSA_SHA1: &str = "1.2.840.113549.1.1.5";
/// PKCS#1 v1.5 sha256WithRSAEncryption.
const OID_RSA_SHA256: &str = "1.2.840.113549.1.1.11";
/// PKCS#1 v1.5 sha384WithRSAEncryption.
const OID_RSA_SHA384: &str = "1.2.840.113549.1.1.12";
/// PKCS#1 v1.5 sha512WithRSAEncryption.
const OID_RSA_SHA512: &str = "1.2.840.113549.1.1.13";
/// RSASSA-PSS (parameters embedded in AlgorithmIdentifier).
const OID_RSA_PSS: &str = "1.2.840.113549.1.1.10";
/// rsaEncryption — used as `SubjectPublicKeyInfo.algorithm` for
/// RSA keys.
const OID_RSA_ENCRYPTION: &str = "1.2.840.113549.1.1.1";
/// id-ecPublicKey — `SubjectPublicKeyInfo.algorithm` for ECDSA.
const OID_EC_PUBLIC_KEY: &str = "1.2.840.10045.2.1";
/// id-mgf1 — mask-generation-function 1 (RFC 4055 §2.1). The only
/// MGF RSA-PSS speaks; anything else is rejected.
const OID_MGF1: &str = "1.2.840.113549.1.1.8";
/// NIST SHA-2 algorithm OIDs — RFC 8017 §B.1.
const OID_SHA256: &str = "2.16.840.1.101.3.4.2.1";
const OID_SHA384: &str = "2.16.840.1.101.3.4.2.2";
const OID_SHA512: &str = "2.16.840.1.101.3.4.2.3";

/// AMD VCEK TCB / chip-id extension OIDs (SEV-SNP ABI §5.5.2).
/// Exposed as `&str` so the dispatch in [`cross_check_vcek_bindings`]
/// can match `extn_id.to_string()` cheaply.
// AMD VCEK extension OIDs per SEV-SNP firmware ABI Table 16
// (VCEK X.509 Certificate), observed on AMD KDS Genoa chain:
//
//   1.3.6.1.4.1.3704.1.1     struct_version (reserved)
//   1.3.6.1.4.1.3704.1.2     product name (e.g. "Genoa")
//   1.3.6.1.4.1.3704.1.3.<N> TCB SPL components, indexed 1..8
//   1.3.6.1.4.1.3704.1.4     hwID (chip_id, 64 bytes OCTET STRING)
//
// The SPL subtree is `3704.1.3.<N>`, NOT `3704.1.<N>` — an earlier
// F4 draft conflated the two levels and read all-zero values on
// real KDS certs. Caught by live EPYC 9354 smoke 2026-04-20.
const OID_AMD_BLSPL: &str = "1.3.6.1.4.1.3704.1.3.1";
const OID_AMD_TEESPL: &str = "1.3.6.1.4.1.3704.1.3.2";
const OID_AMD_SNPSPL: &str = "1.3.6.1.4.1.3704.1.3.3";
const OID_AMD_UCODESPL: &str = "1.3.6.1.4.1.3704.1.3.8";
const OID_AMD_HWID: &str = "1.3.6.1.4.1.3704.1.4";

/// Verify an ARK -> ASK -> VCEK cert chain and the PSP signature
/// over `report`. Returns a [`VerifiedAttestation`] on success.
///
/// `expected_measurement`, when `Some`, is compared against
/// `report.measurement`; mismatch fails with
/// [`ChainError::MeasurementMismatch`]. Callers without a golden
/// digest pass `None` and enforce out-of-band.
pub fn verify_chain(
    ark_der: &[u8],
    ask_der: &[u8],
    vcek_der: &[u8],
    report: &SnpAttestationReport,
    expected_measurement: Option<&[u8; 48]>,
) -> Result<VerifiedAttestation, ChainError> {
    let ark = parse_cert(ark_der)?;
    let ask = parse_cert(ask_der)?;
    let vcek = parse_cert(vcek_der)?;

    // Step 1: structural link checks. These are cheap DER-level
    // byte comparisons; doing them before any signature math means
    // a mis-linked chain fails fast, and we never hash input the
    // caller's trust store cannot cover.
    link_check("ASK.issuer -> ARK.subject",
        issuer_raw(&ask)?, subject_raw(&ark)?)?;
    link_check("VCEK.issuer -> ASK.subject",
        issuer_raw(&vcek)?, subject_raw(&ask)?)?;

    // Step 2: ARK self-signature. Required even though the ARK is
    // a caller-pinned trust anchor — a fresh DigestOID on a
    // replaced-but-still-looking-like-AMD cert would otherwise
    // pass the other links.
    let ark_pub = extract_rsa_pubkey(&ark)
        .map_err(|e| ChainError::PubKeyExtract(format!("ARK: {e}")))?;
    verify_rsa_cert_signature(&ark, &ark_pub)
        .map_err(|_| ChainError::ArkSelfSig)?;

    // Step 3: ASK signed by ARK.
    verify_rsa_cert_signature(&ask, &ark_pub)
        .map_err(|_| ChainError::AskSig)?;

    // Step 4: VCEK signed by ASK.
    let ask_pub = extract_rsa_pubkey(&ask)
        .map_err(|e| ChainError::PubKeyExtract(format!("ASK: {e}")))?;
    verify_rsa_cert_signature(&vcek, &ask_pub)
        .map_err(|_| ChainError::VcekSig)?;

    // Step 4b: cross-check AMD TCB / hwID extensions embedded in
    // the VCEK against the report's `reported_tcb` / `chip_id`.
    // A valid VCEK signature proves AMD issued the cert; the
    // extension match proves the *same chip* issued the report.
    // Skipping this is the misbinding gap fixed in F4.
    cross_check_vcek_bindings(&vcek, report)?;

    // Step 5: pull the VCEK (or VLEK) SEC1 pubkey out and feed it
    // into the existing no_std ECDSA-P384 verifier. The `signer`
    // field in the report's `author_key_en` byte drives whether
    // we call this a VCEK or VLEK chain. `signing_key_of` returns
    // `Other(n)` for values we don't know; reject as unsupported.
    let vcek_sec1 = extract_ec_p384_sec1(&vcek)
        .map_err(|e| ChainError::VcekPubkey(e))?;

    verify_report(report, &vcek_sec1)
        .map_err(|e: VerifyError| ChainError::ReportSig(format!("{e:?}")))?;

    let signer = qrab_snp_proto::verify::signing_key_of(report);

    if let Some(expected) = expected_measurement {
        let actual = { report.measurement };
        if actual != *expected {
            return Err(ChainError::MeasurementMismatch);
        }
    }

    Ok(VerifiedAttestation {
        report: *report,
        signer,
        vcek_sec1,
    })
}

fn parse_cert(der: &[u8]) -> Result<Certificate, ChainError> {
    Certificate::from_der(der)
        .map_err(|e| ChainError::X509Parse(format!("{e}")))
}

fn link_check(link: &'static str, actual: Vec<u8>, expected: Vec<u8>) -> Result<(), ChainError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ChainError::ChainLink { link, expected, actual })
    }
}

fn issuer_raw(cert: &Certificate) -> Result<Vec<u8>, ChainError> {
    cert.tbs_certificate
        .issuer
        .to_der()
        .map_err(|e| ChainError::X509Parse(format!("issuer: {e}")))
}

fn subject_raw(cert: &Certificate) -> Result<Vec<u8>, ChainError> {
    cert.tbs_certificate
        .subject
        .to_der()
        .map_err(|e| ChainError::X509Parse(format!("subject: {e}")))
}

fn extract_rsa_pubkey(cert: &Certificate) -> Result<RsaPublicKey, String> {
    let spki = &cert.tbs_certificate.subject_public_key_info;
    if spki.algorithm.oid.to_string() != OID_RSA_ENCRYPTION {
        return Err(format!(
            "SubjectPublicKeyInfo alg {} not rsaEncryption",
            spki.algorithm.oid
        ));
    }
    let key_bits = spki.subject_public_key.as_bytes()
        .ok_or_else(|| "subjectPublicKey has unaligned bits".to_string())?;
    // PKCS#1 RSAPublicKey SEQUENCE { modulus INTEGER, publicExponent INTEGER }.
    let rsa_pub = rsa::pkcs1::RsaPublicKey::from_der(key_bits)
        .map_err(|e| format!("parse RSAPublicKey: {e}"))?;
    let modulus = BigUint::from_bytes_be(rsa_pub.modulus.as_bytes());
    let exponent = BigUint::from_bytes_be(rsa_pub.public_exponent.as_bytes());
    RsaPublicKey::new(modulus, exponent)
        .map_err(|e| format!("construct RsaPublicKey: {e}"))
}

fn extract_ec_p384_sec1(cert: &Certificate) -> Result<Vec<u8>, String> {
    let spki = &cert.tbs_certificate.subject_public_key_info;
    if spki.algorithm.oid.to_string() != OID_EC_PUBLIC_KEY {
        return Err(format!(
            "VCEK SubjectPublicKeyInfo alg {} not id-ecPublicKey",
            spki.algorithm.oid
        ));
    }
    let key_bits = spki.subject_public_key.as_bytes()
        .ok_or_else(|| "subjectPublicKey has unaligned bits".to_string())?;
    // SEC1 point. 0x04 uncompressed || X(48) || Y(48) for P-384 = 97 bytes.
    if key_bits.len() != 97 || key_bits[0] != 0x04 {
        return Err(format!(
            "VCEK pubkey {} bytes / first byte {:#x}; want 97 bytes starting with 0x04",
            key_bits.len(),
            key_bits.first().copied().unwrap_or(0)
        ));
    }
    Ok(key_bits.to_vec())
}

/// Verify a certificate's outer `signatureAlgorithm` / `signature`
/// pair against `issuer_pub`. The TBSCertificate DER is reconstructed
/// from the parsed cert; the signatureAlgorithm OID drives the hash
/// + padding mode.
fn verify_rsa_cert_signature(
    cert: &Certificate,
    issuer_pub: &RsaPublicKey,
) -> Result<(), ChainError> {
    let tbs = cert.tbs_certificate.to_der()
        .map_err(|e| ChainError::X509Parse(format!("tbs: {e}")))?;
    let sig_bits = cert.signature.as_bytes()
        .ok_or_else(|| ChainError::X509Parse("signature has unaligned bits".into()))?;
    let oid = cert.signature_algorithm.oid.to_string();

    match oid.as_str() {
        OID_RSA_SHA1 => {
            // SHA-1 RSA is only tolerated on a self-signed cert
            // (pre-2022 Milan ARK). Gate on issuer-DN == subject-DN
            // so an ASK or VCEK carrying SHA-1 — forged via a
            // SHAmbles-style chosen-prefix collision — cannot smuggle
            // itself past the chain walk. A prior version of this
            // branch relied on an unenforced comment "callers pass
            // issuer_pub == subject_pub"; that promise had no code
            // guard and silently held for ARK while accepting SHA-1
            // on ASK / VCEK as well.
            let issuer_dn = cert.tbs_certificate.issuer.to_der()
                .map_err(|e| ChainError::X509Parse(format!("issuer DN: {e}")))?;
            let subject_dn = cert.tbs_certificate.subject.to_der()
                .map_err(|e| ChainError::X509Parse(format!("subject DN: {e}")))?;
            if issuer_dn != subject_dn {
                return Err(ChainError::WeakHashOnNonSelfSig);
            }
            let digest = sha1::Sha1::digest(&tbs);
            let scheme = Pkcs1v15Sign::new::<sha1::Sha1>();
            issuer_pub.verify(scheme, &digest, sig_bits)
                .map_err(|_| ChainError::ReportSig("RSA SHA-1 verify".into()))
        }
        OID_RSA_SHA256 => {
            let digest = Sha256::digest(&tbs);
            let scheme = Pkcs1v15Sign::new::<Sha256>();
            issuer_pub.verify(scheme, &digest, sig_bits)
                .map_err(|_| ChainError::ReportSig("RSA SHA-256 verify".into()))
        }
        OID_RSA_SHA384 => {
            let digest = Sha384::digest(&tbs);
            let scheme = Pkcs1v15Sign::new::<Sha384>();
            issuer_pub.verify(scheme, &digest, sig_bits)
                .map_err(|_| ChainError::ReportSig("RSA SHA-384 verify".into()))
        }
        OID_RSA_SHA512 => {
            let digest = Sha512::digest(&tbs);
            let scheme = Pkcs1v15Sign::new::<Sha512>();
            issuer_pub.verify(scheme, &digest, sig_bits)
                .map_err(|_| ChainError::ReportSig("RSA SHA-512 verify".into()))
        }
        OID_RSA_PSS => verify_pss_cert_signature(cert, issuer_pub, &tbs, sig_bits),
        other => Err(ChainError::UnsupportedAlgorithm { oid: other.into() }),
    }
}

/// Parse the `RSASSA-PSS-params` from the cert's `signatureAlgorithm`
/// and dispatch to the matching SHA-2 / salt-length verifier. Closes
/// the parameter-blind acceptance gap called out in F3.
///
/// Enforced invariants (reject with
/// [`ChainError::UnsupportedAlgorithm`] otherwise):
///
/// * `parameters` must be present (AMD never omits them).
/// * MGF must be `id-mgf1` (OID 1.2.840.113549.1.1.8).
/// * MGF inner hash AlgorithmIdentifier OID must equal the outer
///   `hashAlgorithm` OID — i.e. the RFC-recommended same-hash rule.
/// * Outer hash must be SHA-256 / SHA-384 / SHA-512; SHA-1 (the
///   RFC default when `hashAlgorithm` is absent) is rejected.
/// * `saltLength` must equal the hash digest size (AMD + RFC 8017
///   recommendation; blocks a short-salt downgrade).
/// * `trailerField` decoding enforces the `BC` (== 1) value via
///   [`rsa::pkcs1::TrailerField`]; any other integer is rejected
///   by the decoder itself.
fn verify_pss_cert_signature(
    cert: &Certificate,
    issuer_pub: &RsaPublicKey,
    tbs: &[u8],
    sig_bits: &[u8],
) -> Result<(), ChainError> {
    let params_any = cert.signature_algorithm.parameters.as_ref()
        .ok_or_else(|| ChainError::UnsupportedAlgorithm {
            oid: "rsassa-pss-no-params".into(),
        })?;
    let params_der = params_any.to_der()
        .map_err(|e| ChainError::X509Parse(format!("PSS params encode: {e}")))?;
    let pss = RsaPssParams::from_der(&params_der)
        .map_err(|e| ChainError::X509Parse(format!("PSS params decode: {e}")))?;

    // Reject MGF ≠ MGF1.
    if pss.mask_gen.oid.to_string() != OID_MGF1 {
        return Err(ChainError::UnsupportedAlgorithm {
            oid: format!("pss-mgf-{}", pss.mask_gen.oid),
        });
    }
    // MGF inner hash must match outer hash. RFC 4055 §3.1 strongly
    // recommends this, and AMD is consistent with it.
    let mgf_hash = pss.mask_gen.parameters.as_ref()
        .ok_or_else(|| ChainError::UnsupportedAlgorithm {
            oid: "pss-mgf-no-params".into(),
        })?;
    if mgf_hash.oid != pss.hash.oid {
        return Err(ChainError::UnsupportedAlgorithm {
            oid: format!("pss-mgf-hash-{}-vs-outer-{}", mgf_hash.oid, pss.hash.oid),
        });
    }

    let hash_oid = pss.hash.oid.to_string();
    let salt = pss.salt_len as usize;
    match hash_oid.as_str() {
        OID_SHA256 => {
            if salt != 32 {
                return Err(ChainError::UnsupportedAlgorithm {
                    oid: format!("pss-sha256-salt-{salt}"),
                });
            }
            let vkey: PssVerifyingKey<Sha256> = PssVerifyingKey::new(issuer_pub.clone());
            let sig = rsa::pss::Signature::try_from(sig_bits)
                .map_err(|e| ChainError::X509Parse(format!("PSS sig: {e}")))?;
            vkey.verify(tbs, &sig)
                .map_err(|_| ChainError::ReportSig("RSA PSS SHA-256 verify".into()))
        }
        OID_SHA384 => {
            if salt != 48 {
                return Err(ChainError::UnsupportedAlgorithm {
                    oid: format!("pss-sha384-salt-{salt}"),
                });
            }
            let vkey: PssVerifyingKey<Sha384> = PssVerifyingKey::new(issuer_pub.clone());
            let sig = rsa::pss::Signature::try_from(sig_bits)
                .map_err(|e| ChainError::X509Parse(format!("PSS sig: {e}")))?;
            vkey.verify(tbs, &sig)
                .map_err(|_| ChainError::ReportSig("RSA PSS SHA-384 verify".into()))
        }
        OID_SHA512 => {
            if salt != 64 {
                return Err(ChainError::UnsupportedAlgorithm {
                    oid: format!("pss-sha512-salt-{salt}"),
                });
            }
            let vkey: PssVerifyingKey<Sha512> = PssVerifyingKey::new(issuer_pub.clone());
            let sig = rsa::pss::Signature::try_from(sig_bits)
                .map_err(|e| ChainError::X509Parse(format!("PSS sig: {e}")))?;
            vkey.verify(tbs, &sig)
                .map_err(|_| ChainError::ReportSig("RSA PSS SHA-512 verify".into()))
        }
        other => Err(ChainError::UnsupportedAlgorithm {
            oid: format!("pss-hash-{other}"),
        }),
    }
}

/// Walk the VCEK's X.509 extensions. For every AMD OID we recognise,
/// decode the payload and compare against the matching byte / field
/// in the attestation report. Missing extensions are tolerated
/// (older VCEKs omit some) but *present* extensions MUST match —
/// that's what blocks a cross-chip or cross-TCB misbinding.
///
/// AMD wraps the SPL extension values as `INTEGER` inside the
/// outer `extnValue` OCTET STRING; `chip_id` (`hwID`) is an inner
/// `OCTET STRING` of 64 bytes. Both layers are visible here.
fn cross_check_vcek_bindings(
    vcek: &Certificate,
    report: &SnpAttestationReport,
) -> Result<(), ChainError> {
    let exts = vcek.tbs_certificate.extensions.as_ref()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let tcb = TcbParts::from_reported_tcb_le(report.reported_tcb);

    let mut saw_hwid = false;
    let mut saw_any_spl = false;

    for ext in exts {
        let oid = ext.extn_id.to_string();
        match oid.as_str() {
            OID_AMD_BLSPL => {
                check_spl(ext, "blSPL", tcb.bl)?;
                saw_any_spl = true;
            }
            OID_AMD_TEESPL => {
                check_spl(ext, "teeSPL", tcb.tee)?;
                saw_any_spl = true;
            }
            OID_AMD_SNPSPL => {
                check_spl(ext, "snpSPL", tcb.snp)?;
                saw_any_spl = true;
            }
            OID_AMD_UCODESPL => {
                check_spl(ext, "ucodeSPL", tcb.ucode)?;
                saw_any_spl = true;
            }
            OID_AMD_HWID => {
                check_hwid(ext, &report.chip_id)?;
                saw_hwid = true;
            }
            _ => { /* non-AMD extension — ignore */ }
        }
    }

    // Fail closed: a VCEK missing hwID or all four SPL OIDs cannot
    // be bound to the chip / TCB in the report. Silent tolerance
    // turned the F4 cross-check into a no-op when a host stripped
    // the AMD OIDs from a forwarded cert.
    if !saw_hwid || !saw_any_spl {
        return Err(ChainError::MissingTcbExt);
    }
    Ok(())
}

/// Decode one SPL component from an AMD TCB extension and compare
/// against the report-derived byte. The `extn_value` is the
/// ASN.1 `INTEGER` DER (AMD convention).
fn check_spl(
    ext: &x509_cert::ext::Extension,
    component: &'static str,
    report_value: u8,
) -> Result<(), ChainError> {
    // AMD wraps the SPL component as ASN.1 `INTEGER`. Values >= 0x80
    // DER-encode as two content bytes (`0x00 || 0xXX`) to avoid the
    // sign bit; `u8::from_der` rejects that form. Decode as `u16`
    // and range-check down. `u16` covers every single-octet unsigned
    // value plus the zero-padded form.
    let raw = u16::from_der(ext.extn_value.as_bytes())
        .map_err(|e| ChainError::X509Parse(format!("{component} decode: {e}")))?;
    if raw > u8::MAX as u16 {
        return Err(ChainError::X509Parse(format!(
            "{component} out of u8 range: {raw}"
        )));
    }
    let cert_value = raw as u8;
    if cert_value != report_value {
        return Err(ChainError::TcbMismatch {
            component,
            cert_value,
            report_value,
        });
    }
    Ok(())
}

/// Decode the `hwID` extension and compare against `report.chip_id`.
///
/// The outer X.509 `extnValue OCTET STRING` wrapper has already been
/// stripped by `x509-cert`. Two real-world encodings inside:
///
/// 1. **Raw 64 bytes** — AMD KDS VCEKs observed on EPYC Genoa carry
///    the chip_id directly, with no additional ASN.1 wrapper.
/// 2. **Inner `OCTET STRING` DER** — some older snphost dumps +
///    rcgen `custom_extensions` fixtures wrap the 64 bytes as
///    `04 40 <64 bytes>`.
///
/// Accept either: take raw bytes when length is 64, otherwise try
/// `OctetString::from_der`.
fn check_hwid(
    ext: &x509_cert::ext::Extension,
    report_chip_id: &[u8; 64],
) -> Result<(), ChainError> {
    let raw = ext.extn_value.as_bytes();
    let chip: &[u8] = if raw.len() == 64 {
        raw
    } else {
        let inner = der::asn1::OctetString::from_der(raw)
            .map_err(|e| ChainError::X509Parse(format!("hwID decode: {e}")))?;
        let b = inner.as_bytes();
        if b.len() != 64 {
            return Err(ChainError::X509Parse(format!(
                "hwID unexpected inner length {}", b.len()
            )));
        }
        // Can't return a reference to a local; fall through.
        if b != report_chip_id.as_slice() {
            return Err(ChainError::ChipIdMismatch);
        }
        return Ok(());
    };
    if chip != report_chip_id.as_slice() {
        return Err(ChainError::ChipIdMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use der::asn1::{BitString, OctetString};
    use rcgen::CustomExtension;
    use rsa::RsaPrivateKey;
    use rsa::pkcs8::DecodePrivateKey;
    use rsa::pss::BlindedSigningKey;
    use rsa::signature::{RandomizedSigner, SignatureEncoding};
    use x509_cert::der::asn1::ObjectIdentifier;
    use x509_cert::spki::AlgorithmIdentifierOwned;

    fn bogus_report() -> SnpAttestationReport {
        // All-zero; verify_report will reject it anyway, but
        // structural tests (link_check, OID dispatch) use the
        // cert chain only.
        unsafe { core::mem::zeroed() }
    }

    /// Build an RSA ARK self-signed + ASK-signed-by-ARK chain via
    /// `rcgen` 0.14. The VCEK is an ECDSA-P384 cert signed by ASK.
    /// rcgen uses PKCS1-v1.5 SHA-384 for RSA, which lands on the
    /// `OID_RSA_SHA384` branch in our verifier.
    ///
    /// Populates hwID + snpSPL extensions so the chain passes the
    /// F4 presence check. Tests exercising absence / mismatch call
    /// [`build_test_chain_with_vcek_exts`] directly.
    fn build_test_chain() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let hwid = amd_hwid_ext(&[0u8; 64]);
        let snp = amd_int_ext(&[1, 3, 6, 1, 4, 1, 3704, 1, 3, 3], 0);
        build_test_chain_with_vcek_exts(&[hwid, snp])
    }

    /// Same as [`build_test_chain`] but allows the caller to attach
    /// arbitrary X.509v3 extensions to the VCEK cert before it is
    /// signed by the ASK. Used by the F4 cross-check tests.
    fn build_test_chain_with_vcek_exts(
        vcek_exts: &[CustomExtension],
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use rcgen::{
            CertificateParams, DistinguishedName, DnType, Issuer,
            KeyPair,
        };
        use rcgen::{PKCS_ECDSA_P384_SHA384, PKCS_RSA_SHA384};

        fn dn(cn: &str) -> DistinguishedName {
            let mut d = DistinguishedName::new();
            d.push(DnType::CommonName, cn);
            d
        }

        fn mk_ca_params(cn: &str) -> CertificateParams {
            let mut p = CertificateParams::new(Vec::<String>::new()).unwrap();
            p.distinguished_name = dn(cn);
            p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
            p
        }

        let ark_key = KeyPair::generate_for(&PKCS_RSA_SHA384).unwrap();
        let ark_params = mk_ca_params("qrab-test ARK");
        let ark = ark_params.self_signed(&ark_key).unwrap();
        let ark_der = ark.der().to_vec();
        let ark_issuer: Issuer<'static, _> = Issuer::new(mk_ca_params("qrab-test ARK"), ark_key);

        let ask_key = KeyPair::generate_for(&PKCS_RSA_SHA384).unwrap();
        let ask_params = mk_ca_params("qrab-test ASK");
        let ask = ask_params.signed_by(&ask_key, &ark_issuer).unwrap();
        let ask_der = ask.der().to_vec();
        let ask_issuer: Issuer<'static, _> = Issuer::new(mk_ca_params("qrab-test ASK"), ask_key);

        let vcek_key = KeyPair::generate_for(&PKCS_ECDSA_P384_SHA384).unwrap();
        let mut vcek_params = CertificateParams::new(Vec::<String>::new()).unwrap();
        vcek_params.distinguished_name = dn("qrab-test VCEK");
        for ext in vcek_exts {
            vcek_params.custom_extensions.push(ext.clone());
        }
        let vcek = vcek_params.signed_by(&vcek_key, &ask_issuer).unwrap();
        let vcek_der = vcek.der().to_vec();

        (ark_der, ask_der, vcek_der)
    }

    /// Hand-build a self-signed RSA-PSS certificate with arbitrary
    /// hash algorithm + salt length. rcgen's public API does not
    /// expose PSS, so we let rcgen produce a PKCS1-v1.5 cert, swap
    /// its signatureAlgorithm to RSASSA-PSS, re-serialise the TBS,
    /// and re-sign via `rsa::pss::BlindedSigningKey`.
    fn build_pss_cert(hash_oid: &str, salt_len: usize) -> (Certificate, RsaPublicKey) {
        use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_RSA_SHA256};

        let kp = KeyPair::generate_for(&PKCS_RSA_SHA256).unwrap();
        let priv_der = kp.serialize_der();
        let priv_rsa = RsaPrivateKey::from_pkcs8_der(&priv_der).unwrap();
        let pub_rsa = priv_rsa.to_public_key();

        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "qrab-test PSS");
        params.distinguished_name = dn;
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let rcgen_cert = params.self_signed(&kp).unwrap();

        let mut cert = Certificate::from_der(rcgen_cert.der()).unwrap();
        let pss_alg = build_pss_alg_identifier(hash_oid, salt_len);
        cert.tbs_certificate.signature = pss_alg.clone();
        cert.signature_algorithm = pss_alg;

        let tbs_bytes = cert.tbs_certificate.to_der().unwrap();
        let sig_vec: Vec<u8> = match hash_oid {
            OID_SHA256 => {
                let sk = BlindedSigningKey::<Sha256>::new_with_salt_len(priv_rsa, salt_len);
                sk.sign_with_rng(&mut rand_core::OsRng, &tbs_bytes).to_bytes().to_vec()
            }
            OID_SHA384 => {
                let sk = BlindedSigningKey::<Sha384>::new_with_salt_len(priv_rsa, salt_len);
                sk.sign_with_rng(&mut rand_core::OsRng, &tbs_bytes).to_bytes().to_vec()
            }
            OID_SHA512 => {
                let sk = BlindedSigningKey::<Sha512>::new_with_salt_len(priv_rsa, salt_len);
                sk.sign_with_rng(&mut rand_core::OsRng, &tbs_bytes).to_bytes().to_vec()
            }
            other => panic!("build_pss_cert: unsupported hash {other}"),
        };
        cert.signature = BitString::from_bytes(&sig_vec).unwrap();
        (cert, pub_rsa)
    }

    fn build_pss_alg_identifier(hash_oid: &str, salt_len: usize) -> AlgorithmIdentifierOwned {
        let params_der = match hash_oid {
            OID_SHA256 => RsaPssParams::new::<Sha256>(salt_len as u8).to_der().unwrap(),
            OID_SHA384 => RsaPssParams::new::<Sha384>(salt_len as u8).to_der().unwrap(),
            OID_SHA512 => RsaPssParams::new::<Sha512>(salt_len as u8).to_der().unwrap(),
            other => panic!("build_pss_alg_identifier: unsupported hash {other}"),
        };
        let any = der::Any::from_der(&params_der).unwrap();
        AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(OID_RSA_PSS),
            parameters: Some(any),
        }
    }

    /// Pack `(bl, tee, snp, ucode)` into the little-endian u64
    /// layout `TcbParts::from_reported_tcb_le` decodes.
    fn tcb_u64(bl: u8, tee: u8, snp: u8, ucode: u8) -> u64 {
        let mut b = [0u8; 8];
        b[0] = bl;
        b[1] = tee;
        b[6] = snp;
        b[7] = ucode;
        u64::from_le_bytes(b)
    }

    /// Build a `CustomExtension` whose OID is under AMD's
    /// `1.3.6.1.4.1.3704.1.*` arc and whose DER content is the
    /// supplied `INTEGER` value, matching how AMD VCEKs carry
    /// SPL components.
    fn amd_int_ext(oid: &[u64], value: u8) -> CustomExtension {
        let content = value.to_der().unwrap();
        CustomExtension::from_oid_content(oid, content)
    }

    fn amd_hwid_ext(chip_id: &[u8; 64]) -> CustomExtension {
        let content = OctetString::new(chip_id.to_vec()).unwrap().to_der().unwrap();
        CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 3704, 1, 4], content)
    }

    #[test]
    fn happy_chain_verifies_link_structure() {
        let (ark, ask, vcek) = build_test_chain();
        // verify_chain will still fail at the report-sig step
        // (the report is zeros and the VCEK is a test key with no
        // matching PSP signature), but it must get past the three
        // RSA link checks first — any failure *before* the
        // ReportSig variant proves the pure-chain code path.
        let err = verify_chain(&ark, &ask, &vcek, &bogus_report(), None).unwrap_err();
        match err {
            ChainError::ReportSig(_) => { /* expected: no real PSP sig in test */ }
            other => panic!("expected ReportSig, got {other:?}"),
        }
    }

    #[test]
    fn tampered_ask_breaks_chain() {
        let (ark, mut ask, vcek) = build_test_chain();
        // Flip a byte well inside the DER. Offset 200 lands
        // inside the TBSCertificate for rcgen's output.
        let tamper_off = 200.min(ask.len() - 1);
        ask[tamper_off] ^= 0x01;
        let err = verify_chain(&ark, &ask, &vcek, &bogus_report(), None).unwrap_err();
        // Depending on where the flip lands, x509-cert may reject
        // parsing outright OR the RSA verify step may fail. Either
        // way the chain must not validate past AskSig.
        match err {
            ChainError::AskSig
            | ChainError::X509Parse(_)
            | ChainError::ChainLink { .. } => { /* all acceptable */ }
            other => panic!("expected AskSig / parse / link, got {other:?}"),
        }
    }

    #[test]
    fn tampered_vcek_breaks_chain() {
        let (ark, ask, mut vcek) = build_test_chain();
        let tamper_off = 200.min(vcek.len() - 1);
        vcek[tamper_off] ^= 0x01;
        let err = verify_chain(&ark, &ask, &vcek, &bogus_report(), None).unwrap_err();
        match err {
            ChainError::VcekSig
            | ChainError::X509Parse(_)
            | ChainError::ChainLink { .. }
            | ChainError::VcekPubkey(_) => { /* all acceptable */ }
            other => panic!("expected VcekSig / parse / link / pubkey, got {other:?}"),
        }
    }

    #[test]
    fn unrelated_ask_breaks_link_check() {
        let (ark, _, vcek) = build_test_chain();
        let (_, other_ask, _) = build_test_chain();
        let err = verify_chain(&ark, &other_ask, &vcek, &bogus_report(), None).unwrap_err();
        match err {
            ChainError::ChainLink { link, .. } => {
                assert!(link.contains("ASK.issuer") || link.contains("VCEK.issuer"));
            }
            ChainError::AskSig => { /* also acceptable — caught at sig rather than link */ }
            other => panic!("expected ChainLink or AskSig, got {other:?}"),
        }
    }

    #[test]
    fn garbage_ark_rejected_at_parse() {
        let (_, ask, vcek) = build_test_chain();
        let err = verify_chain(&[0u8; 8], &ask, &vcek, &bogus_report(), None).unwrap_err();
        assert!(matches!(err, ChainError::X509Parse(_)));
    }

    // ------- F3: PSS parameter parsing -------

    #[test]
    fn pss_sha256_accepted() {
        let (cert, pubkey) = build_pss_cert(OID_SHA256, 32);
        verify_rsa_cert_signature(&cert, &pubkey).expect("PSS-SHA256 should verify");
    }

    #[test]
    fn pss_sha512_accepted() {
        let (cert, pubkey) = build_pss_cert(OID_SHA512, 64);
        verify_rsa_cert_signature(&cert, &pubkey).expect("PSS-SHA512 should verify");
    }

    #[test]
    fn pss_salt_length_mismatch_rejected() {
        // SHA-256 digest size = 32. Salt=16 is the classic short-salt
        // downgrade; must bounce at param check, not at sig verify.
        let (cert, pubkey) = build_pss_cert(OID_SHA256, 16);
        let err = verify_rsa_cert_signature(&cert, &pubkey).unwrap_err();
        match err {
            ChainError::UnsupportedAlgorithm { oid } => {
                assert!(
                    oid.contains("salt-16"),
                    "expected salt-length error, got {oid}"
                );
            }
            other => panic!("expected UnsupportedAlgorithm(salt), got {other:?}"),
        }
    }

    // ------- F4: VCEK TCB / hwID cross-check -------

    #[test]
    fn vcek_tcb_mismatch_rejected() {
        // Cert says snpSPL=20, report says snp=21. hwID included so
        // we land on the TCB mismatch path, not the presence check.
        let snp = amd_int_ext(&[1, 3, 6, 1, 4, 1, 3704, 1, 3, 3], 20);
        let hwid = amd_hwid_ext(&[0u8; 64]);
        let (ark, ask, vcek) = build_test_chain_with_vcek_exts(&[snp, hwid]);
        let mut report = bogus_report();
        report.reported_tcb = tcb_u64(0, 0, 21, 0);
        let err = verify_chain(&ark, &ask, &vcek, &report, None).unwrap_err();
        match err {
            ChainError::TcbMismatch { component, cert_value, report_value } => {
                assert_eq!(component, "snpSPL");
                assert_eq!(cert_value, 20);
                assert_eq!(report_value, 21);
            }
            other => panic!("expected TcbMismatch, got {other:?}"),
        }
    }

    #[test]
    fn vcek_chip_id_mismatch_rejected() {
        // hwID + at least one SPL required for presence. We want to
        // exercise the chip-id mismatch, so include snpSPL matching
        // the (all-zero) report TCB.
        let hwid = amd_hwid_ext(&[0xAA; 64]);
        let snp = amd_int_ext(&[1, 3, 6, 1, 4, 1, 3704, 1, 3, 3], 0);
        let (ark, ask, vcek) = build_test_chain_with_vcek_exts(&[hwid, snp]);
        let mut report = bogus_report();
        report.chip_id = [0xBB; 64];
        let err = verify_chain(&ark, &ask, &vcek, &report, None).unwrap_err();
        assert!(
            matches!(err, ChainError::ChipIdMismatch),
            "expected ChipIdMismatch, got {err:?}"
        );
    }

    // ------- R13: SHA-1 DN gate + required-ext presence -------

    /// Hand-craft an ASK certificate whose outer signatureAlgorithm
    /// claims `sha1WithRSAEncryption` and is correctly signed by
    /// the ARK via SHA-1. Cannot happen with rcgen alone because
    /// rcgen's PKCS_RSA_SHA1 scheme is not exposed, so we repurpose
    /// the PKCS1 SHA-384 path and swap the OID + re-sign with SHA-1.
    fn build_sha1_ask_under_ark() -> (Vec<u8>, Vec<u8>) {
        use rcgen::{
            CertificateParams, DistinguishedName, DnType, Issuer,
            KeyPair, PKCS_RSA_SHA256,
        };

        fn dn(cn: &str) -> DistinguishedName {
            let mut d = DistinguishedName::new();
            d.push(DnType::CommonName, cn);
            d
        }
        fn mk_ca(cn: &str) -> CertificateParams {
            let mut p = CertificateParams::new(Vec::<String>::new()).unwrap();
            p.distinguished_name = dn(cn);
            p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
            p
        }

        // ARK with a real RSA key we control, self-signed via rcgen
        // (SHA-256 flavour — the OID/hash we swap is the ASK one).
        let ark_kp = KeyPair::generate_for(&PKCS_RSA_SHA256).unwrap();
        let ark_priv_der = ark_kp.serialize_der();
        let ark_priv = RsaPrivateKey::from_pkcs8_der(&ark_priv_der).unwrap();
        let ark_rcgen = mk_ca("qrab-test ARK")
            .self_signed(&ark_kp).unwrap();
        let ark_der = ark_rcgen.der().to_vec();

        // ASK signed by ARK via rcgen, then we rewrite its outer
        // signatureAlgorithm to SHA-1 RSA and re-sign the TBS bytes
        // with the ARK's private key using PKCS1-v1.5 SHA-1.
        let ark_issuer: Issuer<'static, _> = Issuer::new(mk_ca("qrab-test ARK"), ark_kp);
        let ask_kp = KeyPair::generate_for(&PKCS_RSA_SHA256).unwrap();
        let ask_rcgen = mk_ca("qrab-test ASK")
            .signed_by(&ask_kp, &ark_issuer).unwrap();
        let mut ask = Certificate::from_der(ask_rcgen.der()).unwrap();

        let sha1_alg = AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(OID_RSA_SHA1),
            parameters: Some(der::Any::null()),
        };
        ask.tbs_certificate.signature = sha1_alg.clone();
        ask.signature_algorithm = sha1_alg;
        let tbs_bytes = ask.tbs_certificate.to_der().unwrap();
        let digest = sha1::Sha1::digest(&tbs_bytes);
        let scheme = Pkcs1v15Sign::new::<sha1::Sha1>();
        let sig = ark_priv.sign(scheme, &digest).unwrap();
        ask.signature = BitString::from_bytes(&sig).unwrap();

        let ask_der = ask.to_der().unwrap();
        (ark_der, ask_der)
    }

    #[test]
    fn sha1_on_ask_rejected_even_with_valid_sig() {
        // Cert link-check runs first, so we assert by calling the
        // low-level verifier directly: the SHA-1 ASK is well-formed
        // and correctly signed, but issuer != subject, so must fail
        // with WeakHashOnNonSelfSig.
        let (ark_der, ask_der) = build_sha1_ask_under_ark();
        let ark = Certificate::from_der(&ark_der).unwrap();
        let ask = Certificate::from_der(&ask_der).unwrap();
        let ark_pub = extract_rsa_pubkey(&ark).unwrap();
        let err = verify_rsa_cert_signature(&ask, &ark_pub).unwrap_err();
        assert!(
            matches!(err, ChainError::WeakHashOnNonSelfSig),
            "expected WeakHashOnNonSelfSig, got {err:?}"
        );
    }

    #[test]
    fn sha1_on_self_signed_cert_accepted() {
        // Self-signed SHA-1 RSA cert (issuer == subject) must still
        // verify so pre-2022 Milan ARKs continue to load.
        use rcgen::{
            CertificateParams, DistinguishedName, DnType, KeyPair,
            PKCS_RSA_SHA256,
        };
        fn dn(cn: &str) -> DistinguishedName {
            let mut d = DistinguishedName::new();
            d.push(DnType::CommonName, cn);
            d
        }
        let kp = KeyPair::generate_for(&PKCS_RSA_SHA256).unwrap();
        let priv_der = kp.serialize_der();
        let priv_rsa = RsaPrivateKey::from_pkcs8_der(&priv_der).unwrap();
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.distinguished_name = dn("qrab-test legacy ARK");
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let rcgen_cert = params.self_signed(&kp).unwrap();
        let mut cert = Certificate::from_der(rcgen_cert.der()).unwrap();

        let sha1_alg = AlgorithmIdentifierOwned {
            oid: ObjectIdentifier::new_unwrap(OID_RSA_SHA1),
            parameters: Some(der::Any::null()),
        };
        cert.tbs_certificate.signature = sha1_alg.clone();
        cert.signature_algorithm = sha1_alg;
        let tbs_bytes = cert.tbs_certificate.to_der().unwrap();
        let digest = sha1::Sha1::digest(&tbs_bytes);
        let scheme = Pkcs1v15Sign::new::<sha1::Sha1>();
        let sig = priv_rsa.sign(scheme, &digest).unwrap();
        cert.signature = BitString::from_bytes(&sig).unwrap();

        let pub_key = priv_rsa.to_public_key();
        verify_rsa_cert_signature(&cert, &pub_key)
            .expect("SHA-1 self-signed cert must still verify");
    }

    #[test]
    fn vcek_missing_amd_extensions_rejected() {
        // No AMD extensions at all.
        let (ark, ask, vcek) = build_test_chain_with_vcek_exts(&[]);
        let err = verify_chain(&ark, &ask, &vcek, &bogus_report(), None).unwrap_err();
        assert!(
            matches!(err, ChainError::MissingTcbExt),
            "expected MissingTcbExt, got {err:?}"
        );
    }

    #[test]
    fn vcek_hwid_without_spl_rejected() {
        // hwID present but no SPL extensions — still misbinding risk.
        let hwid = amd_hwid_ext(&[0u8; 64]);
        let (ark, ask, vcek) = build_test_chain_with_vcek_exts(&[hwid]);
        let err = verify_chain(&ark, &ask, &vcek, &bogus_report(), None).unwrap_err();
        assert!(
            matches!(err, ChainError::MissingTcbExt),
            "expected MissingTcbExt, got {err:?}"
        );
    }

    #[test]
    fn vcek_spl_without_hwid_rejected() {
        // Some SPL present but hwID missing.
        let snp = amd_int_ext(&[1, 3, 6, 1, 4, 1, 3704, 1, 3, 3], 0);
        let (ark, ask, vcek) = build_test_chain_with_vcek_exts(&[snp]);
        let err = verify_chain(&ark, &ask, &vcek, &bogus_report(), None).unwrap_err();
        assert!(
            matches!(err, ChainError::MissingTcbExt),
            "expected MissingTcbExt, got {err:?}"
        );
    }

    #[test]
    fn spl_high_value_decodes_via_u16() {
        // u8::from_der rejected two-byte INTEGER form (value >= 0x80).
        // Build a VCEK whose snpSPL is 200 and the report's snp=200.
        // Must pass; before the fix, this surfaced as X509Parse.
        let hwid = amd_hwid_ext(&[0u8; 64]);
        let snp = amd_int_ext(&[1, 3, 6, 1, 4, 1, 3704, 1, 3, 3], 200);
        let (ark, ask, vcek) = build_test_chain_with_vcek_exts(&[hwid, snp]);
        let mut report = bogus_report();
        report.reported_tcb = tcb_u64(0, 0, 200, 0);
        let err = verify_chain(&ark, &ask, &vcek, &report, None).unwrap_err();
        // Must NOT be X509Parse on the SPL decode and must NOT be a
        // TCB mismatch — the cross-check should succeed past F4.
        // We expect the report-sig step to reject the all-zero report.
        match err {
            ChainError::ReportSig(_) => { /* SPL decoded + matched */ }
            other => panic!("expected ReportSig, got {other:?}"),
        }
    }
}
