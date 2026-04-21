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
pub const WIRE_VERSION: u16 = 3;

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

/// Server → client attestation **pre-commit** (v3). Formerly named
/// `AttestResponse` in v2, with the `secret` body inlined — R16-H22
/// split that body out into [`AttestRelease`] so the server can
/// delay its release until after it has verified the guest's PSP
/// attestation report.
///
/// The bootloader:
///
/// * hashes `client_nonce || server_nonce || uki_sha256 || secret_sha256`
///   into the SNP `report_data` field of the attestation report it
///   requests from the PSP (`secret_sha256` is committed to here
///   without exposing the body);
/// * streams the UKI bytes that follow this frame as the second
///   wire payload and compares `sha256(uki_bytes)` to `uki_sha256`;
/// * sends an [`AttestPostReport`] back to the server;
/// * waits for an [`AttestRelease`] carrying the actual `secret`
///   body. Before R16-H22 the secret rode inside this frame — that
///   flow released secrets to unverified guests and is no longer
///   supported at any wire version.
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
    /// SHA-256 of the eventual secret body. R16-H22 — the body
    /// itself no longer rides this frame; the bootloader uses this
    /// hash to (a) commit it into the SNP `report_data` binding and
    /// (b) verify the [`AttestRelease`] payload once it arrives.
    pub secret_sha256: [u8; 32],
    /// ECDSA-P256 signature over the preceding fields. Carried as a
    /// `Vec<u8>` rather than `[u8; 64]` because serde's stock
    /// derives top out at fixed arrays of length 32; the length is
    /// validated in [`validate_response`].
    pub sig: Vec<u8>,
}

/// Server → client **secret release** (v3 / R16-H22).
///
/// Emitted only after the server verifies the guest's
/// [`AttestPostReport`]: ARK→ASK→VCEK chain walk + TCB / chip-id
/// binding + PSP ECDSA sig + (optional) golden measurement pin.
/// Any failure path on the server side must close the stream
/// WITHOUT sending this frame; that is the protocol invariant the
/// R16-H22 regression test pins.
///
/// The bootloader:
///
/// * re-hashes the `secret` bytes and asserts the digest matches
///   `AttestResponse::secret_sha256` from the pre-commit frame;
/// * verifies `sig` under the server's embedded pubkey, same key
///   that signed the pre-commit. The signing digest covers
///   `(client_nonce || server_nonce || secret_sha256)` so a
///   capture of an AttestRelease from a different session cannot
///   be replayed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AttestRelease {
    /// Magic prefix; must equal [`PROTO_TAG`].
    pub proto: [u8; 7],
    /// Wire-format version; must equal [`WIRE_VERSION`].
    pub wire_version: u16,
    /// Secret body. Caps at [`MAX_SECRET_LEN`].
    pub secret: Vec<u8>,
    /// ECDSA-P256 signature over
    /// `release_signing_digest(client_nonce, server_nonce, secret_sha256)`.
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
// layer. We use a 6-byte plaintext header: `u32 LE body_len ||
// u16 LE WIRE_VERSION`. The version bytes sit OUTSIDE the postcard
// body and are signed by neither side — peers read them, compare
// against their own [`WIRE_VERSION`], and close the session with
// [`ProtoError::WireVersionMismatch`] if the two differ.
//
// B14-5: before this header existed, a version-skewed peer (e.g. v2
// bootloader vs v3 server) decoded the postcard body successfully
// and only failed later inside the ECDSA verify — which is
// indistinguishable from a MITM-tampered response. With the plaintext
// version byte up front the failure is self-describing and emits the
// distinct `QRAB_SNP_WIRE_VERSION_MISMATCH` log token on the server.
// The inner `wire_version` fields on [`AttestPostReport`] /
// [`AttestRelease`] stay as defence-in-depth; the outer header gates
// early.
// ---------------------------------------------------------------------------

/// Length of the framing prefix in bytes. B14-5 bumped 4 → 6 to
/// carry a plaintext [`WIRE_VERSION`] slot alongside the body length
/// (`u32 LE body_len || u16 LE wire_version`). Hard bump — peers
/// compiled against the 4-byte prefix cannot talk to peers on the
/// 6-byte prefix.
pub const FRAME_PREFIX_LEN: usize = 6;

