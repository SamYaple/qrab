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
use std::time::Instant;

use anyhow::{Context, Result, bail};
use attest_server::verify::{handle_post_report_bytes, reason};
use attest_server::{STUB_UKI, Signer, handle_request_bytes};
use qrab_boot_proto::{FRAME_PREFIX_LEN, MAX_FRAMED_PAYLOAD, WIRE_VERSION};
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

/// R15-5: wall-clock ceiling for an ENTIRE session, not just one
/// syscall. A drip-feed peer that writes one byte every
/// `SESSION_IO_TIMEOUT - epsilon` seconds resets the per-read clock
/// on each call and can otherwise sit on an accept slot indefinitely
/// (and slowly exhaust `MAX_FRAMED_PAYLOAD`). We check elapsed time
/// after every read; once past the deadline the session is dropped
/// with the `QRAB_SNP_SESSION_DEADLINE` log token. The deadline is a
/// superset of `SESSION_IO_TIMEOUT` — the per-syscall timer still
/// unblocks a truly-stuck syscall, the deadline catches the
/// drip-feed case the per-syscall timer misses.
const SESSION_TOTAL_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

/// Produce the length-prefixed wire frame (`u32 LE len || u16 LE
/// WIRE_VERSION || body`) that the bootloader's second
/// `FramedStream::read_frame` call consumes. Built once at startup so
/// the accept loop never reallocates the payload.
///
/// B14-5: the UKI frame carries the same 6-byte outer header as every
/// other framed payload even though the body is opaque public UKI
/// bytes. Uniform framing keeps the bootloader's `FramedStream`
/// reader version-checking every frame — a version-skewed peer fails
/// on the pre-commit frame, so if execution reaches the UKI send path
/// the version field is redundant but still emitted for consistency.
fn build_uki_frame(uki_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_PREFIX_LEN + uki_bytes.len());
    out.extend_from_slice(&(uki_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&WIRE_VERSION.to_le_bytes());
    out.extend_from_slice(uki_bytes);
    out
}

/// Persist the `Fault::Replay` cache blob to `path` with 0o600
/// permissions, atomically. R15-9: a plain `std::fs::write` creates
/// the file under the process's umask (typically 0022 → 0644 on disk)
/// which makes the persisted `(precommit, secret)` pair
/// world-readable. A sibling user (or any local attacker who can
/// read the cache path) would get the held-back secret body on the
/// subsequent boot.
///
/// Strategy:
///
/// 1. Write the blob to `<path>.tmp` via `OpenOptions` with the mode
///    bit set at create time — the file is never observable at 0644,
///    unlike the `write()` + `set_permissions()` pattern which has a
///    TOCTOU window where another process can open the still-0644
///    file.
/// 2. `sync_all` the tmp file so its bytes land on disk before we
///    swing the pointer.
/// 3. `rename` onto the final path. `rename(2)` is atomic on the
///    same filesystem, so the load path at startup sees either the
///    old valid blob, the new valid blob, or `ENOENT` — never a
///    truncated mid-write blob.
///
/// If the process crashes between step 1 and step 3, a stray
/// `<path>.tmp` is left behind. That is harmless — the load path
/// only reads `path`, and the next successful persist overwrites
/// the tmp file in place before renaming. A best-effort cleanup
/// helps tidy operator logs but is not load-bearing.
#[cfg(unix)]
fn persist_replay_cache_0600(path: &str, blob: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt;

    let tmp_path = format!("{path}.tmp");
    {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&tmp_path)?;
        f.write_all(blob)?;
        f.sync_all()?;
    }
    // Atomic on same FS — the load path never sees a partial blob.
    std::fs::rename(&tmp_path, path)
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
    stream: S,
    signer: &Signer,
    uki_bytes: &[u8],
    uki_frame: &[u8],
    fault: Fault,
    replay_cache: &Mutex<Option<(Vec<u8>, Vec<u8>)>>,
    replay_cache_path: Option<&str>,
) -> Result<()> {
    // R15-5: session-wide wall-clock ceiling computed at entry. Both
    // reads and writes check against this so a drip-feed peer that
    // keeps each syscall under `SESSION_IO_TIMEOUT` still gets
    // evicted once cumulative elapsed exceeds the deadline.
    let deadline = Instant::now()
        .checked_add(SESSION_TOTAL_DEADLINE)
        .expect("SESSION_TOTAL_DEADLINE within Instant range");
    handle_stream_with_deadline(
        stream, signer, uki_bytes, uki_frame, fault,
        replay_cache, replay_cache_path, deadline,
    )
}

