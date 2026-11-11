//! Versioned newline-delimited JSON transport over [`Control`] on a private
//! Unix socket, for a BMC or Redfish sidecar sharing the pod's runtime volume.
//!
//! On connect the server sends a [`Greeting`]. Each request is one JSON line
//! ([`Request`]); each reply is one [`Response`] line echoing `id`. A `follow`
//! request turns the connection into a stream of event frames, and a `console`
//! request turns it into the raw VNC byte stream. Neither QMP nor a VNC socket
//! path is ever exposed.
use super::{Action, Config, ConfigView, Control, EventPage, Status, VmEvent};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::Semaphore,
    task::JoinSet,
};

pub const PROTOCOL: &str = "qrab-control";
pub const VERSION: u32 = 1;
/// Largest request line, excluding the newline.
pub const MAX_REQUEST: usize = 64 * 1024;
/// Largest response line: a full event page plus envelope fits comfortably.
pub const MAX_RESPONSE: usize = 4 * 1024 * 1024;
pub const MAX_CONNECTIONS: usize = 8;
pub const MAX_CONSOLES: usize = 4;
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const OPERATIONS: &[&str] = &["status", "config", "events", "follow", "action", "console"];

/// `{runtime_dir}/control/control.sock`: a 0700 directory and 0600 socket.
pub fn socket_path(config: &Config) -> PathBuf {
    config.runtime_dir.join("control").join("control.sock")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greeting {
    pub protocol: String,
    pub version: u32,
    pub vm: String,
    pub operations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(flatten)]
    pub op: Operation,
}

fn page_limit() -> usize {
    super::events::EVENT_PAGE_LIMIT
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    Status,
    Config {
        #[serde(default)]
        redacted: bool,
    },
    Events {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<u64>,
        #[serde(default = "page_limit")]
        limit: usize,
    },
    Follow {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<u64>,
    },
    Action {
        action: Action,
    },
    Console,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    UnsupportedVersion,
    FrameTooLarge,
    Busy,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
}

/// Exactly one of `result`, `error`, `event` or `missed` is present.
/// `event`/`missed` frames only follow an accepted `follow` request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Response {
    pub v: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<VmEvent>,
    /// Events evicted before a follower could read them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missed: Option<u64>,
}

impl Response {
    fn ok(id: Option<u64>, result: impl Serialize) -> Self {
        Self {
            v: VERSION,
            id,
            result: Some(serde_json::to_value(result).unwrap_or(Value::Null)),
            ..Self::default()
        }
    }
    fn error(id: Option<u64>, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            v: VERSION,
            id,
            error: Some(ErrorBody {
                code,
                message: message.into(),
            }),
            ..Self::default()
        }
    }
}

fn frame(value: &impl Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).expect("control frames serialize");
    bytes.push(b'\n');
    bytes
}

