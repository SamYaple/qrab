#![deny(unsafe_code)]

pub mod qga;
pub mod server;

use qapi::QapiCommand;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot, watch, Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::task::{spawn, JoinHandle};

/// Crate-level error type for qapi-protocol.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A QMP/QGA error response from the server.
    #[error(transparent)]
    Qmp(#[from] QmpError),
    /// I/O error on the Unix socket.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// JSON serialization/deserialization error.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Connection or handshake timeout.
    #[error("{0}")]
    Timeout(String),
    /// Protocol-level error (peer closed, oversized message, handshake failure).
    #[error("{0}")]
    Protocol(String),
    /// Internal channel closed unexpectedly.
    #[error("{0}")]
    Channel(String),
}

/// Crate-level Result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Maximum bytes we will buffer for a single JSON line from the peer.
///
/// `AsyncBufReadExt::read_line` has no built-in bound: a broken or
/// malicious QEMU/QGA that sends an endless line (or omits newlines
/// entirely) would otherwise grow the buffer without limit and OOM
/// the process. 1 MiB is comfortably above any real QMP/QGA response
/// — query-machines on a huge host still clocks in well under 100 KiB
/// — and matches the order of magnitude used by libvirt/proxmox for
/// their JSON monitor handling.
pub(crate) const MAX_LINE_BYTES: usize = 1_024 * 1_024;

/// Maximum number of non-OOB QMP commands the server will accept in
/// flight concurrently. Mirrors `QMP_REQ_QUEUE_LEN_MAX` in upstream
/// `monitor/qmp.c`. Sending more than this would have QEMU drop the
/// connection with a "Command queue full" error, so the client refuses
/// to dispatch beyond this depth (callers block on the semaphore).
pub const QMP_QUEUE_CAPACITY: usize = 8;

/// How long an in-flight command may sit unanswered before the reaper
/// considers it dead and drops the reply oneshot. Matches the previous
/// per-command guard from the sequential write_loop.
const COMMAND_STALL_TIMEOUT: Duration = Duration::from_secs(30);

/// How often the reaper task walks the in-flight map looking for stale
/// entries. 5s gives a small bound on extra wait beyond the
/// `COMMAND_STALL_TIMEOUT` while keeping wakeups cheap.
const REAPER_TICK: Duration = Duration::from_secs(5);

/// Result of a bounded line read.
///
/// Separating the overflow case from true I/O errors lets callers that
/// want to stay alive after a rogue oversize line (the read loop) keep
/// going, while callers that treat *any* anomaly as fatal (the
/// handshake) collapse everything into an error.
#[derive(Debug)]
pub(crate) enum LineRead {
    /// Full line, including trailing newline.
    Line(String),
    /// Clean EOF before any data.
    Eof,
    /// Peer sent `>= MAX_LINE_BYTES` without a newline. The oversize
    /// bytes have already been drained up to the next newline (or EOF)
    /// so the stream is resynced.
    TooLong,
}

/// Read a single newline-terminated line from `reader` with a hard
/// byte cap of `MAX_LINE_BYTES`.
///
/// The `take()` adapter enforces the byte cap at the I/O layer so the
/// buffer never grows past the limit even for a pathological peer. On
/// overflow we drain the remainder of the oversize line (again using
/// `take()` each iteration) so subsequent calls resume on a line
/// boundary; the overflow is reported as `LineRead::TooLong` rather
/// than an I/O error so callers can choose whether to bail or resync.
pub(crate) async fn read_line_bounded<R>(reader: &mut R) -> Result<LineRead>
where
    R: AsyncBufReadExt + Unpin,
{
    let mut line = String::new();
    let n = reader
        .take(MAX_LINE_BYTES as u64)
        .read_line(&mut line)
        .await?;
    if n == 0 {
        return Ok(LineRead::Eof);
    }
    // If we hit the cap without seeing a newline, `line` is the leading
    // `MAX_LINE_BYTES` of an oversize line. Drain the rest of it so the
    // next call starts on a clean boundary.
    if line.len() >= MAX_LINE_BYTES && !line.ends_with('\n') {
        tracing::warn!(
            len = line.len(),
            cap = MAX_LINE_BYTES,
            "line exceeded MAX_LINE_BYTES, dropping and resyncing"
        );
        let mut sink = String::new();
        loop {
            sink.clear();
            let m = reader
                .take(MAX_LINE_BYTES as u64)
                .read_line(&mut sink)
                .await?;
            if m == 0 || sink.ends_with('\n') {
                break;
            }
        }
        return Ok(LineRead::TooLong);
    }
    Ok(LineRead::Line(line))
}

/// Write a single JSON line (`payload` + trailing `\n`) as one atomic
/// `write_all`.
///
/// Bug #23: splitting the send into two `write_all`s — `payload` then
/// `b"\n"` — lets a partial failure strand the JSON on the wire with no
/// terminator, so the peer can't parse it and the next command arrives
/// appended to the stranded tail. Build the line in one buffer and
/// issue one call instead: either the full line is accepted or the
/// writer returns an error (and callers treat the stream as poisoned).
pub(crate) async fn write_line<W>(writer: &mut W, payload: &[u8]) -> std::io::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut buf = Vec::with_capacity(payload.len() + 1);
    buf.extend_from_slice(payload);
    buf.push(b'\n');
    writer.write_all(&buf).await
}

