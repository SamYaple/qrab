//! Pure attestation logic for `attest-server`.
//!
//! The binary target (`main.rs`) owns the vsock listener and the key
//! lifecycle; this crate owns the protocol-level handler so it can be
//! unit-tested without the host kernel module. `handle_request_bytes`
//! takes an incoming framed [`AttestRequest`] and returns the framed
//! [`AttestResponse`] bytes to write back — no I/O.
//!
//! As of phase 4 the server signs every response with an ECDSA-P256
//! long-term key. The bootloader embeds the matching public key at
//! build time and rejects any response whose signature fails to
//! verify against [`qrab_boot_proto::response_signing_digest`].

use p256::ecdsa::{signature::Signer as _, Signature, SigningKey, VerifyingKey};
use p256::pkcs8::DecodePrivateKey;
use qrab_boot_proto::{
    AttestRequest, AttestResponse, ProtoError, SIG_LEN, decode_framed, encode_framed,
    response_signing_digest, validate_request,
};

pub mod verify;
use rand_core::{OsRng, RngCore};
use sha2::Digest as _;

/// Errors surfaced by [`handle_request_bytes`]. All variants map to
/// a logged reason on the server side and close the connection.
#[derive(Debug)]
pub enum HandleError {
    /// `decode_framed` rejected the client's bytes.
    Decode(ProtoError),
    /// `validate_request` rejected the decoded payload (wrong magic,
    /// etc).
    Validate(ProtoError),
    /// `encode_framed` rejected our own response — should not happen
    /// in the current fixed-size stub, but propagating is cheaper
    /// than panicking.
    Encode(ProtoError),
}

/// Long-term signing context for the attestation server. Wraps an
/// ECDSA-P256 signing key; exposes the verifying-key side so callers
/// (tests, the binary's `--print-pubkey`) can export the public half.
#[derive(Clone)]
pub struct Signer {
    key: SigningKey,
}

/// Errors from [`Signer::from_pem`] / [`Signer::from_pkcs8_der`].
#[derive(Debug)]
pub enum KeyError {
    /// PEM decoding or parsing failed.
    Pem(p256::pkcs8::Error),
}

impl core::fmt::Display for KeyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Pem(e) => write!(f, "PKCS#8 key decode: {e}"),
        }
    }
}

impl std::error::Error for KeyError {}

impl Signer {
    /// Parse a PKCS#8 PEM-encoded P-256 private key.
    pub fn from_pem(pem: &str) -> Result<Self, KeyError> {
        let key = SigningKey::from_pkcs8_pem(pem).map_err(KeyError::Pem)?;
        Ok(Self { key })
    }

    /// Parse a PKCS#8 DER-encoded P-256 private key.
    pub fn from_pkcs8_der(der: &[u8]) -> Result<Self, KeyError> {
        let key = SigningKey::from_pkcs8_der(der).map_err(KeyError::Pem)?;
        Ok(Self { key })
    }

    /// Generate a fresh ephemeral signing key. Useful for smoke-test
    /// deployments that do not yet have persistent key material.
    pub fn generate() -> Self {
        let key = SigningKey::random(&mut OsRng);
        Self { key }
    }

    /// Return the verifying-key half of this signer as a
    /// SEC1-uncompressed 65-byte blob (`0x04 || X || Y`). This is the
    /// bytes-on-disk format the bootloader's build script accepts via
    /// `QRAB_ATTEST_PUBKEY_SEC1`.
    pub fn pubkey_sec1(&self) -> [u8; 65] {
        let vk: VerifyingKey = *self.key.verifying_key();
        let point = vk.to_encoded_point(/* compress = */ false);
        let bytes = point.as_bytes();
        let mut out = [0u8; 65];
        // `to_encoded_point(false)` returns exactly 65 bytes for P-256.
        out.copy_from_slice(bytes);
        out
    }

    /// Sign the canonical digest for `response` binding the client's
    /// `client_nonce`, returning a 64-byte raw-IEEE-P1363 (r || s)
    /// signature. R12-07: `client_nonce` is part of the signing
    /// domain so a signed reply cannot be replayed to a different
    /// guest session.
    pub fn sign_response(
        &self,
        response: &AttestResponse,
        client_nonce: &[u8; 32],
    ) -> [u8; SIG_LEN] {
        let digest = response_signing_digest(response, client_nonce);
        let sig: Signature = self.key.sign(&digest);
        let bytes = sig.to_bytes();
        let mut out = [0u8; SIG_LEN];
        out.copy_from_slice(&bytes);
        out
    }

