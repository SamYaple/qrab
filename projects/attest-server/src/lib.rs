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
use rand_core::OsRng;
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

    /// Sign the canonical digest for `response`, returning a 64-byte
    /// raw-IEEE-P1363 (r || s) signature.
    pub fn sign_response(&self, response: &AttestResponse) -> [u8; SIG_LEN] {
        let digest = response_signing_digest(response);
        let sig: Signature = self.key.sign(&digest);
        let bytes = sig.to_bytes();
        let mut out = [0u8; SIG_LEN];
        out.copy_from_slice(&bytes);
        out
    }
}

/// Decode a framed [`AttestRequest`], produce the canonical stub
/// [`AttestResponse`] signed by `signer` (with `uki_sha256` pointing
/// at `sha256(uki_bytes)`), and return the framed reply bytes.
pub fn handle_request_bytes(
    bytes: &[u8],
    signer: &Signer,
    uki_bytes: &[u8],
) -> Result<Vec<u8>, HandleError> {
    let request: AttestRequest = decode_framed(bytes).map_err(HandleError::Decode)?;
    validate_request(&request).map_err(HandleError::Validate)?;
    let response = build_signed_response(&request, signer, uki_bytes);
    encode_framed(&response).map_err(HandleError::Encode)
}

/// Build the canonical stub [`AttestResponse`] for `req`, signed with
/// `signer`. The server inverts each byte of the client's nonce so
/// the guest can distinguish a real reply from a loopback echo
/// (`server_nonce[0] = 0xaa` when the client sent `0x55`). The
/// response's `uki_sha256` field commits to the bytes the caller
/// will transmit as the second framed payload.
pub fn build_signed_response(
    req: &AttestRequest,
    signer: &Signer,
    uki_bytes: &[u8],
) -> AttestResponse {
    let mut response = build_body(req, uki_bytes);
    let sig = signer.sign_response(&response);
    response.sig = sig.to_vec();
    response
}

/// Construct the unsigned response body for `req`. The `sig` field is
/// filled with zero bytes of the correct length so
/// [`qrab_boot_proto::response_signing_digest`] produces a stable
/// value before signing.
fn build_body(req: &AttestRequest, uki_bytes: &[u8]) -> AttestResponse {
    let mut server_nonce = [0u8; 32];
    for (dst, src) in server_nonce.iter_mut().zip(req.client_nonce.iter()) {
        *dst = !*src;
    }
    let secret = b"QRAB_STUB_SECRET_v0".to_vec();
    let secret_sha256: [u8; 32] = sha2::Sha256::digest(&secret).into();
    let uki_sha256: [u8; 32] = sha2::Sha256::digest(uki_bytes).into();
    AttestResponse {
        server_nonce,
        uki_url: String::from("vsock://2:1337/stub-uki"),
        uki_sha256,
        secret,
        secret_sha256,
        sig: vec![0u8; SIG_LEN],
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
        verify_secret_hash,
    };
    use sha2::Digest as _;

    fn fixed_signer() -> Signer {
        // Deterministic key for tests. P-256 scalar cannot be zero and
        // must be < n; a small constant well below n is fine.
        let scalar = [1u8; 32];
        let key = SigningKey::from_bytes((&scalar).into())
            .expect("non-zero small scalar is a valid P-256 private key");
        Signer { key }
    }

    #[test]
    fn build_signed_response_inverts_nonce() {
        let req = new_request([0x55; 32]);
        let resp = build_signed_response(&req, &fixed_signer(), STUB_UKI);
        assert!(resp.server_nonce.iter().all(|&b| b == 0xAA));
        assert_eq!(resp.sig.len(), SIG_LEN);
        qrab_boot_proto::validate_response(&resp).expect("stub response must validate");
        assert!(verify_secret_hash(&resp));
    }

    #[test]
    fn sig_verifies_against_matching_pubkey() {
        let signer = fixed_signer();
        let req = new_request([0x77; 32]);
        let resp = build_signed_response(&req, &signer, STUB_UKI);

        let sec1 = signer.pubkey_sec1();
        let vk = VerifyingKey::from_sec1_bytes(&sec1).expect("valid SEC1 pubkey");
        let digest = qrab_boot_proto::response_signing_digest(&resp);
        let sig = P256Sig::from_slice(&resp.sig).expect("sig parses");
        vk.verify(&digest, &sig).expect("valid sig must verify");
    }

    #[test]
    fn sig_rejects_tampered_response() {
        let signer = fixed_signer();
        let req = new_request([0x88; 32]);
        let mut resp = build_signed_response(&req, &signer, STUB_UKI);
        // Flip a bit in a signed field.
        resp.uki_sha256[0] ^= 1;

        let vk = VerifyingKey::from_sec1_bytes(&signer.pubkey_sec1()).unwrap();
        let digest = qrab_boot_proto::response_signing_digest(&resp);
        let sig = P256Sig::from_slice(&resp.sig).unwrap();
        assert!(vk.verify(&digest, &sig).is_err());
    }

    #[test]
    fn handle_roundtrip_happy_path() {
        let signer = fixed_signer();
        let req = new_request([0x77; 32]);
        let wire = qrab_boot_proto::encode_framed(&req).unwrap();
        let reply = handle_request_bytes(&wire, &signer, STUB_UKI).expect("handle must succeed");

        let response: AttestResponse =
            qrab_boot_proto::decode_framed(&reply).expect("reply must decode");
        assert!(response.server_nonce.iter().all(|&b| b == !0x77));
        assert_eq!(response.secret, b"QRAB_STUB_SECRET_v0");
        // Sanity: hash commits to the transported bytes.
        assert_eq!(
            response.secret_sha256,
            <[u8; 32]>::from(sha2::Sha256::digest(&response.secret))
        );

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
}
