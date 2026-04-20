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
use alloc::vec;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Magic prefix every [`AttestRequest`] begins with. Bumped only on
/// breaking wire-format changes. The bootloader refuses any response
/// whose request carried a different tag — deliberate belt-and-suspenders
/// versus the `client_version` field.
pub const PROTO_TAG: [u8; 7] = *b"qrab-v1";

/// Current wire-format version negotiated by the client. Bumped in
/// lockstep with breaking changes to [`AttestRequest`] /
/// [`AttestResponse`] / [`AttestPostReport`].
///
/// v1 → v2 (2026-04-20): introduces [`AttestPostReport`] — a third
/// frame from the guest carrying its signed SNP attestation report +
/// cert blob, so the server can complete a full ARK→ASK→VCEK chain
/// walk via `qrab_attest_verifier::verify_chain`. Hard bump — v1
/// peers are rejected outright. Bump F2 / `followups.md`.
pub const WIRE_VERSION: u16 = 2;

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

/// Byte length of the raw `SnpAttestationReport` struct (AMD ABI
/// §4.5). Shared by `qrab-snp-proto` as a compile-time assertion;
/// this crate repeats the literal so the post-report struct is
/// self-contained and the no_std / alloc-only build path does not
/// need to pull the full SEV-SNP proto crate in.
pub const SNP_ATTESTATION_REPORT_LEN: usize = 1184;

/// Upper bound on [`AttestPostReport::cert_blob`]. Observed AMD
/// cert blobs are ~16 KiB (ARK + ASK + VCEK DER round-tripped
/// through GHCB). 64 KiB leaves generous headroom; the whole
/// post-report frame still fits inside [`MAX_FRAMED_PAYLOAD`].
pub const MAX_CERT_BLOB: usize = 64 * 1024;

/// Guest → server third frame: the attestation report the guest
/// pulled from the PSP plus the host-supplied cert blob. Server
/// feeds both into `qrab_attest_verifier::verify_chain` to close
/// the attestation loop.
///
/// `sig` is reserved for a future guest-side authentication key
/// (VMPL0-derived or TPM-attested). It is currently always all
/// zero — the cryptographic anchor is the embedded PSP signature
/// on `report_bytes` itself, which a valid ARK-rooted chain walk
/// verifies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttestPostReport {
    /// Magic prefix; must equal [`PROTO_TAG`].
    pub proto: [u8; 7],
    /// Wire-format version; must equal [`WIRE_VERSION`].
    pub wire_version: u16,
    /// Raw `SnpAttestationReport` bytes. Length fixed at
    /// [`SNP_ATTESTATION_REPORT_LEN`] to catch truncation at
    /// decode time.
    pub report_bytes: Vec<u8>,
    /// AMD cert-table blob, as returned by
    /// `SVM_VMGEXIT_EXT_GUEST_REQUEST`. Zero-length is legal when
    /// the host has not populated `-object sev-snp-guest,certs-path=`;
    /// the server then falls back to AMD KDS.
    pub cert_blob: Vec<u8>,
    /// Reserved for future guest-side signing. Currently
    /// `[0u8; SIG_LEN]`. Do NOT treat a non-zero value here as
    /// authoritative until the guest-side signing design lands.
    pub sig: Vec<u8>,
}

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
    /// Peer's `client_version` / `wire_version` disagreed with this
    /// build's [`WIRE_VERSION`]. Hard failure under the v1 → v2
    /// hard-bump policy — no backward compat.
    WireVersionMismatch { expected: u16, got: u16 },
    /// `AttestPostReport::report_bytes` length was not exactly
    /// [`SNP_ATTESTATION_REPORT_LEN`].
    ReportLen(usize),
    /// `AttestPostReport::cert_blob` exceeded [`MAX_CERT_BLOB`].
    CertBlobTooLarge(usize),
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
            Self::WireVersionMismatch { expected, got } => {
                write!(f, "wire version mismatch: expected {expected}, got {got}")
            }
            Self::ReportLen(n) => write!(
                f, "post-report length {n} != expected {SNP_ATTESTATION_REPORT_LEN}"
            ),
            Self::CertBlobTooLarge(n) => {
                write!(f, "post-report cert_blob {n}B exceeds MAX_CERT_BLOB")
            }
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
/// wrong-magic payloads and wire-version mismatches before any
/// further handling.
pub fn validate_request(req: &AttestRequest) -> Result<(), ProtoError> {
    if req.proto != PROTO_TAG {
        return Err(ProtoError::WrongTag);
    }
    if req.client_version != WIRE_VERSION {
        return Err(ProtoError::WireVersionMismatch {
            expected: WIRE_VERSION,
            got: req.client_version,
        });
    }
    Ok(())
}

