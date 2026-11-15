//! Versioned newline-delimited JSON transport over [`Control`] on a private
//! Unix socket, for a BMC or Redfish sidecar sharing the pod's runtime volume.
//!
//! On connect the server sends a [`Greeting`]. Each request is one JSON line
//! ([`Request`]); each reply is one [`Response`] line echoing `id`. A `follow`
//! request turns the connection into a stream of event frames, and a `console`
//! request turns it into the raw VNC byte stream. Neither QMP nor a VNC socket
//! path is ever exposed. A client that closes the connection while a
//! [read-only](Operation::is_read_only) request is pending abandons it, which
//! frees the connection at once; every other request runs to its outcome.
use super::{
    Action, AgentStatus, Config, ConfigView, Control, EventPage, FreezeState, Status, UpdateStatus,
    VmEvent,
};
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
pub const MAX_REQUEST: usize = 384 * 1024;
/// Largest response line: a full event page plus envelope fits comfortably.
pub const MAX_RESPONSE: usize = 4 * 1024 * 1024;
pub const MAX_CONNECTIONS: usize = 16;
pub const MAX_CONSOLES: usize = 4;
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const OPERATIONS: &[&str] = &[
    "status",
    "metrics",
    "inventory",
    "resource_usage",
    "serial",
    "agent_network",
    "agent_firmware",
    "agent_secure_boot_databases",
    "agent_secure_boot_update",
    "secure_boot_settings",
    "stage_secure_boot",
    "agent_boot",
    "agent_boot_next",
    "firmware_catalog",
    "firmware_stage_status",
    "firmware_stage",
    "media",
    "probe",
    "config",
    "events",
    "follow",
    "action",
    "console",
    "agent_ping",
    "agent_status",
    "agent_freeze_status",
    "agent_freeze",
    "agent_thaw",
    "agent_update_trigger",
    "agent_update_status",
];

/// `{control_dir}/control.sock`; `control_dir` defaults to
/// `{runtime_dir}/control`. The manager creates the directory 0700 and the
/// socket 0600.
pub fn socket_path(config: &Config) -> PathBuf {
    config
        .control_dir
        .clone()
        .unwrap_or_else(|| config.runtime_dir.join("control"))
        .join("control.sock")
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
    Metrics,
    Inventory,
    ResourceUsage,
    Serial,
    AgentNetwork,
    AgentFirmware,
    AgentSecureBootDatabases,
    AgentSecureBootUpdate {
        request: super::SecureBootUpdateRequest,
    },
    SecureBootSettings,
    StageSecureBoot {
        enabled: bool,
    },
    AgentBoot,
    AgentBootNext {
        reference: Option<String>,
    },
    FirmwareCatalog,
    FirmwareStageStatus,
    FirmwareStage {
        artifact: String,
    },
    Media {
        operation: super::MediaOperation,
    },
    Status,
    /// Liveness (`ready: false`, paused allowed) or readiness (`ready: true`).
    Probe {
        #[serde(default)]
        ready: bool,
    },
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
    /// Cached guest-agent connection state; never waits on the guest.
    AgentStatus,
    /// A guest round trip proving the agent answers.
    AgentPing,
    AgentFreezeStatus,
    /// Freeze all guest filesystems, or only `mountpoints`.
    AgentFreeze {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mountpoints: Option<Vec<String>>,
    },
    AgentThaw,
    /// Start the guest's fixed update hook with bounded token arguments.
    AgentUpdateTrigger {
        #[serde(default)]
        args: Vec<String>,
    },
    AgentUpdateStatus,
}

