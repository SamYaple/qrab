//! qrab-boot attestation server (phase 3b.8b).
//!
//! Listens on `AF_VSOCK` at a configurable port and speaks the
//! postcard + 4-byte-length-prefix dialect from
//! [`qrab_boot_proto`]. On every accepted connection it:
//!
//! 1. Reads a framed [`AttestRequest`] (4-byte LE length + postcard).
//! 2. Validates the magic prefix via
//!    [`qrab_boot_proto::validate_request`].
//! 3. Composes a stub [`AttestResponse`] — fixed dummy values
//!    everywhere except `server_nonce`, which is derived from the
//!    client nonce by inversion so the guest can tell "real server"
//!    and "echo" apart in the log.
//! 4. Sends the response framed.
//! 5. Closes the connection.
//!
//! Signature + crypto production lives in a future commit alongside
//! a proper policy file; this server exists only to exercise the
//! wire format end-to-end under QEMU + vhost-vsock.
//!
//! Prerequisites on the host:
//!   - `vhost_vsock` and `vsock` kernel modules loaded.
//!   - `/dev/vhost-vsock` readable/writable by the invoking user.
//!
//! Exit codes:
//!   0 — clean shutdown (SIGTERM / SIGINT).
//!   1 — setup failure (bind, etc).

use std::io::{Read, Write};

use anyhow::{Context, Result, bail};
use qrab_boot_proto::{
    AttestRequest, AttestResponse, FRAME_PREFIX_LEN, MAX_FRAMED_PAYLOAD, SIG_LEN, decode_framed,
    encode_framed, split_framed, validate_request,
};
use vsock::{VsockAddr, VsockListener, VsockStream, VMADDR_CID_ANY};

/// Default port the server listens on. Matches the bootloader's
/// `VSOCK_HOST_PORT` constant in `qrab-boot/src/main.rs`.
const DEFAULT_PORT: u32 = 1337;

fn main() -> Result<()> {
    let port: u32 = match std::env::args().nth(1) {
        Some(s) => s
            .parse()
            .with_context(|| format!("invalid port argument: {s}"))?,
        None => DEFAULT_PORT,
    };

    let addr = VsockAddr::new(VMADDR_CID_ANY, port);
    let listener =
        VsockListener::bind(&addr).with_context(|| format!("binding to vsock port {port}"))?;
    eprintln!(
        "attest-server: listening on vsock cid=ANY port={port} (max framed payload {MAX_FRAMED_PAYLOAD} B)"
    );

    loop {
        let (stream, peer) = match listener.accept() {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("attest-server: accept failed: {e}");
                continue;
            }
        };
        eprintln!(
            "attest-server: accepted cid={} port={}",
            peer.cid(),
            peer.port()
        );
        if let Err(e) = handle(stream) {
            eprintln!("attest-server: session error: {e:#}");
        }
    }
}

/// One request/reply cycle on the given stream.
fn handle(mut stream: VsockStream) -> Result<()> {
    let req = read_request(&mut stream).context("reading attestation request")?;
    eprintln!(
        "attest-server: request ok (version={}, nonce={:02x}{:02x}…)",
        req.client_version, req.client_nonce[0], req.client_nonce[1],
    );

    let resp = stub_response(&req);
    let bytes = encode_framed(&resp)
        .map_err(|e| anyhow::anyhow!("encode AttestResponse: {e}"))?;
    stream
        .write_all(&bytes)
        .context("writing attestation response")?;
    eprintln!(
        "attest-server: replied {} B (secret_len={} uki_url={:?})",
        bytes.len(),
        resp.secret.len(),
        resp.uki_url
    );
    // A polite close — drops the stream which closes the socket.
    drop(stream);
    Ok(())
}

/// Read one length-prefixed [`AttestRequest`] off the stream.
fn read_request(stream: &mut VsockStream) -> Result<AttestRequest> {
    let mut prefix = [0u8; FRAME_PREFIX_LEN];
    stream
        .read_exact(&mut prefix)
        .context("reading frame prefix")?;
    let len = u32::from_le_bytes(prefix) as usize;
    if len > MAX_FRAMED_PAYLOAD {
        bail!("declared frame length {len} exceeds MAX_FRAMED_PAYLOAD");
    }

    let mut body = vec![0u8; len];
    stream
        .read_exact(&mut body)
        .context("reading frame body")?;

    let mut full = Vec::with_capacity(FRAME_PREFIX_LEN + len);
    full.extend_from_slice(&prefix);
    full.extend_from_slice(&body);

    // `decode_framed` re-validates length bounds; we use the full
    // buffer so the public API path is exercised identically to the
    // bootloader's future consumer.
    let req: AttestRequest =
        decode_framed(&full).map_err(|e| anyhow::anyhow!("decode AttestRequest: {e}"))?;

    // Sanity-check fields via `split_framed` too so we catch prefix
    // drift early. The returned body slice must match what the
    // decoder saw.
    let (_, slice) = split_framed(&full)
        .map_err(|e| anyhow::anyhow!("split_framed AttestRequest: {e}"))?;
    debug_assert_eq!(slice.len(), len);

    validate_request(&req).map_err(|e| anyhow::anyhow!("validate AttestRequest: {e}"))?;
    Ok(req)
}

/// Deterministic stub response. Real ECDSA signing, policy lookup,
/// and UKI URL synthesis land in a later commit; this exists only so
/// the bootloader has something to deserialise.
fn stub_response(req: &AttestRequest) -> AttestResponse {
    // "Inverted" client nonce so the bootloader's log can tell the
    // server reply apart from a loopback echo.
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
