use crate::{
    framing::{self, JsonReader},
    wire, ClientOptions, Error, QmpEvent, Result,
};
use qapi::QapiCommand;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    os::fd::{AsRawFd, OwnedFd, RawFd},
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::UnixStream,
    sync::{mpsc, oneshot, watch, OwnedSemaphorePermit, Semaphore},
    task::AbortHandle,
    time::{timeout, timeout_at, Instant},
};

/// QEMU recommends at most eight unanswered in-band commands so it continues
/// reading OOB requests. See the vendored docs/interop/qmp-spec.rst.
pub const QMP_QUEUE_CAPACITY: usize = 8;
const OOB_CAPACITY: usize = 8;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Greeting {
    pub version: VersionInfo,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionInfo {
    pub qemu: QemuVersion,
    pub package: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct QemuVersion {
    pub major: u32,
    pub minor: u32,
    pub micro: u32,
}

struct Request {
    json: Value,
    // Sent as SCM_RIGHTS with the frame's first bytes, then closed.
    fds: Vec<OwnedFd>,
    deadline: Instant,
    reply: oneshot::Sender<Result<Value>>,
    // The request owns admission, even after its caller stops waiting.
    _permit: OwnedSemaphorePermit,
}

struct Events {
    limit: usize,
    tx: mpsc::Sender<QmpEvent>,
    dropped: Arc<AtomicU64>,
}

impl Events {
    fn publish(&self, value: Value, bytes: usize) -> Result<()> {
        if value.get("return").is_some()
            || value.get("error").is_some()
            || value.get("id").is_some()
        {
            return Err(Error::Protocol("event mixed with response fields".into()));
        }
        if value.get("data").is_some_and(|data| !data.is_object()) {
            return Err(Error::Protocol("event data must be an object".into()));
        }
        if bytes > self.limit {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        let event: QmpEvent = serde_json::from_value(value)?;
        if let Err(mpsc::error::TrySendError::Full(_)) = self.tx.try_send(event) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
        // Dropping the event receiver does not close a healthy QMP connection.
        Ok(())
    }
}

/// One task owns the socket and pending map. No shared mutable routing state,
/// reaper task or secondary event worker is needed.
///
/// Cancellation before dispatch skips the request. After dispatch it cannot
/// undo remote execution: admission stays reserved until the reply. A dispatched
/// command reaching its deadline closes the session, since its outcome and the
/// remote queue depth are then unknown. Commands are never retried automatically.
pub struct Qmp {
    greeting: Greeting,
    oob: bool,
    options: ClientOptions,
    requests: mpsc::Sender<Request>,
    normal: Arc<Semaphore>,
    out_of_band: Arc<Semaphore>,
    depth: watch::Receiver<usize>,
    closed: watch::Receiver<Option<Arc<Error>>>,
    shutdown: watch::Sender<()>,
    abort: AbortHandle,
    dropped_events: Arc<AtomicU64>,
}

impl std::fmt::Debug for Qmp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Qmp")
            .field("version", &self.version())
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl Qmp {
    pub async fn new(path: &Path) -> Result<(Self, mpsc::Receiver<QmpEvent>)> {
        Self::connect_with_options(path, ClientOptions::default()).await
    }

    pub async fn connect_with_options(
        path: &Path,
        options: ClientOptions,
    ) -> Result<(Self, mpsc::Receiver<QmpEvent>)> {
        options.validate()?;
        let stream = timeout(options.connect_timeout, UnixStream::connect(path))
            .await
            .map_err(|_| Error::Timeout("QMP connect timed out".into()))??;
        Self::from_stream_with_options(stream, options).await
    }

    /// Handshake on an already connected Unix socket (including inherited FDs).
    pub async fn from_stream(stream: UnixStream) -> Result<(Self, mpsc::Receiver<QmpEvent>)> {
        Self::from_stream_with_options(stream, ClientOptions::default()).await
    }

    pub async fn from_stream_with_options(
        stream: UnixStream,
        options: ClientOptions,
    ) -> Result<(Self, mpsc::Receiver<QmpEvent>)> {
        options.validate()?;
        let (read, mut write) = stream.into_split();
        let mut reader = JsonReader::new(read, options.max_message_bytes);
        let (event_tx, event_rx) = mpsc::channel(options.event_capacity);
        let dropped_events = Arc::new(AtomicU64::new(0));
        let events = Events {
            limit: options.max_event_bytes,
            tx: event_tx,
            dropped: dropped_events.clone(),
        };
        let (greeting, oob) = timeout(options.handshake_timeout, async {
            #[derive(Deserialize)]
            struct Envelope {
                #[serde(rename = "QMP")]
                qmp: Greeting,
            }
            let greeting = serde_json::from_value::<Envelope>(reader.next().await?)?.qmp;
            let oob = greeting.capabilities.iter().any(|cap| cap == "oob");
            let mut request = json!({"execute": "qmp_capabilities", "id": 0});
            if oob {
                request["arguments"] = json!({"enable": ["oob"]});
            }
            framing::write_json(&mut write, &request, options.max_message_bytes).await?;
            loop {
                let response = reader.next().await?;
                if response.get("event").is_some() {
                    events.publish(response, reader.last_message_bytes)?;
                    continue;
                }
                if response.get("id").and_then(Value::as_u64) != Some(0) {
                    return Err(Error::Protocol(
                        "capabilities reply has wrong or missing ID".into(),
                    ));
                }
                let value = wire::response(response)?;
                if !value.as_object().is_some_and(|o| o.is_empty()) {
                    return Err(Error::Protocol(
                        "capabilities reply must return an empty object".into(),
                    ));
                }
                return Ok((greeting, oob));
            }
        })
        .await
        .map_err(|_| Error::Timeout("QMP handshake timed out".into()))??;

        let normal = Arc::new(Semaphore::new(QMP_QUEUE_CAPACITY));
        let out_of_band = Arc::new(Semaphore::new(OOB_CAPACITY));
        let (requests, request_rx) = mpsc::channel(QMP_QUEUE_CAPACITY + OOB_CAPACITY);
        let (depth_tx, depth) = watch::channel(0);
        let (closed_tx, closed) = watch::channel(None);
        let (shutdown, shutdown_rx) = watch::channel(());
        let lifecycle = Lifecycle {
            normal: normal.clone(),
            oob: out_of_band.clone(),
            depth: depth_tx,
            closed: closed_tx,
            reason: None,
        };
        let limit = options.max_message_bytes;
        let task = tokio::spawn(async move {
            let mut lifecycle = lifecycle;
            let result = drive(
                &mut reader,
                &mut write,
                request_rx,
                shutdown_rx,
                events,
                &lifecycle.depth,
                limit,
            )
            .await;
            lifecycle.reason = Some(Arc::new(result.err().unwrap_or(Error::Closed)));
        });
        let abort = task.abort_handle();
        // Lifecycle always runs on normal exit, panic or abort; dropping this
        // JoinHandle detaches ownership, not cleanup responsibility.
        drop(task);
        Ok((
            Self {
                greeting,
                oob,
                options,
                requests,
                normal,
                out_of_band,
                depth,
                closed,
                shutdown,
                abort,
                dropped_events,
            },
            event_rx,
        ))
    }

    pub fn greeting(&self) -> &Greeting {
        &self.greeting
    }
    pub fn version(&self) -> (u32, u32, u32) {
        let v = self.greeting.version.qemu;
        (v.major, v.minor, v.micro)
    }
    pub fn oob_supported(&self) -> bool {
        self.oob
    }
    pub fn queue_capacity(&self) -> usize {
        QMP_QUEUE_CAPACITY
    }
    /// Number of dispatched requests awaiting a reply, including cancelled callers.
    pub fn queue_depth(&self) -> usize {
        *self.depth.borrow()
    }
    pub fn subscribe_queue(&self) -> watch::Receiver<usize> {
        self.depth.clone()
    }
    pub fn dropped_events(&self) -> u64 {
        self.dropped_events.load(Ordering::Relaxed)
    }
    pub fn is_closed(&self) -> bool {
        self.closed.borrow().is_some()
    }
    pub fn close_reason(&self) -> Option<Arc<Error>> {
        self.closed.borrow().clone()
    }

    /// Wait for a peer disconnect, protocol failure, timeout or local shutdown.
    pub async fn closed(&self) -> Arc<Error> {
        let mut closed = self.closed.clone();
        loop {
            if let Some(reason) = closed.borrow().clone() {
                return reason;
            }
            if closed.changed().await.is_err() {
                return Arc::new(Error::Closed);
            }
        }
    }

    /// Can be called through Arc<Qmp> while other tasks await replies.
    pub async fn shutdown(&self) {
        self.shutdown.send_replace(());
        let _ = self.closed().await;
    }

    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        self.execute_with_timeout(cmd, self.options.command_timeout)
            .await
    }

    pub async fn execute_with_timeout<C: QapiCommand>(
        &self,
        cmd: &C,
        duration: Duration,
    ) -> Result<C::Returns> {
        if !C::SUCCESS_RESPONSE {
            return Err(Error::InvalidRequest(
                "command does not send a success response".into(),
            ));
        }
        let request = qapi::with_version(self.version(), || {
            wire::request(C::NAME, serde_json::to_value(cmd)?, false)
        })?;
        wire::decode(self.dispatch(request, Vec::new(), false, duration).await?)
    }

    pub async fn execute_oob<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        if !C::ALLOW_OOB {
            return Err(Error::InvalidRequest(format!(
                "{} does not allow OOB execution",
                C::NAME
            )));
        }
        if !C::SUCCESS_RESPONSE {
            return Err(Error::InvalidRequest(
                "command does not send a success response".into(),
            ));
        }
        let request = qapi::with_version(self.version(), || {
            wire::request(C::NAME, serde_json::to_value(cmd)?, true)
        })?;
        wire::decode(
            self.dispatch(request, Vec::new(), true, self.options.command_timeout)
                .await?,
        )
    }

    /// Raw envelopes contain execute and optional object arguments. Caller IDs
    /// and the opposite execution verb are rejected before any socket I/O.
    pub async fn execute_raw(&self, request: Value) -> Result<Value> {
        self.execute_raw_with_timeout(request, self.options.command_timeout)
            .await
    }
    pub async fn execute_raw_with_timeout(
        &self,
        request: Value,
        duration: Duration,
    ) -> Result<Value> {
        self.dispatch(request, Vec::new(), false, duration).await
    }
    pub async fn execute_raw_oob(&self, request: Value) -> Result<Value> {
        self.dispatch(request, Vec::new(), true, self.options.command_timeout)
            .await
    }

    /// Run an in-band raw `request` with `fds` attached to it as SCM_RIGHTS,
    /// as QMP's `getfd` and `add-fd` expect. The descriptors are moved in and
    /// closed here once sent, or when the request is refused or expires.
    pub async fn execute_raw_with_fds(&self, request: Value, fds: Vec<OwnedFd>) -> Result<Value> {
        self.dispatch(request, fds, false, self.options.command_timeout)
            .await
    }

    /// `getfd`: hand QEMU `fd` under `name` for a later `netdev_add`
    /// (`fds=name:...`) or `device_add`.
    pub async fn getfd(&self, name: &str, fd: OwnedFd) -> Result<()> {
        self.execute_raw_with_fds(
            json!({"execute":"getfd","arguments":{"fdname":name}}),
            vec![fd],
        )
        .await
        .map(drop)
    }

    fn disconnected(&self) -> Error {
        Error::Disconnected(
            self.close_reason()
                .unwrap_or_else(|| Arc::new(Error::Closed)),
        )
    }

    async fn dispatch(
        &self,
        request: Value,
        fds: Vec<OwnedFd>,
        oob: bool,
        duration: Duration,
    ) -> Result<Value> {
        wire::validate_request(&request, oob)?;
        framing::encode(&request, self.options.max_message_bytes)?;
        if oob && !self.oob {
            return Err(Error::InvalidRequest("OOB was not negotiated".into()));
        }
        if self.is_closed() {
            return Err(self.disconnected());
        }
        let deadline = Instant::now()
            .checked_add(duration)
            .ok_or_else(|| Error::InvalidRequest("timeout is too large".into()))?;
        let operation = async {
            let permits = if oob { &self.out_of_band } else { &self.normal };
            let permit = permits
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| self.disconnected())?;
            let (reply, receive) = oneshot::channel();
            self.requests
                .send(Request {
                    json: request,
                    fds,
                    deadline,
                    reply,
                    _permit: permit,
                })
                .await
                .map_err(|_| self.disconnected())?;
            match receive.await {
                Ok(result) => result,
                Err(_) => Err(Error::Disconnected(self.closed().await)),
            }
        };
        timeout_at(deadline, operation).await.map_err(|_| {
            Error::Timeout("QMP command deadline elapsed (execution may have occurred)".into())
        })?
    }
}