impl Operation {
    /// Operations without side effects. The manager abandons one of these when
    /// its client disconnects; every other operation runs to its outcome.
    pub fn is_read_only(&self) -> bool {
        match self {
            Self::Metrics
            | Self::Inventory
            | Self::ResourceUsage
            | Self::Serial
            | Self::AgentNetwork
            | Self::AgentFirmware
            | Self::AgentSecureBootDatabases
            | Self::SecureBootSettings
            | Self::AgentBoot
            | Self::FirmwareCatalog
            | Self::FirmwareStageStatus
            | Self::Status
            | Self::Probe { .. }
            | Self::Config { .. }
            | Self::Events { .. }
            | Self::AgentStatus
            | Self::AgentPing
            | Self::AgentFreezeStatus
            | Self::AgentUpdateStatus => true,
            Self::Media { operation } => *operation == super::MediaOperation::Status,
            Self::AgentSecureBootUpdate { .. }
            | Self::StageSecureBoot { .. }
            | Self::AgentBootNext { .. }
            | Self::FirmwareStage { .. }
            | Self::Follow { .. }
            | Self::Action { .. }
            | Self::Console
            | Self::AgentFreeze { .. }
            | Self::AgentThaw
            | Self::AgentUpdateTrigger { .. } => false,
        }
    }
}

/// The `agent_*` operations; each maps to one `Control::agent_*` method and
/// the update hook path stays fixed inside the guest.
enum AgentCommand {
    Status,
    Ping,
    FreezeStatus,
    Freeze { mountpoints: Option<Vec<String>> },
    Thaw,
    UpdateTrigger { args: Vec<String> },
    UpdateStatus,
}

/// One reply shape and error mapping for every `agent_*` operation.
async fn agent_reply(control: &Control, id: Option<u64>, command: AgentCommand) -> Response {
    match agent(control, command).await {
        Ok(value) => Response::ok(id, value),
        Err(error) => error_reply(id, error),
    }
}

fn error_reply(id: Option<u64>, error: Error) -> Response {
    let code = match error {
        Error::ShuttingDown => ErrorCode::ShuttingDown,
        Error::PoweredOff => ErrorCode::PoweredOff,
        // A request the manager refused on its merits, e.g. update arguments.
        Error::Config(_) => ErrorCode::BadRequest,
        _ => ErrorCode::Unavailable,
    };
    Response::error(id, code, error.to_string())
}

async fn agent(control: &Control, command: AgentCommand) -> Result<Value> {
    Ok(match command {
        AgentCommand::Status => serde_json::to_value(control.agent_status().await?)?,
        AgentCommand::Ping => {
            control.agent_ping().await?;
            Value::Null
        }
        AgentCommand::FreezeStatus => serde_json::to_value(control.agent_freeze_status().await?)?,
        AgentCommand::Freeze { mountpoints } => control.agent_freeze(mountpoints).await?.into(),
        AgentCommand::Thaw => control.agent_thaw().await?.into(),
        AgentCommand::UpdateTrigger { args } => {
            serde_json::to_value(control.agent_update_trigger(args).await?)?
        }
        AgentCommand::UpdateStatus => serde_json::to_value(control.agent_update_status().await?)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadRequest,
    UnsupportedVersion,
    FrameTooLarge,
    Busy,
    Unavailable,
    /// The supervisor is stopping the VM ([`Error::ShuttingDown`]).
    ShuttingDown,
    /// QEMU is powered off ([`Error::PoweredOff`]).
    PoweredOff,
    /// A code from a newer manager.
    #[serde(other)]
    Other,
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
    /// The QMP client's new dropped-event count ([`EventPage::qmp_dropped`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qmp_dropped: Option<u64>,
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
        let reply = match request.op {
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
                    Err(error) => error_reply(id, error),
                }
            }
            // A client that goes away abandons its read; actions run to their
            // outcome whether or not anyone is still waiting for it.
            op if op.is_read_only() => tokio::select! {
                reply = respond(&control, id, op) => reply,
                () = peer_gone(&mut stream) => return,
            },
            op => respond(&control, id, op).await,
        };
        if !write(&mut stream, &frame(&reply)).await {
            return;
        }
    }
}

