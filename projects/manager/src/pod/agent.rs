//! Host side of the guest-agent channel: one task owns the QGA connection to
//! QEMU's private chardev socket and reconnects whenever the agent is absent,
//! restarting or desynchronized.
use crate::{Error, Result};
use qapi::qga::{
    commands::{
        GuestFsfreezeFreeze, GuestFsfreezeFreezeList, GuestFsfreezeStatusCommand,
        GuestFsfreezeThaw, GuestPing,
    },
    GuestFsfreezeStatus,
};
use qapi_protocol::{qga::Qga, ClientOptions};
use qrab_guest_agent::proto::{validate_update_args, QrabUpdateStatus, QrabUpdateTrigger};
pub use qrab_guest_agent::proto::{UpdateState, UpdateStatus};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
use tokio::sync::{mpsc, oneshot, watch};

/// Connection state of the in-guest agent, as last observed by the manager.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentStatus {
    pub enabled: bool,
    pub connected: bool,
    pub version: Option<String>,
    pub connected_since_unix_ms: Option<u64>,
    pub last_seen_unix_ms: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FreezeState {
    Frozen,
    Thawed,
}

type Reply<T> = oneshot::Sender<Result<T>>;

pub(super) enum AgentOp {
    Ping(Reply<()>),
    FreezeStatus(Reply<FreezeState>),
    Freeze(Option<Vec<String>>, Reply<i64>),
    Thaw(Reply<i64>),
    UpdateTrigger(Vec<String>, Reply<UpdateStatus>),
    UpdateStatus(Reply<UpdateStatus>),
}

impl AgentOp {
    pub(super) fn fail(self, message: &str) {
        self.reject(|| Error::Spawn(message.to_owned()));
    }

    pub(super) fn reject(self, error: impl Fn() -> Error) {
        match self {
            Self::Ping(r) => drop(r.send(Err(error()))),
            Self::FreezeStatus(r) => drop(r.send(Err(error()))),
            Self::Freeze(_, r) | Self::Thaw(r) => drop(r.send(Err(error()))),
            Self::UpdateTrigger(_, r) | Self::UpdateStatus(r) => drop(r.send(Err(error()))),
        }
    }

    fn abandoned(&self) -> bool {
        match self {
            Self::Ping(r) => r.is_closed(),
            Self::FreezeStatus(r) => r.is_closed(),
            Self::Freeze(_, r) | Self::Thaw(r) => r.is_closed(),
            Self::UpdateTrigger(_, r) | Self::UpdateStatus(r) => r.is_closed(),
        }
    }

    /// Arguments are validated before reaching the guest; the agent also
    /// enforces the same bound.
    fn validate(&self) -> Result<()> {
        if let Self::UpdateTrigger(args, _) = self {
            validate_update_args(args).map_err(Error::Config)?;
        }
        Ok(())
    }
}

/// Timeouts for the agent connection. A freeze can legitimately take
/// several seconds while dirty data is flushed.
#[derive(Debug, Clone)]
pub(super) struct AgentTimings {
    pub handshake: Duration,
    pub command: Duration,
    pub retry: Duration,
    pub ping: Duration,
}

impl Default for AgentTimings {
    fn default() -> Self {
        Self {
            handshake: Duration::from_secs(3),
            command: Duration::from_secs(20),
            retry: Duration::from_secs(2),
            ping: Duration::from_secs(10),
        }
    }
}