/// Write a QGA line prefixed with the `0xFF` sync sentinel as one
/// atomic `write_all`. Same rationale as [`write_line`] but for the
/// guest-sync-delimited handshake path, where the 0xFF + JSON + `\n`
/// triplet must reach the peer as a single contiguous chunk or not
/// at all.
pub(crate) async fn write_line_with_sentinel<W>(
    writer: &mut W,
    sentinel: u8,
    payload: &[u8],
) -> std::io::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut buf = Vec::with_capacity(payload.len() + 2);
    buf.push(sentinel);
    buf.extend_from_slice(payload);
    buf.push(b'\n');
    writer.write_all(&buf).await
}

/// A raw QMP event as received from QEMU.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QmpEvent {
    pub event: String,
    pub timestamp: Timestamp,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// QMP event timestamp.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Timestamp {
    pub seconds: i64,
    pub microseconds: u32,
}

/// A typed QMP/QGA error response.
///
/// When `Qmp::execute` (and friends) receive an `{"error": {...}}`
/// response from QEMU, they wrap it in `Error::Qmp(QmpError { .. })`.
/// Callers that need to branch on the error class (e.g. retry on
/// `CommandNotFound` but bail on `GenericError`) can match on
/// `Error::Qmp(ref e)` to recover the typed value.
///
/// `class` is `Some` when QEMU reported a known [`qapi::QapiErrorClass`]
/// variant and `None` when the wire string did not match any known
/// class (older/forked QEMU, or a malformed response — we still
/// preserve `desc` either way so no information is lost).
#[derive(Debug, Clone)]
pub struct QmpError {
    /// The QMP error class, if it parsed as a known variant.
    pub class: Option<qapi::QapiErrorClass>,
    /// The free-form human-readable error description.
    pub desc: String,
}

impl std::fmt::Display for QmpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.class {
            Some(class) => write!(f, "QMP error {}: {}", class, self.desc),
            None => write!(f, "QMP error: {}", self.desc),
        }
    }
}

impl std::error::Error for QmpError {}

/// Build a [`QmpError`] from the `error` sub-object of a QMP response.
///
/// Falls back to `class = None` when the wire class string is absent or
/// does not match any known [`qapi::QapiErrorClass`] variant, so
/// forward-compatible error propagation is always possible.
pub(crate) fn build_qmp_error(error: &serde_json::Value) -> QmpError {
    let class_str = error.get("class").and_then(|v| v.as_str()).unwrap_or("");
    let class = if class_str.is_empty() {
        None
    } else {
        serde_json::from_value::<qapi::QapiErrorClass>(serde_json::Value::String(
            class_str.to_string(),
        ))
        .ok()
    };
    let desc = error
        .get("desc")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown error")
        .to_string();
    QmpError { class, desc }
}

/// One in-flight QMP command awaiting its response.
///
/// `reply_tx` is the oneshot the caller is parked on. `sent_at` lets
/// the reaper task evict entries whose response never arrives (BUG 34
/// follow-up — a stuck QEMU otherwise wedges the caller forever). `oob`
/// is recorded for diagnostics and to keep the depth metric honest.
#[derive(Debug)]
struct PendingCmd {
    reply_tx: oneshot::Sender<serde_json::Value>,
    sent_at: Instant,
    /// Recorded for diagnostics and future routing decisions; not yet
    /// read on a hot path.
    #[allow(dead_code)]
    oob: bool,
}

/// What the public `execute*` methods queue onto the writer task.
///
/// Sent across the `command_tx` mpsc channel; the writer adds the `id`
/// (which the caller has already inserted into `in_flight`), serializes
/// to a single JSON line, and writes to the socket. The writer no
/// longer waits for a reply — id-based dispatch happens entirely in the
/// reader task.
struct Outbound {
    id: u64,
    json: serde_json::Value,
}

/// Async QMP client connected to a QEMU instance.
///
/// The client is concurrent: up to `QMP_QUEUE_CAPACITY` non-OOB
/// commands may be in flight at once (matching QEMU's monitor queue
/// depth), with response routing performed by id. Out-of-band commands
/// — those whose schema declares `allow-oob` and which QEMU has
/// negotiated as supported in the greeting — bypass the queue
/// semaphore entirely and are answered immediately by the monitor.
#[derive(Debug)]
pub struct Qmp {
    version: (u32, u32, u32),
    command_tx: mpsc::Sender<Outbound>,
    in_flight: Arc<Mutex<HashMap<u64, PendingCmd>>>,
    queue_permits: Arc<Semaphore>,
    queue_depth_tx: watch::Sender<usize>,
    id_counter: Arc<AtomicU64>,
    oob_available: Arc<AtomicBool>,
    /// Set to `true` while the connection is healthy, flipped to
    /// `false` by either the read_loop on EOF/I/O-error or by
    /// `shutdown()`. Used by `dispatch` to short-circuit new commands
    /// after the underlying socket has died — without this, a late
    /// `execute()` could insert into `in_flight` *after* the read_loop
    /// has already cleared it on exit, leaving the entry to be
    /// reaped 30s later instead of failing fast.
    connection_alive: Arc<AtomicBool>,
    shutdown_signal: watch::Sender<()>,
    read_handle: Option<JoinHandle<()>>,
    write_handle: Option<JoinHandle<()>>,
    event_handle: Option<JoinHandle<()>>,
    reaper_handle: Option<JoinHandle<()>>,
}

