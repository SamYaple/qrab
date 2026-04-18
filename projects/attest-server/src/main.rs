//! qrab-boot attestation server — vsock binary entry point.
//!
//! All protocol logic lives in the companion [`attest_server`] lib
//! (`src/lib.rs`); this file is just the I/O wrapper. Keeping them
//! separate lets the lib's unit tests exercise the full
//! postcard → stub → postcard path without needing a host kernel
//! with `vhost_vsock` loaded.

use std::io::{Read, Write};

use anyhow::{Context, Result, bail};
use attest_server::handle_request_bytes;
use qrab_boot_proto::{FRAME_PREFIX_LEN, MAX_FRAMED_PAYLOAD};
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
    let request_bytes = read_framed(&mut stream).context("reading attestation request")?;

    let reply_bytes = handle_request_bytes(&request_bytes)
        .map_err(|e| anyhow::anyhow!("handle_request_bytes: {e:?}"))?;

    stream
        .write_all(&reply_bytes)
        .context("writing attestation response")?;
    eprintln!("attest-server: replied {} B", reply_bytes.len());
    drop(stream);
    Ok(())
}

/// Read one length-prefixed frame off the stream, returning the
/// full `prefix || body` buffer so the caller can feed it directly
/// into `qrab_boot_proto::decode_framed`.
fn read_framed(stream: &mut VsockStream) -> Result<Vec<u8>> {
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
    Ok(full)
}