/// Owned by the supervisor for one QEMU lifetime; dropping it stops the task.
pub(super) struct GuestAgent {
    ops: mpsc::Sender<AgentOp>,
    status: watch::Receiver<AgentStatus>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for GuestAgent {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl GuestAgent {
    pub(super) fn spawn(socket: PathBuf, timings: AgentTimings) -> Self {
        let (ops, receiver) = mpsc::channel(4);
        let (publisher, status) = watch::channel(AgentStatus {
            enabled: true,
            ..AgentStatus::default()
        });
        let task = tokio::spawn(worker(socket, timings, receiver, publisher));
        Self { ops, status, task }
    }

    pub(super) fn status(&self) -> AgentStatus {
        self.status.borrow().clone()
    }

    /// Never waits: a busy agent queue is reported instead of stalling the
    /// supervisor loop.
    pub(super) fn submit(&self, op: AgentOp) {
        if let Err(Error::Config(message)) = op.validate() {
            return op.reject(|| Error::Config(message.clone()));
        }
        match self.ops.try_send(op) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(op)) => op.fail("guest agent is busy"),
            Err(mpsc::error::TrySendError::Closed(op)) => op.fail("guest agent task stopped"),
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

async fn worker(
    socket: PathBuf,
    timings: AgentTimings,
    mut ops: mpsc::Receiver<AgentOp>,
    status: watch::Sender<AgentStatus>,
) {
    let options = ClientOptions {
        connect_timeout: Duration::from_secs(1),
        handshake_timeout: timings.handshake,
        command_timeout: timings.command,
        max_message_bytes: 1024 * 1024,
        ..ClientOptions::default()
    };
    loop {
        // Requests queue during one bounded handshake attempt. QEMU accepts
        // the socket even when no agent runs, so absence shows as a timeout.
        match Qga::connect_with_options(&socket, options.clone()).await {
            Ok(qga) => {
                let version = qga.version();
                status.send_modify(|s| {
                    s.connected = true;
                    s.version = Some(format!("{}.{}.{}", version.0, version.1, version.2));
                    s.connected_since_unix_ms = Some(now_ms());
                    s.last_seen_unix_ms = s.connected_since_unix_ms;
                    s.last_error = None;
                });
                eprintln!(
                    "{}",
                    serde_json::json!({"event":"guest_agent_connected","version":version})
                );
                if !serve(qga, &timings, &mut ops, &status).await {
                    return;
                }
                eprintln!(
                    "{}",
                    serde_json::json!({"event":"guest_agent_disconnected","error":status.borrow().last_error})
                );
            }
            Err(error) => status.send_modify(|s| {
                s.connected = false;
                s.last_error = Some(error.to_string());
            }),
        }
        let retry = tokio::time::sleep(timings.retry);
        tokio::pin!(retry);
        loop {
            tokio::select! {
                _ = &mut retry => break,
                op = ops.recv() => match op {
                    Some(op) => op.fail("guest agent is not connected"),
                    None => return,
                },
            }
        }
    }
}

/// Returns false when the op channel closed and the worker should end.
async fn serve(
    mut qga: Qga,
    timings: &AgentTimings,
    ops: &mut mpsc::Receiver<AgentOp>,
    status: &watch::Sender<AgentStatus>,
) -> bool {
    let mut ping = tokio::time::interval(timings.ping);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping.tick().await;
    loop {
        let healthy = tokio::select! {
            op = ops.recv() => match op {
                Some(op) if op.abandoned() => continue,
                Some(op) => execute(&mut qga, op).await,
                None => return false,
            },
            _ = ping.tick() => qga.execute(&GuestPing {}).await.map_err(|e| e.to_string()),
        };
        match healthy {
            Ok(()) => status.send_modify(|s| s.last_seen_unix_ms = Some(now_ms())),
            Err(error) => status.send_modify(|s| s.last_error = Some(error)),
        }
        if qga.is_closed() {
            status.send_modify(|s| {
                s.connected = false;
                s.connected_since_unix_ms = None;
            });
            return true;
        }
    }
}

/// Executes one request and reports whether the agent answered at all; a
/// valid agent error still proves the session is alive.
async fn execute(qga: &mut Qga, op: AgentOp) -> std::result::Result<(), String> {
    fn finish<T>(
        reply: Reply<T>,
        result: qapi_protocol::Result<T>,
    ) -> std::result::Result<(), String> {
        let alive = match &result {
            Ok(_) | Err(qapi_protocol::Error::Qmp(_)) => Ok(()),
            Err(error) => Err(error.to_string()),
        };
        let _ = reply.send(result.map_err(Error::Protocol));
        alive
    }
    match op {
        AgentOp::Ping(reply) => finish(reply, qga.execute(&GuestPing {}).await),
        AgentOp::FreezeStatus(reply) => {
            let result = qga.execute(&GuestFsfreezeStatusCommand {}).await;
            finish(
                reply,
                result.map(|state| match state {
                    GuestFsfreezeStatus::Frozen => FreezeState::Frozen,
                    GuestFsfreezeStatus::Thawed => FreezeState::Thawed,
                }),
            )
        }
        AgentOp::Freeze(Some(mountpoints), reply) => {
            let command = GuestFsfreezeFreezeList {
                mountpoints: Some(mountpoints),
            };
            finish(reply, qga.execute(&command).await)
        }
        AgentOp::Freeze(None, reply) => finish(reply, qga.execute(&GuestFsfreezeFreeze {}).await),
        AgentOp::Thaw(reply) => finish(reply, qga.execute(&GuestFsfreezeThaw {}).await),
        AgentOp::UpdateTrigger(args, reply) => {
            finish(reply, qga.execute(&QrabUpdateTrigger { args }).await)
        }
        AgentOp::UpdateStatus(reply) => finish(reply, qga.execute(&QrabUpdateStatus {}).await),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qapi_protocol::server::QgaSession;
    use serde_json::json;
    use tokio::net::UnixListener;

    fn fast() -> AgentTimings {
        AgentTimings {
            handshake: Duration::from_millis(300),
            command: Duration::from_millis(500),
            retry: Duration::from_millis(50),
            ping: Duration::from_secs(3600),
        }
    }

    async fn call<T>(agent: &GuestAgent, make: impl FnOnce(Reply<T>) -> AgentOp) -> Result<T> {
        let (tx, rx) = oneshot::channel();
        agent.submit(make(tx));
        tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .expect("reply")
            .expect("sender kept")
    }

    async fn accept(listener: &UnixListener) -> QgaSession {
        let mut session = QgaSession::accept(listener).await.unwrap();
        session.handle_sync().await.unwrap();
        let (name, _) = session.recv_command().await.unwrap();
        assert_eq!(name, "guest-info");
        session
            .send_return(json!({"version":"11.1.3-qrab","supported_commands":[]}))
            .await
            .unwrap();
        session
    }

    async fn wait_connected(agent: &GuestAgent, connected: bool) {
        for _ in 0..200 {
            if agent.status().connected == connected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("agent connected state did not become {connected}");
    }

    #[tokio::test]
    async fn absent_agent_fails_fast_then_connects_and_serves_requests() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("qga.sock");
        let agent = GuestAgent::spawn(path.clone(), fast());
        let error = call(&agent, AgentOp::Ping).await.unwrap_err();
        assert!(error.to_string().contains("not connected"), "{error}");
        assert!(!agent.status().connected);
        assert!(agent.status().last_error.is_some());

        let listener = UnixListener::bind(&path).unwrap();
        let server = tokio::spawn(async move {
            let mut session = accept(&listener).await;
            let expected = [
                ("guest-ping", json!({}), json!({})),
                (
                    "guest-fsfreeze-freeze-list",
                    json!({"mountpoints":["/data"]}),
                    json!(1),
                ),
                ("guest-fsfreeze-status", json!({}), json!("frozen")),
                ("guest-fsfreeze-thaw", json!({}), json!(1)),
                (
                    "qrab-update-trigger",
                    json!({"args":["v2"]}),
                    json!({
                    "state":"running","run-id":4,"args":["v2"],"started-unix-ms":1,
                    "finished-unix-ms":null,"exit-code":null,"signal":null,"error":null,
                    "output":"","output-truncated":false,"hook-installed":true,"agent-version":"0.1.0"}),
                ),
            ];
            for (name, args, reply) in expected {
                let (got, got_args) = session.recv_command().await.unwrap();
                assert_eq!((got.as_str(), &got_args), (name, &args));
                session.send_return(reply).await.unwrap();
            }
            let (name, _) = session.recv_command().await.unwrap();
            assert_eq!(name, "qrab-update-status");
            session
                .send_error(
                    "CommandNotFound",
                    "Command qrab-update-status has been disabled",
                )
                .await
                .unwrap();
            // Keep the session open until the client is done.
            let _ = session.recv_command().await;
        });
        wait_connected(&agent, true).await;
        assert_eq!(agent.status().version.as_deref(), Some("11.1.3"));
        call(&agent, AgentOp::Ping).await.unwrap();
        assert_eq!(
            call(&agent, |r| AgentOp::Freeze(Some(vec!["/data".into()]), r))
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            call(&agent, AgentOp::FreezeStatus).await.unwrap(),
            FreezeState::Frozen
        );
        assert_eq!(call(&agent, AgentOp::Thaw).await.unwrap(), 1);
        let started = call(&agent, |r| AgentOp::UpdateTrigger(vec!["v2".into()], r))
            .await
            .unwrap();
        assert_eq!((started.state, started.run_id), (UpdateState::Running, 4));
        match call(&agent, AgentOp::UpdateStatus).await {
            Err(Error::Protocol(qapi_protocol::Error::Qmp(e))) => {
                assert_eq!(e.class, "CommandNotFound")
            }
            other => panic!("{other:?}"),
        }
        // A valid agent error leaves the session connected.
        assert!(agent.status().connected);
        drop(agent);
        server.abort();
    }

    #[tokio::test]
    async fn invalid_update_arguments_never_reach_the_guest() {
        let dir = tempfile::tempdir().unwrap();
        let agent = GuestAgent::spawn(dir.path().join("qga.sock"), fast());
        let error = call(&agent, |r| AgentOp::UpdateTrigger(vec!["--evil".into()], r))
            .await
            .unwrap_err();
        assert!(matches!(error, Error::Config(_)), "{error}");
    }

    #[tokio::test]
    async fn restarted_agent_is_resynchronized_after_a_lost_reply() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("qga.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let agent = GuestAgent::spawn(path, fast());
        let server = tokio::spawn(async move {
            // The first agent instance dies before answering the ping.
            let mut first = accept(&listener).await;
            let _ = first.recv_command().await.unwrap();
            drop(first);
            let mut second = accept(&listener).await;
            let (name, _) = second.recv_command().await.unwrap();
            assert_eq!(name, "guest-fsfreeze-thaw");
            second.send_return(json!(0)).await.unwrap();
            let _ = second.recv_command().await;
        });
        wait_connected(&agent, true).await;
        assert!(call(&agent, AgentOp::Ping).await.is_err());
        wait_connected(&agent, false).await;
        wait_connected(&agent, true).await;
        assert_eq!(call(&agent, AgentOp::Thaw).await.unwrap(), 0);
        drop(agent);
        server.abort();
    }
}
