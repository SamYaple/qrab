use super::{
    events::{EventLog, EventPage},
    Config, ConfigView, Deployment, Status,
};
use crate::{Error, Result};
use qapi::commands::{Cont, Stop, SystemReset};
use qapi_protocol::{Qmp, QmpEvent};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio::{
    net::UnixStream,
    sync::{mpsc, oneshot},
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
}

/// State readable without a supervisor round trip.
struct Shared {
    events: EventLog,
    deployment: Deployment,
    config: RwLock<Option<Config>>,
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
    let shared = Arc::new(Shared {
        events: EventLog::default(),
        deployment,
        config: RwLock::new(config),
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

    pub(super) fn set_config(&self, config: &Config) {
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
        let (tx, rx) = oneshot::channel();
        self.requests
            .try_send(make(tx))
            .map_err(|e| Error::Spawn(format!("VM control unavailable: {e}")))?;
        // A caller timeout never retries a potentially dispatched action.
        tokio::time::timeout(std::time::Duration::from_secs(5), rx)
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

    /// The last status record published by the supervisor heartbeat.
    pub fn status(&self) -> Result<Status> {
        super::status(&self.config()?)
    }

    /// The full configuration view, including sensitive fields. Callers that
    /// serve less-privileged users must use [`ConfigView::redacted`].
    pub fn config_view(&self) -> Result<ConfigView> {
        Ok(ConfigView::new(&self.config()?, &self.shared.deployment))
    }

    /// See [`EventPage`]. `after: None` is a snapshot of the newest events.
    pub fn events(&self, after: Option<u64>, limit: usize) -> EventPage {
        self.shared.events.page(after, limit)
    }

    /// Resolve once an event with a sequence number above `cursor` exists.
    pub async fn wait_events(&self, cursor: u64) {
        self.shared.events.wait_after(cursor).await
    }
}

impl Request {
    /// True requests the supervisor's bounded ACPI/quit/reap shutdown path.
    pub(super) async fn dispatch(self, qmp: &Qmp, console: Option<&std::path::Path>) -> bool {
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
            _ => {}
        }
        false
    }
}