impl Qmp {
    /// Connect to a QEMU QMP socket and perform the capabilities negotiation.
    ///
    /// Returns the `Qmp` handle and a receiver for QMP events.
    /// Also sets the global QEMU version via `qapi::set_qemu_version()`.
    ///
    /// If the server's greeting advertises `"oob"` in `QMP.capabilities`,
    /// the handshake enables it via `{"execute":"qmp_capabilities",
    /// "arguments":{"enable":["oob"]}}`. Otherwise the client falls back
    /// to the bare capability handshake for backward compatibility with
    /// older QEMU/mock peers that don't speak OOB.
    #[must_use = "dropping the Qmp handle immediately tears the connection down; bind it to keep the QMP session alive"]
    pub async fn new(socket_path: &Path) -> Result<(Self, mpsc::Receiver<QmpEvent>)> {
        // Bound `connect(2)` itself. A present-but-unserved socket —
        // e.g. bound without `listen()`, stuck in socket activation, or
        // living on a hung FUSE mount — would otherwise leave the
        // caller blocked indefinitely. 5s comfortably covers a healthy
        // local connect while still surfacing a dead peer quickly.
        let stream = tokio::time::timeout(Duration::from_secs(5), UnixStream::connect(socket_path))
            .await
            .map_err(|_| Error::Timeout("QMP connect timed out after 5s".into()))??;
        let (read_half, write_half) = stream.into_split();

        let mut reader = BufReader::new(read_half);
        let mut write_half = write_half;
        let mut oob_supported = false;
        let mut version = (0, 0, 0);

        // Bound the full greeting + qmp_capabilities handshake. All reads
        // below are otherwise unbounded — if QEMU accepts the connect(2)
        // but hangs before writing the greeting (or mid-handshake) we'd
        // block `Qmp::new()` forever. 10s is plenty for a healthy peer.
        tokio::time::timeout(Duration::from_secs(10), async {
            // Read greeting synchronously before spawning workers
            let greeting_line = match read_line_bounded(&mut reader).await? {
                LineRead::Line(line) => line,
                LineRead::Eof => {
                    return Err(Error::Protocol(
                        "QMP peer closed before sending greeting".into(),
                    ));
                }
                LineRead::TooLong => {
                    return Err(Error::Protocol(format!(
                        "QMP greeting exceeded {} bytes",
                        MAX_LINE_BYTES
                    )));
                }
            };
            let parsed: serde_json::Value = serde_json::from_str(&greeting_line)
                .map_err(|e| Error::Protocol(format!("failed to parse QMP greeting: {}", e)))?;
            let qemu = parsed
                .get("QMP")
                .and_then(|q| q.get("version"))
                .and_then(|v| v.get("qemu"))
                .ok_or_else(|| {
                    Error::Protocol(format!("QMP greeting missing QMP.version.qemu: {}", parsed))
                })?;
            let component = |name: &str| -> Result<u32> {
                qemu[name]
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .ok_or_else(|| {
                        Error::Protocol(format!("Invalid QMP version component: {name}"))
                    })
            };
            let major = component("major")?;
            let minor = component("minor")?;
            let micro = component("micro")?;
            version = (major, minor, micro);
            // Log but don't fail if version was already set (multi-VM process).
            let _ = qapi::set_qemu_version(major, minor, micro);

            // Detect OOB support from greeting capabilities array. QEMU
            // 2.12+ advertises "oob" here when the monitor was built with
            // out-of-band command support; absence on older builds means
            // we must use the bare capability enable below.
            oob_supported = parsed
                .get("QMP")
                .and_then(|q| q.get("capabilities"))
                .and_then(|c| c.as_array())
                .map(|arr| arr.iter().any(|v| v.as_str() == Some("oob")))
                .unwrap_or(false);

            // Send qmp_capabilities and read the response. We only ask
            // the server to enable "oob" if it told us it was available
            // — sending `enable: ["oob"]` to a peer that didn't advertise
            // it would error out the handshake.
            let caps_request = if oob_supported {
                br#"{"execute":"qmp_capabilities","arguments":{"enable":["oob"]}}"#.as_slice()
            } else {
                br#"{"execute":"qmp_capabilities"}"#.as_slice()
            };
            // Bug #23: send as one atomic line so a mid-write failure can't
            // strand half a command on the wire.
            write_line(&mut write_half, caps_request).await?;

            let caps_resp: serde_json::Value = loop {
                let caps_line = match read_line_bounded(&mut reader).await? {
                    LineRead::Line(line) => line,
                    LineRead::Eof => {
                        return Err(Error::Protocol(
                            "QMP peer closed before qmp_capabilities reply".into(),
                        ));
                    }
                    LineRead::TooLong => {
                        return Err(Error::Protocol(format!(
                            "qmp_capabilities reply exceeded {} bytes",
                            MAX_LINE_BYTES
                        )));
                    }
                };
                let parsed: serde_json::Value = serde_json::from_str(&caps_line)?;
                if parsed.get("event").is_some() {
                    continue;
                }
                break parsed;
            };
            if let Some(error) = caps_resp.get("error") {
                return Err(Error::Protocol(format!(
                    "qmp_capabilities failed: {}",
                    error["desc"].as_str().unwrap_or("unknown")
                )));
            }

            Ok::<(), Error>(())
        })
        .await
        .map_err(|_| Error::Timeout("QMP handshake timed out".into()))??;

        // Build shared state. Channels are sized for the queue depth so
        // a burst of `QMP_QUEUE_CAPACITY` concurrent submitters never
        // blocks in the mpsc send (the semaphore is the real backpressure).
        let (command_tx, command_rx) = mpsc::channel::<Outbound>(QMP_QUEUE_CAPACITY * 2);
        let (raw_event_tx, raw_event_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);
        let (shutdown_tx, shutdown_rx) = watch::channel(());
        let (queue_depth_tx, _queue_depth_rx) = watch::channel(0usize);
        let in_flight: Arc<Mutex<HashMap<u64, PendingCmd>>> = Arc::new(Mutex::new(HashMap::new()));
        let queue_permits = Arc::new(Semaphore::new(QMP_QUEUE_CAPACITY));
        let oob_available = Arc::new(AtomicBool::new(oob_supported));
        let connection_alive = Arc::new(AtomicBool::new(true));
        let id_counter = Arc::new(AtomicU64::new(1));

        let read_handle = spawn(Self::read_loop(
            reader,
            in_flight.clone(),
            queue_depth_tx.clone(),
            raw_event_tx,
            connection_alive.clone(),
            shutdown_rx.clone(),
        ));

        let write_handle = spawn(Self::write_loop(
            write_half,
            command_rx,
            in_flight.clone(),
            queue_depth_tx.clone(),
            connection_alive.clone(),
            shutdown_rx.clone(),
        ));

        let event_handle = spawn(Self::event_worker(
            raw_event_rx,
            shutdown_rx.clone(),
            event_tx,
        ));

        let reaper_handle = spawn(Self::reaper_loop(
            in_flight.clone(),
            queue_depth_tx.clone(),
            shutdown_rx.clone(),
        ));

        let qmp = Self {
            version,
            command_tx,
            in_flight,
            queue_permits,
            queue_depth_tx,
            id_counter,
            oob_available,
            connection_alive,
            shutdown_signal: shutdown_tx,
            read_handle: Some(read_handle),
            write_handle: Some(write_handle),
            event_handle: Some(event_handle),
            reaper_handle: Some(reaper_handle),
        };

        Ok((qmp, event_rx))
    }

