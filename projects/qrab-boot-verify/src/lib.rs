//! Verification of detached RSA-PKCS#1 v1.5 SHA-256 signatures for
//! qrab-boot UKI payloads.
//!
//! The intended trust flow (Phase 2b and beyond):
//!
//!   1. The attestation server signs a UKI with an RSA-2048 key whose
//!      X.509 certificate lives at `contrib/sb-keys/db.crt`.
//!   2. The public-key `SubjectPublicKeyInfo` (DER-encoded) is extracted
//!      at firmware-build time and embedded in the `qrab-boot.efi`
//!      binary. That binary is in turn embedded in the AmdSev OVMF
//!      firmware volume measured by the AMD PSP, so tampering with the
//!      embedded pubkey changes the launch digest visible to the
//!      attestation server.
//!   3. At runtime, qrab-boot fetches a UKI and its detached RSA
//!      signature over `sha256(uki_bytes)`. This crate verifies the pair
//!      against the embedded pubkey before the UKI is loaded.
//!
//! We do *not* use PE Authenticode here: AmdSev OVMF has no Secure Boot
//! infrastructure (no `SecurityPkg`, no NV varstore — SEV-SNP cannot use
//! pflash). Firmware SB is therefore off in our build, and the
//! bootloader enforces the signature itself. The detached/raw format
//! keeps this crate tiny: one SHA-256, one PKCS#1 v1.5 verify, one
//! SPKI parse, all `no_std` with `alloc`.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]

extern crate alloc;

pub mod secure_boot;

use alloc::string::ToString;
use alloc::vec::Vec;
use core::fmt;

use rsa::{
    RsaPublicKey,
    pkcs1v15::{Signature, VerifyingKey},
    signature::Verifier,
    traits::PublicKeyParts,
};
use sha2::Sha256;

/// Errors returned by [`verify_detached`].
#[derive(Debug)]
pub enum VerifyError {
    /// The embedded public key could not be parsed as an RSA
    /// `SubjectPublicKeyInfo` DER blob.
    PubkeyParse,
    /// The RSA modulus is shorter than the minimum we accept.
    PubkeyTooWeak { bits: usize },
    /// The detached signature bytes do not decode as a PKCS#1 v1.5
    /// signature of the expected length.
    SignatureDecode,
    /// The SHA-256 hash of the payload did not match the RSA-recovered
    /// digest — either the payload is tampered or the signature is
    /// wrong for this payload.
    BadSignature,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PubkeyParse => f.write_str("embedded public key is not a valid RSA SPKI"),
            Self::PubkeyTooWeak { bits } => {
                write!(f, "embedded RSA key is {bits}-bit, below the 2048-bit minimum")
            }
            Self::SignatureDecode => f.write_str("detached signature failed to decode"),
            Self::BadSignature => f.write_str("signature did not verify against payload + pubkey"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for VerifyError {}

/// Minimum RSA modulus length we accept, in bits.
pub const MIN_KEY_BITS: usize = 2048;

/// Verify a detached PKCS#1 v1.5 SHA-256 signature over `payload`.
///
/// `pubkey_spki_der` is the DER-encoded `SubjectPublicKeyInfo`
/// (`openssl rsa -pubout -outform DER`).
/// `signature` is raw bytes (`openssl dgst -sha256 -sign key ...`).
///
/// # Errors
///
/// See [`VerifyError`].
pub fn verify_detached(
    payload: &[u8],
    signature: &[u8],
    pubkey_spki_der: &[u8],
) -> Result<(), VerifyError> {
    // 1. Parse SPKI → RsaPublicKey.
    let pubkey = parse_rsa_spki(pubkey_spki_der)?;

    // 2. Enforce minimum strength. 1024-bit moduli have been forgeable
    //    on well-funded adversary budgets for years; we require 2048+.
    let bits = pubkey.n().bits();
    if bits < MIN_KEY_BITS {
        return Err(VerifyError::PubkeyTooWeak { bits });
    }

    // 3. Decode the detached signature. The rsa crate's PKCS#1 v1.5
    //    `Signature::try_from(&[u8])` expects the raw big-endian
    //    signature octets and requires `len == modulus_bytes`.
    let sig = Signature::try_from(signature).map_err(|_| VerifyError::SignatureDecode)?;

    // 4. Verify.
    let verifier: VerifyingKey<Sha256> = VerifyingKey::new(pubkey);
    verifier
        .verify(payload, &sig)
        .map_err(|_| VerifyError::BadSignature)
}

/// Parse an RSA `SubjectPublicKeyInfo` DER blob into an [`RsaPublicKey`].
///
/// Accepts the standard PKIX format produced by `openssl rsa -pubout
/// -outform DER` or `openssl x509 -pubkey` + `openssl rsa -pubin
/// -outform DER`.
fn parse_rsa_spki(spki_der: &[u8]) -> Result<RsaPublicKey, VerifyError> {
    use pkcs1::RsaPublicKey as Pkcs1Rsa;
    use pkcs1::der::Decode;
    use spki::SubjectPublicKeyInfoRef;

    // `SubjectPublicKeyInfoRef::from_der` validates the outer structure
    // and gives us the raw BIT STRING contents, which for RSA should be
    // an `RSAPublicKey` SEQUENCE per RFC 8017.
    let spki = SubjectPublicKeyInfoRef::from_der(spki_der).map_err(|_| VerifyError::PubkeyParse)?;

    // Algorithm OID must be rsaEncryption (1.2.840.113549.1.1.1).
    const OID_RSA: &str = "1.2.840.113549.1.1.1";
    if spki.algorithm.oid.to_string() != OID_RSA {
        return Err(VerifyError::PubkeyParse);
    }

    let pk_bytes = spki
        .subject_public_key
        .as_bytes()
        .ok_or(VerifyError::PubkeyParse)?;
    let pkcs1 = Pkcs1Rsa::from_der(pk_bytes).map_err(|_| VerifyError::PubkeyParse)?;
    let n: Vec<u8> = pkcs1.modulus.as_bytes().to_vec();
    let e: Vec<u8> = pkcs1.public_exponent.as_bytes().to_vec();

    let n = rsa::BigUint::from_bytes_be(&n);
    let e = rsa::BigUint::from_bytes_be(&e);
    RsaPublicKey::new(n, e).map_err(|_| VerifyError::PubkeyParse)
}
