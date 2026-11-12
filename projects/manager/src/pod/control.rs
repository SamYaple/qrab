use super::agent::{AgentOp, AgentStatus, FreezeState, GuestAgent, UpdateStatus};
use super::{
    events::{EventLog, EventPage},
    Config, ConfigView, Deployment, Status,
};
use crate::{Error, Result};
use qapi::commands::{Cont, Stop, SystemReset};
use qapi_protocol::{Qmp, QmpEvent};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};
use tokio::{
    net::UnixStream,
    sync::{mpsc, oneshot, watch},
};

/// The intentionally small control surface exposed to a pod's BMC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Pause,
    Resume,
    Reset,
    Powerdown,
}

pub(super) enum Request {
    Action(Action, oneshot::Sender<Result<()>>),
    Console(oneshot::Sender<Result<UnixStream>>),
    Agent(AgentOp),
}

/// State readable without a supervisor round trip.
struct Shared {
    events: EventLog,
    deployment: Deployment,
    config: RwLock<Option<Config>>,
    /// Every record the supervisor publishes, as it publishes it.
    status: Arc<watch::Sender<Option<Status>>>,
    /// Set once the supervisor stops serving requests.
    stopping: AtomicBool,
    /// Guest-agent connection state, published by the agent task.
    agent: Arc<watch::Sender<AgentStatus>>,
}

/// The source control API for one VM. Every transport (in-process BMC, the
/// control socket) is a thin layer over these methods.
#[derive(Clone)]
pub struct Control {
    requests: mpsc::Sender<Request>,
    shared: Arc<Shared>,
}

/// The supervisor end of a control channel.
pub struct ControlReceiver {
    pub(super) requests: mpsc::Receiver<Request>,
    // Kept so supervisor-owned transports can always issue requests.
    sender: mpsc::Sender<Request>,
    shared: Arc<Shared>,
}

/// Bound admission rather than allowing HTTP requests to grow an unbounded queue.
pub fn control_channel() -> (Control, ControlReceiver) {
    channel(None, Deployment::default())
}

/// A channel whose configuration view is available before the supervisor
/// starts. The supervisor replaces it with the configuration it runs.
pub fn control_channel_for(config: &Config, deployment: Deployment) -> (Control, ControlReceiver) {
    channel(Some(config.clone()), deployment)
}

fn channel(config: Option<Config>, deployment: Deployment) -> (Control, ControlReceiver) {
    let (tx, rx) = mpsc::channel(16);
    let agent = AgentStatus {
        enabled: config.as_ref().is_some_and(|c| c.guest_agent),
        ..AgentStatus::default()
    };
    let shared = Arc::new(Shared {
        events: EventLog::default(),
        deployment,
        config: RwLock::new(config),
        status: Arc::new(watch::channel(None).0),
        stopping: AtomicBool::new(false),
        agent: Arc::new(watch::channel(agent).0),
    });
    (
        Control {
            requests: tx.clone(),
            shared: shared.clone(),
        },
        ControlReceiver {
            requests: rx,
            sender: tx,
            shared,
        },
    )
}

impl ControlReceiver {
    /// Record a QMP event in the channel's bounded history.
    pub fn record_event(&self, event: QmpEvent) {
        self.shared.events.record(event);
    }

    /// Forward events from QMP as they arrive, independent of request
    /// dispatch, so a slow command cannot back up QMP's bounded event queue.
    pub(super) fn pump_events(
        &self,
        mut events: mpsc::Receiver<QmpEvent>,
    ) -> tokio::task::JoinHandle<()> {
        let shared = self.shared.clone();
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                shared.events.record(event);
            }
        })
    }

    /// Events the QMP client discarded because its queue was full or an
    /// event exceeded its size bound.
    pub(super) fn set_qmp_dropped(&self, dropped: u64) {
        self.shared.events.set_qmp_dropped(dropped);
    }

    /// From now on requests fail at once with [`Error::ShuttingDown`].
    pub(super) fn set_stopping(&self) {
        self.shared.stopping.store(true, Ordering::Release);
    }

    pub(super) fn status_sink(&self) -> super::state::StatusSink {
        super::state::StatusSink::new(self.shared.status.clone())
    }

    pub(super) fn agent_status_publisher(&self) -> Arc<watch::Sender<AgentStatus>> {
        self.shared.agent.clone()
    }

    pub(super) fn set_config(&self, config: &Config) {
        self.shared
            .agent
            .send_modify(|status| status.enabled = config.guest_agent);
        *self
            .shared
            .config
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(config.clone());
    }

    /// A sender for transports owned by the supervisor itself.
    pub(super) fn control(&self) -> Control {
        Control {
            requests: self.sender.clone(),
            shared: self.shared.clone(),
        }
    }
}

