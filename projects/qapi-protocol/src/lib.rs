#![deny(unsafe_code)]

pub mod qga;
pub mod server;

use anyhow::Result;
use qapi::QapiCommand;

use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::{spawn, JoinHandle};

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
/// response from QEMU, they wrap it in this type and return it via
/// `anyhow::Error::new(QmpError { .. })`. Callers that need to branch
/// on the error class (e.g. retry on `CommandNotFound` but bail on
/// `GenericError`) can recover the typed value with
/// [`anyhow::Error::downcast_ref::<QmpError>`][anyhow::Error::downcast_ref].
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

/// Async QMP client connected to a QEMU instance.
#[derive(Debug)]
pub struct Qmp {
    command_tx: mpsc::Sender<(String, oneshot::Sender<serde_json::Value>)>,
    shutdown_signal: watch::Sender<()>,
    read_handle: Option<JoinHandle<()>>,
    write_handle: Option<JoinHandle<()>>,
    event_handle: Option<JoinHandle<()>>,
}

impl Qmp {
    /// Connect to a QEMU QMP socket and perform the capabilities negotiation.
    ///
    /// Returns the `Qmp` handle and a receiver for QMP events.
    /// Also sets the global QEMU version via `qapi::set_qemu_version()`.
    #[must_use = "dropping the Qmp handle immediately tears the connection down; bind it to keep the QMP session alive"]
    pub async fn new(socket_path: &Path) -> Result<(Self, mpsc::Receiver<QmpEvent>)> {
        // Bound `connect(2)` itself. A present-but-unserved socket —
        // e.g. bound without `listen()`, stuck in socket activation, or
        // living on a hung FUSE mount — would otherwise leave the
        // caller blocked indefinitely. 5s comfortably covers a healthy
        // local connect while still surfacing a dead peer quickly.
        let stream = tokio::time::timeout(
            Duration::from_secs(5),
            UnixStream::connect(socket_path),
        )
        .await
        .map_err(|_| anyhow::anyhow!("QMP connect timed out after 5s"))??;
        let (read_half, write_half) = stream.into_split();

        let mut reader = BufReader::new(read_half);
        let mut write_half = write_half;

        // Bound the full greeting + qmp_capabilities handshake. All reads
        // below are otherwise unbounded — if QEMU accepts the connect(2)
        // but hangs before writing the greeting (or mid-handshake) we'd
        // block `Qmp::new()` forever. 10s is plenty for a healthy peer.
        tokio::time::timeout(Duration::from_secs(10), async {
            // Read greeting synchronously before spawning workers
            let greeting_line = match read_line_bounded(&mut reader).await? {
                LineRead::Line(line) => line,
                LineRead::Eof => {
                    anyhow::bail!("QMP peer closed before sending greeting")
                }
                LineRead::TooLong => {
                    anyhow::bail!("QMP greeting exceeded {} bytes", MAX_LINE_BYTES)
                }
            };
            let parsed: serde_json::Value = serde_json::from_str(&greeting_line)
                .map_err(|e| anyhow::anyhow!("failed to parse QMP greeting: {}", e))?;
            let qemu = parsed
                .get("QMP")
                .and_then(|q| q.get("version"))
                .and_then(|v| v.get("qemu"))
                .ok_or_else(|| {
                    anyhow::anyhow!("QMP greeting missing QMP.version.qemu: {}", parsed)
                })?;
            let major = qemu["major"].as_u64().unwrap_or(0) as u32;
            let minor = qemu["minor"].as_u64().unwrap_or(0) as u32;
            let micro = qemu["micro"].as_u64().unwrap_or(0) as u32;
            // Log but don't fail if version was already set (multi-VM process).
            let _ = qapi::set_qemu_version(major, minor, micro);

            // Send qmp_capabilities and read the response
            write_half
                .write_all(b"{\"execute\":\"qmp_capabilities\"}\n")
                .await?;

            let caps_resp: serde_json::Value = loop {
                let caps_line = match read_line_bounded(&mut reader).await? {
                    LineRead::Line(line) => line,
                    LineRead::Eof => {
                        anyhow::bail!("QMP peer closed before qmp_capabilities reply")
                    }
                    LineRead::TooLong => {
                        anyhow::bail!(
                            "qmp_capabilities reply exceeded {} bytes",
                            MAX_LINE_BYTES
                        )
                    }
                };
                let parsed: serde_json::Value = serde_json::from_str(&caps_line)?;
                if parsed.get("event").is_some() {
                    continue;
                }
                break parsed;
            };
            if let Some(error) = caps_resp.get("error") {
                anyhow::bail!(
                    "qmp_capabilities failed: {}",
                    error["desc"].as_str().unwrap_or("unknown")
                );
            }

            Ok::<(), anyhow::Error>(())
        })
        .await
        .map_err(|_| anyhow::anyhow!("QMP handshake timed out"))??;

        // Now spawn the background workers
        let (command_tx, command_rx) = mpsc::channel(1);
        let (response_tx, response_rx) = mpsc::channel(1);
        let (raw_event_tx, raw_event_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);
        let (shutdown_tx, shutdown_rx) = watch::channel(());

        let read_handle = spawn(Self::read_loop(
            reader,
            response_tx,
            raw_event_tx,
            shutdown_rx.clone(),
        ));

        let write_handle = spawn(Self::write_loop(
            write_half,
            command_rx,
            response_rx,
            shutdown_rx.clone(),
        ));

        let event_handle = spawn(Self::event_worker(
            raw_event_rx,
            shutdown_rx.clone(),
            event_tx,
        ));

        let qmp = Self {
            command_tx,
            shutdown_signal: shutdown_tx,
            read_handle: Some(read_handle),
            write_handle: Some(write_handle),
            event_handle: Some(event_handle),
        };

        Ok((qmp, event_rx))
    }