/// Convenience: validate a deserialised [`AttestPostReport`].
/// Rejects wrong-magic, wrong-version, wrong report length, and
/// oversize cert blobs before the server walks the chain.
pub fn validate_post_report(p: &AttestPostReport) -> Result<(), ProtoError> {
    if p.proto != PROTO_TAG {
        return Err(ProtoError::WrongTag);
    }
    if p.wire_version != WIRE_VERSION {
        return Err(ProtoError::WireVersionMismatch {
            expected: WIRE_VERSION,
            got: p.wire_version,
        });
    }
    if p.report_bytes.len() != SNP_ATTESTATION_REPORT_LEN {
        return Err(ProtoError::ReportLen(p.report_bytes.len()));
    }
    if p.cert_blob.len() > MAX_CERT_BLOB {
        return Err(ProtoError::CertBlobTooLarge(p.cert_blob.len()));
    }
    if p.sig.len() != SIG_LEN {
        return Err(ProtoError::SigLen(p.sig.len()));
    }
    Ok(())
}

/// Construct a well-formed [`AttestPostReport`] from raw report
/// bytes + cert blob. `sig` is pre-filled with zeros — the guest
/// does not yet hold a signing key; authenticity rides on the
/// PSP-signed report inside.
pub fn new_post_report(report_bytes: Vec<u8>, cert_blob: Vec<u8>) -> AttestPostReport {
    AttestPostReport {
        proto: PROTO_TAG,
        wire_version: WIRE_VERSION,
        report_bytes,
        cert_blob,
        sig: vec![0u8; SIG_LEN],
    }
}