impl Control {
    async fn request<T>(
        &self,
        make: impl FnOnce(oneshot::Sender<Result<T>>) -> Request,
    ) -> Result<T> {
        self.request_within(std::time::Duration::from_secs(5), make)
            .await
    }

    async fn request_within<T>(
        &self,
        deadline: std::time::Duration,
        make: impl FnOnce(oneshot::Sender<Result<T>>) -> Request,
    ) -> Result<T> {
        if self.shared.stopping.load(Ordering::Acquire) {
            return Err(Error::ShuttingDown);
        }
        let (tx, rx) = oneshot::channel();
        self.requests
            .try_send(make(tx))
            .map_err(|e| Error::Spawn(format!("VM control unavailable: {e}")))?;
        // A caller timeout never retries a potentially dispatched action.
        tokio::time::timeout(deadline, rx)
            .await
            .map_err(|_| Error::Spawn("VM control timed out; outcome may be unknown".into()))?
            .map_err(|_| Error::Spawn("VM supervisor stopped".into()))?
    }

    pub async fn action(&self, action: Action) -> Result<()> {
        self.request(|tx| Request::Action(action, tx)).await
    }

    /// Only an established stream leaves the supervisor. Socket paths and QMP
    /// ownership stay private to its lifetime and cannot be supplied by HTTP.
    pub async fn console(&self) -> Result<UnixStream> {
        self.request(Request::Console).await
    }

    fn config(&self) -> Result<Config> {
        self.shared
            .config
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
            .ok_or_else(|| Error::Spawn("VM supervisor has not started".into()))
    }

    /// The last status record published by the supervisor, from memory once
    /// this channel's supervisor has published; otherwise from `status.json`.
    pub fn status(&self) -> Result<Status> {
        if let Some(status) = self.shared.status.borrow().clone() {
            return Ok(status);
        }
        super::status(&self.config()?)
    }

    /// Changes whenever the supervisor publishes a status record.
    pub fn watch_status(&self) -> watch::Receiver<Option<Status>> {
        self.shared.status.subscribe()
    }

    /// Liveness (`ready: false`) or readiness probe of the supervised VM.
    pub fn probe(&self, ready: bool) -> Result<()> {
        super::probe(&self.config()?, ready)
    }

    /// The full configuration view, including sensitive fields. Callers that
    /// serve less-privileged users must use [`ConfigView::redacted`].
    pub fn config_view(&self) -> Result<ConfigView> {
        let config = self.config()?;
        let view = ConfigView::new(&config, &self.shared.deployment);
        // Resize progress is best effort: the status record may not exist yet.
        let status = super::status(&config).ok();
        Ok(view.with_resize(status.as_ref().and_then(|s| s.resize.as_ref())))
    }

    /// See [`EventPage`]. `after: None` is a snapshot of the newest events.
    pub fn events(&self, after: Option<u64>, limit: usize) -> EventPage {
        self.shared.events.page(after, limit)
    }

    /// Resolve once an event with a sequence number above `cursor` exists.
    pub async fn wait_events(&self, cursor: u64) {
        self.shared.events.wait_after(cursor).await
    }

    /// Resolve once [`EventPage::qmp_dropped`] differs from `known`.
    pub async fn wait_qmp_dropped(&self, known: u64) {
        self.shared.events.wait_dropped(known).await
    }

    /// Guest-agent requests wait for one reconnect attempt plus a command,
    /// which can include flushing dirty data before a freeze.
    async fn agent<T>(
        &self,
        make: impl FnOnce(oneshot::Sender<Result<T>>) -> AgentOp,
    ) -> Result<T> {
        // Fail at once rather than queue behind startup or a reconnect.
        let status = self.shared.agent.borrow().clone();
        if !status.enabled {
            return Err(Error::Spawn("guest agent channel is disabled".into()));
        }
        if !status.connected {
            return Err(Error::Spawn("guest agent is not connected".into()));
        }
        self.request_within(AGENT_REQUEST_TIMEOUT, |tx| Request::Agent(make(tx)))
            .await
    }