/// The reply to every operation that does not take over the connection.
async fn respond(control: &Control, id: Option<u64>, op: Operation) -> Response {
    let unavailable = |error: Error| error_reply(id, error);
    match op {
        Operation::AgentSecureBootDatabases => control
            .agent_secure_boot_databases()
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::AgentSecureBootUpdate { request } => control
            .agent_secure_boot_update(request)
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::SecureBootSettings => control
            .secure_boot_settings()
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::StageSecureBoot { enabled } => control
            .stage_secure_boot(enabled)
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::AgentBoot => control
            .agent_boot()
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::AgentBootNext { reference } => control
            .agent_boot_next(reference)
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::FirmwareCatalog => control
            .firmware_catalog()
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::FirmwareStageStatus => control
            .firmware_stage_status()
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::FirmwareStage { artifact } => control
            .firmware_stage(artifact)
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::Serial => Response::ok(id, control.serial()),
        Operation::Media { operation } => control
            .media(operation)
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::AgentFirmware => control
            .agent_firmware()
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::ResourceUsage => Response::ok(id, control.resource_usage()),
        Operation::Inventory => control
            .inventory()
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::AgentNetwork => control
            .agent_network()
            .await
            .map_or_else(unavailable, |v| Response::ok(id, v)),
        Operation::Metrics => control
            .metrics()
            .await
            .map_or_else(unavailable, |m| Response::ok(id, m)),
        Operation::Status => control
            .status()
            .map_or_else(unavailable, |s| Response::ok(id, s)),
        Operation::Probe { ready } => control
            .probe(ready)
            .map_or_else(unavailable, |()| Response::ok(id, Value::Null)),
        Operation::Config { redacted } => control.config_view().map_or_else(unavailable, |view| {
            Response::ok(id, if redacted { view.redacted() } else { view })
        }),
        Operation::Events { after, limit } => Response::ok(id, control.events(after, limit)),
        Operation::Action { action } => control
            .action(action)
            .await
            .map_or_else(unavailable, |()| Response::ok(id, Value::Null)),
        Operation::AgentStatus => agent_reply(control, id, AgentCommand::Status).await,
        Operation::AgentPing => agent_reply(control, id, AgentCommand::Ping).await,
        Operation::AgentFreezeStatus => agent_reply(control, id, AgentCommand::FreezeStatus).await,
        Operation::AgentFreeze { mountpoints } => {
            agent_reply(control, id, AgentCommand::Freeze { mountpoints }).await
        }
        Operation::AgentThaw => agent_reply(control, id, AgentCommand::Thaw).await,
        Operation::AgentUpdateTrigger { args } => {
            agent_reply(control, id, AgentCommand::UpdateTrigger { args }).await
        }
        Operation::AgentUpdateStatus => agent_reply(control, id, AgentCommand::UpdateStatus).await,
        Operation::Follow { .. } | Operation::Console => Response::error(
            id,
            ErrorCode::BadRequest,
            "stream operations take over the connection",
        ),
    }
}

/// Resolves once the client has closed the connection (or it failed) while a
/// request runs. Bytes a client sends meanwhile stay buffered for the next
/// request, and the connection is then no longer watched: sidecars never
/// pipeline, so only a closed client reaches this.
async fn peer_gone(stream: &mut BufReader<UnixStream>) {
    match stream.fill_buf().await {
        Ok([]) | Err(_) => {}
        Ok(_) => std::future::pending().await,
    }
}

