use crate::{Error, Result};
use qapi::commands::{Cont, Stop, SystemReset};
use qapi_protocol::Qmp;
use serde::{Deserialize, Serialize};
use tokio::{
    net::UnixStream,
    sync::{mpsc, oneshot},
};

/// The intentionally small control surface exposed to a pod's BMC.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Clone)]
pub struct Control(mpsc::Sender<Request>);
pub struct ControlReceiver(pub(super) mpsc::Receiver<Request>);

/// Bound admission rather than allowing HTTP requests to grow an unbounded queue.
pub fn control_channel() -> (Control, ControlReceiver) {
    let (tx, rx) = mpsc::channel(16);
    (Control(tx), ControlReceiver(rx))
}

impl Control {
    async fn request<T>(
        &self,
        make: impl FnOnce(oneshot::Sender<Result<T>>) -> Request,
    ) -> Result<T> {
        let (tx, rx) = oneshot::channel();
        self.0
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