/// Convenience: validate a deserialised [`AttestResponse`]. Bounds-
/// checks the secret length and the ECDSA signature length; the
/// actual ECDSA-P256 verification lives in the bootloader / server
/// (pulls in `p256`, which this protocol crate deliberately does not
/// depend on).
pub fn validate_response(resp: &AttestResponse) -> Result<(), ProtoError> {
    if resp.secret.len() > MAX_SECRET_LEN {
        return Err(ProtoError::SecretTooLarge);
    }
    if resp.sig.len() != SIG_LEN {
        return Err(ProtoError::SigLen(resp.sig.len()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Signing domain.
//
// We sign a 32-byte SHA-256 digest computed over the canonical byte
// sequence below. The server owns an ECDSA-P256 long-term key, the
// bootloader embeds the matching public key at build time, and both
// sides recompute the digest identically before signing / verifying.
//
// Each length-prefixed field uses a little-endian `u32` to keep the
// domain compact and endian-stable across host and UEFI targets.
// ---------------------------------------------------------------------------

/// Domain-separation tag for [`response_signing_digest`]. Bumping the
/// tag invalidates every pre-existing signed response.
pub const SIG_DOMAIN_TAG: &[u8] = b"qrab-v1-attest-resp";

/// Compute the 32-byte SHA-256 digest that the attestation server
/// signs and the bootloader verifies. Domain-separated by
/// [`SIG_DOMAIN_TAG`] so an identical field layout can never be
/// mistaken for another protocol.
///
/// The digest commits to:
///   * the server nonce,
///   * the UKI URL (length-prefixed),
///   * the UKI SHA-256,
///   * the secret SHA-256 (committing to the secret transitively
///     through [`AttestResponse::secret_sha256`] rather than rehashing
///     the whole secret body).
pub fn response_signing_digest(resp: &AttestResponse) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SIG_DOMAIN_TAG);
    hasher.update([SIG_DOMAIN_TAG.len() as u8]); // cheap ambiguity break
    hasher.update(resp.server_nonce);
    let url_bytes = resp.uki_url.as_bytes();
    hasher.update((url_bytes.len() as u32).to_le_bytes());
    hasher.update(url_bytes);
    hasher.update(resp.uki_sha256);
    hasher.update(resp.secret_sha256);
    let out = hasher.finalize();
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&out);
    digest
}

/// Recompute [`AttestResponse::secret_sha256`] from [`AttestResponse::secret`]
/// and compare constant-time. Callers use this to make sure the
/// (signed) digest actually matches the transported bytes; a server
/// that lies about one or the other will be rejected before the
/// secret is handed to the guest kernel.
pub fn verify_secret_hash(resp: &AttestResponse) -> bool {
    let computed = Sha256::digest(&resp.secret);
    // Short constant-time compare — `secret_sha256` is 32 bytes so
    // branchless XOR-accumulate is cheap and avoids timing leaks on
    // the hash itself.
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= computed[i] ^ resp.secret_sha256[i];
    }
    diff == 0
}

