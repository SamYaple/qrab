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
//! 1. `QRAB_ATTEST_KEY_PATH` env — path to a PKCS#8 PEM file.
//! 2. Else: ephemeral key is generated and its SEC1 pubkey is logged
//!    so the bootloader can be rebuilt against it for dev flows.
//!
//! R12-11: the previous `QRAB_ATTEST_KEY_PEM` mode (PKCS#8 bytes
//! passed inline through the environment) was removed — env vars
//! are world-readable via `/proc/PID/environ` on Linux and inherit
//! into any child process the server spawns, so a sibling-process
//! exfil could read the signing key. The path form keeps the bytes
//! on disk where standard POSIX permissions (`chmod 0600`) apply.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
    // R12-11: refuse the legacy inline-PEM env var with a loud
    // error. Previous builds accepted it silently; we bail so an
    // upgrade path lands correctly (operator re-provisions to
    // `QRAB_ATTEST_KEY_PATH`). The env is probed but never read
    // past the name so the actual PEM bytes cannot enter the
    // process image via this code.
    if std::env::var_os("QRAB_ATTEST_KEY_PEM").is_some() {
        bail!(
            "QRAB_ATTEST_KEY_PEM is no longer supported (R12-11 — \
             env vars are readable via /proc/PID/environ and inherited \
             by child processes). Write the PKCS#8 PEM to a 0600 file \
             and set QRAB_ATTEST_KEY_PATH instead."
        );
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
        "attest-server: no QRAB_ATTEST_KEY_PATH set; using ephemeral key"
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
    /// Accept-and-audit: cache the first signed `AttestResponse`
    /// wire frame and replay identical bytes (same `server_nonce`,
    /// same signature, same `uki_sha256`) on every subsequent
    /// accept. The bootloader does **not** reject a replay by
    /// design — the response's ECDSA-P256 signature still verifies
    /// against the embedded pubkey and `report_data` still binds
    /// both nonces, so the wire-level authenticity check passes.
    /// Freshness is a *policy* concern: a post-boot auditor
    /// (matrix harness, BMC, external verifier) compares
    /// `server_nonce` across boots and flags reuse. See matrix
    /// scenario 10 in `contrib/matrix-smoke.sh` for the
    /// double-boot audit that turns this fault into a detectable
    /// signal.
    Replay,
}