async fn follow(mut stream: BufReader<UnixStream>, control: &Control, mut cursor: u64) {
    let mut probe = [0u8; 1];
    let mut dropped = 0;
    loop {
        let page = control.events(Some(cursor), 64);
        if page.qmp_dropped != dropped {
            dropped = page.qmp_dropped;
            let frame_ = Response {
                v: VERSION,
                qmp_dropped: Some(dropped),
                ..Response::default()
            };
            if !write(&mut stream, &frame(&frame_)).await {
                return;
            }
        }
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
            _ = control.wait_qmp_dropped(dropped) => {}
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

    async fn send(&mut self, op: Operation) -> Result<u64> {
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
        Ok(id)
    }

    /// Typed JSON-only operations for a UID-authorized local operational client.
    pub async fn request_json(&mut self, op: Operation) -> Result<Value> {
        if matches!(op, Operation::Console | Operation::Follow { .. }) {
            return Err(Error::Config(
                "stream operations are not JSON requests".into(),
            ));
        }
        self.call(op).await
    }
    async fn call(&mut self, op: Operation) -> Result<Value> {
        let id = self.send(op).await?;
        let response: Response = Self::read_frame(&mut self.stream).await?;
        Self::result(id, response)
    }

    fn result(id: u64, response: Response) -> Result<Value> {
        // `"result": null` deserializes as `None`.
        match (response.id, response.result, response.error) {
            (Some(got), result, None) if got == id => Ok(result.unwrap_or(Value::Null)),
            (_, _, Some(error)) if error.code == ErrorCode::ShuttingDown => {
                Err(Error::ShuttingDown)
            }
            (_, _, Some(error)) if error.code == ErrorCode::PoweredOff => Err(Error::PoweredOff),
            (_, _, Some(error)) if error.code == ErrorCode::BadRequest => {
                Err(Error::Config(error.message))
            }
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

    pub async fn probe(&mut self, ready: bool) -> Result<()> {
        self.call(Operation::Probe { ready }).await.map(|_| ())
    }

    pub async fn resource_usage(&mut self) -> Result<super::ResourceUsage> {
        Ok(serde_json::from_value(
            self.call(Operation::ResourceUsage).await?,
        )?)
    }
    pub async fn agent_firmware(&mut self) -> Result<super::FirmwareSnapshot> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentFirmware).await?,
        )?)
    }
    pub async fn media(&mut self, operation: super::MediaOperation) -> Result<super::MediaStatus> {
        Ok(serde_json::from_value(
            self.call(Operation::Media { operation }).await?,
        )?)
    }
    pub async fn serial(&mut self) -> Result<super::SerialSnapshot> {
        Ok(serde_json::from_value(self.call(Operation::Serial).await?)?)
    }
    pub async fn firmware_catalog(&mut self) -> Result<super::FirmwareCatalogView> {
        Ok(serde_json::from_value(
            self.call(Operation::FirmwareCatalog).await?,
        )?)
    }
    pub async fn firmware_stage_status(&mut self) -> Result<super::FirmwareStageStatus> {
        Ok(serde_json::from_value(
            self.call(Operation::FirmwareStageStatus).await?,
        )?)
    }
    pub async fn firmware_stage(&mut self, artifact: String) -> Result<super::FirmwareStageStatus> {
        Ok(serde_json::from_value(
            self.call(Operation::FirmwareStage { artifact }).await?,
        )?)
    }
    pub async fn agent_boot(&mut self) -> Result<super::BootSnapshot> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentBoot).await?,
        )?)
    }
    pub async fn agent_boot_next(
        &mut self,
        reference: Option<String>,
    ) -> Result<super::BootSnapshot> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentBootNext { reference }).await?,
        )?)
    }
    pub async fn agent_secure_boot_databases(
        &mut self,
    ) -> Result<super::SecureBootDatabaseSnapshot> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentSecureBootDatabases).await?,
        )?)
    }
    pub async fn agent_secure_boot_update(
        &mut self,
        request: super::SecureBootUpdateRequest,
    ) -> Result<super::SecureBootDatabaseSnapshot> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentSecureBootUpdate { request })
                .await?,
        )?)
    }
    pub async fn secure_boot_settings(&mut self) -> Result<super::SecureBootSettings> {
        Ok(serde_json::from_value(
            self.call(Operation::SecureBootSettings).await?,
        )?)
    }
    pub async fn stage_secure_boot(&mut self, enabled: bool) -> Result<super::SecureBootSettings> {
        Ok(serde_json::from_value(
            self.call(Operation::StageSecureBoot { enabled }).await?,
        )?)
    }
    pub async fn inventory(&mut self) -> Result<super::HardwareInventory> {
        Ok(serde_json::from_value(
            self.call(Operation::Inventory).await?,
        )?)
    }
    pub async fn agent_network(&mut self) -> Result<super::GuestNetworkSnapshot> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentNetwork).await?,
        )?)
    }
    pub async fn metrics(&mut self) -> Result<super::VmMetrics> {
        Ok(serde_json::from_value(
            self.call(Operation::Metrics).await?,
        )?)
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

    pub async fn agent_ping(&mut self) -> Result<()> {
        self.call(Operation::AgentPing).await.map(|_| ())
    }

    pub async fn agent_status(&mut self) -> Result<AgentStatus> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentStatus).await?,
        )?)
    }

    pub async fn agent_freeze_status(&mut self) -> Result<FreezeState> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentFreezeStatus).await?,
        )?)
    }

    /// Returns the number of frozen filesystems.
    pub async fn agent_freeze(&mut self, mountpoints: Option<Vec<String>>) -> Result<i64> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentFreeze { mountpoints }).await?,
        )?)
    }

    /// Returns the number of thawed filesystems.
    pub async fn agent_thaw(&mut self) -> Result<i64> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentThaw).await?,
        )?)
    }

    pub async fn agent_update_trigger(&mut self, args: Vec<String>) -> Result<UpdateStatus> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentUpdateTrigger { args }).await?,
        )?)
    }

    pub async fn agent_update_status(&mut self) -> Result<UpdateStatus> {
        Ok(serde_json::from_value(
            self.call(Operation::AgentUpdateStatus).await?,
        )?)
    }

    /// Consume the connection as an event stream starting after `after`
    /// (the current latest event when `None`).
    pub async fn follow(mut self, after: Option<u64>) -> Result<Follower> {
        self.call(Operation::Follow { after }).await?;
        Ok(Follower(self.stream))
    }

    /// Consume the connection as the VM's raw VNC (RFB) byte stream. The
    /// reply is read unbuffered so no RFB bytes are left behind in a buffer.
    pub async fn console(mut self) -> Result<UnixStream> {
        debug_assert!(self.stream.buffer().is_empty());
        let id = self.send(Operation::Console).await?;
        let mut stream = self.stream.into_inner();
        let mut line = Vec::new();
        let read = async {
            let mut byte = [0u8; 1];
            while line.len() <= MAX_REQUEST {
                if stream.read_exact(&mut byte).await.is_err() {
                    return Err(Error::Spawn("control socket closed".into()));
                }
                if byte[0] == b'\n' {
                    return Ok(());
                }
                line.push(byte[0]);
            }
            Err(Error::Spawn("control frame too large".into()))
        };
        tokio::time::timeout(IDLE_TIMEOUT, read)
            .await
            .map_err(|_| Error::Spawn("control socket timed out".into()))??;
        Self::result(id, serde_json::from_slice(&line)?)?;
        Ok(stream)
    }
}

pub struct Follower(BufReader<UnixStream>);

/// One frame of a followed event stream.
#[derive(Debug, Clone, PartialEq)]
pub enum FollowFrame {
    Event(VmEvent),
    Missed(u64),
    QmpDropped(u64),
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
        match (response.event, response.missed, response.qmp_dropped) {
            (Some(event), None, None) => Ok(FollowFrame::Event(event)),
            (None, Some(missed), None) => Ok(FollowFrame::Missed(missed)),
            (None, None, Some(dropped)) => Ok(FollowFrame::QmpDropped(dropped)),
            _ => Err(Error::Spawn("malformed follow frame".into())),
        }
    }
}
