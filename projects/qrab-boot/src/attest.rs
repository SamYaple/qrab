//! Attestation response verification.
//!
//! The bootloader ships the attestation server's public key inside
//! its `.rodata` (embedded by `build.rs` from
//! `contrib/attest-keys/server.sec1`). On every boot the response
//! arriving over vsock is:
//!
//! 1. decoded into a [`qrab_boot_proto::AttestResponse`] (upstream of
//!    this module — in `main.rs`);
//! 2. validated for shape by
//!    [`qrab_boot_proto::validate_response`];
//! 3. fed to [`verify`] here, which:
//!    a. recomputes the canonical signing digest via
//!       [`qrab_boot_proto::response_signing_digest`];
//!    b. parses the SEC1 pubkey + the IEEE-P1363 raw signature;
//!    c. delegates to `p256::ecdsa::VerifyingKey::verify`;
//!    d. also re-hashes the transported `secret` and checks it
//!       matches `secret_sha256` via
//!       [`qrab_boot_proto::verify_secret_hash`] — because the signed
//!       digest only commits to the hash, not to the bytes.
//!
//! A build without a server pubkey (fresh clone, no
//! `contrib/attest-keys/server.sec1` on disk) sets the
//! `qrab_no_server_pubkey` cfg. `main.rs` switches to a stub that
//! logs `QRAB_ATTEST_NO_PUBKEY` and fails the boot; there is no
//! "accept unverified" mode.
//!
//! This module is `#[deny(unsafe_code)]` because everything here is
//! pure crypto — all the unsafe SNP plumbing lives in `ghcb.rs` and
//! `vsock_hal.rs`.

#![deny(unsafe_code)]

use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use qrab_boot_proto::{
    AttestResponse, response_signing_digest, verify_secret_hash,
};

/// Server public key embedded at build time. Contents come from
/// `<OUT_DIR>/attest-server-pubkey.sec1`, populated by `build.rs`
/// from `contrib/attest-keys/server.sec1` or
/// `$QRAB_ATTEST_PUBKEY_SEC1`.
///
/// On a fresh clone with no key material on disk, `build.rs` writes
/// 65 zero bytes and sets `qrab_no_server_pubkey`. We still include
/// the placeholder so `include_bytes!` paths resolve; [`verify`]
/// rejects a response in that case before reaching the crypto crate.
pub const EMBEDDED_SERVER_PUBKEY_SEC1: &[u8; 65] =
    include_bytes!(concat!(env!("OUT_DIR"), "/attest-server-pubkey.sec1"));

/// Distinguished errors from [`verify`]. Each maps to a `QRAB_ATTEST_*`
/// serial token so the smoke harness can grep the specific cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyError {
    /// The embedded pubkey is the zero-filled placeholder; this build
    /// has no server key and cannot verify anything.
    NoPubkey,
    /// `VerifyingKey::from_sec1_bytes` rejected the embedded pubkey —
    /// the SEC1 bytes were non-empty but not a valid P-256 point.
    MalformedPubkey,
    /// `Signature::from_slice` rejected the response's sig field —
    /// not a valid IEEE-P1363 r || s pair.
    MalformedSig,
    /// `VerifyingKey::verify` rejected the signed digest.
    BadSignature,
    /// The transported secret's SHA-256 did not match `secret_sha256`.
    SecretHashMismatch,
}

impl core::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoPubkey => f.write_str("no embedded server pubkey"),
            Self::MalformedPubkey => f.write_str("embedded server pubkey is not valid SEC1"),
            Self::MalformedSig => f.write_str("sig field is not a valid IEEE-P1363 signature"),
            Self::BadSignature => f.write_str("signature does not verify against server pubkey"),
            Self::SecretHashMismatch => {
                f.write_str("secret_sha256 did not match sha256(secret)")
            }
        }
    }
}

/// Fully verify a received [`AttestResponse`] against the embedded
/// server pubkey.
///
/// Returns `Ok(())` only when every check below passes:
///
///   * the embedded SEC1 pubkey is a valid P-256 point;
///   * the `sig` field parses as a 64-byte raw ECDSA signature;
///   * that signature verifies the canonical signing digest;
///   * the transported `secret` hashes to `secret_sha256`.
///
/// The secret-hash check is intentionally done *after* the signature
/// check — any attacker that can flip the secret bytes without
/// adjusting `secret_sha256` will already have been caught by the
/// signature verify; doing the hash afterwards means the cheap
/// constant-time compare only runs on signatures that already
/// verify, which simplifies error triage on the serial log.
pub fn verify(
    resp: &AttestResponse,
    client_nonce: &[u8; 32],
) -> Result<(), VerifyError> {
    if EMBEDDED_SERVER_PUBKEY_SEC1.iter().all(|&b| b == 0) {
        return Err(VerifyError::NoPubkey);
    }
    let vk = VerifyingKey::from_sec1_bytes(EMBEDDED_SERVER_PUBKEY_SEC1)
        .map_err(|_| VerifyError::MalformedPubkey)?;
    let sig = Signature::from_slice(&resp.sig).map_err(|_| VerifyError::MalformedSig)?;
    // R12-07: client_nonce is part of the signed digest so a response
    // signed for session A cannot be replayed to session B.
    let digest = response_signing_digest(resp, client_nonce);
    vk.verify(&digest, &sig).map_err(|_| VerifyError::BadSignature)?;
    if !verify_secret_hash(resp) {
        return Err(VerifyError::SecretHashMismatch);
    }
    Ok(())
}

/// Returns `true` if `build.rs` emitted a real pubkey and `false`
/// when the placeholder is active. Used by `main.rs` to route
/// logging before even attempting a verify.
pub const fn has_embedded_pubkey() -> bool {
    !cfg!(qrab_no_server_pubkey)
}
