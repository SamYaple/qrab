//! The control API as seen from another container: a socket-backed client
//! with a local event mirror, and [`Backend`], which lets a BMC or Redfish
//! service run either in-process or as a sidecar.
use super::{
    events::{EventLog, EventPage},
    socket::{Client, FollowFrame},
    Action, AgentStatus, ConfigView, Control, FreezeState, Status, UpdateStatus,
};
use crate::Result;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::net::UnixStream;

struct Inner {
    path: PathBuf,
    events: EventLog,
}

/// A sidecar's view of the manager behind its control socket. Requests use
/// short-lived connections; [`Remote::mirror`] keeps one following
/// connection that copies events, with their sequence numbers, into a local
/// bounded log.
#[derive(Clone)]
pub struct Remote(Arc<Inner>);

impl Remote {
    pub fn new(path: PathBuf) -> Self {
        Self(Arc::new(Inner {
            path,
            events: EventLog::default(),
        }))
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0.path
    }

    async fn client(&self) -> Result<Client> {
        Client::connect(&self.0.path).await
    }

    /// Mirror the manager's events forever, reconnecting after a second.
    /// The manager normally starts after its sidecars.
    pub async fn mirror(&self) {
        loop {
            let _ = self.mirror_once().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn mirror_once(&self) -> Result<()> {
        let mut client = self.client().await?;
        loop {
            let page = client
                .events(Some(self.0.events.latest()), super::EVENT_PAGE_LIMIT)
                .await?;
            let caught_up = page.next >= page.latest;
            self.0.events.set_qmp_dropped(page.qmp_dropped);
            for event in page.events {
                self.0.events.insert(event, Some(page.rejected));
            }
            if caught_up {
                break;
            }
        }
        let mut follower = client.follow(Some(self.0.events.latest())).await?;
        loop {
            match follower.next().await? {
                FollowFrame::Event(event) => self.0.events.insert(event, None),
                FollowFrame::QmpDropped(dropped) => self.0.events.set_qmp_dropped(dropped),
                FollowFrame::Missed(_) => {}
            }
        }
    }

    pub async fn status(&self) -> Result<Status> {
        self.client().await?.status().await
    }

    pub async fn probe(&self, ready: bool) -> Result<()> {
        self.client().await?.probe(ready).await
    }

    pub async fn config_view(&self) -> Result<ConfigView> {
        self.client().await?.config(false).await
    }

    pub async fn action(&self, action: Action) -> Result<()> {
        self.client().await?.action(action).await
    }

    pub async fn console(&self) -> Result<UnixStream> {
        self.client().await?.console().await
    }

    pub async fn agent_status(&self) -> Result<AgentStatus> {
        self.client().await?.agent_status().await
    }

    pub async fn agent_freeze_status(&self) -> Result<FreezeState> {
        self.client().await?.agent_freeze_status().await
    }

    pub async fn agent_freeze(&self, mountpoints: Option<Vec<String>>) -> Result<i64> {
        self.client().await?.agent_freeze(mountpoints).await
    }

    pub async fn agent_thaw(&self) -> Result<i64> {
        self.client().await?.agent_thaw().await
    }

    pub async fn agent_update_trigger(&self, args: Vec<String>) -> Result<UpdateStatus> {
        self.client().await?.agent_update_trigger(args).await
    }

    pub async fn agent_update_status(&self) -> Result<UpdateStatus> {
        self.client().await?.agent_update_status().await
    }
}

/// Where a BMC-like service reaches its VM.
#[derive(Clone)]
pub enum Backend {
    /// The supervisor runs in this process.
    Local(Control),
    /// The supervisor is another container's `qrab-manager`.
    Socket(Remote),
}

impl Backend {
    pub async fn status(&self) -> Result<Status> {
        match self {
            Self::Local(control) => control.status(),
            Self::Socket(remote) => remote.status().await,
        }
    }

    /// Evaluated by the manager, which owns the process identities.
    pub async fn probe(&self, ready: bool) -> Result<()> {
        match self {
            Self::Local(control) => control.probe(ready),
            Self::Socket(remote) => remote.probe(ready).await,
        }
    }

    /// The full view; serve [`ConfigView::redacted`] to less-privileged users.
    pub async fn config_view(&self) -> Result<ConfigView> {
        match self {
            Self::Local(control) => control.config_view(),
            Self::Socket(remote) => remote.config_view().await,
        }
    }

    pub fn events(&self, after: Option<u64>, limit: usize) -> EventPage {
        match self {
            Self::Local(control) => control.events(after, limit),
            Self::Socket(remote) => remote.0.events.page(after, limit),
        }
    }

    pub async fn wait_events(&self, cursor: u64) {
        match self {
            Self::Local(control) => control.wait_events(cursor).await,
            Self::Socket(remote) => remote.0.events.wait_after(cursor).await,
        }
    }

    pub async fn action(&self, action: Action) -> Result<()> {
        match self {
            Self::Local(control) => control.action(action).await,
            Self::Socket(remote) => remote.action(action).await,
        }
    }

    pub async fn console(&self) -> Result<UnixStream> {
        match self {
            Self::Local(control) => control.console().await,
            Self::Socket(remote) => remote.console().await,
        }
    }

    pub async fn agent_status(&self) -> Result<AgentStatus> {
        match self {
            Self::Local(control) => control.agent_status().await,
            Self::Socket(remote) => remote.agent_status().await,
        }
    }

    pub async fn agent_freeze_status(&self) -> Result<FreezeState> {
        match self {
            Self::Local(control) => control.agent_freeze_status().await,
            Self::Socket(remote) => remote.agent_freeze_status().await,
        }
    }

    pub async fn agent_freeze(&self, mountpoints: Option<Vec<String>>) -> Result<i64> {
        match self {
            Self::Local(control) => control.agent_freeze(mountpoints).await,
            Self::Socket(remote) => remote.agent_freeze(mountpoints).await,
        }
    }

    pub async fn agent_thaw(&self) -> Result<i64> {
        match self {
            Self::Local(control) => control.agent_thaw().await,
            Self::Socket(remote) => remote.agent_thaw().await,
        }
    }

    pub async fn agent_update_trigger(&self, args: Vec<String>) -> Result<UpdateStatus> {
        match self {
            Self::Local(control) => control.agent_update_trigger(args).await,
            Self::Socket(remote) => remote.agent_update_trigger(args).await,
        }
    }

    pub async fn agent_update_status(&self) -> Result<UpdateStatus> {
        match self {
            Self::Local(control) => control.agent_update_status().await,
            Self::Socket(remote) => remote.agent_update_status().await,
        }
    }
}
