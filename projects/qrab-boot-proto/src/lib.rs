//! Shared wire types for the qrab-boot attestation flow.
//!
//! The bootloader (UEFI, no_std) and the attestation server (host,
//! std) both depend on this crate so the serialisation stays in
//! lockstep. We deliberately keep the API surface minimal:
//!
//! * [`PROTO_TAG`] — magic prefix that every request begins with so
//!   a mis-wired peer is caught before deserialisation drifts.
//! * [`AttestRequest`] — client → server opener. Carries a fresh
//!   nonce plus the client's wire-format version so the server can
//!   version-gate responses.
//! * [`AttestResponse`] — server → client reply. Carries the secret
//!   the bootloader is allowed to inject into the guest kernel and a
//!   digest bundle that subsequent commits will feed into the SNP
//!   attestation report (`report_data`).
//!
//! The chosen encoding is postcard. It is no_std-friendly, compact
//! (≈100 bytes for a minimal `AttestRequest`), and has a stable
//! canonical form so two peers pinned to the same crate version
//! always agree on bytes.
//!
//! This crate ships no I/O helpers and no network dependencies. The
//! bootloader moves bytes with `vsock_connect::send_and_recv`; the
//! attestation server uses the host `std::net` equivalents. Both
//! call into [`encode`] / [`decode`] in this module.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

/// Magic prefix every [`AttestRequest`] begins with. Bumped only on
/// breaking wire-format changes. The bootloader refuses any response
/// whose request carried a different tag — deliberate belt-and-suspenders
/// versus the `client_version` field.
pub const PROTO_TAG: [u8; 7] = *b"qrab-v1";

/// Current wire-format version negotiated by the client. Bumped in
/// lockstep with breaking changes to [`AttestRequest`] /
/// [`AttestResponse`].
pub const WIRE_VERSION: u16 = 1;

/// Client → server attestation opener.
///
/// The server is expected to validate `proto == PROTO_TAG` and
/// `client_version <= WIRE_VERSION_SUPPORTED_BY_SERVER`, then echo the
/// `client_nonce` back inside [`AttestResponse::server_nonce`]
/// additively (the bootloader hashes both into the SNP `report_data`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttestRequest {
    /// Magic prefix; must equal [`PROTO_TAG`].
    pub proto: [u8; 7],
    /// Fresh 32-byte client nonce. The server must not reuse it in
    /// subsequent sessions.
    pub client_nonce: [u8; 32],
    /// Client wire-format version. Equal to [`WIRE_VERSION`] on a
    /// matched peer.
    pub client_version: u16,
}

/// Server → client attestation reply.
///
/// The bootloader:
///
/// * hashes `client_nonce || server_nonce || uki_sha256 || secret_sha256`
///   into the SNP `report_data` field of the second attestation
///   report it requests;
/// * `sha256(secret)` must equal `secret_sha256`, else the response
///   is rejected;
/// * publishes `secret` to the guest kernel via an EFI runtime
///   variable (see the plan's phase 6).
///
/// The `sig` field is an ECDSA-P256 signature over the
/// postcard-serialised bytes of every field *before* `sig`, using the
/// attestation server's long-term key; the bootloader embeds the
/// corresponding public key at build time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttestResponse {
    /// Fresh 32-byte server nonce.
    pub server_nonce: [u8; 32],
    /// URL the bootloader should GET to fetch the UKI. Over the
    /// current vsock-only transport this doubles as an opaque tag
    /// the server picks; future non-vsock transports will actually
    /// dereference it.
    pub uki_url: String,
    /// Expected SHA-256 of the UKI bytes.
    pub uki_sha256: [u8; 32],
    /// Opaque bytes injected into the guest kernel. Caps at
    /// [`MAX_SECRET_LEN`] so the bootloader can size its receive
    /// buffer without a dynamic allocation surprise.
    pub secret: Vec<u8>,
    /// SHA-256 of [`AttestResponse::secret`].
    pub secret_sha256: [u8; 32],
    /// ECDSA-P256 signature over the preceding fields. Carried as a
    /// `Vec<u8>` rather than `[u8; 64]` because serde's stock
    /// derives top out at fixed arrays of length 32; the length is
    /// validated in [`validate_response`].
    pub sig: Vec<u8>,
}

/// Expected length of [`AttestResponse::sig`] for ECDSA-P256.
pub const SIG_LEN: usize = 64;