/// Deadline-parameterised variant so the R15-5 regression test can
/// dial in a short session ceiling without waiting 60s wall-clock.
/// Production callers go through [`handle_stream`], which pins the
/// deadline to [`SESSION_TOTAL_DEADLINE`].
fn handle_stream_with_deadline<S: Read + Write>(
    mut stream: S,
    signer: &Signer,
    uki_bytes: &[u8],
    uki_frame: &[u8],
    fault: Fault,
    replay_cache: &Mutex<Option<(Vec<u8>, Vec<u8>)>>,
    replay_cache_path: Option<&str>,
    deadline: Instant,
) -> Result<()> {
    // --- Frame 1 in: AttestRequest ---
    // The first frame (AttestRequest) is required — a peer that
    // closes before sending it is a protocol error, not a skip.
    let request_bytes = read_framed(&mut stream, deadline)
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
                        match persist_replay_cache_0600(path, &blob) {
                            Ok(()) => eprintln!(
                                "attest-server: QRAB_ATTEST_REPLAY cache_persisted path={path} bytes_len={} mode=0600",
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
    // R15-5: check session deadline after the write in case a slow
    // receiver dripped the send queue for the full budget.
    if Instant::now() >= deadline {
        eprintln!(
            "attest-server: QRAB_SNP_SESSION_DEADLINE after_precommit_write \
             (secret NOT released — R15-5 gate)"
        );
        drop(stream);
        return Ok(());
    }

    // --- Frame 3 out: UKI body ---
    // Safe to ship pre-verify — the UKI is public material, signed
    // separately. Only the secret needs gating.
    stream
        .write_all(uki_frame)
        .context("writing UKI body")?;
    if Instant::now() >= deadline {
        eprintln!(
            "attest-server: QRAB_SNP_SESSION_DEADLINE after_uki_write \
             (secret NOT released — R15-5 gate)"
        );
        drop(stream);
        return Ok(());
    }

    eprintln!(
        "attest-server: wrote pre-commit {} B + UKI {} B (secret held back pending post-report verify)",
        precommit_bytes.len(),
        uki_frame.len()
    );

    // --- Frame 4 in: AttestPostReport ---
    // Guest computes report_data from the pre-commit fields, asks
    // the PSP for an attestation report, and sends it here. The
    // server now decides whether to release the secret.
    let post_bytes = match read_framed(&mut stream, deadline) {
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

            // R15-5: final deadline check before emitting the
            // secret. If a malicious peer dragged the session past
            // the total ceiling — e.g. the chain walk ran while the
            // deadline was already expiring — drop rather than
            // release. The chain walk is CPU-bound so this is a
            // belt-and-braces gate, not the main defence.
            if Instant::now() >= deadline {
                eprintln!(
                    "attest-server: QRAB_SNP_SESSION_DEADLINE pre_release \
                     (secret NOT released — R15-5 gate)"
                );
                drop(stream);
                return Ok(());
            }
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
/// * `Err(_)` — partial frame (prefix read, body short), session
///   deadline exceeded (R15-5), or any other IO failure.
///   Discriminated at the call site so the log token distinguishes a
///   peer protocol mismatch from a network / read error that hides a
///   real regression. B12-13.
///
/// `deadline` is the session-wide wall-clock ceiling (R15-5). We
/// check it after every `read()` return so a drip-feed peer cannot
/// slowly exhaust `MAX_FRAMED_PAYLOAD` by staying under the
/// per-syscall timeout on each individual call.
fn read_framed<S: Read>(stream: &mut S, deadline: Instant) -> Result<Option<Vec<u8>>> {
    use std::io::ErrorKind;
    // B14-5: the outer framing header is 6 bytes — `u32 LE body_len ||
    // u16 LE wire_version`. We parse both fields and reject a version
    // mismatch with a distinct log token so an operator can tell a
    // version-skewed peer apart from a tampered / MITM'd payload
    // (which used to surface as an opaque `BadSignature` downstream).
    let mut prefix = [0u8; FRAME_PREFIX_LEN];
    let mut read = 0;
    while read < FRAME_PREFIX_LEN {
        let n = match stream.read(&mut prefix[read..]) {
            Ok(0) => {
                // Connection closed. If we got zero bytes up to this
                // point, it's a clean EOF — signal via `Ok(None)`.
                // If we got 1..=5 bytes and then EOF, that's a
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
        if Instant::now() >= deadline {
            bail!(
                "QRAB_SNP_SESSION_DEADLINE prefix_progress={read}/{FRAME_PREFIX_LEN}"
            );
        }
    }
    let len_bytes: [u8; 4] = prefix[..4].try_into().expect("slice length");
    let ver_bytes: [u8; 2] = prefix[4..FRAME_PREFIX_LEN]
        .try_into()
        .expect("slice length");
    let len = u32::from_le_bytes(len_bytes) as usize;
    let peer_version = u16::from_le_bytes(ver_bytes);
    if len > MAX_FRAMED_PAYLOAD {
        bail!("declared frame length {len} exceeds MAX_FRAMED_PAYLOAD");
    }
    if peer_version != WIRE_VERSION {
        // B14-5: distinct log token — operator can filter on this
        // without false-positive against MITM / BadSignature rows.
        bail!(
            "QRAB_SNP_WIRE_VERSION_MISMATCH expected={WIRE_VERSION} got={peer_version} \
             (peer compiled against a different qrab-boot-proto revision)"
        );
    }

    // Body read loop is manual (not `read_exact`) so the session
    // deadline applies per-syscall. `read_exact` would happily spin
    // through hundreds of 1-byte reads as long as each one returned
    // within `SESSION_IO_TIMEOUT` — the exact drip-feed shape R15-5
    // closes.
    let mut body = vec![0u8; len];
    let mut filled = 0;
    while filled < len {
        let n = match stream.read(&mut body[filled..]) {
            Ok(0) => bail!(
                "read_framed: EOF after {filled}/{len} body bytes"
            ),
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(anyhow::Error::new(e).context("reading frame body")),
        };
        filled += n;
        if Instant::now() >= deadline {
            bail!(
                "QRAB_SNP_SESSION_DEADLINE body_progress={filled}/{len}"
            );
        }
    }

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

#[cfg(test)]
mod r15_5_tests {
    //! R15-5 regression harness — slowloris drip-feed.
    //!
    //! Before this fix, `SESSION_IO_TIMEOUT` capped each individual
    //! syscall at 30 s but placed no ceiling on total session
    //! wall-clock. A peer that sent one byte every few seconds could
    //! keep each `read()` syscall well inside the timeout and hold an
    //! accept slot for arbitrarily long — enough to exhaust
    //! `MAX_FRAMED_PAYLOAD` (16 MiB) at a trickle rate. The R15 audit
    //! called this out as MED DOS.
    //!
    //! `slowloris_drip_feed_gets_evicted_before_secret_release`
    //! drives the server over a `UnixStream::pair` with a dialed-down
    //! deadline, sends the AttestRequest one byte at a time with
    //! inter-byte sleeps that cumulatively exceed the deadline, and
    //! asserts:
    //!
    //! 1. The server closes the session (peer-side read returns EOF).
    //! 2. No secret bytes ever reach the wire.
    //!
    //! The deadline is dialed down via `handle_stream_with_deadline`
    //! to keep the test under a second; production still uses the
    //! 60 s `SESSION_TOTAL_DEADLINE`.
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    fn secret_bytes() -> &'static [u8] {
        attest_server::STUB_SECRET
    }

    /// Span the server-side `handle_stream_with_deadline` call and
    /// return `(handle, start)` so the test can assert on total
    /// wall-clock. The deadline is dialed down so the test completes
    /// in under a second.
    fn run_server_timed(
        stream: UnixStream,
        deadline_ms: u64,
    ) -> (thread::JoinHandle<Instant>, Instant) {
        let start = Instant::now();
        let signer = Signer::generate();
        let uki = STUB_UKI.to_vec();
        let uki_frame: Vec<u8> = build_uki_frame(&uki);
        let cache: Arc<Mutex<Option<(Vec<u8>, Vec<u8>)>>> = Arc::new(Mutex::new(None));
        let deadline = start + Duration::from_millis(deadline_ms);
        let join = thread::spawn(move || {
            let _ = handle_stream_with_deadline(
                stream, &signer, &uki, &uki_frame, Fault::None,
                &cache, None, deadline,
            );
            // Return the Instant the handler exited so the test can
            // measure elapsed and assert on deadline-obedience.
            Instant::now()
        });
        (join, start)
    }

    /// Poll-join the server thread with a hard wall-clock cap. Used
    /// by the drip-feed tests so a regression where
    /// `SESSION_TOTAL_DEADLINE` is not enforced does not hang the
    /// harness forever — we just detect that the thread is still
    /// running past `cap` and fail the test.
    fn join_before(join: thread::JoinHandle<Instant>, cap: Duration) -> Option<Instant> {
        let start = Instant::now();
        while start.elapsed() < cap {
            if join.is_finished() {
                return Some(join.join().expect("server thread panic"));
            }
            thread::sleep(Duration::from_millis(20));
        }
        None
    }

    /// Drip-feed writer: sends `bytes` one at a time with `interval`
    /// inter-byte sleep until either the stream is ready for it,
    /// OR the shared `stop` flag is set. Runs on a thread so the
    /// test can continue to observe the server's exit timing while
    /// the client side is still feeding. Abort via `stop` so the
    /// test does not have to wait for the full wire to drip before
    /// reaping the thread — important when a regression makes the
    /// server hang instead of evicting.
    fn spawn_drip_writer(
        mut client: UnixStream,
        bytes: Vec<u8>,
        interval: Duration,
        stop: Arc<std::sync::atomic::AtomicBool>,
    ) -> thread::JoinHandle<UnixStream> {
        thread::spawn(move || {
            for b in bytes.iter() {
                if stop.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                if client.write_all(std::slice::from_ref(b)).is_err() {
                    break;
                }
                let _ = client.flush();
                thread::sleep(interval);
            }
            client
        })
    }

    #[test]
    fn slowloris_drip_feed_gets_evicted_before_secret_release() {
        // Dial the session deadline down to 300 ms. Client drips
        // each byte of a valid AttestRequest one at a time with
        // 80 ms inter-byte sleeps. The per-syscall
        // `SESSION_IO_TIMEOUT` (30 s) never fires because every
        // server-side `read()` returns within ~80 ms of being
        // posted; only the cumulative session deadline gets us out
        // before the full request is delivered (~3 s of feed).
        //
        // The critical assertion: the server thread exits within
        // ~deadline + slop. Before the fix, the server would sit
        // in the prefix/body read loop until the client closed —
        // the client here keeps dripping through the whole test,
        // so a missing deadline never bails.
        let (client, server) = UnixStream::pair().expect("unix pair");
        let (join, start) = run_server_timed(server, 300);

        // 80 ms per byte. AttestRequest wire image is ~40 bytes so
        // the drip runs ~3.2 s wall-clock — well over 10x the
        // 300 ms deadline.
        let req = qrab_boot_proto::new_request([0xA5u8; 32]);
        let wire = qrab_boot_proto::encode_framed(&req).expect("encode");
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let drip = spawn_drip_writer(
            client, wire, Duration::from_millis(80), Arc::clone(&stop),
        );

        // Wait for server to exit. Cap 1500 ms = 5x deadline —
        // tight enough that a missing deadline (server still
        // reading bytes at 80 ms/byte) trips the cap, generous
        // enough to survive CI scheduler jitter.
        let exit_at = join_before(join, Duration::from_millis(1500));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let client = drip.join().expect("drip thread panic");

        // Drain any bytes the server wrote (normally empty — the
        // request never finished decoding, so `handle_stream`
        // bails out of `read_framed` before the write phase). Use
        // a short read timeout so a regression that left the
        // server alive (no eviction, no precommit emitted) does
        // not hang this drain — we just see 0 bytes.
        client
            .set_read_timeout(Some(Duration::from_millis(200)))
            .expect("set read timeout");
        let mut collected = Vec::new();
        let mut chunk = [0u8; 4096];
        for _ in 0..16 {
            let mut c = &client;
            match c.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => collected.extend_from_slice(&chunk[..n]),
                Err(_) => break,
            }
        }
        drop(client);

        let exit_at = exit_at.expect(
            "R15-5 REGRESSION: server thread still running 1500 ms \
             into a drip-fed session with 300 ms deadline — \
             `SESSION_TOTAL_DEADLINE` is not evicting the slowloris \
             peer. Per-syscall `SESSION_IO_TIMEOUT` alone does not \
             close this hole.",
        );
        let elapsed = exit_at.duration_since(start);
        assert!(
            elapsed < Duration::from_millis(1200),
            "R15-5 REGRESSION: server ran {:?} on a slowloris \
             session with 300 ms deadline.",
            elapsed,
        );

        let secret = secret_bytes();
        let leaked = collected.windows(secret.len()).any(|w| w == secret);
        assert!(
            !leaked,
            "R15-5 REGRESSION: secret bytes on the wire during \
             slowloris (read {} B total).",
            collected.len(),
        );
    }

    #[test]
    fn drip_feed_body_phase_also_gets_evicted() {
        // Variant: send a well-formed prefix declaring a large
        // body, then drip body bytes at 80 ms intervals. Before
        // the fix, the body read used `std::io::Read::read_exact`,
        // which spins through drip-feed forever as long as each
        // individual read stays inside the per-syscall timeout.
        let (client, server) = UnixStream::pair().expect("unix pair");
        let (join, start) = run_server_timed(server, 300);

        // Declare a 4096-byte body with a MATCHING wire-version slot
        // (B14-5 — prefix is now 6 bytes) so the prefix read succeeds
        // and we actually exercise the body-read loop, not a fast
        // bail on version mismatch. Then drip enough body bytes to
        // keep the socket non-empty past the deadline.
        let declared_len: u32 = 4096;
        let mut wire = Vec::with_capacity(qrab_boot_proto::FRAME_PREFIX_LEN + 60);
        wire.extend_from_slice(&declared_len.to_le_bytes());
        wire.extend_from_slice(&qrab_boot_proto::WIRE_VERSION.to_le_bytes());
        wire.extend(std::iter::repeat_n(0u8, 60));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let drip = spawn_drip_writer(
            client, wire, Duration::from_millis(80), Arc::clone(&stop),
        );

        let exit_at = join_before(join, Duration::from_millis(1500));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let client = drip.join().expect("drip thread panic");

        client
            .set_read_timeout(Some(Duration::from_millis(200)))
            .expect("set read timeout");
        let mut collected = Vec::new();
        let mut chunk = [0u8; 4096];
        for _ in 0..16 {
            let mut c = &client;
            match c.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => collected.extend_from_slice(&chunk[..n]),
                Err(_) => break,
            }
        }
        drop(client);

        let exit_at = exit_at.expect(
            "R15-5 REGRESSION (body phase): server thread still \
             running 1500 ms into a drip-fed session — the \
             frame-body read loop is not honouring \
             `SESSION_TOTAL_DEADLINE`. A `read_exact`-shaped body \
             loop cannot enforce a session-wide deadline.",
        );
        let elapsed = exit_at.duration_since(start);
        assert!(
            elapsed < Duration::from_millis(1200),
            "R15-5 REGRESSION (body phase): server ran {:?} on a \
             drip-fed frame body with 300 ms deadline.",
            elapsed,
        );

        let secret = secret_bytes();
        let leaked = collected.windows(secret.len()).any(|w| w == secret);
        assert!(
            !leaked,
            "R15-5 REGRESSION (body phase): secret bytes on the \
             wire during drip-feed of frame body (read {} B).",
            collected.len(),
        );
    }
}

#[cfg(test)]
#[cfg(unix)]
mod r15_9_tests {
    //! R15-9 regression harness.
    //!
    //! The `Fault::Replay` cache persists `(precommit, secret)` to
    //! `QRAB_ATTEST_REPLAY_CACHE_PATH` so a multi-boot matrix run
    //! replays identical wire bytes across attest-server process
    //! restarts. Before the fix, the persist path used
    //! `std::fs::write`, which creates the file under the process's
    //! umask — typically 0o644 on disk, world-readable. A sibling
    //! user on the host could read the file and recover the secret
    //! body that the v3 protocol otherwise holds back until chain
    //! walk passes. The fix opens the file with `mode(0o600)` at
    //! create time (no TOCTOU window) and renames into place so the
    //! load path never sees a truncated blob.
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn persist_writes_0600_mode() {
        let tmp = std::env::temp_dir().join(format!(
            "qrab_r15_9_mode_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let path = tmp.to_str().expect("utf8 tmp path").to_string();
        let blob = b"QRAB_R15_9_FAKE_CACHE_BLOB_0123456789".to_vec();

        persist_replay_cache_0600(&path, &blob).expect("persist ok");

        let meta = std::fs::metadata(&path).expect("stat cache file");
        let mode = meta.permissions().mode() & 0o777;
        // Clean up before asserting so a failed run still tidies
        // $TMPDIR.
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{path}.tmp"));

        assert_eq!(
            mode, 0o600,
            "R15-9 REGRESSION: replay cache at {path} written with \
             mode 0o{mode:o} — expected 0o600. Default umask would \
             leave the persisted (precommit, secret) pair readable \
             by any local user on the host."
        );
    }

    #[test]
    fn persist_overwrite_also_0600() {
        // If the persist path is called more than once (e.g. the
        // first boot loses its network and a later accept reprimes
        // the cache), the file must still end up 0o600 — not
        // whatever mode the pre-existing file happened to carry.
        let tmp = std::env::temp_dir().join(format!(
            "qrab_r15_9_overwrite_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let path = tmp.to_str().expect("utf8 tmp path").to_string();

        // Seed a world-readable file at `path` so we can prove the
        // persist helper replaces it with a 0o600 file.
        std::fs::write(&path, b"stale").expect("seed file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .expect("set stale mode");

        persist_replay_cache_0600(&path, b"fresh blob")
            .expect("persist overwrites");

        let meta = std::fs::metadata(&path).expect("stat cache file");
        let mode = meta.permissions().mode() & 0o777;
        let bytes = std::fs::read(&path).expect("read cache file");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(format!("{path}.tmp"));

        assert_eq!(
            mode, 0o600,
            "R15-9 REGRESSION (overwrite): cache file landed at 0o{mode:o} \
             after overwriting a stale 0o644 file — must be 0o600."
        );
        assert_eq!(
            bytes, b"fresh blob",
            "overwrite produced stale contents",
        );
    }

    #[test]
    fn tmp_file_not_left_at_final_path_on_success() {
        // With the write-then-rename scheme, `<path>` must hold the
        // blob and `<path>.tmp` must not exist after a successful
        // persist (the rename consumes it).
        let tmp = std::env::temp_dir().join(format!(
            "qrab_r15_9_notmp_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let path = tmp.to_str().expect("utf8 tmp path").to_string();

        persist_replay_cache_0600(&path, b"abc").expect("persist ok");

        let tmp_path = format!("{path}.tmp");
        let tmp_exists = std::fs::metadata(&tmp_path).is_ok();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&tmp_path);

        assert!(
            !tmp_exists,
            "R15-9: <path>.tmp sidecar still present after persist — \
             rename(2) should have consumed it."
        );
    }
}