    /// Sign a 32-byte digest with the server's long-term P-256 key.
    /// Used by [`build_signed_release`] to sign the R16-H22 release
    /// frame with its own domain separator.
    pub fn sign_digest(&self, digest: &[u8; 32]) -> [u8; SIG_LEN] {
        let sig: Signature = self.key.sign(digest);
        let bytes = sig.to_bytes();
        let mut out = [0u8; SIG_LEN];
        out.copy_from_slice(&bytes);
        out
    }
}

/// Decode a framed [`AttestRequest`] and produce the v3 pre-commit
/// [`AttestResponse`] (signed; NO secret body) plus the secret the
/// server holds back to send as a follow-up [`AttestRelease`] after
/// post-report verify. R16-H22.
///
/// Returns `(precommit_frame_bytes, client_nonce, session)` so the
/// caller can:
///   1. write the pre-commit bytes immediately,
///   2. stream the UKI,
///   3. read the guest's `AttestPostReport`,
///   4. run the chain walk,
///   5. on OK, pass `session.secret` + the remembered `client_nonce`
///      + the signed `server_nonce` to [`build_signed_release`].
pub fn handle_request_bytes(
    bytes: &[u8],
    signer: &Signer,
    uki_bytes: &[u8],
) -> Result<([u8; 32], ServerSession, Vec<u8>), HandleError> {
    let request: AttestRequest = decode_framed(bytes).map_err(HandleError::Decode)?;
    validate_request(&request).map_err(HandleError::Validate)?;
    let session = build_signed_response(&request, signer, uki_bytes);
    let frame = encode_framed(&session.response).map_err(HandleError::Encode)?;
    Ok((request.client_nonce, session, frame))
}

/// Build the v3 pre-commit: `AttestResponse` with `secret_sha256`
/// filled but no secret body, signed with `signer`. The server
/// fills `server_nonce` with OS-supplied random bytes so each
/// response is fresh and unpredictable — mixing real entropy into
/// the transcript defeats replay and guards against a downstream
/// binding that treats `server_nonce` as a challenge. The
/// response's `uki_sha256` field commits to the bytes the caller
/// will transmit as the second framed payload.
///
/// Returns a [`ServerSession`] so the caller also gets the held-
/// back secret body for the later [`AttestRelease`].
pub fn build_signed_response(
    req: &AttestRequest,
    signer: &Signer,
    uki_bytes: &[u8],
) -> ServerSession {
    let mut session = build_body(req, uki_bytes);
    let sig = signer.sign_response(&session.response, &req.client_nonce);
    session.response.sig = sig.to_vec();
    session
}

/// Build a signed [`AttestRelease`] carrying the secret body. The
/// server MUST only call this after the guest's
/// [`qrab_boot_proto::AttestPostReport`] has verified (full chain
/// walk + PSP sig + optional measurement pin). R16-H22 protocol
/// invariant — the test
/// `server_does_not_leak_secret_before_post_report` pins it.
pub fn build_signed_release(
    signer: &Signer,
    client_nonce: &[u8; 32],
    session: &ServerSession,
) -> qrab_boot_proto::AttestRelease {
    let mut release = qrab_boot_proto::new_release(session.secret.clone());
    let digest = qrab_boot_proto::release_signing_digest(
        client_nonce,
        &session.response.server_nonce,
        &session.response.secret_sha256,
    );
    release.sig = signer.sign_digest(&digest).to_vec();
    release
}

/// Construct the unsigned response body for `req`. The `sig` field is
/// filled with zero bytes of the correct length so
/// [`qrab_boot_proto::response_signing_digest`] produces a stable
/// value before signing.
/// Stub secret the built-in handler injects into each response.
/// Real deployments override via a secret-source plugin (not yet
/// implemented). Exposed for tests that need to window-scan the
/// wire for the canary bytes — R16-H22 regression.
pub const STUB_SECRET: &[u8] = b"QRAB_STUB_SECRET_v0";

