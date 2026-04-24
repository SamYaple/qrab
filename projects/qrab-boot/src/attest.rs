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
//! A build without a server pubkey fails in `build.rs` — the
//! retired `QRAB_DEV_NO_PUBKEY=1` bypass used to produce a zero-
//! filled placeholder + a `cfg(qrab_no_server_pubkey)` gate, but the
//! opt-out was removed 2026-04-24 since attestation is mandatory on
//! qrab's CoCo path and the panic in `build.rs` already directs
//! fresh clones to `contrib/gen-attest-keys.sh`. [`VerifyError::NoPubkey`]
//! remains as a defensive fail-closed branch: if the embedded bytes
//! are somehow all zero at runtime (should be unreachable after
//! `build.rs`), `verify` still refuses to proceed.
//!
//! This module is `#[deny(unsafe_code)]` because everything here is
//! pure crypto — all the unsafe SNP plumbing lives in `ghcb.rs` and
//! `vsock_hal.rs`.

#![deny(unsafe_code)]

use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use qrab_boot_proto::{
    AttestRelease, AttestResponse, release_signing_digest,
    response_signing_digest, verify_release_hash,
};

/// Server public key embedded at build time. Contents come from
/// `<OUT_DIR>/attest-server-pubkey.sec1`, populated by `build.rs`
/// from `contrib/attest-keys/server.sec1` or
/// `$QRAB_ATTEST_PUBKEY_SEC1`.
///
/// `build.rs` fails the build outright when neither source is
/// available — no zero-fill dev bypass. [`VerifyError::NoPubkey`] is
/// a defensive fail-closed check that nevertheless runs at runtime.
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

/// Verify a received v3 [`AttestResponse`] pre-commit against the
/// embedded server pubkey. R16-H22: the secret body no longer
/// rides this frame, so the only things to check are the sig and
/// the binding to `client_nonce`. The secret is confirmed later
/// via [`verify_release`] once the server sends the release after
/// post-report verify.
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
    Ok(())
}

/// Verify a received [`AttestRelease`] against the embedded server
/// pubkey + the pre-commit this session already received. R16-H22:
/// the release carries the actual secret body, arrives only after
/// the server has chain-walked the guest's post-report, and must
/// bind to (a) the session's `client_nonce` / `server_nonce` and
/// (b) the `secret_sha256` from the pre-commit.
///
/// Returns `Ok(())` only when:
///   * release.sig verifies under the server pubkey with the
///     dedicated release signing digest (distinct domain tag);
///   * `sha256(release.secret)` equals `expected_secret_sha256`,
///     which the guest keeps from the pre-commit frame.
pub fn verify_release(
    release: &AttestRelease,
    client_nonce: &[u8; 32],
    server_nonce: &[u8; 32],
    expected_secret_sha256: &[u8; 32],
) -> Result<(), VerifyError> {
    if EMBEDDED_SERVER_PUBKEY_SEC1.iter().all(|&b| b == 0) {
        return Err(VerifyError::NoPubkey);
    }
    let vk = VerifyingKey::from_sec1_bytes(EMBEDDED_SERVER_PUBKEY_SEC1)
        .map_err(|_| VerifyError::MalformedPubkey)?;
    let sig = Signature::from_slice(&release.sig).map_err(|_| VerifyError::MalformedSig)?;
    let digest =
        release_signing_digest(client_nonce, server_nonce, expected_secret_sha256);
    vk.verify(&digest, &sig).map_err(|_| VerifyError::BadSignature)?;
    if !verify_release_hash(release, expected_secret_sha256) {
        return Err(VerifyError::SecretHashMismatch);
    }
    Ok(())
}