    /// Cached connection state. Never waits on the guest or the supervisor
    /// queue, so it answers before QEMU starts and while it stops.
    pub async fn agent_status(&self) -> Result<AgentStatus> {
        Ok(self.shared.agent.borrow().clone())
    }

    pub async fn agent_ping(&self) -> Result<()> {
        self.agent(AgentOp::Ping).await
    }

    pub async fn agent_freeze_status(&self) -> Result<FreezeState> {
        self.agent(AgentOp::FreezeStatus).await
    }

    /// Freeze all guest filesystems, or only the listed mountpoints. The
    /// agent thaws automatically after its bounded freeze timeout.
    pub async fn agent_freeze(&self, mountpoints: Option<Vec<String>>) -> Result<i64> {
        self.agent(|tx| AgentOp::Freeze(mountpoints, tx)).await
    }

    pub async fn agent_thaw(&self) -> Result<i64> {
        self.agent(AgentOp::Thaw).await
    }

    /// Start the guest's fixed update hook with bounded token arguments.
    pub async fn agent_update_trigger(&self, args: Vec<String>) -> Result<UpdateStatus> {
        qrab_guest_agent::proto::validate_update_args(&args).map_err(Error::Config)?;
        self.agent(|tx| AgentOp::UpdateTrigger(args, tx)).await
    }

    pub async fn agent_update_status(&self) -> Result<UpdateStatus> {
        self.agent(AgentOp::UpdateStatus).await
    }
}

const AGENT_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

impl Request {
    /// Answer a request that arrives while the VM is being stopped. A
    /// repeated powerdown is already in effect.
    pub(super) fn shutting_down(self) {
        match self {
            Self::Action(Action::Powerdown, reply) => drop(reply.send(Ok(()))),
            Self::Action(_, reply) => drop(reply.send(Err(Error::ShuttingDown))),
            Self::Console(reply) => drop(reply.send(Err(Error::ShuttingDown))),
            Self::Agent(op) => op.reject(|| Error::ShuttingDown),
        }
    }

