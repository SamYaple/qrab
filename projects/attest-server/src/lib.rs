//! Pure attestation logic for `attest-server`.
//!
//! The binary target (`main.rs`) owns the vsock listener; this crate
//! owns the protocol-level handler so it can be unit-tested without
//! the host kernel module. `handle_request_bytes` takes an incoming
//! framed [`AttestRequest`] and returns the framed [`AttestResponse`]
//! bytes to write back — no I/O.

use qrab_boot_proto::{
    AttestRequest, AttestResponse, ProtoError, SIG_LEN, decode_framed, encode_framed,
    validate_request,
};

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

/// Decode a framed [`AttestRequest`], produce the canonical stub
/// [`AttestResponse`] for it, and return the framed reply bytes.
pub fn handle_request_bytes(bytes: &[u8]) -> Result<Vec<u8>, HandleError> {
    let request: AttestRequest = decode_framed(bytes).map_err(HandleError::Decode)?;
    validate_request(&request).map_err(HandleError::Validate)?;
    let response = stub_response(&request);
    encode_framed(&response).map_err(HandleError::Encode)
}

/// Deterministic stub response. The server inverts each byte of the
/// client's nonce so the guest's log can distinguish a real reply
/// from a loopback echo (`server_nonce[0] = 0xaa` when the client
/// sent `0x55`).
pub fn stub_response(req: &AttestRequest) -> AttestResponse {
    let mut server_nonce = [0u8; 32];
    for (dst, src) in server_nonce.iter_mut().zip(req.client_nonce.iter()) {
        *dst = !*src;
    }
    AttestResponse {
        server_nonce,
        uki_url: String::from("vsock://2:1337/stub-uki"),
        uki_sha256: [0xAA; 32],
        secret: b"QRAB_STUB_SECRET_v0".to_vec(),
        secret_sha256: [0xBB; 32],
        sig: vec![0x11; SIG_LEN],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qrab_boot_proto::{
        AttestResponse, PROTO_TAG, WIRE_VERSION, new_request, split_framed,
    };

    #[test]
    fn stub_response_inverts_nonce_and_passes_validation() {
        let req = new_request([0x55; 32]);
        let resp = stub_response(&req);
        assert!(resp.server_nonce.iter().all(|&b| b == 0xAA));
        assert_eq!(resp.sig.len(), SIG_LEN);
        qrab_boot_proto::validate_response(&resp).expect("stub response must validate");
    }

    #[test]
    fn handle_roundtrip_happy_path() {
        let req = new_request([0x77; 32]);
        let wire = qrab_boot_proto::encode_framed(&req).unwrap();
        let reply = handle_request_bytes(&wire).expect("handle must succeed");

        // Reply must frame-decode to our stub AttestResponse.
        let response: AttestResponse =
            qrab_boot_proto::decode_framed(&reply).expect("reply must decode");
        assert!(response.server_nonce.iter().all(|&b| b == !0x77));
        assert_eq!(response.secret, b"QRAB_STUB_SECRET_v0");

        // And the framing prefix matches the body length.
        let (len, body) = split_framed(&reply).unwrap();
        assert_eq!(len + qrab_boot_proto::FRAME_PREFIX_LEN, reply.len());
        assert_eq!(body.len(), len);
    }

    #[test]
    fn rejects_wrong_magic() {
        let mut req = new_request([0; 32]);
        req.proto = *b"XXX-v99";
        let wire = qrab_boot_proto::encode_framed(&req).unwrap();
        let err = handle_request_bytes(&wire).unwrap_err();
        match err {
            HandleError::Validate(ProtoError::WrongTag) => {}
            other => panic!("expected Validate(WrongTag), got {other:?}"),
        }
    }

    #[test]
    fn rejects_garbage_bytes() {
        let err = handle_request_bytes(&[0, 0, 0, 0]).unwrap_err();
        assert!(matches!(err, HandleError::Decode(_)));
    }

    #[test]
    fn response_matches_current_wire_version() {
        // Tripwire: bumping qrab-boot-proto's wire version without
        // updating the server must fail this test so the bump is
        // noticed in CI.
        let req = new_request([0xFF; 32]);
        assert_eq!(req.client_version, WIRE_VERSION);
        assert_eq!(req.proto, PROTO_TAG);
    }
}
