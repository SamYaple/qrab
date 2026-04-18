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
    /// A framed payload declared a length past [`MAX_FRAMED_PAYLOAD`].
    FrameTooLarge(usize),
    /// A framed payload was cut short (fewer bytes on the wire than
    /// the length prefix declared).
    FrameTruncated,
}

impl core::fmt::Display for ProtoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Postcard(e) => write!(f, "postcard error: {e}"),
            Self::WrongTag => f.write_str("wrong protocol tag (expected `qrab-v1`)"),
            Self::SecretTooLarge => f.write_str("secret larger than MAX_SECRET_LEN"),
            Self::SigLen(n) => write!(f, "sig length {n} != expected {SIG_LEN}"),
            Self::FrameTooLarge(n) => {
                write!(f, "framed payload {n}B exceeds MAX_FRAMED_PAYLOAD")
            }
            Self::FrameTruncated => f.write_str("framed payload truncated on the wire"),
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

// ---------------------------------------------------------------------------
// Framed (length-prefixed) encoding.
//
// virtio-vsock is a stream transport — it preserves byte order but not
// message boundaries. Postcard writes variable-length primitives and
// does not self-delimit at the top level, so both peers need a framing
// layer. We use the simplest possible shape: 4-byte little-endian
// `u32` length prefix followed by the postcard payload. Matches what
// the attestation-server crate will expect.
// ---------------------------------------------------------------------------

/// Length of the framing prefix in bytes.
pub const FRAME_PREFIX_LEN: usize = 4;

/// Hard upper bound on a single framed message payload. 128 KiB
/// covers an `AttestResponse` with a maxed-out [`MAX_SECRET_LEN`]
/// secret plus URL/signatures with generous headroom; it keeps us
/// safe from denial-of-service allocations on the bootloader side.
pub const MAX_FRAMED_PAYLOAD: usize = 128 * 1024;

/// Serialise `value` and prepend a 4-byte little-endian length
/// header. The returned buffer can be written straight to a stream
/// and read back with [`split_framed`].
pub fn encode_framed<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtoError> {
    let body = encode(value)?;
    if body.len() > MAX_FRAMED_PAYLOAD {
        return Err(ProtoError::FrameTooLarge(body.len()));
    }
    let mut out = Vec::with_capacity(FRAME_PREFIX_LEN + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Parse the length prefix from the front of `bytes` and return
/// `(payload_len, &payload_bytes)`. Requires `bytes` to already hold
/// at least `FRAME_PREFIX_LEN + payload_len` bytes; callers driving a
/// stream are responsible for reading until that much is buffered.
///
/// Rejects frames whose declared length exceeds [`MAX_FRAMED_PAYLOAD`]
/// *before* slicing so a bogus header cannot trigger a large
/// allocation.
pub fn split_framed(bytes: &[u8]) -> Result<(usize, &[u8]), ProtoError> {
    if bytes.len() < FRAME_PREFIX_LEN {
        return Err(ProtoError::FrameTruncated);
    }
    let prefix: [u8; FRAME_PREFIX_LEN] = bytes[..FRAME_PREFIX_LEN]
        .try_into()
        .expect("slice length matches array");
    let len = u32::from_le_bytes(prefix) as usize;
    if len > MAX_FRAMED_PAYLOAD {
        return Err(ProtoError::FrameTooLarge(len));
    }
    let rest = &bytes[FRAME_PREFIX_LEN..];
    if rest.len() < len {
        return Err(ProtoError::FrameTruncated);
    }
    Ok((len, &rest[..len]))
}

/// One-shot helper: split the frame off `bytes` and postcard-decode
/// the payload in a single call.
pub fn decode_framed<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, ProtoError> {
    let (_, body) = split_framed(bytes)?;
    decode(body)
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
    fn roundtrip_framed() {
        let req = new_request([0xAA; 32]);
        let bytes = encode_framed(&req).unwrap();
        assert!(bytes.len() > FRAME_PREFIX_LEN);
        let (len, body) = split_framed(&bytes).unwrap();
        assert_eq!(len + FRAME_PREFIX_LEN, bytes.len());
        assert_eq!(body.len(), len);
        let back: AttestRequest = decode_framed(&bytes).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn split_framed_truncated_prefix_fails() {
        let err = split_framed(&[1, 2]).unwrap_err();
        assert!(matches!(err, ProtoError::FrameTruncated));
    }

    #[test]
    fn split_framed_truncated_body_fails() {
        // Declare 64-byte payload but supply only 3 bytes of it.
        let bytes = {
            let mut v = Vec::new();
            v.extend_from_slice(&64u32.to_le_bytes());
            v.extend_from_slice(&[0; 3]);
            v
        };
        let err = split_framed(&bytes).unwrap_err();
        assert!(matches!(err, ProtoError::FrameTruncated));
    }

    #[test]
    fn split_framed_rejects_oversize_prefix() {
        let bytes = {
            let mut v = Vec::new();
            v.extend_from_slice(&((MAX_FRAMED_PAYLOAD + 1) as u32).to_le_bytes());
            v
        };
        let err = split_framed(&bytes).unwrap_err();
        assert!(matches!(err, ProtoError::FrameTooLarge(n) if n == MAX_FRAMED_PAYLOAD + 1));
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