/// Maximum `secret` length we accept. 64 KiB is plenty for rootfs
/// decryption keys + small config blobs and fits comfortably in a
/// single virtio-vsock frame pair.
pub const MAX_SECRET_LEN: usize = 64 * 1024;

/// Errors returned by [`encode`] / [`decode`].
#[derive(Debug)]
pub enum ProtoError {
    /// `postcard::to_allocvec` / `postcard::from_bytes` rejected the
    /// payload.
    Postcard(postcard::Error),
    /// `AttestRequest::proto` did not match [`PROTO_TAG`].
    WrongTag,
    /// `AttestResponse::secret` exceeded [`MAX_SECRET_LEN`].
    SecretTooLarge,
    /// `AttestResponse::sig` was not exactly [`SIG_LEN`] bytes.
    SigLen(usize),
}

impl core::fmt::Display for ProtoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Postcard(e) => write!(f, "postcard error: {e}"),
            Self::WrongTag => f.write_str("wrong protocol tag (expected `qrab-v1`)"),
            Self::SecretTooLarge => f.write_str("secret larger than MAX_SECRET_LEN"),
            Self::SigLen(n) => write!(f, "sig length {n} != expected {SIG_LEN}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ProtoError {}

/// Serialise any `T: Serialize` into a fresh `Vec<u8>` using the
/// postcard format. The qrab-boot side uses this directly for
/// [`AttestRequest`]; the server side for [`AttestResponse`].
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtoError> {
    postcard::to_allocvec(value).map_err(ProtoError::Postcard)
}

/// Deserialise a `T: Deserialize` from a byte slice.
pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, ProtoError> {
    postcard::from_bytes(bytes).map_err(ProtoError::Postcard)
}

/// Convenience: construct a fresh [`AttestRequest`] with [`PROTO_TAG`]
/// and [`WIRE_VERSION`] pre-filled and the caller's nonce copied in.
pub fn new_request(client_nonce: [u8; 32]) -> AttestRequest {
    AttestRequest {
        proto: PROTO_TAG,
        client_nonce,
        client_version: WIRE_VERSION,
    }
}

/// Convenience: validate a deserialised [`AttestRequest`]. Rejects
/// wrong-magic payloads before any further handling.
pub fn validate_request(req: &AttestRequest) -> Result<(), ProtoError> {
    if req.proto != PROTO_TAG {
        return Err(ProtoError::WrongTag);
    }
    Ok(())
}

/// Convenience: validate a deserialised [`AttestResponse`]. Currently
/// only bounds-checks the secret length; sig verification lives in
/// qrab-boot-verify (not here — this crate intentionally has no
/// cryptographic dependencies).
pub fn validate_response(resp: &AttestResponse) -> Result<(), ProtoError> {
    if resp.secret.len() > MAX_SECRET_LEN {
        return Err(ProtoError::SecretTooLarge);
    }
    if resp.sig.len() != SIG_LEN {
        return Err(ProtoError::SigLen(resp.sig.len()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn roundtrip_request() {
        let req = new_request([0x55; 32]);
        let bytes = encode(&req).unwrap();
        let back: AttestRequest = decode(&bytes).unwrap();
        assert_eq!(req, back);
        validate_request(&back).unwrap();
    }

    #[test]
    fn rejects_wrong_tag() {
        let mut req = new_request([0; 32]);
        req.proto = *b"XXX-v1?";
        let bytes = encode(&req).unwrap();
        let back: AttestRequest = decode(&bytes).unwrap();
        assert!(matches!(
            validate_request(&back),
            Err(ProtoError::WrongTag)
        ));
    }

    #[test]
    fn rejects_oversize_secret() {
        let resp = AttestResponse {
            server_nonce: [0; 32],
            uki_url: String::from("vsock://2:1337/uki"),
            uki_sha256: [0; 32],
            secret: vec![0u8; MAX_SECRET_LEN + 1],
            secret_sha256: [0; 32],
            sig: vec![0u8; SIG_LEN],
        };
        assert!(matches!(
            validate_response(&resp),
            Err(ProtoError::SecretTooLarge)
        ));
    }

    #[test]
    fn rejects_wrong_sig_length() {
        let resp = AttestResponse {
            server_nonce: [0; 32],
            uki_url: String::from("vsock://2:1337/uki"),
            uki_sha256: [0; 32],
            secret: vec![],
            secret_sha256: [0; 32],
            sig: vec![0u8; SIG_LEN - 1],
        };
        assert!(matches!(
            validate_response(&resp),
            Err(ProtoError::SigLen(n)) if n == SIG_LEN - 1
        ));
    }
}