fn parse_fault() -> Result<Fault> {
    match std::env::var("QRAB_ATTEST_FAULT").as_deref() {
        Ok("bad_sig") => Ok(Fault::BadSig),
        Ok("wrong_hash") => Ok(Fault::WrongHash),
        Ok("replay") => Ok(Fault::Replay),
        Ok("none") | Err(_) => Ok(Fault::None),
        Ok(other) => bail!(
            "QRAB_ATTEST_FAULT={other} — expected one of none|bad_sig|wrong_hash|replay"
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

    // Announce whether a golden measurement is configured. Operators
    // forgetting `QRAB_ATTEST_VERIFIER_MEASUREMENT_HEX` otherwise
    // silently get a weaker green — chain walks pass on any
    // measurement value. Emitting this at startup makes the policy
    // gap visible in one scroll.
    match std::env::var(attest_server::verify::ENV_EXPECTED_MEASUREMENT_HEX) {
        Ok(_) => eprintln!(
            "attest-server: QRAB_ATTEST_MEASUREMENT_CHECK_ENABLED env={}",
            attest_server::verify::ENV_EXPECTED_MEASUREMENT_HEX,
        ),
        Err(_) => eprintln!(
            "attest-server: QRAB_ATTEST_MEASUREMENT_CHECK_SKIPPED env={}_unset — \
             chain walks will succeed on any measurement value",
            attest_server::verify::ENV_EXPECTED_MEASUREMENT_HEX,
        ),
    }

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

    // Fault::Replay cache: Option<Vec<u8>> keyed on "have we handled
    // one request yet". First request runs real handle_request_bytes
    // and stores its reply_bytes; every subsequent request skips the
    // handler and writes the cached bytes verbatim. `Arc<Mutex<..>>`
    // survives the design-level move to a threaded handler without
    // reshaping the signature again.
    //
    // `QRAB_ATTEST_REPLAY_CACHE_PATH` optionally persists the cache
    // to disk, so a matrix harness that spawns one attest-server
    // process per boot still sees identical bytes on boot N>1. Scope
    // narrow: only honoured when fault==Replay. Any error reading
    // a pre-existing file is logged then ignored (treat as
    // cold-start cache).
    let replay_cache_path = std::env::var("QRAB_ATTEST_REPLAY_CACHE_PATH").ok();
    let replay_cache: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    if fault == Fault::Replay {
        if let Some(ref path) = replay_cache_path {
            match std::fs::read(path) {
                Ok(bytes) if !bytes.is_empty() => {
                    eprintln!(
                        "attest-server: QRAB_ATTEST_REPLAY cache_loaded path={path} bytes_len={}",
                        bytes.len()
                    );
                    *replay_cache.lock().unwrap() = Some(bytes);
                }
                Ok(_) => {
                    eprintln!(
                        "attest-server: QRAB_ATTEST_REPLAY cache_file_empty path={path} — will prime on first request"
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    eprintln!(
                        "attest-server: QRAB_ATTEST_REPLAY cache_file_missing path={path} — will prime on first request"
                    );
                }
                Err(e) => {
                    eprintln!(
                        "attest-server: QRAB_ATTEST_REPLAY cache_load_error path={path} err={e}"
                    );
                }
            }
        } else {
            eprintln!(
                "attest-server: QRAB_ATTEST_REPLAY in-memory only (set QRAB_ATTEST_REPLAY_CACHE_PATH to persist across processes)"
            );
        }
    }

    loop {
        let (stream, peer) = match listener.accept() {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("attest-server: accept failed: {e}");
                continue;
            }
        };
        // R12-21 partial: cap how long a single slow client can
        // wedge the single-thread accept loop. A full fix is
        // thread-per-conn + a real timeout budget, but setting
        // SO_RCVTIMEO / SO_SNDTIMEO on the accepted stream already
        // bounds the pathological case ("client TCPs then never
        // sends the prefix") to `SESSION_IO_TIMEOUT` rather than
        // forever. The timeout is conservative enough that a slow
        // ARM test target on a busy box still fits in it.
        if let Err(e) = stream.set_read_timeout(Some(SESSION_IO_TIMEOUT)) {
            eprintln!(
                "attest-server: set_read_timeout failed ({e}) — \
                 dropping session cid={} port={}",
                peer.cid(), peer.port(),
            );
            continue;
        }
        if let Err(e) = stream.set_write_timeout(Some(SESSION_IO_TIMEOUT)) {
            eprintln!(
                "attest-server: set_write_timeout failed ({e}) — \
                 dropping session cid={} port={}",
                peer.cid(), peer.port(),
            );
            continue;
        }
        eprintln!(
            "attest-server: accepted cid={} port={} rcv_timeout={}s wr_timeout={}s",
            peer.cid(),
            peer.port(),
            SESSION_IO_TIMEOUT.as_secs(),
            SESSION_IO_TIMEOUT.as_secs(),
        );
        let hash_source: &[u8] = match fault {
            Fault::WrongHash => &decoy_for_hash,
            _ => &uki_bytes,
        };
        if let Err(e) = handle(
            stream,
            &signer,
            hash_source,
            &uki_frame,
            fault,
            &replay_cache,
            replay_cache_path.as_deref(),
        ) {
            eprintln!("attest-server: session error: {e:#}");
        }
    }
}

/// Budget for a single session (per-op read / write). Chosen to
/// comfortably cover real SNP boot — the slowest observed boot-time
/// vsock exchange on an EPYC 9354 dev host is ~2 s end-to-end — but
/// small enough that a slow / stuck client cannot wedge the single-
/// thread accept loop indefinitely. R12-21 partial; the full fix
/// is thread-per-conn.
const SESSION_IO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

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
///
/// Under `Fault::Replay` the first accept populates `replay_cache`
/// with the freshly-signed reply bytes; every subsequent accept
/// skips `handle_request_bytes` and writes the cached frame
/// verbatim so `server_nonce` / signature / `uki_sha256` all repeat
/// byte-for-byte. The UKI frame is deterministic from `uki_bytes`,
/// so writing the same `uki_frame` on both boots is automatic. The
/// incoming request bytes are still consumed to keep the stream
/// ordered for the (best-effort) post-report frame read that
/// follows.
fn handle(
    mut stream: VsockStream,
    signer: &Signer,
    uki_bytes: &[u8],
    uki_frame: &[u8],
    fault: Fault,
    replay_cache: &Mutex<Option<Vec<u8>>>,
    replay_cache_path: Option<&str>,
) -> Result<()> {
    // The first frame (AttestRequest) is required — a peer that
    // closes before sending it is a protocol error, not a skip.
    let request_bytes = read_framed(&mut stream)
        .context("reading attestation request")?
        .ok_or_else(|| anyhow::anyhow!("peer closed before sending AttestRequest"))?;

    let reply_bytes = if fault == Fault::Replay {
        // Cached? Serve verbatim. Else run real handler, store, serve.
        let mut guard = replay_cache
            .lock()
            .map_err(|_| anyhow::anyhow!("replay cache mutex poisoned"))?;
        match guard.as_ref() {
            Some(cached) => {
                // `cache_age` logs every served replay so the smoke
                // harness can grep `QRAB_ATTEST_REPLAY cache_age=`
                // counts. Counter is process-local — under the
                // disk-backed path each fresh attest-server process
                // starts at 1 after loading from disk.
                eprintln!(
                    "attest-server: QRAB_ATTEST_REPLAY cache_age={} bytes_len={}",
                    1,
                    cached.len(),
                );
                cached.clone()
            }
            None => {
                let fresh = handle_request_bytes(&request_bytes, signer, uki_bytes)
                    .map_err(|e| anyhow::anyhow!("handle_request_bytes: {e:?}"))?;
                eprintln!(
                    "attest-server: QRAB_ATTEST_REPLAY cache_primed bytes_len={}",
                    fresh.len()
                );
                // Persist to disk so the next attest-server process
                // (next smoke invocation / next boot) serves identical
                // bytes. Write-error is logged but not fatal — in-memory
                // cache still serves correct replays inside this process.
                if let Some(path) = replay_cache_path {
                    match std::fs::write(path, &fresh) {
                        Ok(()) => eprintln!(
                            "attest-server: QRAB_ATTEST_REPLAY cache_persisted path={path} bytes_len={}",
                            fresh.len()
                        ),
                        Err(e) => eprintln!(
                            "attest-server: QRAB_ATTEST_REPLAY cache_persist_error path={path} err={e}"
                        ),
                    }
                }
                *guard = Some(fresh.clone());
                fresh
            }
        }
    } else {
        handle_request_bytes(&request_bytes, signer, uki_bytes)
            .map_err(|e| anyhow::anyhow!("handle_request_bytes: {e:?}"))?
    };

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
        Ok(Some(post_bytes)) => match handle_post_report_bytes(&post_bytes, None, None) {
            Ok(outcome) => {
                let v = &outcome.verified;
                // Distinct token when the chain walk succeeded but
                // NO golden measurement was compared — downstream
                // dashboards / smoke treat this as amber, not green.
                let token = if outcome.measurement_enforced {
                    "QRAB_SNP_POST_VERIFY_OK"
                } else {
                    "QRAB_SNP_POST_VERIFY_OK_NO_MEASUREMENT"
                };
                eprintln!(
                    "attest-server: {token} chain=ark->ask->vcek \
                     signer={:?} measurement_prefix={:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                    v.signer,
                    v.report.measurement[0], v.report.measurement[1],
                    v.report.measurement[2], v.report.measurement[3],
                    v.report.measurement[4], v.report.measurement[5],
                    v.report.measurement[6], v.report.measurement[7],
                );
            }
            Err(e) => eprintln!(
                "attest-server: QRAB_SNP_POST_VERIFY_FAIL reason={} detail={}",
                reason(&e), e
            ),
        },
        Ok(None) => {
            // Clean EOF: a v1-shaped client or a TCG guest that
            // skipped the SNP branch. Not a server-side error.
            eprintln!("attest-server: QRAB_SNP_POST_VERIFY_SKIP no_frame");
        }
        Err(e) => {
            // Mid-frame truncation or underlying IO failure.
            // Distinct token from the clean-EOF skip so a fuzzer or
            // a network regression cannot hide behind `no_frame`
            // (B12-13).
            eprintln!(
                "attest-server: QRAB_SNP_POST_VERIFY_IO_ERR detail={e}"
            );
        }
    }

    drop(stream);
    Ok(())
}

/// Read one length-prefixed frame off the stream, returning the
/// full `prefix || body` buffer so the caller can feed it directly
/// into `qrab_boot_proto::decode_framed`.
/// Read a length-prefixed frame off `stream`.
///
/// Returns:
/// * `Ok(Some(bytes))` — full `prefix || body` frame.
/// * `Ok(None)` — client cleanly closed the stream before sending
///   any bytes, i.e. a v1-shaped client or a TCG guest that skipped
///   the SNP post-report path. Treated as "no frame", not an error.
/// * `Err(_)` — partial frame (prefix read, body short) or any
///   other IO failure. Discriminated at the call site so the log
///   token distinguishes a peer protocol mismatch from a network /
///   read error that hides a real regression. B12-13.
fn read_framed(stream: &mut VsockStream) -> Result<Option<Vec<u8>>> {
    use std::io::{ErrorKind, Read as _};
    let mut prefix = [0u8; FRAME_PREFIX_LEN];
    let mut read = 0;
    while read < FRAME_PREFIX_LEN {
        let n = match stream.read(&mut prefix[read..]) {
            Ok(0) => {
                // Connection closed. If we got zero bytes up to this
                // point, it's a clean EOF — signal via `Ok(None)`.
                // If we got 1..=3 bytes and then EOF, that's a
                // partial-frame error because the peer promised a
                // length-prefix and truncated mid-way.
                if read == 0 {
                    return Ok(None);
                }
                bail!("read_framed: EOF after {read}/{FRAME_PREFIX_LEN} prefix bytes");
            }
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(anyhow::Error::new(e).context("reading frame prefix")),
        };
        read += n;
    }
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
    Ok(Some(full))
}