    /// Execute a typed QMP command and deserialize the response.
    #[must_use = "QMP commands can fail with a QmpError; ignoring the Result hides protocol errors"]
    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        let args = serde_json::to_value(cmd)?;
        let request = if args.as_object().is_some_and(|o| o.is_empty()) {
            serde_json::json!({ "execute": C::NAME })
        } else {
            serde_json::json!({ "execute": C::NAME, "arguments": args })
        };

        let request_str = serde_json::to_string(&request)?;
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send((request_str, reply_tx))
            .await
            .map_err(|_| anyhow::anyhow!("command channel closed"))?;

        // NOTE: the `write_loop` drops `reply_tx` in two cases that are
        // indistinguishable from the client's perspective — the 30s
        // per-command response timeout (see `write_loop`'s inner select!)
        // and a natural shutdown/broken pipe. The `RecvError` from
        // `reply_rx.await` collapses both into a single "sender dropped"
        // signal. We surface a combined message so callers at least know
        // that a timeout is one of the plausible causes; distinguishing
        // them cleanly would require a second channel or a sentinel value
        // and is tracked as a follow-up (BUG 34).
        let response = reply_rx
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "QMP command channel closed — likely timed out or connection lost"
                )
            })?;

        if let Some(error) = response.get("error") {
            return Err(anyhow::Error::new(build_qmp_error(error)));
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

    /// Execute a typed QMP command with a caller-supplied timeout.
    ///
    /// The timeout bounds the wait on the reply oneshot. `write_loop`
    /// still enforces its own 30s per-command guard internally, so in
    /// practice the effective timeout is `min(timeout, 30s)` — passing
    /// a longer duration will never wait longer than 30s because the
    /// write_loop will have already dropped the reply sender. Use this
    /// method when you want a *tighter* timeout than the internal
    /// default (e.g. a 2s deadline for a health probe).
    ///
    /// `execute()` is intentionally NOT routed through this method —
    /// chaining a 60s outer timeout on top of the 30s write_loop guard
    /// would just add a redundant layer that never actually fires, so
    /// `execute()` keeps its original direct-await shape.
    #[must_use = "QMP commands can fail with a QmpError or time out; ignoring the Result hides protocol errors"]
    pub async fn execute_with_timeout<C: QapiCommand>(
        &self,
        cmd: &C,
        timeout: Duration,
    ) -> Result<C::Returns> {
        let args = serde_json::to_value(cmd)?;
        let request = if args.as_object().is_some_and(|o| o.is_empty()) {
            serde_json::json!({ "execute": C::NAME })
        } else {
            serde_json::json!({ "execute": C::NAME, "arguments": args })
        };

        let request_str = serde_json::to_string(&request)?;
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send((request_str, reply_tx))
            .await
            .map_err(|_| anyhow::anyhow!("command channel closed"))?;

        // Wrap the reply wait in the caller's timeout. The `??` unwraps
        // first the Elapsed error (our outer timeout) and then the
        // RecvError (reply_tx dropped — write_loop timed out or the
        // connection died). We translate Elapsed into an explicit
        // command-name timeout message for observability.
        let response = tokio::time::timeout(timeout, reply_rx)
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "QMP command {} timed out after {:?}",
                    C::NAME,
                    timeout
                )
            })?
            .map_err(|_| {
                anyhow::anyhow!(
                    "QMP command channel closed — likely timed out or connection lost"
                )
            })?;

        if let Some(error) = response.get("error") {
            return Err(anyhow::Error::new(build_qmp_error(error)));
        }

        let ret = response
            .get("return")
            .cloned()
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        // Mirror execute()'s empty-object → unit handling for commands
        // that return ().
        let ret = if ret.as_object().is_some_and(|o| o.is_empty()) {
            match serde_json::from_value::<C::Returns>(ret.clone()) {
                Ok(v) => return Ok(v),
                Err(_) => serde_json::Value::Null,
            }
        } else {
            ret
        };

        Ok(serde_json::from_value(ret)?)
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
    /// `id` field is injected by `write_loop` for correlation and must
    /// not be set by the caller — see BUG 42. On success the returned
    /// value is the contents of the `return` field (an empty object
    /// `{}` if the command has no payload).
    #[must_use = "QMP commands can fail with a QmpError; ignoring the Result hides protocol errors"]
    pub async fn execute_raw(
        &self,
        raw: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let request_str = serde_json::to_string(&raw)?;
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send((request_str, reply_tx))
            .await
            .map_err(|_| anyhow::anyhow!("command channel closed"))?;

        let response = reply_rx.await.map_err(|_| {
            anyhow::anyhow!("QMP command channel closed — likely timed out or connection lost")
        })?;

        if let Some(error) = response.get("error") {
            return Err(anyhow::Error::new(build_qmp_error(error)));
        }

        Ok(response
            .get("return")
            .cloned()
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new())))
    }

    /// Gracefully shut down all background tasks.
    pub async fn shutdown(&mut self) {
        let _ = self.shutdown_signal.send(());
        // Bound each handle wait: read_loop in particular may keep picking
        // next_line() under heavy event traffic and never observe the
        // shutdown_rx.changed() branch. Timeout → abort → await (ignoring
        // errors) guarantees forward progress. Order matters less than
        // completeness — every live handle gets the same bounded wait.
        shutdown_handle(self.write_handle.take()).await;
        shutdown_handle(self.event_handle.take()).await;
        shutdown_handle(self.read_handle.take()).await;
    }

    async fn read_loop(
        mut reader: BufReader<OwnedReadHalf>,
        response_tx: mpsc::Sender<serde_json::Value>,
        event_tx: mpsc::Sender<String>,
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
                                    } else if response_tx.send(parsed).await.is_err() {
                                        // Response channel closed — write_loop
                                        // is gone; no point continuing to read.
                                        break;
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
    }

    async fn write_loop(
        mut write_half: OwnedWriteHalf,
        mut command_rx: mpsc::Receiver<(String, oneshot::Sender<serde_json::Value>)>,
        mut response_rx: mpsc::Receiver<serde_json::Value>,
        mut shutdown_rx: watch::Receiver<()>,
    ) {
        // BUG 42 fix: command correlation via QMP `id` field.
        //
        // Previously, a per-command 30s response-wait in an inner select!
        // dropped `reply_tx` on timeout and fell through to the outer loop.
        // If QEMU then produced the late response, `read_loop` pushed it
        // into `response_rx` and the NEXT command's `recv()` collected the
        // stale response as its own reply — cross-command desync.
        //
        // The QMP spec lets clients tag each request with an `id` field
        // and guarantees the server echoes it verbatim on the matching
        // response. We now inject a monotonic counter before writing the
        // command, and the response-matching loop below discards any
        // reply whose `id` does not match the expected one. Responses
        // without any `id` field are accepted as a fallback to keep mock
        // servers and pre-`id` peers working — real QEMU always echoes.
        let mut id_counter: u64 = 0;
        loop {
            tokio::select! {
                Some((command, reply_tx)) = command_rx.recv() => {
                    id_counter = id_counter.wrapping_add(1);
                    let expected_id = id_counter;

                    // Inject `id` into the command JSON. If the payload
                    // is not a JSON object we still try to send it raw —
                    // correlation is best-effort, not a hard requirement.
                    let tagged = match serde_json::from_str::<serde_json::Value>(&command) {
                        Ok(mut value) => {
                            if let Some(obj) = value.as_object_mut() {
                                obj.insert("id".to_string(), serde_json::json!(expected_id));
                            }
                            serde_json::to_string(&value).unwrap_or(command)
                        }
                        Err(_) => command,
                    };

                    if write_half.write_all(tagged.as_bytes()).await.is_err() {
                        break;
                    }
                    if write_half.write_all(b"\n").await.is_err() {
                        break;
                    }

                    // Bound the response wait: a hung QEMU would otherwise
                    // block this task forever. On timeout, drop reply_tx
                    // (client sees "channel closed") and keep going. On
                    // shutdown, break out of the loop entirely. Inside the
                    // select we loop on stale responses — a late reply from
                    // a previously-timed-out command carries the old `id`
                    // and must be discarded rather than delivered to the
                    // current caller.
                    let deadline = tokio::time::sleep(Duration::from_secs(30));
                    tokio::pin!(deadline);

                    enum Outcome {
                        Delivered,
                        TimedOut,
                        Shutdown,
                        Closed,
                    }
                    let mut reply_tx = Some(reply_tx);
                    let outcome = loop {
                        tokio::select! {
                            response = response_rx.recv() => {
                                match response {
                                    Some(response) => {
                                        let resp_id = response
                                            .get("id")
                                            .and_then(|v| v.as_u64());
                                        if resp_id.is_some() && resp_id != Some(expected_id) {
                                            tracing::warn!(
                                                got = ?resp_id,
                                                expected = expected_id,
                                                "discarding stale QMP response (late reply from prior command)"
                                            );
                                            continue;
                                        }
                                        if let Some(tx) = reply_tx.take() {
                                            let _ = tx.send(response);
                                        }
                                        break Outcome::Delivered;
                                    }
                                    None => break Outcome::Closed,
                                }
                            }
                            _ = shutdown_rx.changed() => break Outcome::Shutdown,
                            _ = &mut deadline => break Outcome::TimedOut,
                        }
                    };
                    match outcome {
                        Outcome::Delivered => continue,
                        Outcome::TimedOut => {
                            // reply_tx still in the Option — dropping here
                            // signals "channel closed" to the waiting caller.
                            drop(reply_tx);
                            continue;
                        }
                        Outcome::Shutdown | Outcome::Closed => break,
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
        // way to guarantee spawned tasks don't leak.
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
    }
}
