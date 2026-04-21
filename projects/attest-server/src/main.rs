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
    // R16-H22: cache stores both the pre-commit frame AND the
    // held-back secret body so both halves round-trip identically
    // across attest-server process restarts. Persisted on-disk
    // layout: `u32 LE precommit_len || precommit_bytes || secret_bytes`.
    let replay_cache: Arc<Mutex<Option<(Vec<u8>, Vec<u8>)>>> = Arc::new(Mutex::new(None));
    if fault == Fault::Replay {
        if let Some(ref path) = replay_cache_path {
            match std::fs::read(path) {
                Ok(bytes) if bytes.len() >= 4 => {
                    let prec_len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
                    if bytes.len() >= 4 + prec_len {
                        let precommit = bytes[4..4 + prec_len].to_vec();
                        let secret = bytes[4 + prec_len..].to_vec();
                        eprintln!(
                            "attest-server: QRAB_ATTEST_REPLAY cache_loaded path={path} precommit_len={} secret_len={}",
                            precommit.len(), secret.len(),
                        );
                        *replay_cache.lock().unwrap() = Some((precommit, secret));
                    } else {
                        eprintln!(
                            "attest-server: QRAB_ATTEST_REPLAY cache_file_truncated path={path} — will prime on first request"
                        );
                    }
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
    stream: VsockStream,
    signer: &Signer,
    uki_bytes: &[u8],
    uki_frame: &[u8],
    fault: Fault,
    replay_cache: &Mutex<Option<(Vec<u8>, Vec<u8>)>>,
    replay_cache_path: Option<&str>,
) -> Result<()> {
    handle_stream(
        stream, signer, uki_bytes, uki_frame, fault,
        replay_cache, replay_cache_path,
    )
}

/// Stream-generic version of [`handle`]. Split out so integration
/// tests can drive the attestation protocol over `UnixStream::pair`
/// without bringing up a real vsock. The protocol flow (R16-H22:
/// v3, secret gated behind post-report verify) is:
///
/// 1. Read `AttestRequest`.
/// 2. Write `AttestResponse` precommit (no secret body).
/// 3. Stream the UKI body.
/// 4. Read `AttestPostReport`.
/// 5. Run the chain walk.
/// 6. **Only on chain-walk OK**, write `AttestRelease` with the
///    secret body. On any failure path — chain walk fails, peer
///    closes early, IO error — the secret is never written.
///
/// Protocol invariant enforced by test
/// `server_does_not_leak_secret_before_post_report`.
fn handle_stream<S: Read + Write>(
    mut stream: S,
    signer: &Signer,
    uki_bytes: &[u8],
    uki_frame: &[u8],
    fault: Fault,
    replay_cache: &Mutex<Option<(Vec<u8>, Vec<u8>)>>,
    replay_cache_path: Option<&str>,
) -> Result<()> {
    // --- Frame 1 in: AttestRequest ---
    // The first frame (AttestRequest) is required — a peer that
    // closes before sending it is a protocol error, not a skip.
    let request_bytes = read_framed(&mut stream)
        .context("reading attestation request")?
        .ok_or_else(|| anyhow::anyhow!("peer closed before sending AttestRequest"))?;

    // Build a v3 session: the pre-commit frame bytes + the held-back
    // secret. Under `Fault::Replay`, both halves are remembered so
    // re-boots see identical pre-commit bytes AND identical secret
    // material. Persisted to disk as `(precommit_len:u32 LE ||
    // precommit_bytes || secret_bytes)` so a fresh attest-server
    // process after a crash still replays correctly.
    let (client_nonce, session, precommit_bytes): ([u8; 32], attest_server::ServerSession, Vec<u8>) =
        if fault == Fault::Replay {
            let mut guard = replay_cache
                .lock()
                .map_err(|_| anyhow::anyhow!("replay cache mutex poisoned"))?;
            match guard.as_ref() {
                Some((cached_precommit, cached_secret)) => {
                    eprintln!(
                        "attest-server: QRAB_ATTEST_REPLAY cache_age={} precommit_len={} secret_len={}",
                        1, cached_precommit.len(), cached_secret.len(),
                    );
                    // Decode the request just to recover the
                    // client_nonce so the release we eventually
                    // sign still binds THIS session.
                    let request: qrab_boot_proto::AttestRequest =
                        qrab_boot_proto::decode_framed(&request_bytes)
                            .map_err(|e| anyhow::anyhow!("decode replay request: {e:?}"))?;
                    let pre: qrab_boot_proto::AttestResponse =
                        qrab_boot_proto::decode_framed(cached_precommit)
                            .map_err(|e| anyhow::anyhow!("decode cached precommit: {e:?}"))?;
                    let sess = attest_server::ServerSession {
                        response: pre,
                        secret: cached_secret.clone(),
                    };
                    (request.client_nonce, sess, cached_precommit.clone())
                }
                None => {
                    let (nonce, sess, frame) = handle_request_bytes(
                        &request_bytes, signer, uki_bytes,
                    )
                    .map_err(|e| anyhow::anyhow!("handle_request_bytes: {e:?}"))?;
                    eprintln!(
                        "attest-server: QRAB_ATTEST_REPLAY cache_primed precommit_len={} secret_len={}",
                        frame.len(), sess.secret.len(),
                    );
                    if let Some(path) = replay_cache_path {
                        let mut blob = Vec::with_capacity(4 + frame.len() + sess.secret.len());
                        blob.extend_from_slice(&(frame.len() as u32).to_le_bytes());
                        blob.extend_from_slice(&frame);
                        blob.extend_from_slice(&sess.secret);
                        match std::fs::write(path, &blob) {
                            Ok(()) => eprintln!(
                                "attest-server: QRAB_ATTEST_REPLAY cache_persisted path={path} bytes_len={}",
                                blob.len()
                            ),
                            Err(e) => eprintln!(
                                "attest-server: QRAB_ATTEST_REPLAY cache_persist_error path={path} err={e}"
                            ),
                        }
                    }
                    *guard = Some((frame.clone(), sess.secret.clone()));
                    (nonce, sess, frame)
                }
            }
        } else {
            handle_request_bytes(&request_bytes, signer, uki_bytes)
                .map_err(|e| anyhow::anyhow!("handle_request_bytes: {e:?}"))?
        };

    // --- Frame 2 out: AttestResponse pre-commit (NO secret body) ---
    stream
        .write_all(&precommit_bytes)
        .context("writing attestation pre-commit")?;

    // --- Frame 3 out: UKI body ---
    // Safe to ship pre-verify — the UKI is public material, signed
    // separately. Only the secret needs gating.
    stream
        .write_all(uki_frame)
        .context("writing UKI body")?;

    eprintln!(
        "attest-server: wrote pre-commit {} B + UKI {} B (secret held back pending post-report verify)",
        precommit_bytes.len(),
        uki_frame.len()
    );

    // --- Frame 4 in: AttestPostReport ---
    // Guest computes report_data from the pre-commit fields, asks
    // the PSP for an attestation report, and sends it here. The
    // server now decides whether to release the secret.
    let post_bytes = match read_framed(&mut stream) {
        Ok(Some(b)) => b,
        Ok(None) => {
            // Clean EOF before post-report: peer either dropped or
            // was a stale v2 client. Either way, the secret stays
            // on the server. This is the R16-H22 close.
            eprintln!(
                "attest-server: QRAB_SNP_POST_VERIFY_SKIP no_frame \
                 (secret NOT released — R16-H22 gate)"
            );
            drop(stream);
            return Ok(());
        }
        Err(e) => {
            eprintln!(
                "attest-server: QRAB_SNP_POST_VERIFY_IO_ERR detail={e} \
                 (secret NOT released — R16-H22 gate)"
            );
            drop(stream);
            return Err(e.context("reading attestation post-report"));
        }
    };

    // --- Chain walk ---
    match handle_post_report_bytes(&post_bytes, None, None) {
        Ok(outcome) => {
            let v = &outcome.verified;
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

            // --- Frame 5 out: AttestRelease (secret body) ---
            // Only reached on verify OK. Bind the release to this
            // session's client_nonce + pre-committed secret_sha256
            // via a domain-separated ECDSA-P256 signature.
            let release = attest_server::build_signed_release(
                signer, &client_nonce, &session,
            );
            let release_frame = qrab_boot_proto::encode_framed(&release)
                .context("encoding AttestRelease")?;
            stream
                .write_all(&release_frame)
                .context("writing AttestRelease")?;
            eprintln!(
                "attest-server: QRAB_SNP_SECRET_RELEASED bytes={} (post-verify)",
                release_frame.len(),
            );
        }
        Err(e) => {
            eprintln!(
                "attest-server: QRAB_SNP_POST_VERIFY_FAIL reason={} detail={} \
                 (secret NOT released — R16-H22 gate)",
                reason(&e), e
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
fn read_framed<S: Read>(stream: &mut S) -> Result<Option<Vec<u8>>> {
    use std::io::ErrorKind;
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

#[cfg(test)]
mod r16_h22_tests {
    //! R16-H22 regression harness.
    //!
    //! `server_does_not_leak_secret_before_post_report` drives the
    //! server over a `UnixStream::pair` and proves that no bytes of
    //! the server-provisioned secret flow to the peer before the
    //! server has read + verified `AttestPostReport`. Before the v3
    //! protocol landed this test failed — the server wrote the
    //! whole `AttestResponse` (secret body and all) up front, and
    //! any malicious guest could grab the secret and drop the
    //! connection. After the fix, the secret lives only inside an
    //! `AttestRelease` frame that the server emits exclusively on
    //! chain-walk OK.
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    /// A distinctive, rare-in-random-data byte sequence we embed in
    /// the server's secret so the exploit test can spot any leakage
    /// by window-scanning what the peer sees.
    const SECRET_CANARY: &[u8] = b"QRAB_R16_H22_SECRET_CANARY_DO_NOT_LEAK_0123456789";

    fn run_server_with_stream(stream: UnixStream) -> thread::JoinHandle<()> {
        // Build a server-side signer + uki_frame + replay cache and
        // spawn handle_stream on a thread. Uses a dev-generated
        // signer so the bootloader's embedded pubkey does not need
        // to match — the exploit test only cares about byte flow,
        // not sig-verify outcomes on the peer.
        let signer = Signer::generate();
        let uki = STUB_UKI.to_vec();
        let uki_frame: Vec<u8> = build_uki_frame(&uki);
        let cache: Arc<Mutex<Option<(Vec<u8>, Vec<u8>)>>> = Arc::new(Mutex::new(None));
        thread::spawn(move || {
            // Ignore the Result — the test's job is to assert what
            // bytes flow, not the function's return value.
            let _ = handle_stream(
                stream, &signer, &uki, &uki_frame, Fault::None,
                &cache, None,
            );
        })
    }

    /// Drop-in swap of the default STUB_UKI secret injection: build
    /// a signer whose `build_signed_response` / `build_body` paths
    /// will install our canary as the secret. Requires that the
    /// server calls `build_body` under the hood — today
    /// `handle_request_bytes` hard-codes `STUB_SECRET` rather than
    /// accepting an explicit secret. We mirror the existing API by
    /// substituting `STUB_SECRET` for the duration of the test via
    /// a `once_cell` / `Mutex` would be heavy; instead we rely on
    /// the protocol-level invariant directly: the wire-format v3
    /// `AttestResponse` carries NO secret body regardless of what
    /// the server is configured to hand out. The canary is only
    /// needed if we want to assert on a specific byte sequence;
    /// use `STUB_SECRET` bytes as the canary instead.
    fn secret_bytes_in_use() -> &'static [u8] {
        attest_server::STUB_SECRET
    }

    #[test]
    fn server_does_not_leak_secret_before_post_report() {
        // Set up a UnixStream pair; the "server" end is handed to
        // handle_stream, the "client" end drives the exploit.
        let (mut client, server) = UnixStream::pair().expect("unix pair");
        client
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("set read timeout");
        let join = run_server_with_stream(server);

        // Send a valid AttestRequest.
        let req = qrab_boot_proto::new_request([0x42u8; 32]);
        let wire = qrab_boot_proto::encode_framed(&req).expect("encode");
        client.write_all(&wire).expect("write request");
        client.flush().ok();

        // Pull everything the server writes BEFORE we send the
        // post-report. Reads block on the timeout; after the server
        // finishes writing its pre-verify frames it waits for our
        // post-report, which we never send. The timeout fires and
        // we assert on what we collected.
        let mut collected = Vec::new();
        let mut chunk = [0u8; 65536];
        for _ in 0..64 {
            match client.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => collected.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(_) => break,
            }
        }

        // Drop the client before joining so the server unblocks.
        drop(client);
        let _ = join.join();

        let secret = secret_bytes_in_use();
        let leaked = collected
            .windows(secret.len())
            .any(|w| w == secret);
        assert!(
            !leaked,
            "R16-H22 REGRESSION: secret bytes appeared on the \
             wire ({} B read) before the peer sent any \
             AttestPostReport frame. This is the protocol flaw the \
             round-16 audit flagged; the v3 split (AttestPreCommit \
             + AttestRelease) must NOT write any secret body until \
             after the server verifies the post-report.",
            collected.len(),
        );
    }

    #[test]
    fn server_does_not_leak_secret_on_malformed_post_report() {
        // Send a valid AttestRequest, read the pre-commit + UKI,
        // then send a garbage post-report frame. The server's chain
        // walk must fail; AttestRelease must NOT be sent; the
        // secret canary must not appear anywhere on the wire.
        let (mut client, server) = UnixStream::pair().unwrap();
        client.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
        let join = run_server_with_stream(server);

        let req = qrab_boot_proto::new_request([0xABu8; 32]);
        let wire = qrab_boot_proto::encode_framed(&req).unwrap();
        client.write_all(&wire).expect("write request");

        // Read until the server finishes its pre-commit + UKI write.
        let mut pre_post = Vec::new();
        let mut chunk = [0u8; 65536];
        for _ in 0..32 {
            match client.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    pre_post.extend_from_slice(&chunk[..n]);
                    // Heuristic: server writes ~200 B precommit + ~516 B UKI.
                    // Once we've seen >= 700 B, move on to the post-report
                    // step rather than blocking forever on the timeout.
                    if pre_post.len() >= 700 {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(_) => break,
            }
        }

        // Send a deliberately-malformed "post-report" frame: valid
        // length prefix but junk body. The server will try to
        // decode it, fail, and close the stream without emitting
        // AttestRelease.
        let junk = vec![0xFFu8; 64];
        let len = (junk.len() as u32).to_le_bytes();
        client.write_all(&len).ok();
        client.write_all(&junk).ok();

        // Drain any post-chain-walk bytes (should be empty).
        let mut post_verify = Vec::new();
        for _ in 0..32 {
            match client.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => post_verify.extend_from_slice(&chunk[..n]),
                Err(_) => break,
            }
        }

        drop(client);
        let _ = join.join();

        let all: Vec<u8> = pre_post.into_iter().chain(post_verify).collect();
        let secret = secret_bytes_in_use();
        assert!(
            !all.windows(secret.len()).any(|w| w == secret),
            "R16-H22 REGRESSION: server wrote the secret after a \
             malformed post-report (read {} B total). Chain-walk \
             failure must NEVER emit AttestRelease.",
            all.len(),
        );
    }
}
