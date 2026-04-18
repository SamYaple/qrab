//! qrab-boot attestation server — vsock binary entry point.
//!
//! All protocol logic lives in the companion [`attest_server`] lib
//! (`src/lib.rs`); this file is just the I/O wrapper plus key
//! lifecycle. Keeping them separate lets the lib's unit tests
//! exercise the full postcard → stub → postcard path without needing
//! a host kernel with `vhost_vsock` loaded.
//!
//! CLI shape:
//!
//! ```text
//! attest-server [PORT]                       # listen (default 1337)
//! attest-server --print-pubkey               # emit SEC1 hex of the loaded key
//! attest-server --write-pubkey-sec1 PATH     # export SEC1 bytes to PATH
//! ```
//!
//! Key material is resolved in this order:
//!
//! 1. `QRAB_ATTEST_KEY_PEM` env — PKCS#8 PEM inline.
//! 2. `QRAB_ATTEST_KEY_PATH` env — path to a PKCS#8 PEM file.
//! 3. Else: ephemeral key is generated and its SEC1 pubkey is logged
//!    so the bootloader can be rebuilt against it for dev flows.

use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use attest_server::{STUB_UKI, Signer, handle_request_bytes};
use qrab_boot_proto::{FRAME_PREFIX_LEN, MAX_FRAMED_PAYLOAD};
use vsock::{VsockAddr, VsockListener, VsockStream, VMADDR_CID_ANY};

/// Default port the server listens on. Matches the bootloader's
/// `VSOCK_HOST_PORT` constant in `qrab-boot/src/main.rs`.
const DEFAULT_PORT: u32 = 1337;

enum Mode {
    Serve { port: u32 },
    PrintPubkey,
    WritePubkey { path: PathBuf },
}

fn parse_mode() -> Result<Mode> {
    let mut args = std::env::args().skip(1);
    let mut mode = Mode::Serve { port: DEFAULT_PORT };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--print-pubkey" => mode = Mode::PrintPubkey,
            "--write-pubkey-sec1" => {
                let path = args
                    .next()
                    .context("--write-pubkey-sec1 requires a PATH argument")?;
                mode = Mode::WritePubkey {
                    path: PathBuf::from(path),
                };
            }
            other if other.chars().all(|c| c.is_ascii_digit()) => {
                let port: u32 = other
                    .parse()
                    .with_context(|| format!("invalid port argument: {other}"))?;
                mode = Mode::Serve { port };
            }
            other => bail!("unknown argument: {other}"),
        }
    }
    Ok(mode)
}

fn load_signer() -> Result<Signer> {
    if let Ok(pem) = std::env::var("QRAB_ATTEST_KEY_PEM") {
        return Signer::from_pem(&pem)
            .context("parsing QRAB_ATTEST_KEY_PEM as PKCS#8 PEM");
    }
    if let Ok(path) = std::env::var("QRAB_ATTEST_KEY_PATH") {
        let pem = std::fs::read_to_string(&path)
            .with_context(|| format!("reading QRAB_ATTEST_KEY_PATH={path}"))?;
        return Signer::from_pem(&pem)
            .with_context(|| format!("parsing {path} as PKCS#8 PEM"));
    }
    let signer = Signer::generate();
    let sec1 = signer.pubkey_sec1();
    eprintln!(
        "attest-server: no QRAB_ATTEST_KEY_PEM / QRAB_ATTEST_KEY_PATH set; \
         using ephemeral key"
    );
    eprintln!("attest-server: ephemeral pubkey (SEC1): {}", hex(&sec1));
    Ok(signer)
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        write!(&mut s, "{b:02x}").unwrap();
    }
    s
}

fn main() -> Result<()> {
    let mode = parse_mode()?;
    let signer = load_signer()?;
    let uki_bytes = load_uki_bytes()?;

    match mode {
        Mode::Serve { port } => serve(port, signer, uki_bytes),
        Mode::PrintPubkey => {
            println!("{}", hex(&signer.pubkey_sec1()));
            Ok(())
        }
        Mode::WritePubkey { path } => {
            std::fs::write(&path, signer.pubkey_sec1())
                .with_context(|| format!("writing pubkey to {}", path.display()))?;
            eprintln!("attest-server: wrote SEC1 pubkey to {}", path.display());
            Ok(())
        }
    }
}

/// Load the UKI body attest-server will stream as the second framed
/// payload. `$QRAB_ATTEST_UKI_PATH` overrides; otherwise we fall back
/// to the built-in 512-byte stub.
fn load_uki_bytes() -> Result<Vec<u8>> {
    if let Ok(path) = std::env::var("QRAB_ATTEST_UKI_PATH") {
        let body = std::fs::read(&path)
            .with_context(|| format!("reading QRAB_ATTEST_UKI_PATH={path}"))?;
        if body.len() > MAX_FRAMED_PAYLOAD {
            bail!(
                "UKI at {path} is {} bytes — exceeds MAX_FRAMED_PAYLOAD {MAX_FRAMED_PAYLOAD}",
                body.len()
            );
        }
        Ok(body)
    } else {
        Ok(STUB_UKI.to_vec())
    }
}

fn serve(port: u32, signer: Signer, uki_bytes: Vec<u8>) -> Result<()> {
    let addr = VsockAddr::new(VMADDR_CID_ANY, port);
    let listener =
        VsockListener::bind(&addr).with_context(|| format!("binding to vsock port {port}"))?;
    eprintln!(
        "attest-server: listening on vsock cid=ANY port={port} \
         (max framed payload {MAX_FRAMED_PAYLOAD} B)"
    );
    eprintln!(
        "attest-server: signing pubkey (SEC1): {}",
        hex(&signer.pubkey_sec1())
    );
    eprintln!("attest-server: UKI body {} B ready to stream", uki_bytes.len());

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
        if let Err(e) = handle(stream, &signer, &uki_bytes) {
            eprintln!("attest-server: session error: {e:#}");
        }
    }
}

/// One request/reply cycle on the given stream: we read the framed
/// `AttestRequest`, write the signed `AttestResponse` frame, then
/// write a second framed payload containing the UKI body.
fn handle(mut stream: VsockStream, signer: &Signer, uki_bytes: &[u8]) -> Result<()> {
    let request_bytes = read_framed(&mut stream).context("reading attestation request")?;

    let reply_bytes = handle_request_bytes(&request_bytes, signer, uki_bytes)
        .map_err(|e| anyhow::anyhow!("handle_request_bytes: {e:?}"))?;

    stream
        .write_all(&reply_bytes)
        .context("writing attestation response")?;

    // Second frame: UKI body. The bootloader's `FramedStream` reads
    // frames back-to-back on the same connection and verifies the
    // hash against the signed `uki_sha256` from frame 1.
    let mut uki_frame = Vec::with_capacity(FRAME_PREFIX_LEN + uki_bytes.len());
    uki_frame.extend_from_slice(&(uki_bytes.len() as u32).to_le_bytes());
    uki_frame.extend_from_slice(uki_bytes);
    stream
        .write_all(&uki_frame)
        .context("writing UKI body")?;

    eprintln!(
        "attest-server: replied {} B (response) + {} B (UKI)",
        reply_bytes.len(),
        uki_frame.len()
    );
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