/// Create the private directory and bind the socket. The caller must hold the
/// runtime owner lock, which makes removing a stale socket safe.
fn bind(config: &Config) -> Result<UnixListener> {
    let path = socket_path(config);
    if path.as_os_str().as_encoded_bytes().len() > 107 {
        return Err(Error::Config(
            "runtime directory is too long for the control socket".into(),
        ));
    }
    let directory = path.parent().expect("socket has a parent");
    match std::fs::symlink_metadata(directory) {
        Ok(meta) if meta.is_dir() && meta.uid() == nix::unistd::geteuid().as_raw() => {}
        Ok(_) => {
            return Err(Error::Config(
                "control socket directory is not a directory owned by the manager".into(),
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::DirBuilder::new().mode(0o700).create(directory)?
        }
        Err(error) => return Err(error.into()),
    }
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
    match std::fs::remove_file(&path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Owns the socket server task; dropping it aborts every connection and
/// removes the socket.
pub(super) struct Server {
    task: tokio::task::AbortHandle,
    path: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Bind and serve the control socket for one supervisor run.
pub(super) fn spawn(config: &Config, control: Control) -> Result<Server> {
    let listener = bind(config)?;
    Ok(Server {
        task: tokio::spawn(serve(listener, control, config.name.clone())).abort_handle(),
        path: socket_path(config),
    })
}

/// Serve until aborted. Connection tasks are owned and abort with this task.
async fn serve(listener: UnixListener, control: Control, vm: String) {
    let consoles = Arc::new(Semaphore::new(MAX_CONSOLES));
    let greeting = Arc::new(frame(&Greeting {
        protocol: PROTOCOL.into(),
        version: VERSION,
        vm,
        operations: OPERATIONS.iter().map(|s| s.to_string()).collect(),
    }));
    let uid = nix::unistd::geteuid().as_raw();
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            biased;
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
            accepted = listener.accept() => match accepted {
                Ok((mut stream, _)) => {
                    if stream.peer_cred().map(|cred| cred.uid()).ok() != Some(uid) {
                        continue;
                    }
                    while connections.try_join_next().is_some() {}
                    if connections.len() >= MAX_CONNECTIONS {
                        // A fresh socket accepts a small frame at once; never stall accept.
                        let busy = frame(&Response::error(None, ErrorCode::Busy, "too many control connections"));
                        let _ = tokio::time::timeout(Duration::from_millis(50), stream.write_all(&busy)).await;
                        continue;
                    }
                    connections.spawn(connection(stream, control.clone(), consoles.clone(), greeting.clone()));
                }
                // Descriptor exhaustion and similar errors are transient.
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            },
        }
    }
}

async fn write(stream: &mut BufReader<UnixStream>, bytes: &[u8]) -> bool {
    matches!(
        tokio::time::timeout(WRITE_TIMEOUT, stream.get_mut().write_all(bytes)).await,
        Ok(Ok(()))
    )
}

enum Line {
    Frame(Vec<u8>),
    TooLarge,
    Closed,
}

async fn read_line(stream: &mut BufReader<UnixStream>, max: usize, idle: Option<Duration>) -> Line {
    let mut line = Vec::new();
    let limit = (max + 1) as u64;
    let mut bounded = (&mut *stream).take(limit);
    let read = bounded.read_until(b'\n', &mut line);
    let read = match idle {
        Some(idle) => tokio::time::timeout(idle, read).await,
        None => Ok(read.await),
    };
    match read {
        Ok(Ok(0)) | Ok(Err(_)) | Err(_) => Line::Closed,
        Ok(Ok(_)) if line.last() == Some(&b'\n') => {
            line.pop();
            Line::Frame(line)
        }
        Ok(Ok(_)) if line.len() as u64 >= limit => Line::TooLarge,
        // EOF without a terminator.
        Ok(Ok(_)) => Line::Closed,
    }
}

async fn connection(
    stream: UnixStream,
    control: Control,
    consoles: Arc<Semaphore>,
    greeting: Arc<Vec<u8>>,
) {
    let mut stream = BufReader::new(stream);
    if !write(&mut stream, &greeting).await {
        return;
    }
    loop {
        let line = match read_line(&mut stream, MAX_REQUEST, Some(IDLE_TIMEOUT)).await {
            Line::Frame(line) => line,
            Line::TooLarge => {
                let reply = Response::error(None, ErrorCode::FrameTooLarge, "request too large");
                let _ = write(&mut stream, &frame(&reply)).await;
                return;
            }
            Line::Closed => return,
        };
        let request = match serde_json::from_slice::<Value>(&line) {
            Ok(value) => {
                let id = value.get("id").and_then(Value::as_u64);
                match value.get("v").and_then(Value::as_u64) {
                    Some(v) if v == u64::from(VERSION) => serde_json::from_value::<Request>(value)
                        .map_err(|e| Response::error(id, ErrorCode::BadRequest, e.to_string())),
                    _ => Err(Response::error(
                        id,
                        ErrorCode::UnsupportedVersion,
                        format!("this manager speaks {PROTOCOL} version {VERSION}"),
                    )),
                }
            }
            Err(error) => Err(Response::error(
                None,
                ErrorCode::BadRequest,
                error.to_string(),
            )),
        };
        let request = match request {
            Ok(request) => request,
            Err(reply) => {
                if write(&mut stream, &frame(&reply)).await {
                    continue;
                }
                return;
            }
        };
        let id = request.id;
        let unavailable =
            |error: Error| Response::error(id, ErrorCode::Unavailable, error.to_string());
        let reply = match request.op {
            Operation::Status => control
                .status()
                .map_or_else(unavailable, |s| Response::ok(id, s)),
            Operation::Config { redacted } => {
                control.config_view().map_or_else(unavailable, |view| {
                    Response::ok(id, if redacted { view.redacted() } else { view })
                })
            }
            Operation::Events { after, limit } => Response::ok(id, control.events(after, limit)),
            Operation::Action { action } => control
                .action(action)
                .await
                .map_or_else(unavailable, |()| Response::ok(id, Value::Null)),
            Operation::Follow { after } => {
                let cursor = after.unwrap_or_else(|| control.events(None, 1).latest);
                if write(
                    &mut stream,
                    &frame(&Response::ok(id, serde_json::json!({"after":cursor}))),
                )
                .await
                {
                    follow(stream, &control, cursor).await;
                }
                return;
            }
            Operation::Console => {
                let Ok(_permit) = consoles.try_acquire() else {
                    let reply = Response::error(id, ErrorCode::Busy, "too many console streams");
                    if write(&mut stream, &frame(&reply)).await {
                        continue;
                    }
                    return;
                };
                match control.console().await {
                    Ok(mut vnc) => {
                        if write(&mut stream, &frame(&Response::ok(id, Value::Null))).await {
                            let _ = tokio::io::copy_bidirectional(&mut stream, &mut vnc).await;
                        }
                        return;
                    }
                    Err(error) => unavailable(error),
                }
            }
        };
        if !write(&mut stream, &frame(&reply)).await {
            return;
        }
    }
}

async fn follow(mut stream: BufReader<UnixStream>, control: &Control, mut cursor: u64) {
    let mut probe = [0u8; 1];
    loop {
        let page = control.events(Some(cursor), 64);
        if page.missed > 0 {
            let missed = Response {
                v: VERSION,
                missed: Some(page.missed),
                ..Response::default()
            };
            if !write(&mut stream, &frame(&missed)).await {
                return;
            }
        }
        cursor = page.next;
        for event in page.events {
            let event = Response {
                v: VERSION,
                event: Some(event),
                ..Response::default()
            };
            if !write(&mut stream, &frame(&event)).await {
                return;
            }
        }
        tokio::select! {
            _ = control.wait_events(cursor) => {}
            // The follower sends nothing; input or EOF ends the stream.
            _ = stream.read(&mut probe) => return,
        }
    }
}

/// A typed client for the control socket.
pub struct Client {
    stream: BufReader<UnixStream>,
    greeting: Greeting,
    next_id: u64,
}

impl Client {
    pub async fn connect(path: &Path) -> Result<Self> {
        let stream = tokio::time::timeout(WRITE_TIMEOUT, UnixStream::connect(path))
            .await
            .map_err(|_| Error::Spawn("control socket connect timed out".into()))??;
        let mut stream = BufReader::new(stream);
        let greeting: Greeting = Self::read_frame(&mut stream).await?;
        if greeting.protocol != PROTOCOL || greeting.version != VERSION {
            return Err(Error::Spawn(format!(
                "unsupported control protocol {} {}",
                greeting.protocol, greeting.version
            )));
        }
        Ok(Self {
            stream,
            greeting,
            next_id: 0,
        })
    }

    pub fn greeting(&self) -> &Greeting {
        &self.greeting
    }

    async fn read_frame<T: serde::de::DeserializeOwned>(
        stream: &mut BufReader<UnixStream>,
    ) -> Result<T> {
        match read_line(stream, MAX_RESPONSE, Some(IDLE_TIMEOUT)).await {
            Line::Frame(line) => Ok(serde_json::from_slice(&line)?),
            Line::TooLarge => Err(Error::Spawn("control frame too large".into())),
            Line::Closed => Err(Error::Spawn("control socket closed".into())),
        }
    }

    async fn call(&mut self, op: Operation) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let request = frame(&Request {
            v: VERSION,
            id: Some(id),
            op,
        });
        if !write(&mut self.stream, &request).await {
            return Err(Error::Spawn("control socket write failed".into()));
        }
        let response: Response = Self::read_frame(&mut self.stream).await?;
        // `"result": null` deserializes as `None`.
        match (response.id, response.result, response.error) {
            (Some(got), result, None) if got == id => Ok(result.unwrap_or(Value::Null)),
            (_, _, Some(error)) => Err(Error::Spawn(format!(
                "control request failed ({:?}): {}",
                error.code, error.message
            ))),
            _ => Err(Error::Spawn("malformed control response".into())),
        }
    }

    pub async fn status(&mut self) -> Result<Status> {
        Ok(serde_json::from_value(self.call(Operation::Status).await?)?)
    }

    pub async fn config(&mut self, redacted: bool) -> Result<ConfigView> {
        Ok(serde_json::from_value(
            self.call(Operation::Config { redacted }).await?,
        )?)
    }

    pub async fn events(&mut self, after: Option<u64>, limit: usize) -> Result<EventPage> {
        Ok(serde_json::from_value(
            self.call(Operation::Events { after, limit }).await?,
        )?)
    }

    pub async fn action(&mut self, action: Action) -> Result<()> {
        self.call(Operation::Action { action }).await.map(|_| ())
    }

    /// Consume the connection as an event stream starting after `after`
    /// (the current latest event when `None`).
    pub async fn follow(mut self, after: Option<u64>) -> Result<Follower> {
        self.call(Operation::Follow { after }).await?;
        Ok(Follower(self.stream))
    }

    /// Consume the connection as the VM's raw VNC (RFB) byte stream.
    pub async fn console(mut self) -> Result<BufReader<UnixStream>> {
        self.call(Operation::Console).await?;
        Ok(self.stream)
    }
}

pub struct Follower(BufReader<UnixStream>);

/// One frame of a followed event stream.
#[derive(Debug, Clone, PartialEq)]
pub enum FollowFrame {
    Event(VmEvent),
    Missed(u64),
}

impl Follower {
    /// The next frame. Waits indefinitely; wrap in a timeout when needed.
    pub async fn next(&mut self) -> Result<FollowFrame> {
        let line = match read_line(&mut self.0, MAX_RESPONSE, None).await {
            Line::Frame(line) => line,
            Line::TooLarge => return Err(Error::Spawn("control frame too large".into())),
            Line::Closed => return Err(Error::Spawn("control socket closed".into())),
        };
        let response: Response = serde_json::from_slice(&line)?;
        match (response.event, response.missed) {
            (Some(event), None) => Ok(FollowFrame::Event(event)),
            (None, Some(missed)) => Ok(FollowFrame::Missed(missed)),
            _ => Err(Error::Spawn("malformed follow frame".into())),
        }
    }
}