/// Compute the `report_data` field the bootloader feeds to
/// `SNP_GUEST_REQUEST MSG_REPORT_REQ`. Architecture §"Report data"
/// in `reference/qrab-boot-plan.md`:
///
/// ```text
/// report_data = sha256(PROTO_TAG || client_nonce || server_nonce ||
///                      uki_sha256 || secret_sha256)
/// ```
///
/// The PSP signs the resulting SNP attestation report; a verifier
/// that knows the wire-level nonces + hashes can recompute
/// `report_data` and confirm the guest both observed the same
/// attestation flow and booted the measured UKI.
pub fn report_data(
    client_nonce: &[u8; 32],
    resp: &AttestResponse,
) -> [u8; 64] {
    let mut hasher = Sha256::new();
    hasher.update(PROTO_TAG);
    hasher.update(client_nonce);
    hasher.update(resp.server_nonce);
    hasher.update(resp.uki_sha256);
    hasher.update(resp.secret_sha256);
    let digest = hasher.finalize();
    // `report_data` is a 64-byte field in the SNP report; pad the
    // 32-byte SHA-256 with zeros so the unused tail is deterministic.
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&digest);
    out
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

    fn sample_response() -> AttestResponse {
        AttestResponse {
            server_nonce: [0xAA; 32],
            uki_url: String::from("vsock://2:1337/uki"),
            uki_sha256: [0x11; 32],
            secret: b"hello".to_vec(),
            secret_sha256: sha2::Sha256::digest(b"hello").into(),
            sig: vec![0u8; SIG_LEN],
        }
    }

    #[test]
    fn signing_digest_stable_for_equal_responses() {
        let a = sample_response();
        let b = sample_response();
        assert_eq!(response_signing_digest(&a), response_signing_digest(&b));
    }

    #[test]
    fn signing_digest_changes_with_server_nonce() {
        let a = sample_response();
        let mut b = sample_response();
        b.server_nonce[0] ^= 0xFF;
        assert_ne!(response_signing_digest(&a), response_signing_digest(&b));
    }

    #[test]
    fn signing_digest_changes_with_uki_url() {
        let a = sample_response();
        let mut b = sample_response();
        b.uki_url = String::from("vsock://2:1337/uki2");
        assert_ne!(response_signing_digest(&a), response_signing_digest(&b));
    }

    #[test]
    fn signing_digest_ignores_signature_bytes() {
        let a = sample_response();
        let mut b = sample_response();
        b.sig[0] ^= 0xAA;
        // sig itself is not part of the signing domain — flipping it
        // must not move the digest.
        assert_eq!(response_signing_digest(&a), response_signing_digest(&b));
    }

    #[test]
    fn verify_secret_hash_accepts_matching() {
        let r = sample_response();
        assert!(verify_secret_hash(&r));
    }

    #[test]
    fn verify_secret_hash_rejects_wrong() {
        let mut r = sample_response();
        r.secret_sha256[0] ^= 1;
        assert!(!verify_secret_hash(&r));
    }

    #[test]
    fn report_data_is_stable() {
        let nonce = [0x55u8; 32];
        let r = sample_response();
        let a = report_data(&nonce, &r);
        let b = report_data(&nonce, &r);
        assert_eq!(a, b);
        // Tail after SHA-256 is padded with zeros.
        assert!(a[32..].iter().all(|&b| b == 0));
    }

    #[test]
    fn report_data_changes_with_nonce() {
        let r = sample_response();
        let a = report_data(&[0x00; 32], &r);
        let b = report_data(&[0x01; 32], &r);
        assert_ne!(a, b);
    }

    fn sample_post_report() -> AttestPostReport {
        new_post_report(
            vec![0xABu8; SNP_ATTESTATION_REPORT_LEN],
            b"dummy-cert-blob".to_vec(),
        )
    }

    #[test]
    fn post_report_roundtrip_framed() {
        let p = sample_post_report();
        let wire = encode_framed(&p).expect("encode");
        let back: AttestPostReport = decode_framed(&wire).expect("decode");
        assert_eq!(p, back);
        validate_post_report(&back).expect("validates");
    }

    #[test]
    fn post_report_rejects_wrong_wire_version() {
        let mut p = sample_post_report();
        p.wire_version = 1;
        let err = validate_post_report(&p).unwrap_err();
        assert!(matches!(
            err,
            ProtoError::WireVersionMismatch { expected: WIRE_VERSION, got: 1 }
        ));
    }

    #[test]
    fn post_report_rejects_wrong_tag() {
        let mut p = sample_post_report();
        p.proto = *b"XXX-v99";
        assert!(matches!(
            validate_post_report(&p),
            Err(ProtoError::WrongTag)
        ));
    }

    #[test]
    fn post_report_rejects_wrong_report_len() {
        let mut p = sample_post_report();
        p.report_bytes.truncate(SNP_ATTESTATION_REPORT_LEN - 4);
        let err = validate_post_report(&p).unwrap_err();
        assert!(matches!(err, ProtoError::ReportLen(n) if n == SNP_ATTESTATION_REPORT_LEN - 4));
    }

    #[test]
    fn post_report_rejects_oversize_cert_blob() {
        let mut p = sample_post_report();
        p.cert_blob = vec![0u8; MAX_CERT_BLOB + 1];
        let err = validate_post_report(&p).unwrap_err();
        assert!(matches!(err, ProtoError::CertBlobTooLarge(n) if n == MAX_CERT_BLOB + 1));
    }

    #[test]
    fn post_report_rejects_wrong_sig_len() {
        let mut p = sample_post_report();
        p.sig = vec![0u8; SIG_LEN - 1];
        assert!(matches!(
            validate_post_report(&p),
            Err(ProtoError::SigLen(n)) if n == SIG_LEN - 1
        ));
    }

    #[test]
    fn attest_request_rejects_wrong_wire_version() {
        let mut req = new_request([0; 32]);
        req.client_version = 1;
        let err = validate_request(&req).unwrap_err();
        assert!(matches!(
            err,
            ProtoError::WireVersionMismatch { expected: WIRE_VERSION, got: 1 }
        ));
    }

    #[test]
    fn wire_version_is_two() {
        assert_eq!(WIRE_VERSION, 2);
    }
}