    /// Version advertised by this peer, independent of other connections.
    pub fn version(&self) -> (u32, u32, u32) {
        self.version
    }

    /// Maximum number of non-OOB commands that may be in flight at once.
    pub fn queue_capacity(&self) -> usize {
        QMP_QUEUE_CAPACITY
    }

    /// Number of commands currently awaiting a response (OOB included).
    pub fn queue_depth(&self) -> usize {
        *self.queue_depth_tx.borrow()
    }

    /// Subscribe to live updates of the in-flight queue depth.
    ///
    /// The watch sender is updated on every insert/remove from the
    /// in-flight map, so a UI can render the current load without
    /// polling.
    pub fn subscribe_queue(&self) -> watch::Receiver<usize> {
        self.queue_depth_tx.subscribe()
    }

    /// Whether the server advertised `"oob"` in its greeting and the
    /// client successfully negotiated it during handshake.
    pub fn oob_supported(&self) -> bool {
        self.oob_available.load(Ordering::Acquire)
    }

    /// Execute a typed QMP command and deserialize the response.
    ///
    /// Acquires one of [`queue_capacity()`](Self::queue_capacity)
    /// permits; if all permits are taken the call awaits until one
    /// becomes available, mirroring QEMU's `QMP_REQ_QUEUE_LEN_MAX`
    /// flow-control. Concurrent calls multiplex over the same socket;
    /// responses are routed by the QMP `id` field.
    #[must_use = "QMP commands can fail with a QmpError; ignoring the Result hides protocol errors"]
    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        let request = qapi::with_version(self.version, || build_request(cmd, false))?;
        let response = self.dispatch(request, false).await?;
        decode_response::<C>(response)
    }

    /// Execute a typed QMP command with a caller-supplied timeout.
    ///
    /// The timeout bounds the wait on the reply oneshot. The
    /// background reaper still enforces its own
    /// [`COMMAND_STALL_TIMEOUT`] guard; passing a value larger than
    /// that won't extend the wait — the reaper will drop the reply
    /// sender first. Use this method when you want a *tighter*
    /// deadline than the global default.
    #[must_use = "QMP commands can fail with a QmpError or time out; ignoring the Result hides protocol errors"]
    pub async fn execute_with_timeout<C: QapiCommand>(
        &self,
        cmd: &C,
        timeout: Duration,
    ) -> Result<C::Returns> {
        let request = qapi::with_version(self.version, || build_request(cmd, false))?;
        let response = self
            .dispatch_with_timeout(request, false, Some((C::NAME, timeout)))
            .await?;
        decode_response::<C>(response)
    }

    /// Execute an arbitrary QMP command from a raw JSON request and
    /// return the raw `return` payload.
    ///
    /// Mirrors `Qga::execute_raw` for callers that need to issue
    /// commands which aren't yet in the typed schema (e.g. a brand new
    /// QEMU release, an out-of-tree fork, or an internal monitor verb)
    /// without dropping down to the channel machinery directly.
    ///
    /// The `raw` argument should be a full QMP request object, e.g.
    /// `json!({"execute": "some-command", "arguments": {...}})`. The
    /// `id` field is injected internally for correlation and must not
    /// be set by the caller — see BUG 42. On success the returned
    /// value is the contents of the `return` field (an empty object
    /// `{}` if the command has no payload).
    #[must_use = "QMP commands can fail with a QmpError; ignoring the Result hides protocol errors"]
    pub async fn execute_raw(&self, raw: serde_json::Value) -> Result<serde_json::Value> {
        let response = self.dispatch(raw, false).await?;
        decode_raw(response)
    }

    /// Execute a typed QMP command out-of-band.
    ///
    /// OOB commands bypass the monitor's command queue: the envelope
    /// uses `"exec-oob"` rather than `"execute"`, and QEMU answers
    /// from the I/O thread directly without queueing behind in-flight
    /// non-OOB work. Only commands whose schema declares
    /// `allow-oob = true` may be issued here; the bound is enforced
    /// at compile time via `const { assert!(C::ALLOW_OOB) }`.
    ///
    /// At runtime the handshake must also have negotiated OOB support
    /// (see [`oob_supported`](Self::oob_supported)); if the peer does
    /// not advertise OOB in its greeting this call returns
    /// `Error::Protocol`.
    #[must_use = "QMP commands can fail with a QmpError; ignoring the Result hides protocol errors"]
    pub async fn execute_oob<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        // Compile-time guard: the schema-generated trait constant
        // forces this assertion to succeed for OOB-allowed commands and
        // fail at compile time for everything else, eliminating the
        // possibility of a runtime "command not allowed OOB" surprise.
        const { assert!(C::ALLOW_OOB, "execute_oob requires C::ALLOW_OOB == true") };

        if !self.oob_supported() {
            return Err(Error::Protocol(
                "OOB not negotiated with peer (server did not advertise \"oob\" in greeting)"
                    .into(),
            ));
        }
        let request = qapi::with_version(self.version, || build_request(cmd, true))?;
        let response = self.dispatch(request, true).await?;
        decode_response::<C>(response)
    }

    /// Execute an arbitrary OOB command from a raw JSON request and
    /// return the raw `return` payload.
    ///
    /// Mirrors [`execute_raw`](Self::execute_raw) but emits the
    /// `"exec-oob"` envelope and skips the queue semaphore. Because
    /// raw requests carry no compile-time schema info, OOB-eligibility
    /// is checked only at runtime: a peer that never advertised OOB
    /// will receive `Error::Protocol`.
    #[must_use = "QMP commands can fail with a QmpError; ignoring the Result hides protocol errors"]
    pub async fn execute_raw_oob(&self, raw: serde_json::Value) -> Result<serde_json::Value> {
        if !self.oob_supported() {
            return Err(Error::Protocol(
                "OOB not negotiated with peer (server did not advertise \"oob\" in greeting)"
                    .into(),
            ));
        }
        let response = self.dispatch(raw, true).await?;
        decode_raw(response)
    }

    /// Submit `request` and await the matching reply with no extra
    /// timeout. The reaper still enforces a global stall guard.
    async fn dispatch(&self, request: serde_json::Value, oob: bool) -> Result<serde_json::Value> {
        self.dispatch_with_timeout(request, oob, None).await
    }

    /// Submit `request`, await the matching reply, optionally bounded
    /// by a per-call timeout (used by `execute_with_timeout`).
    async fn dispatch_with_timeout(
        &self,
        mut request: serde_json::Value,
        oob: bool,
        timeout: Option<(&'static str, Duration)>,
    ) -> Result<serde_json::Value> {
        // Non-OOB commands respect QEMU's monitor queue: hold a permit
        // for the entire round-trip so the in-flight count never exceeds
        // QMP_QUEUE_CAPACITY. OOB commands bypass this entirely — the
        // monitor handles them on the I/O thread without queueing.
        let _permit: Option<OwnedSemaphorePermit> = if oob {
            None
        } else {
            Some(
                self.queue_permits
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(|_| Error::Channel("QMP queue semaphore closed".into()))?,
            )
        };

        // Fast-fail when the connection has already died — without this
        // a late `execute()` could insert into `in_flight` *after*
        // `read_loop` has cleared it on EOF, leaving the entry to be
        // reaped 30s later instead of failing fast.
        if !self.connection_alive.load(Ordering::Acquire) {
            return Err(Error::Channel("QMP connection closed".into()));
        }

        let id = self.id_counter.fetch_add(1, Ordering::Relaxed);
        if let Some(obj) = request.as_object_mut() {
            obj.insert("id".to_string(), serde_json::json!(id));
        }

        let (reply_tx, reply_rx) = oneshot::channel();
        // Insert the pending entry *before* sending the request so that a
        // very fast reply (mock server, in-process) cannot race ahead of
        // the reader looking up the id. Briefly publish the new depth to
        // any subscribed UI. We use `send_replace` rather than `send`
        // because the latter discards the value when there are no live
        // receivers — and we always want the gauge to be observable via
        // `queue_depth()` even before any subscriber attaches.
        //
        // Re-check `connection_alive` *while holding the lock* to close
        // the race with `read_loop`'s exit clearing the map: either we
        // see the flag false (and bail without inserting), or we insert
        // and read_loop's subsequent clear will drop our reply_tx.
        {
            let mut guard = self.in_flight.lock().await;
            if !self.connection_alive.load(Ordering::Acquire) {
                return Err(Error::Channel("QMP connection closed".into()));
            }
            guard.insert(
                id,
                PendingCmd {
                    reply_tx,
                    sent_at: Instant::now(),
                    oob,
                },
            );
            self.queue_depth_tx.send_replace(guard.len());
        }

        // Send envelope to writer. If the writer is gone, the in-flight
        // entry will never be answered — clean it up before returning.
        if self
            .command_tx
            .send(Outbound { id, json: request })
            .await
            .is_err()
        {
            self.evict(id).await;
            return Err(Error::Channel("command channel closed".into()));
        }

        // Await the reply with optional per-call timeout. On any error
        // path we evict the in-flight entry so a late response can't
        // accumulate forever.
        let response = match timeout {
            Some((name, dur)) => match tokio::time::timeout(dur, reply_rx).await {
                Ok(Ok(resp)) => resp,
                Ok(Err(_)) => {
                    self.evict(id).await;
                    return Err(Error::Channel(
                        "QMP command channel closed \u{2014} likely timed out or connection lost"
                            .into(),
                    ));
                }
                Err(_) => {
                    self.evict(id).await;
                    return Err(Error::Timeout(format!(
                        "QMP command {} timed out after {:?}",
                        name, dur
                    )));
                }
            },
            None => match reply_rx.await {
                Ok(resp) => resp,
                Err(_) => {
                    self.evict(id).await;
                    return Err(Error::Channel(
                        "QMP command channel closed \u{2014} likely timed out or connection lost"
                            .into(),
                    ));
                }
            },
        };

        Ok(response)
    }

    /// Remove an id from `in_flight` and publish the new depth.
    async fn evict(&self, id: u64) {
        let mut guard = self.in_flight.lock().await;
        guard.remove(&id);
        self.queue_depth_tx.send_replace(guard.len());
    }

    /// Gracefully shut down all background tasks.
    ///
    /// Drops every pending in-flight reply oneshot first so that any
    /// caller currently parked on `reply_rx.await` wakes immediately
    /// with `Error::Channel` rather than waiting for the per-task
    /// timeout. Then the worker handles are joined with a 500ms cap
    /// before falling back to abort.
    pub async fn shutdown(&mut self) {
        // Mark connection dead first so any concurrent dispatch fails
        // fast on its `connection_alive` check.
        self.connection_alive.store(false, Ordering::Release);
        let _ = self.shutdown_signal.send(());

        // Wake all parked callers. Dropping the senders translates to
        // `RecvError` on each `reply_rx.await`, which `dispatch` maps
        // to `Error::Channel`. Doing this here (rather than relying on
        // the worker tasks to drop them on exit) guarantees fast
        // wakeup even if a worker is wedged.
        {
            let mut guard = self.in_flight.lock().await;
            guard.clear();
            self.queue_depth_tx.send_replace(0);
        }

        // Bound each handle wait. Order matters less than completeness:
        // every live handle gets the same bounded join-or-abort.
        shutdown_handle(self.write_handle.take()).await;
        shutdown_handle(self.event_handle.take()).await;
        shutdown_handle(self.read_handle.take()).await;
        shutdown_handle(self.reaper_handle.take()).await;
    }

    /// Read-side of the QMP socket: parses each line, dispatches
    /// responses to the matching in-flight oneshot via `id`, and
    /// forwards events to the event worker.
    async fn read_loop(
        mut reader: BufReader<OwnedReadHalf>,
        in_flight: Arc<Mutex<HashMap<u64, PendingCmd>>>,
        queue_depth_tx: watch::Sender<usize>,
        event_tx: mpsc::Sender<String>,
        connection_alive: Arc<AtomicBool>,
        mut shutdown_rx: watch::Receiver<()>,
    ) {
        // We read one line at a time with a hard byte cap
        // (`read_line_bounded`) so a malicious or broken peer sending an
        // unterminated line cannot exhaust process memory. On `TooLong`
        // we've already resynced to the next newline and just log;
        // `Eof`/real I/O errors end the loop so we don't spin.
        loop {
            tokio::select! {
                result = read_line_bounded(&mut reader) => {
                    match result {
                        Ok(LineRead::Line(line)) => {
                            match serde_json::from_str::<serde_json::Value>(&line) {
                                Ok(parsed) => {
                                    if parsed.get("event").is_some() {
                                        // BUG 43 fix: event channel backpressure
                                        // must not stall the read loop. The raw
                                        // `event_tx` has a bounded capacity (100)
                                        // and a stuck `event_worker` (or a caller
                                        // that isn't draining events) would
                                        // otherwise suspend `send().await` here,
                                        // blocking QMP command responses too.
                                        // Drop events on overflow instead — we
                                        // prefer command liveness over event
                                        // completeness.
                                        use tokio::sync::mpsc::error::TrySendError;
                                        match event_tx.try_send(line) {
                                            Ok(()) => {}
                                            Err(TrySendError::Full(_)) => {
                                                tracing::warn!(
                                                    "QMP event channel full, dropping event"
                                                );
                                            }
                                            Err(TrySendError::Closed(_)) => break,
                                        }
                                    } else {
                                        // Response: route by id. Take the
                                        // oneshot out of the in-flight map and
                                        // hand the parsed value over. Drops the
                                        // permit (held inside `dispatch`) when
                                        // the caller's future resumes.
                                        let resp_id = parsed
                                            .get("id")
                                            .and_then(|v| v.as_u64());
                                        if let Some(id) = resp_id {
                                            let mut guard = in_flight.lock().await;
                                            if let Some(pending) = guard.remove(&id) {
                                                queue_depth_tx.send_replace(guard.len());
                                                drop(guard);
                                                let _ = pending.reply_tx.send(parsed);
                                            } else {
                                                tracing::warn!(
                                                    id = id,
                                                    "received QMP response for unknown id (likely reaper-evicted)"
                                                );
                                            }
                                        } else {
                                            tracing::warn!(
                                                ?parsed,
                                                "received QMP response without id, dropping"
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, line = %line, "failed to parse QMP JSON line");
                                    continue;
                                }
                            }
                        }
                        Ok(LineRead::TooLong) => {
                            tracing::warn!("dropped oversize QMP line");
                            continue;
                        }
                        Ok(LineRead::Eof) | Err(_) => break,
                    }
                }
                _ = shutdown_rx.changed() => break,
            }
        }
        // Connection going away: signal that new dispatches should fail
        // fast (so they don't insert into a map that will never be
        // drained), then wake every existing waiter.
        connection_alive.store(false, Ordering::Release);
        let mut guard = in_flight.lock().await;
        guard.clear();
        queue_depth_tx.send_replace(0);
    }

    /// Write-side of the QMP socket: pulls envelopes off the command
    /// channel, serializes, and writes. No longer waits inline for a
    /// reply — concurrency is the whole point of this rewrite.
    async fn write_loop(
        mut write_half: OwnedWriteHalf,
        mut command_rx: mpsc::Receiver<Outbound>,
        in_flight: Arc<Mutex<HashMap<u64, PendingCmd>>>,
        queue_depth_tx: watch::Sender<usize>,
        connection_alive: Arc<AtomicBool>,
        mut shutdown_rx: watch::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                next = command_rx.recv() => {
                    let Some(out) = next else { break };
                    // Re-serialize to a single line. The id was already
                    // injected inside `dispatch_with_timeout`.
                    let serialized = match serde_json::to_string(&out.json) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!(error = %e, "failed to serialize QMP command");
                            // Drop the in-flight entry so caller wakes
                            // up with Error::Channel.
                            let mut guard = in_flight.lock().await;
                            guard.remove(&out.id);
                            queue_depth_tx.send_replace(guard.len());
                            continue;
                        }
                    };
                    // Bug #23: a split json + \n write let a partial-failure
                    // strand a truncated line; the next command would then be
                    // appended to it and poison the session. Issue one
                    // atomic write so either the full line lands or none of
                    // it, and on error break out so no further commands are
                    // dispatched into a dead socket.
                    if write_line(&mut write_half, serialized.as_bytes()).await.is_err() {
                        break;
                    }
                }
                _ = shutdown_rx.changed() => break,
            }
        }
        // I/O dead: wake every waiter. The read_loop does this too on
        // its own exit, but doing it here as well covers the case where
        // the writer dies first (e.g. SIGPIPE on a broken pipe).
        connection_alive.store(false, Ordering::Release);
        let mut guard = in_flight.lock().await;
        guard.clear();
        queue_depth_tx.send_replace(0);
    }

    /// Background reaper: every `REAPER_TICK` seconds, evict in-flight
    /// entries older than `COMMAND_STALL_TIMEOUT`. Dropping the
    /// oneshot wakes the parked caller with `Error::Channel`.
    async fn reaper_loop(
        in_flight: Arc<Mutex<HashMap<u64, PendingCmd>>>,
        queue_depth_tx: watch::Sender<usize>,
        mut shutdown_rx: watch::Receiver<()>,
    ) {
        let mut ticker = tokio::time::interval(REAPER_TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let now = Instant::now();
                    let mut guard = in_flight.lock().await;
                    let stale: Vec<u64> = guard
                        .iter()
                        .filter(|(_, p)| now.duration_since(p.sent_at) >= COMMAND_STALL_TIMEOUT)
                        .map(|(id, _)| *id)
                        .collect();
                    if !stale.is_empty() {
                        for id in stale {
                            tracing::warn!(
                                id,
                                "evicting stale QMP command (>{:?} without response)",
                                COMMAND_STALL_TIMEOUT
                            );
                            guard.remove(&id);
                        }
                        queue_depth_tx.send_replace(guard.len());
                    }
                }
                _ = shutdown_rx.changed() => break,
            }
        }
    }

    async fn event_worker(
        mut raw_event_rx: mpsc::Receiver<String>,
        mut shutdown_rx: watch::Receiver<()>,
        event_tx: mpsc::Sender<QmpEvent>,
    ) {
        loop {
            tokio::select! {
                Some(raw) = raw_event_rx.recv() => {
                    match serde_json::from_str::<QmpEvent>(&raw) {
                        Ok(event) => {
                            // Break if consumer dropped the receiver —
                            // otherwise this worker spins forever.
                            if event_tx.send(event).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, line = %raw, "failed to parse QMP event");
                            continue;
                        }
                    }
                }
                _ = shutdown_rx.changed() => break,
            }
        }
    }
}