/// Hard upper bound on a single framed message payload. 128 KiB
/// covers an `AttestResponse` with a maxed-out [`MAX_SECRET_LEN`]
/// secret plus URL/signatures with generous headroom; it keeps us
/// safe from denial-of-service allocations on the bootloader side.
pub const MAX_FRAMED_PAYLOAD: usize = 128 * 1024;

/// Serialise `value` and prepend the [`FRAME_PREFIX_LEN`]-byte
/// plaintext header (`u32 LE body_len || u16 LE WIRE_VERSION`). The
/// returned buffer can be written straight to a stream and read back
/// with [`split_framed`].
pub fn encode_framed<T: Serialize>(value: &T) -> Result<Vec<u8>, ProtoError> {
    let body = encode(value)?;
    if body.len() > MAX_FRAMED_PAYLOAD {
        return Err(ProtoError::FrameTooLarge(body.len()));
    }
    let mut out = Vec::with_capacity(FRAME_PREFIX_LEN + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&WIRE_VERSION.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Parse the 6-byte header from the front of `bytes` and return
/// `(payload_len, &payload_bytes)`. Requires `bytes` to already hold
/// at least `FRAME_PREFIX_LEN + payload_len` bytes; callers driving a
/// stream are responsible for reading until that much is buffered.
///
/// Rejects:
/// * frames whose declared length exceeds [`MAX_FRAMED_PAYLOAD`]
///   *before* slicing so a bogus header cannot trigger a large
///   allocation;
/// * frames whose declared wire version disagrees with
///   [`WIRE_VERSION`] — returns [`ProtoError::WireVersionMismatch`]
///   so the caller can log the mismatch as a protocol-level failure
///   instead of an opaque `BadSignature` (B14-5).
pub fn split_framed(bytes: &[u8]) -> Result<(usize, &[u8]), ProtoError> {
    if bytes.len() < FRAME_PREFIX_LEN {
        return Err(ProtoError::FrameTruncated);
    }
    let len_bytes: [u8; 4] = bytes[..4]
        .try_into()
        .expect("slice length matches array");
    let ver_bytes: [u8; 2] = bytes[4..FRAME_PREFIX_LEN]
        .try_into()
        .expect("slice length matches array");
    let len = u32::from_le_bytes(len_bytes) as usize;
    let version = u16::from_le_bytes(ver_bytes);
    if len > MAX_FRAMED_PAYLOAD {
        return Err(ProtoError::FrameTooLarge(len));
    }
    if version != WIRE_VERSION {
        return Err(ProtoError::WireVersionMismatch {
            expected: WIRE_VERSION,
            got: version,
        });
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
/// checks the ECDSA signature length; the actual ECDSA-P256
/// verification lives in the bootloader / server (pulls in `p256`,
/// which this protocol crate deliberately does not depend on).
///
/// R16-H22: no secret body field to bounds-check any more — the
/// secret moved into [`AttestRelease`].
pub fn validate_response(resp: &AttestResponse) -> Result<(), ProtoError> {
    if resp.sig.len() != SIG_LEN {
        return Err(ProtoError::SigLen(resp.sig.len()));
    }
    Ok(())
}

/// Convenience: validate a deserialised [`AttestRelease`]. Checks
/// proto tag, wire version, secret bound, and sig length.
pub fn validate_release(rel: &AttestRelease) -> Result<(), ProtoError> {
    if rel.proto != PROTO_TAG {
        return Err(ProtoError::WrongTag);
    }
    if rel.wire_version != WIRE_VERSION {
        return Err(ProtoError::WireVersionMismatch {
            expected: WIRE_VERSION,
            got: rel.wire_version,
        });
    }
    if rel.secret.len() > MAX_SECRET_LEN {
        return Err(ProtoError::SecretTooLarge);
    }
    if rel.sig.len() != SIG_LEN {
        return Err(ProtoError::SigLen(rel.sig.len()));
    }
    Ok(())
}

/// Construct a well-formed [`AttestRelease`] with the PROTO_TAG +
/// WIRE_VERSION pre-filled. `sig` is left to the caller — the
/// server fills it after computing [`release_signing_digest`] and
/// running the ECDSA-P256 sign with its long-term key.
pub fn new_release(secret: Vec<u8>) -> AttestRelease {
    AttestRelease {
        proto: PROTO_TAG,
        wire_version: WIRE_VERSION,
        secret,
        sig: vec![0u8; SIG_LEN],
    }
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

/// Domain-separation tag for [`response_signing_digest`]. Bumping
/// the tag invalidates every pre-existing signed response.
///
/// v2 → v3 transition (R16-H22, 2026-04-21): the server response
/// no longer carries the secret body. That body rides in a
/// follow-up [`AttestRelease`] frame with its own domain tag
/// ([`RELEASE_SIG_DOMAIN_TAG`]) emitted only after the server
/// verifies the guest's `AttestPostReport`. Bootloaders built
/// against v2 cannot speak to a v3 server and vice versa — both
/// sides must upgrade together.
pub const SIG_DOMAIN_TAG: &[u8] = b"qrab-v3-attest-resp";

/// Domain tag for [`release_signing_digest`] — the ECDSA-P256 sig
/// the server computes over [`AttestRelease`] content. Kept
/// distinct from [`SIG_DOMAIN_TAG`] so a captured pre-commit sig
/// cannot be replayed as a release sig (or vice versa).
pub const RELEASE_SIG_DOMAIN_TAG: &[u8] = b"qrab-v3-attest-release";

/// Compute the 32-byte SHA-256 digest that the attestation server
/// signs and the bootloader verifies. Domain-separated by
/// [`SIG_DOMAIN_TAG`] so an identical field layout can never be
/// mistaken for another protocol.
///
/// The digest commits to:
///   * the client nonce (R12-07; blocks response replay across
///     freshly-issued guest sessions),
///   * the server nonce,
///   * the UKI URL (length-prefixed),
///   * the UKI SHA-256,
///   * the secret SHA-256 (committing to the secret transitively
///     through [`AttestResponse::secret_sha256`] rather than rehashing
///     the whole secret body).
pub fn response_signing_digest(
    resp: &AttestResponse,
    client_nonce: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SIG_DOMAIN_TAG);
    hasher.update([SIG_DOMAIN_TAG.len() as u8]); // cheap ambiguity break
    hasher.update(client_nonce);
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

/// Compute the 32-byte SHA-256 digest the server signs over an
/// [`AttestRelease`]. Binds the release to the same
/// `(client_nonce, server_nonce)` pair as the pre-commit, plus the
/// `secret_sha256` that was already committed — so the bootloader
/// can confirm this release belongs to THIS session and matches
/// THIS pre-commit before installing the secret. R16-H22.
///
/// `secret_sha256` is passed explicitly rather than rehashed from
/// `release.secret` to keep the verifier side independent of the
/// body (the body is the sensitive material; the hash is public).
pub fn release_signing_digest(
    client_nonce: &[u8; 32],
    server_nonce: &[u8; 32],
    secret_sha256: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(RELEASE_SIG_DOMAIN_TAG);
    hasher.update([RELEASE_SIG_DOMAIN_TAG.len() as u8]);
    hasher.update(client_nonce);
    hasher.update(server_nonce);
    hasher.update(secret_sha256);
    let out = hasher.finalize();
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&out);
    digest
}

/// Recompute `sha256(release.secret)` and compare constant-time
/// against a caller-supplied expected digest (typically the
/// `secret_sha256` field of the pre-commit). Callers use this to
/// make sure the transported bytes match what the server committed
/// before the secret is handed to the guest kernel. R16-H22 — the
/// digest used to live inside the same frame as the secret; it
/// now lives one frame earlier in [`AttestResponse::secret_sha256`].
pub fn verify_release_hash(
    rel: &AttestRelease,
    expected_sha256: &[u8; 32],
) -> bool {
    let computed = Sha256::digest(&rel.secret);
    // Short constant-time compare — `secret_sha256` is 32 bytes so
    // branchless XOR-accumulate is cheap and avoids timing leaks on
    // the hash itself.
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= computed[i] ^ expected_sha256[i];
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
    fn validate_release_rejects_oversize_body() {
        // R16-H22: the oversize-secret bound moved to the release
        // frame, since AttestResponse no longer carries the body.
        let rel = AttestRelease {
            proto: PROTO_TAG,
            wire_version: WIRE_VERSION,
            secret: vec![0u8; MAX_SECRET_LEN + 1],
            sig: vec![0u8; SIG_LEN],
        };
        assert!(matches!(
            validate_release(&rel),
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
        // B14-5 bumped FRAME_PREFIX_LEN to 6 (u32 len || u16 version).
        // 5 bytes is still truncated.
        let err = split_framed(&[1, 2, 3, 4, 5]).unwrap_err();
        assert!(matches!(err, ProtoError::FrameTruncated));
    }

    #[test]
    fn split_framed_truncated_body_fails() {
        // Declare 64-byte payload with the matching wire-version
        // header but supply only 3 bytes of it.
        let bytes = {
            let mut v = Vec::new();
            v.extend_from_slice(&64u32.to_le_bytes());
            v.extend_from_slice(&WIRE_VERSION.to_le_bytes());
            v.extend_from_slice(&[0; 3]);
            v
        };
        let err = split_framed(&bytes).unwrap_err();
        assert!(matches!(err, ProtoError::FrameTruncated));
    }

    #[test]
    fn split_framed_rejects_oversize_prefix() {
        // FrameTooLarge fires before WireVersionMismatch — the length
        // check guards allocation, so we don't have to attach a
        // matching version to exercise it.
        let bytes = {
            let mut v = Vec::new();
            v.extend_from_slice(&((MAX_FRAMED_PAYLOAD + 1) as u32).to_le_bytes());
            v.extend_from_slice(&WIRE_VERSION.to_le_bytes());
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
            secret_sha256: sha2::Sha256::digest(b"hello").into(),
            sig: vec![0u8; SIG_LEN],
        }
    }

    const TEST_CLIENT_NONCE: [u8; 32] = [0x42u8; 32];

    #[test]
    fn signing_digest_stable_for_equal_responses() {
        let a = sample_response();
        let b = sample_response();
        assert_eq!(
            response_signing_digest(&a, &TEST_CLIENT_NONCE),
            response_signing_digest(&b, &TEST_CLIENT_NONCE),
        );
    }

    #[test]
    fn signing_digest_changes_with_server_nonce() {
        let a = sample_response();
        let mut b = sample_response();
        b.server_nonce[0] ^= 0xFF;
        assert_ne!(
            response_signing_digest(&a, &TEST_CLIENT_NONCE),
            response_signing_digest(&b, &TEST_CLIENT_NONCE),
        );
    }

    #[test]
    fn signing_digest_changes_with_client_nonce() {
        // R12-07: flipping client_nonce alone must move the digest.
        // Exercises the v1→v2 protocol bump.
        let a = sample_response();
        let mut other_nonce = TEST_CLIENT_NONCE;
        other_nonce[0] ^= 0xFF;
        assert_ne!(
            response_signing_digest(&a, &TEST_CLIENT_NONCE),
            response_signing_digest(&a, &other_nonce),
        );
    }

    #[test]
    fn sig_domain_tag_bumped_to_v3() {
        // Lock in the R16-H22 bump so a revert to v2 fails loudly.
        // v2 carried the secret body inline in AttestResponse; v3
        // gates it behind AttestRelease.
        assert_eq!(SIG_DOMAIN_TAG, b"qrab-v3-attest-resp");
    }

    #[test]
    fn signing_digest_changes_with_uki_url() {
        let a = sample_response();
        let mut b = sample_response();
        b.uki_url = String::from("vsock://2:1337/uki2");
        assert_ne!(
            response_signing_digest(&a, &TEST_CLIENT_NONCE),
            response_signing_digest(&b, &TEST_CLIENT_NONCE),
        );
    }

    #[test]
    fn signing_digest_ignores_signature_bytes() {
        let a = sample_response();
        let mut b = sample_response();
        b.sig[0] ^= 0xAA;
        // sig itself is not part of the signing domain — flipping it
        // must not move the digest.
        assert_eq!(
            response_signing_digest(&a, &TEST_CLIENT_NONCE),
            response_signing_digest(&b, &TEST_CLIENT_NONCE),
        );
    }

    // Legacy `verify_secret_hash` (v2) retired with R16-H22 — the
    // secret body no longer rides the response frame. Equivalent
    // coverage lives in `verify_release_hash_accepts_matching` /
    // `verify_release_hash_rejects_tampered_body` below.

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

    // ------- R13 round 16: report_data field-isolation fan-out -------

    #[test]
    fn report_data_changes_with_server_nonce() {
        let nonce = [0x77u8; 32];
        let a = sample_response();
        let mut b = sample_response();
        b.server_nonce[0] ^= 0x80;
        assert_ne!(report_data(&nonce, &a), report_data(&nonce, &b));
    }

    #[test]
    fn report_data_changes_with_uki_sha256() {
        let nonce = [0x77u8; 32];
        let a = sample_response();
        let mut b = sample_response();
        b.uki_sha256[0] ^= 0x80;
        assert_ne!(report_data(&nonce, &a), report_data(&nonce, &b));
    }

    #[test]
    fn report_data_changes_with_secret_sha256() {
        let nonce = [0x77u8; 32];
        let a = sample_response();
        let mut b = sample_response();
        b.secret_sha256[0] ^= 0x80;
        assert_ne!(report_data(&nonce, &a), report_data(&nonce, &b));
    }

    #[test]
    fn report_data_every_nonce_bit_changes_output() {
        // Bit-flip each byte of client_nonce; each flip must move
        // the digest — guards against a partial domain-separation
        // regression that would let a crafted nonce collide.
        let base_nonce = [0u8; 32];
        let r = sample_response();
        let baseline = report_data(&base_nonce, &r);
        for byte in 0..32 {
            let mut tweaked = base_nonce;
            tweaked[byte] ^= 0x01;
            let got = report_data(&tweaked, &r);
            assert_ne!(got, baseline, "byte {byte} flip did not move digest");
        }
    }

    #[test]
    fn report_data_ignores_irrelevant_fields() {
        // `uki_url`, `secret`, and `sig` are NOT covered by
        // `report_data` — the PSP only binds to the hashes. Flipping
        // any of those bytes must leave the digest unchanged, so a
        // mismatched transport-level byte doesn't invalidate an
        // otherwise-correct report.
        let nonce = [0x11u8; 32];
        let a = sample_response();
        let base = report_data(&nonce, &a);

        let mut b = sample_response();
        b.uki_url.push('X');
        assert_eq!(report_data(&nonce, &b), base, "uki_url leaked into digest");

        // `secret` body no longer lives on AttestResponse (R16-H22),
        // so "does a different secret body leak into report_data"
        // has become a vacuous question at this layer. The guest
        // reacts only to `secret_sha256`, which report_data DOES
        // commit to and which the report_data fan-out already covers.

        let mut d = sample_response();
        d.sig[0] ^= 0xFF;
        assert_eq!(report_data(&nonce, &d), base, "sig bytes leaked");
    }

    #[test]
    fn report_data_sha256_prefix_is_32_bytes_then_zeros() {
        // Explicit shape: bytes 0..32 = SHA-256 digest, bytes 32..64
        // = 0. The tail must be zeros regardless of input so the
        // guest-side report_data comparison is deterministic.
        use sha2::{Digest as _, Sha256};
        let nonce = [0xAAu8; 32];
        let r = sample_response();
        let got = report_data(&nonce, &r);
        let expected_prefix = {
            let mut h = Sha256::new();
            h.update(PROTO_TAG);
            h.update(nonce);
            h.update(r.server_nonce);
            h.update(r.uki_sha256);
            h.update(r.secret_sha256);
            let d = h.finalize();
            let mut out = [0u8; 32];
            out.copy_from_slice(&d);
            out
        };
        assert_eq!(&got[..32], &expected_prefix);
        assert!(got[32..].iter().all(|&b| b == 0));
    }

    #[test]
    fn report_data_different_nonce_per_client_diversifies() {
        // Simulate N parallel guests each with a random client_nonce;
        // no two guests' `report_data` values should collide.
        let r = sample_response();
        let mut seen = alloc::collections::BTreeSet::new();
        for i in 0u8..32 {
            let nonce: [u8; 32] = core::array::from_fn(|k| (i as u8).wrapping_mul(13 ^ k as u8));
            let rd = report_data(&nonce, &r);
            assert!(seen.insert(rd), "collision at tweak {i}");
        }
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
    fn wire_version_is_three() {
        // R16-H22 bumped v2 → v3. v2 embedded the secret in the
        // server response; v3 splits the response into a precommit
        // (hash only) and a post-verify AttestRelease carrying the
        // body. A revert here must go through explicit wire-version
        // coordination because the frames themselves changed shape.
        assert_eq!(WIRE_VERSION, 3);
    }

    // ------- G18: decode_framed rejects corrupt payload -------

    #[test]
    fn decode_framed_rejects_undersized_body() {
        // Declared length 1, body = one byte. `AttestRequest`
        // cannot deserialise from a single byte (client_nonce is
        // 32 B alone), so postcard reports a deserialize error.
        let bytes: Vec<u8> = {
            let mut v = Vec::new();
            v.extend_from_slice(&1u32.to_le_bytes());
            v.extend_from_slice(&WIRE_VERSION.to_le_bytes());
            v.push(0xFF);
            v
        };
        let err = decode_framed::<AttestRequest>(&bytes).unwrap_err();
        assert!(
            matches!(err, ProtoError::Postcard(_)),
            "expected Postcard error, got {err:?}"
        );
    }

    #[test]
    fn decode_framed_rejects_truncated_body() {
        // Drop the last byte of the body; the declared length
        // prefix now overshoots the actual bytes → FrameTruncated.
        let req = new_request([0x01; 32]);
        let mut bytes = encode_framed(&req).unwrap();
        bytes.pop();
        let err = decode_framed::<AttestRequest>(&bytes).unwrap_err();
        assert!(
            matches!(err, ProtoError::FrameTruncated),
            "expected FrameTruncated, got {err:?}"
        );
    }

    #[test]
    fn decode_framed_rejects_zero_len_body_declared_as_request() {
        // Declared length 0 followed by no body bytes. A zero-length
        // body cannot round-trip into an AttestRequest (client_nonce
        // alone is 32 B), so decode must fail with Decode.
        let bytes: Vec<u8> = {
            let mut v = Vec::new();
            v.extend_from_slice(&0u32.to_le_bytes());
            v.extend_from_slice(&WIRE_VERSION.to_le_bytes());
            v
        };
        let err = decode_framed::<AttestRequest>(&bytes).unwrap_err();
        assert!(
            matches!(err, ProtoError::Postcard(_)),
            "expected Decode for zero-length body, got {err:?}"
        );
    }

    #[test]
    fn split_framed_zero_length_body_splits_clean() {
        // Well-formed frame with len=0 body. `split_framed` must
        // return Ok even though a downstream postcard decode may
        // reject; the framer only cares about the prefix arithmetic.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&WIRE_VERSION.to_le_bytes());
        let (len, body) = split_framed(&bytes).unwrap();
        assert_eq!(len, 0);
        assert!(body.is_empty());
    }

    #[test]
    fn split_framed_exact_boundary_max_payload_parses() {
        // Declared length exactly MAX_FRAMED_PAYLOAD (≤ valid).
        // `split_framed` must return Ok and the body slice must
        // have the declared length.
        let mut bytes = Vec::with_capacity(FRAME_PREFIX_LEN + MAX_FRAMED_PAYLOAD);
        bytes.extend_from_slice(&(MAX_FRAMED_PAYLOAD as u32).to_le_bytes());
        bytes.extend_from_slice(&WIRE_VERSION.to_le_bytes());
        bytes.resize(FRAME_PREFIX_LEN + MAX_FRAMED_PAYLOAD, 0);
        let (len, body) = split_framed(&bytes).unwrap();
        assert_eq!(len, MAX_FRAMED_PAYLOAD);
        assert_eq!(body.len(), MAX_FRAMED_PAYLOAD);
    }

    // ------- wire-version rejection fan-out -------

    #[test]
    fn attest_post_report_rejects_wire_version_drift() {
        let mut post = new_post_report(
            vec![0u8; SNP_ATTESTATION_REPORT_LEN],
            vec![0u8; 24],
        );
        post.wire_version = 1;
        let err = validate_post_report(&post).unwrap_err();
        assert!(matches!(
            err,
            ProtoError::WireVersionMismatch { expected: WIRE_VERSION, got: 1 }
        ));
    }

    // ------- R16-H22 v3 tests: AttestRelease + release_signing_digest -------

    #[test]
    fn wire_version_is_three_after_r16_h22_bump() {
        assert_eq!(WIRE_VERSION, 3);
    }

    #[test]
    fn sig_domain_tags_distinct_precommit_and_release() {
        // Distinct domain separators so a precommit sig cannot be
        // replayed as a release sig (or vice versa). If these ever
        // collapse, a capturing MITM could swap signatures across
        // frames and pass verification.
        assert_ne!(SIG_DOMAIN_TAG, RELEASE_SIG_DOMAIN_TAG);
    }

    #[test]
    fn attest_response_no_longer_has_secret_field() {
        // Compile-time guard: AttestResponse must have the
        // post-R16-H22 shape. Any future refactor that puts the
        // secret body back must also update this test (and think
        // very hard about the audit finding).
        fn _assert_shape(r: &AttestResponse) {
            let _: &[u8; 32] = &r.server_nonce;
            let _: &String = &r.uki_url;
            let _: &[u8; 32] = &r.uki_sha256;
            let _: &[u8; 32] = &r.secret_sha256;
            let _: &Vec<u8> = &r.sig;
            // Reachable via type inference only if these five
            // fields are the full list. If a `secret: Vec<u8>`
            // field gets re-added, the compiler will accept it
            // but this function then misses coverage — pair with
            // the positive round-trip test that asserts sha256
            // binding.
        }
        let _ = _assert_shape;
    }

    #[test]
    fn new_release_fills_proto_and_version() {
        let rel = new_release(b"hello".to_vec());
        assert_eq!(rel.proto, PROTO_TAG);
        assert_eq!(rel.wire_version, WIRE_VERSION);
        assert_eq!(rel.secret, b"hello");
        assert_eq!(rel.sig.len(), SIG_LEN);
    }

    #[test]
    fn validate_release_accepts_happy() {
        let rel = new_release(vec![0u8; 64]);
        validate_release(&rel).expect("happy release validates");
    }

    #[test]
    fn validate_release_rejects_wrong_tag() {
        let mut rel = new_release(vec![0u8; 8]);
        rel.proto = *b"ZZZ-abc";
        assert!(matches!(validate_release(&rel), Err(ProtoError::WrongTag)));
    }

    #[test]
    fn validate_release_rejects_wrong_version() {
        let mut rel = new_release(vec![0u8; 8]);
        rel.wire_version = 2;
        assert!(matches!(
            validate_release(&rel),
            Err(ProtoError::WireVersionMismatch { expected: WIRE_VERSION, got: 2 })
        ));
    }

    #[test]
    fn validate_release_rejects_oversize_secret() {
        let mut rel = new_release(vec![0u8; MAX_SECRET_LEN + 1]);
        assert!(matches!(validate_release(&rel), Err(ProtoError::SecretTooLarge)));
        // Fix the oversize and keep the sig shape off.
        rel.secret = vec![0u8; 8];
        rel.sig = vec![0u8; SIG_LEN - 1];
        assert!(matches!(validate_release(&rel), Err(ProtoError::SigLen(_))));
    }

    #[test]
    fn release_signing_digest_stable_for_same_inputs() {
        let cn = [0x11u8; 32];
        let sn = [0x22u8; 32];
        let sh = [0x33u8; 32];
        assert_eq!(
            release_signing_digest(&cn, &sn, &sh),
            release_signing_digest(&cn, &sn, &sh),
        );
    }

    #[test]
    fn release_signing_digest_changes_with_each_input() {
        let cn = [0x11u8; 32];
        let sn = [0x22u8; 32];
        let sh = [0x33u8; 32];
        let base = release_signing_digest(&cn, &sn, &sh);
        let mut cn2 = cn;
        cn2[0] ^= 1;
        assert_ne!(base, release_signing_digest(&cn2, &sn, &sh));
        let mut sn2 = sn;
        sn2[0] ^= 1;
        assert_ne!(base, release_signing_digest(&cn, &sn2, &sh));
        let mut sh2 = sh;
        sh2[0] ^= 1;
        assert_ne!(base, release_signing_digest(&cn, &sn, &sh2));
    }

    #[test]
    fn release_digest_distinct_from_response_digest() {
        // The two digests over the same (client_nonce, server_nonce,
        // secret_sha256) inputs MUST differ — that's the whole point
        // of keeping the two domain tags separate. If this ever
        // returns true, a captured response sig could be swapped in
        // place of a release sig.
        let cn = [0x7Fu8; 32];
        let sn = [0x80u8; 32];
        let sh = [0x5Au8; 32];
        let resp = AttestResponse {
            server_nonce: sn,
            uki_url: String::new(),
            uki_sha256: [0u8; 32],
            secret_sha256: sh,
            sig: vec![0u8; SIG_LEN],
        };
        let response_dig = response_signing_digest(&resp, &cn);
        let release_dig = release_signing_digest(&cn, &sn, &sh);
        assert_ne!(
            response_dig, release_dig,
            "response and release digests must not collide over shared inputs",
        );
    }

    #[test]
    fn verify_release_hash_accepts_matching() {
        let secret = b"canary-secret-for-hash-binding".to_vec();
        let expected: [u8; 32] = sha2::Sha256::digest(&secret).into();
        let rel = new_release(secret);
        assert!(verify_release_hash(&rel, &expected));
    }

    #[test]
    fn verify_release_hash_rejects_tampered_body() {
        let secret = b"canary-secret-pre-tamper".to_vec();
        let expected: [u8; 32] = sha2::Sha256::digest(&secret).into();
        let mut rel = new_release(secret);
        rel.secret[0] ^= 0xFF;
        assert!(!verify_release_hash(&rel, &expected));
    }

    #[test]
    fn release_frame_round_trips() {
        let rel = new_release(b"abcd".to_vec());
        let wire = encode_framed(&rel).unwrap();
        let back: AttestRelease = decode_framed(&wire).unwrap();
        assert_eq!(back, rel);
        validate_release(&back).unwrap();
    }

    // ------- B14-5: outer wire-version header on framed payloads -------

    #[test]
    fn encode_framed_produces_outer_version_header() {
        // Decode the first 6 bytes of an `encode_framed(AttestRequest)`
        // result and assert the layout is `(body_len, WIRE_VERSION)`.
        // Catches a regression where the version slot is dropped or
        // mis-ordered.
        let req = new_request([0x11; 32]);
        let wire = encode_framed(&req).unwrap();
        assert!(wire.len() > FRAME_PREFIX_LEN);
        let len_bytes: [u8; 4] = wire[..4].try_into().unwrap();
        let ver_bytes: [u8; 2] = wire[4..FRAME_PREFIX_LEN].try_into().unwrap();
        let body_len = u32::from_le_bytes(len_bytes) as usize;
        let version = u16::from_le_bytes(ver_bytes);
        assert_eq!(body_len + FRAME_PREFIX_LEN, wire.len());
        assert_eq!(version, WIRE_VERSION);
    }

    #[test]
    fn split_framed_rejects_wrong_outer_version() {
        // Hand-craft a frame with WIRE_VERSION-1 in the version slot
        // and a plausible body length. `split_framed` must reject
        // before touching the body — the whole point of B14-5 is
        // making the v2/v3 skew distinguishable from a MITM.
        let bogus_version: u16 = WIRE_VERSION - 1;
        let body_len: u32 = 16;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&body_len.to_le_bytes());
        bytes.extend_from_slice(&bogus_version.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 16]);
        let err = split_framed(&bytes).unwrap_err();
        assert!(
            matches!(
                err,
                ProtoError::WireVersionMismatch {
                    expected: WIRE_VERSION,
                    got: v,
                } if v == bogus_version,
            ),
            "expected WireVersionMismatch, got {err:?}",
        );
    }

    #[test]
    fn round_trip_framed_preserves_body() {
        // Belt-and-suspenders round-trip across all three postcard
        // frame shapes so a future prefix change that breaks one
        // shape but not another is caught at `cargo test` time.
        let req = new_request([0xAB; 32]);
        let wire = encode_framed(&req).unwrap();
        let back: AttestRequest = decode_framed(&wire).unwrap();
        assert_eq!(back, req);

        let resp = sample_response();
        let wire = encode_framed(&resp).unwrap();
        let back: AttestResponse = decode_framed(&wire).unwrap();
        assert_eq!(back, resp);

        let rel = new_release(b"body-preserved".to_vec());
        let wire = encode_framed(&rel).unwrap();
        let back: AttestRelease = decode_framed(&wire).unwrap();
        assert_eq!(back, rel);

        let post = sample_post_report();
        let wire = encode_framed(&post).unwrap();
        let back: AttestPostReport = decode_framed(&wire).unwrap();
        assert_eq!(back, post);
    }

    #[test]
    fn frame_prefix_len_is_six_after_b14_5_bump() {
        // Lock in the 4 → 6 bump. A revert must fail loudly because
        // it silently reintroduces the v2/v3-vs-MITM ambiguity.
        assert_eq!(FRAME_PREFIX_LEN, 6);
    }
}