/// Bundle of everything the server needs to drive the v3 protocol
/// for one session: the pre-commit `AttestResponse` it writes early
/// (sans secret), and the secret body it holds back to ship inside
/// the post-verify [`AttestRelease`] frame. R16-H22.
#[derive(Debug, Clone)]
pub struct ServerSession {
    pub response: AttestResponse,
    /// Secret body the server will send inside `AttestRelease` if —
    /// and only if — the guest's `AttestPostReport` verifies.
    pub secret: Vec<u8>,
}

fn build_body(_req: &AttestRequest, uki_bytes: &[u8]) -> ServerSession {
    // Fresh random bytes per request. `rand_core::OsRng` reads from
    // the kernel CSPRNG on every `fill_bytes` call, so two requests
    // with identical `client_nonce`s still see distinct
    // `server_nonce`s. The size mirrors `client_nonce`'s 32-byte
    // array in `AttestRequest`.
    let mut server_nonce = [0u8; 32];
    OsRng.fill_bytes(&mut server_nonce);
    let secret = STUB_SECRET.to_vec();
    let secret_sha256: [u8; 32] = sha2::Sha256::digest(&secret).into();
    let uki_sha256: [u8; 32] = sha2::Sha256::digest(uki_bytes).into();
    ServerSession {
        response: AttestResponse {
            server_nonce,
            uki_url: String::from("vsock://2:1337/stub-uki"),
            uki_sha256,
            secret_sha256,
            sig: vec![0u8; SIG_LEN],
        },
        secret,
    }
}