impl Drop for Qmp {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

struct Lifecycle {
    normal: Arc<Semaphore>,
    oob: Arc<Semaphore>,
    depth: watch::Sender<usize>,
    closed: watch::Sender<Option<Arc<Error>>>,
    reason: Option<Arc<Error>>,
}
impl Drop for Lifecycle {
    fn drop(&mut self) {
        self.closed.send_replace(Some(
            self.reason
                .clone()
                .unwrap_or_else(|| Arc::new(Error::Closed)),
        ));
        self.normal.close();
        self.oob.close();
        self.depth.send_replace(0);
    }
}

struct Pending {
    deadline: Instant,
    reply: oneshot::Sender<Result<Value>>,
    _permit: OwnedSemaphorePermit,
}

struct Writing {
    id: u64,
    bytes: Vec<u8>,
    offset: usize,
    fds: Vec<OwnedFd>,
}

/// One `sendmsg` carrying `fds` with (a prefix of) `bytes`. Like `write()`
/// it is cancellation safe: nothing is sent until it returns.
async fn send_with_fds(
    stream: &UnixStream,
    bytes: &[u8],
    fds: &[OwnedFd],
) -> std::io::Result<usize> {
    use nix::sys::socket::{sendmsg, ControlMessage, MsgFlags};
    let raw: Vec<RawFd> = fds.iter().map(AsRawFd::as_raw_fd).collect();
    stream
        .async_io(tokio::io::Interest::WRITABLE, || {
            let iov = [std::io::IoSlice::new(bytes)];
            let cmsg = [ControlMessage::ScmRights(&raw)];
            sendmsg::<()>(
                stream.as_raw_fd(),
                &iov,
                &cmsg,
                MsgFlags::MSG_NOSIGNAL,
                None,
            )
            .map_err(std::io::Error::from)
        })
        .await
}

async fn drive(
    reader: &mut JsonReader<tokio::net::unix::OwnedReadHalf>,
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    mut requests: mpsc::Receiver<Request>,
    mut shutdown: watch::Receiver<()>,
    events: Events,
    depth: &watch::Sender<usize>,
    limit: usize,
) -> Result<()> {
    let mut pending: HashMap<u64, Pending> = HashMap::new();
    let mut writing: Option<Writing> = None;
    let mut id = 0_u64;
    loop {
        let deadline = pending.values().map(|request| request.deadline).min();
        let expire = async {
            match deadline {
                Some(deadline) => tokio::time::sleep_until(deadline).await,
                None => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            _ = shutdown.changed() => return Ok(()),
            _ = expire => return Err(Error::Timeout("dispatched QMP command expired; remote outcome unknown".into())),
            message = reader.next() => {
                let value = message?;
                if value.get("event").is_some() { events.publish(value, reader.last_message_bytes)?; continue; }
                if value.get("id").is_none() {
                    // QEMU may report a parser error before reading a request
                    // ID. It cannot be correlated, so close and retain its cause.
                    return match wire::response(value) {
                        Err(error @ Error::Qmp(_)) => Err(error),
                        _ => Err(Error::Protocol("QMP response has no ID".into())),
                    };
                }
                // QMP IDs can be arbitrary JSON. Anything that is not one of
                // our numeric IDs is unknown, not a malformed protocol value.
                let Some(response_id) = value.get("id").and_then(Value::as_u64) else { continue; };
                if !pending.contains_key(&response_id) { continue; }
                if writing.as_ref().is_some_and(|w| w.id == response_id) {
                    return Err(Error::Protocol("response preceded completion of its request".into()));
                }
                let response = wire::response(value);
                if !matches!(response, Ok(_) | Err(Error::Qmp(_))) { return response.map(|_| ()); }
                let request = pending.remove(&response_id).unwrap();
                depth.send_replace(pending.len());
                let _ = request.reply.send(response);
            }
            // write(), unlike write_all(), is cancellation safe. Retaining its
            // offset lets reads, deadlines and shutdown progress under socket
            // backpressure without losing or repeating bytes in the frame.
            written = async {
                let frame = writing.as_ref().unwrap();
                if frame.fds.is_empty() {
                    writer.write(&frame.bytes[frame.offset..]).await
                } else {
                    send_with_fds(writer.as_ref(), &frame.bytes[frame.offset..], &frame.fds).await
                }
            }, if writing.is_some() => {
                let n = written?;
                if n == 0 { return Err(std::io::Error::from(std::io::ErrorKind::WriteZero).into()); }
                let frame = writing.as_mut().unwrap();
                // The descriptors went with these bytes; QEMU holds its own copies.
                frame.fds.clear();
                frame.offset += n;
                if frame.offset == frame.bytes.len() { writing = None; }
            }
            request = requests.recv(), if writing.is_none() => {
                let Some(Request { mut json, fds, deadline: request_deadline, reply, _permit }) = request else { return Ok(()); };
                if reply.is_closed() { continue; }
                if deadline.is_some_and(|d| d <= Instant::now()) {
                    return Err(Error::Timeout("dispatched QMP command expired; remote outcome unknown".into()));
                }
                if request_deadline <= Instant::now() {
                    let _ = reply.send(Err(Error::Timeout("QMP deadline elapsed before dispatch".into())));
                    continue;
                }
                id = id.checked_add(1).ok_or_else(|| Error::Protocol("QMP request ID space exhausted".into()))?;
                json["id"] = json!(id);
                let bytes = match framing::encode(&json, limit) {
                    Ok(bytes) => bytes,
                    Err(error) => { let _ = reply.send(Err(error)); continue; }
                };
                // Keep only the encoded frame while writing, not a second copy
                // of potentially large arguments throughout remote execution.
                drop(json);
                pending.insert(id, Pending { deadline: request_deadline, reply, _permit });
                writing = Some(Writing { id, bytes, offset: 0, fds });
                depth.send_replace(pending.len());
            }
        }
    }
}