    /// True requests the supervisor's bounded ACPI/quit/reap shutdown path.
    pub(super) async fn dispatch(
        self,
        qmp: &Qmp,
        console: Option<&std::path::Path>,
        agent: Option<&GuestAgent>,
    ) -> bool {
        match self {
            Self::Action(Action::Powerdown, reply) if !reply.is_closed() => {
                let _ = reply.send(Ok(()));
                return true;
            }
            Self::Action(action, reply) if !reply.is_closed() => {
                let result = match action {
                    Action::Pause => qmp.execute(&Stop {}).await,
                    Action::Resume => qmp.execute(&Cont {}).await,
                    Action::Reset => qmp.execute(&SystemReset {}).await,
                    Action::Powerdown => unreachable!(),
                }
                .map(|_| ())
                .map_err(Error::Protocol);
                let _ = reply.send(result);
            }
            Self::Console(reply) if !reply.is_closed() => {
                let result = match console {
                    Some(path) => match tokio::time::timeout(
                        std::time::Duration::from_secs(1),
                        UnixStream::connect(path),
                    )
                    .await
                    {
                        Ok(result) => result.map_err(Error::Io),
                        Err(_) => Err(Error::Spawn("VNC connection timed out".into())),
                    },
                    None => Err(Error::Config("graphical console is disabled".into())),
                };
                let _ = reply.send(result);
            }
            Self::Agent(op) => match agent {
                Some(agent) => agent.submit(op),
                None => op.fail("guest agent channel is disabled"),
            },
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pod::agent::AgentTimings;
    use qapi_protocol::server::QmpSession;
    use qrab_guest_agent::sys::{Mount, Platform, ShutdownMode};
    use std::{sync::Arc, time::Duration};

    /// One real filesystem whose freeze/thaw only flips a flag.
    #[derive(Default)]
    struct Guest(std::sync::Mutex<bool>);
    impl Platform for Guest {
        fn mounts(&self) -> std::io::Result<Vec<Mount>> {
            Ok(vec![Mount {
                dir: "/".into(),
                fstype: "ext4".into(),
                device: (253, 1),
            }])
        }
        fn freeze(&self, _: &Mount) -> std::io::Result<()> {
            *self.0.lock().unwrap() = true;
            Ok(())
        }
        fn thaw(&self, _: &Mount) -> std::io::Result<()> {
            let mut frozen = self.0.lock().unwrap();
            if !std::mem::replace(&mut *frozen, false) {
                return Err(std::io::Error::from_raw_os_error(22));
            }
            Ok(())
        }
        fn trim(&self, _: &Mount, _: u64) -> std::io::Result<(u64, u64)> {
            Ok((0, 0))
        }
        fn shutdown(&self, _: ShutdownMode) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn control_reaches_a_real_agent_through_the_supervisor_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        let qmp_path = dir.path().join("qmp.sock");
        let qmp_listener = tokio::net::UnixListener::bind(&qmp_path).unwrap();
        let qmp_server = tokio::spawn(async move {
            let mut session = QmpSession::accept(&qmp_listener).await.unwrap();
            session.send_greeting(11, 1, 3).await.unwrap();
            session.negotiate().await.unwrap();
            let _ = session.recv_command().await;
        });
        let (qmp, _events) = Qmp::new(&qmp_path).await.unwrap();

        // The production agent library, serving its QGA protocol on the
        // socket where QEMU's chardev would be.
        let qga_path = dir.path().join("qga.sock");
        let qga_listener = std::os::unix::net::UnixListener::bind(&qga_path).unwrap();
        let hook_dir = tempfile::tempdir().unwrap();
        let guest = Arc::new(Guest::default());
        let agent = qrab_guest_agent::Agent::new(
            qrab_guest_agent::Config {
                update: qrab_guest_agent::HookPolicy {
                    path: hook_dir.path().join("update-hook"),
                    require_root: false,
                    timeout: Duration::from_secs(5),
                },
                ..Default::default()
            },
            guest.clone(),
        );
        std::thread::spawn(move || {
            for stream in qga_listener.incoming() {
                let stream = stream.unwrap();
                let _ = agent.serve(&stream, &stream);
            }
        });
        let timings = AgentTimings {
            retry: Duration::from_millis(50),
            ..Default::default()
        };
        let (control, mut receiver) = control_channel();
        let guest_agent =
            GuestAgent::spawn_publishing(qga_path, timings, receiver.agent_status_publisher());
        let supervisor = async {
            while let Some(request) = receiver.requests.recv().await {
                request.dispatch(&qmp, None, Some(&guest_agent)).await;
            }
        };
        let client = async {
            for _ in 0..200 {
                if control.agent_status().await.unwrap().connected {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let status = control.agent_status().await.unwrap();
            assert!(status.enabled && status.connected, "{status:?}");
            assert_eq!(status.version.as_deref(), Some("11.1.3"));
            control.agent_ping().await.unwrap();
            assert_eq!(control.agent_freeze(None).await.unwrap(), 1);
            assert!(*guest.0.lock().unwrap());
            assert_eq!(
                control.agent_freeze_status().await.unwrap(),
                FreezeState::Frozen
            );
            // The trigger is disabled while frozen, and invalid arguments are
            // refused before reaching the guest.
            assert!(control.agent_update_trigger(vec![]).await.is_err());
            assert!(matches!(
                control.agent_update_trigger(vec!["$(x)".into()]).await,
                Err(Error::Config(_))
            ));
            assert_eq!(control.agent_thaw().await.unwrap(), 1);
            assert!(!*guest.0.lock().unwrap());
            let update = control.agent_update_status().await.unwrap();
            assert!(!update.hook_installed);
            // Without an installed hook the trigger reports a disabled command.
            let error = control.agent_update_trigger(vec![]).await.unwrap_err();
            assert!(error.to_string().contains("disabled"), "{error}");
        };
        tokio::select! {
            _ = supervisor => panic!("control channel closed"),
            _ = client => {}
        }

        // A configuration without the channel answers immediately.
        let (control, mut receiver) = control_channel();
        let supervisor = async {
            while let Some(request) = receiver.requests.recv().await {
                request.dispatch(&qmp, None, None).await;
            }
        };
        let client = async {
            assert!(!control.agent_status().await.unwrap().enabled);
            let error = control.agent_ping().await.unwrap_err();
            assert!(error.to_string().contains("disabled"), "{error}");
        };
        tokio::select! {
            _ = supervisor => panic!("control channel closed"),
            _ = client => {}
        }
        qmp_server.abort();
    }
}