/// Convenience: the built-in stub UKI payload. 512 bytes of a repeating
/// magic string is enough to exercise the two-frame wire path without
/// bloating the boot-time log or stressing the vsock ring. Operators
/// point `$QRAB_ATTEST_UKI_PATH` at a real signed UKI to replace this.
pub const STUB_UKI: &[u8; 512] = &{
    let mut out = [0u8; 512];
    let tag = b"QRAB_STUB_UKI_v0\n";
    let mut i = 0;
    while i < out.len() {
        out[i] = tag[i % tag.len()];
        i += 1;
    }
    out
};

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::{signature::Verifier as _, Signature as P256Sig, VerifyingKey};
    use qrab_boot_proto::{
        AttestResponse, PROTO_TAG, WIRE_VERSION, new_request, split_framed,
    };

    fn fixed_signer() -> Signer {
        // Deterministic key for tests. P-256 scalar cannot be zero and
        // must be < n; a small constant well below n is fine.
        let scalar = [1u8; 32];
        let key = SigningKey::from_bytes((&scalar).into())
            .expect("non-zero small scalar is a valid P-256 private key");
        Signer { key }
    }

    #[test]
    fn build_signed_response_has_fresh_random_nonce() {
        // Two calls with the same input must produce distinct
        // `server_nonce`s (the old `!client_nonce` rule was
        // deterministic and leaked no entropy). We also reject the
        // all-zero and all-inverted sentinels so a regression to a
        // deterministic derivation trips this test.
        let req = new_request([0x55; 32]);
        let signer = fixed_signer();
        let a = build_signed_response(&req, &signer, STUB_UKI).response;
        let b = build_signed_response(&req, &signer, STUB_UKI).response;
        assert_ne!(a.server_nonce, b.server_nonce, "nonce must be random per call");
        assert!(!a.server_nonce.iter().all(|&x| x == 0), "nonce must not be all-zero");
        assert!(
            !a.server_nonce.iter().all(|&x| x == 0xAA),
            "nonce must not echo legacy !0x55 pattern"
        );
        assert_eq!(a.sig.len(), SIG_LEN);
        qrab_boot_proto::validate_response(&a).expect("stub response must validate");
        // `secret_sha256` commits to the built-in stub secret.
        assert_eq!(
            a.secret_sha256,
            <[u8; 32]>::from(sha2::Sha256::digest(STUB_SECRET)),
        );
    }

    #[test]
    fn sig_verifies_against_matching_pubkey() {
        let signer = fixed_signer();
        let req = new_request([0x77; 32]);
        let resp = build_signed_response(&req, &signer, STUB_UKI).response;

        let sec1 = signer.pubkey_sec1();
        let vk = VerifyingKey::from_sec1_bytes(&sec1).expect("valid SEC1 pubkey");
        let digest = qrab_boot_proto::response_signing_digest(&resp, &req.client_nonce);
        let sig = P256Sig::from_slice(&resp.sig).expect("sig parses");
        vk.verify(&digest, &sig).expect("valid sig must verify");
    }

    #[test]
    fn sig_rejects_tampered_response() {
        let signer = fixed_signer();
        let req = new_request([0x88; 32]);
        let mut resp = build_signed_response(&req, &signer, STUB_UKI).response;
        // Flip a bit in a signed field.
        resp.uki_sha256[0] ^= 1;

        let vk = VerifyingKey::from_sec1_bytes(&signer.pubkey_sec1()).unwrap();
        let digest = qrab_boot_proto::response_signing_digest(&resp, &req.client_nonce);
        let sig = P256Sig::from_slice(&resp.sig).unwrap();
        assert!(vk.verify(&digest, &sig).is_err());
    }

    #[test]
    fn sig_rejects_mismatched_client_nonce() {
        // R12-07: a response signed for session A's client_nonce must
        // not verify when presented to session B.
        let signer = fixed_signer();
        let req_a = new_request([0x77; 32]);
        let resp = build_signed_response(&req_a, &signer, STUB_UKI).response;

        let wrong_nonce = [0x88u8; 32];
        let vk = VerifyingKey::from_sec1_bytes(&signer.pubkey_sec1()).unwrap();
        let digest = qrab_boot_proto::response_signing_digest(&resp, &wrong_nonce);
        let sig = P256Sig::from_slice(&resp.sig).unwrap();
        assert!(
            vk.verify(&digest, &sig).is_err(),
            "cross-session response must not verify with foreign client_nonce"
        );
    }

    #[test]
    fn handle_roundtrip_happy_path() {
        let signer = fixed_signer();
        let req = new_request([0x77; 32]);
        let wire = qrab_boot_proto::encode_framed(&req).unwrap();
        let (client_nonce, session, reply) =
            handle_request_bytes(&wire, &signer, STUB_UKI).expect("handle must succeed");
        assert_eq!(client_nonce, req.client_nonce);
        assert_eq!(session.secret, STUB_SECRET);

        let response: AttestResponse =
            qrab_boot_proto::decode_framed(&reply).expect("reply must decode");
        // `server_nonce` is fresh random bytes, not `!client_nonce`,
        // so just check it isn't the legacy deterministic pattern and
        // isn't all-zero.
        assert!(!response.server_nonce.iter().all(|&b| b == !0x77));
        assert!(!response.server_nonce.iter().all(|&b| b == 0));
        // R16-H22: the pre-commit carries secret_sha256 but NOT the
        // secret body. The body lives in the caller's `session` and
        // ships later in `AttestRelease`.
        assert_eq!(
            response.secret_sha256,
            <[u8; 32]>::from(sha2::Sha256::digest(STUB_SECRET)),
        );
        assert_eq!(session.secret, STUB_SECRET);

        let (len, body) = split_framed(&reply).unwrap();
        assert_eq!(len + qrab_boot_proto::FRAME_PREFIX_LEN, reply.len());
        assert_eq!(body.len(), len);
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut req = new_request([0; 32]);
        req.proto = *b"XXX-v99";
        let wire = qrab_boot_proto::encode_framed(&req).unwrap();
        let err = handle_request_bytes(&wire, &fixed_signer(), STUB_UKI).unwrap_err();
        match err {
            HandleError::Validate(ProtoError::WrongTag) => {}
            other => panic!("expected Validate(WrongTag), got {other:?}"),
        }
    }

    #[test]
    fn rejects_garbage_bytes() {
        let err = handle_request_bytes(&[0, 0, 0, 0], &fixed_signer(), STUB_UKI).unwrap_err();
        assert!(matches!(err, HandleError::Decode(_)));
    }

    #[test]
    fn response_matches_current_wire_version() {
        let req = new_request([0xFF; 32]);
        assert_eq!(req.client_version, WIRE_VERSION);
        assert_eq!(req.proto, PROTO_TAG);
    }

    #[test]
    fn pubkey_sec1_is_uncompressed() {
        let sec1 = fixed_signer().pubkey_sec1();
        assert_eq!(sec1.len(), 65);
        assert_eq!(sec1[0], 0x04, "uncompressed SEC1 starts with 0x04");
    }

    #[test]
    fn generated_keys_are_distinct() {
        let a = Signer::generate();
        let b = Signer::generate();
        assert_ne!(a.pubkey_sec1(), b.pubkey_sec1());
    }

    // ------- R16-H22 v3 protocol tests -------

    #[test]
    fn wire_version_bumped_to_three() {
        assert_eq!(WIRE_VERSION, 3);
        assert_eq!(qrab_boot_proto::SIG_DOMAIN_TAG, b"qrab-v3-attest-resp");
        assert_eq!(
            qrab_boot_proto::RELEASE_SIG_DOMAIN_TAG,
            b"qrab-v3-attest-release"
        );
    }

    #[test]
    fn precommit_frame_never_carries_secret_bytes() {
        // Structural proof at the wire level: the serialised bytes
        // of a well-formed AttestResponse must not contain the
        // secret body. R16-H22 — the vuln was the secret riding in
        // this frame. v3's AttestResponse only has secret_sha256.
        let signer = fixed_signer();
        let req = new_request([0x77; 32]);
        let (_, session, precommit_bytes) =
            handle_request_bytes(&qrab_boot_proto::encode_framed(&req).unwrap(), &signer, STUB_UKI)
                .unwrap();
        assert!(
            !precommit_bytes
                .windows(STUB_SECRET.len())
                .any(|w| w == STUB_SECRET),
            "R16-H22: secret leaked into AttestResponse precommit frame",
        );
        // But the server's held-back secret DOES match STUB_SECRET.
        assert_eq!(session.secret, STUB_SECRET);
    }

    #[test]
    fn build_signed_release_round_trips_through_verifier() {
        use p256::ecdsa::{signature::Verifier as _, Signature as P256Sig, VerifyingKey};
        let signer = fixed_signer();
        let req = new_request([0xA5; 32]);
        let session = build_signed_response(&req, &signer, STUB_UKI);

        let release = build_signed_release(&signer, &req.client_nonce, &session);

        // Release validates against the server's pubkey using the
        // dedicated release digest (distinct domain from the
        // pre-commit signature).
        let vk = VerifyingKey::from_sec1_bytes(&signer.pubkey_sec1()).unwrap();
        let digest = qrab_boot_proto::release_signing_digest(
            &req.client_nonce,
            &session.response.server_nonce,
            &session.response.secret_sha256,
        );
        let sig = P256Sig::from_slice(&release.sig).expect("sig parses");
        vk.verify(&digest, &sig).expect("release sig must verify");

        // Release body hashes to the committed secret_sha256.
        assert!(
            qrab_boot_proto::verify_release_hash(&release, &session.response.secret_sha256),
            "release.secret must hash to session.secret_sha256"
        );
        // Release body equals the held-back session secret.
        assert_eq!(release.secret, STUB_SECRET);
        // Wire fields pinned.
        assert_eq!(release.proto, PROTO_TAG);
        assert_eq!(release.wire_version, WIRE_VERSION);
    }

    #[test]
    fn release_sig_rejected_on_mismatched_client_nonce() {
        use p256::ecdsa::{signature::Verifier as _, Signature as P256Sig, VerifyingKey};
        // A release signed for session A must not verify against
        // session B's client_nonce. Mirrors R12-07's binding
        // rationale but at the release-digest layer.
        let signer = fixed_signer();
        let req = new_request([0xBB; 32]);
        let session = build_signed_response(&req, &signer, STUB_UKI);
        let release = build_signed_release(&signer, &req.client_nonce, &session);

        let wrong_nonce = [0xCCu8; 32];
        let vk = VerifyingKey::from_sec1_bytes(&signer.pubkey_sec1()).unwrap();
        let digest = qrab_boot_proto::release_signing_digest(
            &wrong_nonce,
            &session.response.server_nonce,
            &session.response.secret_sha256,
        );
        let sig = P256Sig::from_slice(&release.sig).unwrap();
        assert!(
            vk.verify(&digest, &sig).is_err(),
            "release signed for nonce A must not verify under nonce B"
        );
    }

    #[test]
    fn release_sig_rejected_on_tampered_secret_body() {
        // Server issues release; attacker swaps the secret bytes
        // on the wire. Guest runs verify_release_hash and rejects.
        let signer = fixed_signer();
        let req = new_request([0xDD; 32]);
        let session = build_signed_response(&req, &signer, STUB_UKI);
        let mut release = build_signed_release(&signer, &req.client_nonce, &session);
        // Tamper the secret body.
        release.secret[0] ^= 0xFF;
        assert!(
            !qrab_boot_proto::verify_release_hash(&release, &session.response.secret_sha256),
            "tampered release body must not hash to committed digest"
        );
    }
}