/// Build a QMP request envelope from a typed command and OOB flag.
///
/// Factored out so `execute`, `execute_with_timeout`, `execute_oob`
/// share the exact same shape — empty-arguments objects collapse to
/// the bare `{"execute":"NAME"}` (or `{"exec-oob":"NAME"}`) form to
/// match the wire format QEMU's docs use.
fn build_request<C: QapiCommand>(cmd: &C, oob: bool) -> Result<serde_json::Value> {
    let args = serde_json::to_value(cmd)?;
    let verb = if oob { "exec-oob" } else { "execute" };
    let request = if args.as_object().is_some_and(|o| o.is_empty()) {
        serde_json::json!({ verb: C::NAME })
    } else {
        serde_json::json!({ verb: C::NAME, "arguments": args })
    };
    Ok(request)
}

/// Decode a typed command's response, surfacing QMP errors and
/// handling the special-case empty `{}` payload for unit returns.
fn decode_response<C: QapiCommand>(response: serde_json::Value) -> Result<C::Returns> {
    if let Some(error) = response.get("error") {
        return Err(Error::Qmp(build_qmp_error(error)));
    }

    let ret = response
        .get("return")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    // Handle commands that return () — QMP returns {"return":{}} which
    // can't deserialize as (). Treat empty objects as null for unit types.
    let ret = if ret.as_object().is_some_and(|o| o.is_empty()) {
        // Try deserializing as-is first; fall back to null for unit return types.
        match serde_json::from_value::<C::Returns>(ret.clone()) {
            Ok(v) => return Ok(v),
            Err(_) => serde_json::Value::Null,
        }
    } else {
        ret
    };

    Ok(serde_json::from_value(ret)?)
}

