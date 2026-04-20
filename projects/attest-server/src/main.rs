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
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use attest_server::verify::{handle_post_report_bytes, reason};
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

/// Fault-injection variants the matrix harness can dial in via
/// `QRAB_ATTEST_FAULT`. Real deployments always set `Fault::None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    /// Default: sign correctly, advertise the true UKI hash.
    None,
    /// Sign with a fresh ephemeral key instead of the loaded
    /// `$QRAB_ATTEST_KEY_PATH` signer. The bootloader's embedded
    /// SEC1 pubkey no longer matches, so `p256::verify` rejects.
    /// Matrix scenario 5 (`bad_sig`).
    BadSig,
    /// Advertise a `uki_sha256` computed over a fake payload, but
    /// stream the real UKI bytes on frame 2. Bootloader's
    /// `sha256(uki_bytes) == response.uki_sha256` check fails.
    /// Matrix scenario 4 (`wrong_hash`).
    WrongHash,
}

fn parse_fault() -> Result<Fault> {
    match std::env::var("QRAB_ATTEST_FAULT").as_deref() {
        Ok("bad_sig") => Ok(Fault::BadSig),
        Ok("wrong_hash") => Ok(Fault::WrongHash),
        Ok("none") | Err(_) => Ok(Fault::None),
        Ok(other) => bail!(
            "QRAB_ATTEST_FAULT={other} — expected one of none|bad_sig|wrong_hash"
        ),
    }
}

fn main() -> Result<()> {
    let mode = parse_mode()?;
    let mut signer = load_signer()?;
    let uki_bytes = load_uki_bytes()?;
    let fault = parse_fault()?;

    if fault == Fault::BadSig {
        // Swap the loaded signer for a throwaway key. The real
        // pubkey is still broadcast in the logs so a human reading
        // the output can see the mismatch; the bootloader's
        // embedded-key verify fails because the signing key no
        // longer matches. Matrix scenario 5.
        let fresh = Signer::generate();
        eprintln!(
            "attest-server: QRAB_ATTEST_FAULT=bad_sig — \
             substituting ephemeral signer (pubkey {})",
            hex(&fresh.pubkey_sec1())
        );
        signer = fresh;
    }

    match mode {
        Mode::Serve { port } => serve(port, signer, uki_bytes, fault),
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

fn serve(port: u32, signer: Signer, uki_bytes: Vec<u8>, fault: Fault) -> Result<()> {
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
    eprintln!(
        "attest-server: UKI body {} B ready to stream (fault={fault:?})",
        uki_bytes.len()
    );

    // Pre-build the second-frame wire image once at startup. Real
    // UKIs run ~50 MB; rebuilding `len_prefix || body` on every
    // accept would burn a per-request memcpy of the full payload.
    // Wrapping in `Arc<[u8]>` keeps the hot path to an atomic refcount
    // bump even if this handler is later moved behind a worker pool.
    let uki_frame: Arc<[u8]> = build_uki_frame(&uki_bytes).into();
    // Raw UKI bytes are only needed for the SHA-256 that gets signed
    // into frame 1. Hoist into `Arc<[u8]>` for symmetry with
    // `uki_frame`, so a future threaded handler clones a refcount
    // rather than the body.
    let uki_bytes: Arc<[u8]> = uki_bytes.into();

    // For `Fault::WrongHash` we sign a hash over this decoy payload
    // while still streaming the real UKI on frame 2. Any bytes work
    // — the bootloader compares SHA-256s, and `sha256(decoy) !=
    // sha256(uki_bytes)` is all that matters.
    let decoy_for_hash: Arc<[u8]> = b"QRAB_FAULT_WRONG_HASH_v1".to_vec().into();

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
        let hash_source: &[u8] = match fault {
            Fault::WrongHash => &decoy_for_hash,
            _ => &uki_bytes,
        };
        if let Err(e) = handle(stream, &signer, hash_source, &uki_frame) {
            eprintln!("attest-server: session error: {e:#}");
        }
    }
}

/// Produce the length-prefixed wire frame (`u32 LE len || body`)
/// that the bootloader's second `FramedStream::read_frame` call
/// consumes. Built once at startup so the accept loop never
/// reallocates the payload.
fn build_uki_frame(uki_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_PREFIX_LEN + uki_bytes.len());
    out.extend_from_slice(&(uki_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(uki_bytes);
    out
}

/// One request/reply cycle on the given stream: we read the framed
/// `AttestRequest`, write the signed `AttestResponse` frame, then
/// write the pre-built UKI frame. `uki_bytes` feeds the SHA-256 that
/// is signed into frame 1; `uki_frame` is the already-framed wire
/// image transmitted as frame 2.
fn handle(
    mut stream: VsockStream,
    signer: &Signer,
    uki_bytes: &[u8],
    uki_frame: &[u8],
) -> Result<()> {
    let request_bytes = read_framed(&mut stream).context("reading attestation request")?;

    let reply_bytes = handle_request_bytes(&request_bytes, signer, uki_bytes)
        .map_err(|e| anyhow::anyhow!("handle_request_bytes: {e:?}"))?;

    stream
        .write_all(&reply_bytes)
        .context("writing attestation response")?;

    // Second frame: UKI body. The bootloader's `FramedStream` reads
    // frames back-to-back on the same connection and verifies the
    // hash against the signed `uki_sha256` from frame 1.
    stream
        .write_all(uki_frame)
        .context("writing UKI body")?;

    eprintln!(
        "attest-server: replied {} B (response) + {} B (UKI)",
        reply_bytes.len(),
        uki_frame.len()
    );

    // F2 — wire v2: guest sends AttestPostReport after consuming
    // the UKI frame. Chain-walk via qrab_attest_verifier::verify_chain.
    // Best-effort: a v1 guest or a host without `certs-path=` set
    // produces a clean no-op path.
    match read_framed(&mut stream) {
        Ok(post_bytes) => match handle_post_report_bytes(&post_bytes, None, None) {
            Ok(v) => eprintln!(
                "attest-server: QRAB_SNP_POST_VERIFY_OK chain=ark->ask->vcek \
                 signer={:?} measurement_prefix={:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                v.signer,
                v.report.measurement[0], v.report.measurement[1],
                v.report.measurement[2], v.report.measurement[3],
                v.report.measurement[4], v.report.measurement[5],
                v.report.measurement[6], v.report.measurement[7],
            ),
            Err(e) => eprintln!(
                "attest-server: QRAB_SNP_POST_VERIFY_FAIL reason={} detail={}",
                reason(&e), e
            ),
        },
        Err(_) => {
            // No post-report frame arrived — v1-shaped client, or
            // the guest skipped the SNP branch on TCG. Not an
            // error from the server's perspective.
            eprintln!("attest-server: QRAB_SNP_POST_VERIFY_SKIP no_frame");
        }
    }

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