/// Decode a raw command's response: returns the contents of `return`
/// (an empty object for unit responses) or the typed `QmpError`.
fn decode_raw(response: serde_json::Value) -> Result<serde_json::Value> {
    if let Some(error) = response.get("error") {
        return Err(Error::Qmp(build_qmp_error(error)));
    }
    Ok(response
        .get("return")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new())))
}

/// Drive a background task handle to completion with a bounded wait.
///
/// Factored out of `Qmp::shutdown` where the same pattern was repeated
/// once per spawned worker: give the task a short grace period to exit
/// cleanly on the shutdown signal, fall back to `abort()` + `await` if
/// it's stuck. Taking `&mut handle` inside `timeout` lets us recover
/// the owned handle after the timeout fires so we can still `abort()`
/// it — the alternative `timeout(500ms, handle)` consumes the handle
/// on elapsed and there's nothing left to abort.
async fn shutdown_handle(handle: Option<JoinHandle<()>>) {
    if let Some(mut handle) = handle {
        let handle_ref: &mut JoinHandle<()> = &mut handle;
        if tokio::time::timeout(Duration::from_millis(500), handle_ref)
            .await
            .is_err()
        {
            handle.abort();
            let _ = handle.await;
        }
    }
}

impl Drop for Qmp {
    fn drop(&mut self) {
        // Signal shutdown first so well-behaved loops exit cleanly, then
        // abort() as a fallback. Drop can't await, so abort() is the only
        // way to guarantee spawned tasks don't leak. The Mutex<HashMap>
        // is held in an Arc; once every clone goes out of scope the
        // pending senders inside it are dropped, waking every parked
        // caller with Error::Channel.
        let _ = self.shutdown_signal.send(());
        if let Some(h) = self.read_handle.take() {
            h.abort();
        }
        if let Some(h) = self.write_handle.take() {
            h.abort();
        }
        if let Some(h) = self.event_handle.take() {
            h.abort();
        }
        if let Some(h) = self.reaper_handle.take() {
            h.abort();
        }
    }
}

#[cfg(test)]
mod write_line_tests {
    //! Bug #23: verify the atomic-write shape. `write_line` must deliver
    //! `payload + \n` as a single `write_all`, and on short-write
    //! failures the peer must see either the full line or nothing of
    //! the current line — never a truncated mid-line with the next
    //! command glued to the tail.
    use super::{write_line, write_line_with_sentinel};
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::AsyncWrite;

    /// `AsyncWrite` wrapper that records every `poll_write` and
    /// optionally injects a failure after the first `fail_after` bytes
    /// have been accepted. Used to prove that `write_line` issues a
    /// single `write_all` and that a mid-buffer failure cannot leave a
    /// stranded partial line behind a subsequent successful write.
    struct FailAfter {
        sink: Vec<u8>,
        writes: Vec<usize>,
        fail_after: Option<usize>,
    }
    impl FailAfter {
        fn new(fail_after: Option<usize>) -> Self {
            Self {
                sink: Vec::new(),
                writes: Vec::new(),
                fail_after,
            }
        }
    }
    impl AsyncWrite for FailAfter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.writes.push(buf.len());
            if let Some(limit) = self.fail_after {
                let remaining = limit.saturating_sub(self.sink.len());
                if remaining == 0 {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "FailAfter: injected fault",
                    )));
                }
                let take = remaining.min(buf.len());
                self.sink.extend_from_slice(&buf[..take]);
                return Poll::Ready(Ok(take));
            }
            self.sink.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }
        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn write_line_single_poll_write_happy_path() {
        let mut w = FailAfter::new(None);
        write_line(&mut w, br#"{"execute":"query-status"}"#)
            .await
            .unwrap();
        // Bug #23 invariant: JSON + \n delivered as one contiguous
        // buffer. `tokio::io::AsyncWriteExt::write_all` may still loop
        // internally on short polls, but our *caller* path is one
        // prepared buffer — so the peer's first read sees the terminator
        // alongside the payload.
        assert!(w.sink.ends_with(b"\n"));
        assert_eq!(w.sink, b"{\"execute\":\"query-status\"}\n");
        assert_eq!(w.writes, vec![w.sink.len()]);
    }

    #[tokio::test]
    async fn write_line_partial_failure_never_strands_newline_for_next_send() {
        // First send fails after N bytes — simulating a broken pipe
        // mid-command. `write_line` must return Err, and the caller
        // must treat the stream as poisoned (i.e. *not* attempt a
        // follow-up write). The invariant being tested is: no second
        // `write_line` call will glue its payload onto the tail of the
        // stranded first.
        let payload = br#"{"execute":"query-status"}"#;
        let mut w = FailAfter::new(Some(5));
        let err = write_line(&mut w, payload).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
        // At most the first 5 bytes made it onto the wire.
        assert_eq!(w.sink.len(), 5);
        assert!(payload.starts_with(&w.sink));
        // And crucially: the newline is *not* present in that stranded
        // prefix, so the peer's line reader still blocks — but because
        // the caller sees Err, no second command is pushed after it.
        assert!(!w.sink.contains(&b'\n'));
    }

    #[tokio::test]
    async fn write_line_with_sentinel_atomic() {
        let mut w = FailAfter::new(None);
        write_line_with_sentinel(&mut w, 0xFF, br#"{"execute":"guest-sync-delimited"}"#)
            .await
            .unwrap();
        assert_eq!(w.sink.first(), Some(&0xFFu8));
        assert!(w.sink.ends_with(b"\n"));
        // One contiguous buffer: sentinel + payload + \n in a single
        // `poll_write` from our perspective.
        assert_eq!(w.writes, vec![w.sink.len()]);
    }
}
