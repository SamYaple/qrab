//! The control API as seen from another container: a socket-backed client
//! with a local event mirror, and [`Backend`], which lets a BMC or Redfish
//! service run either in-process or as a sidecar.
use super::{
    events::{EventLog, EventPage},
    socket::{Client, FollowFrame},
    Action, AgentStatus, ConfigView, Control, FreezeState, Status, UpdateStatus,
};
use crate::{Error, Result};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::net::UnixStream;

/// `pooled!(self, retry, method(args))` runs one `Client` method through
/// [`Remote::call`], handing the connection back with the result.
macro_rules! pooled {
    ($remote:expr, $retry:expr, $method:ident($($arg:ident),*)) => {
        $remote
            .call($retry, |mut client| {
                $(let $arg = $arg.clone();)*
                async move {
                    let result = client.$method($($arg),*).await;
                    (client, result)
                }
            })
            .await
    };
}

/// Idle connections kept for reuse. The manager closes connections idle for
/// 60 s, so pooled ones are reused only while comfortably younger than that.
pub const POOL_SIZE: usize = 4;
const POOL_IDLE: Duration = Duration::from_secs(30);

struct Idle {
    client: Client,
    since: Instant,
}

struct Inner {
    path: PathBuf,
    events: EventLog,
    pool: Mutex<Vec<Idle>>,
    opened: AtomicU64,
}

/// A sidecar's view of the manager behind its control socket. Requests reuse
/// a small pool of connections; [`Remote::mirror`] keeps one following
/// connection that copies events, with their sequence numbers, into a local
/// bounded log.
#[derive(Clone)]
pub struct Remote(Arc<Inner>);

impl Remote {
    pub fn new(path: PathBuf) -> Self {
        Self(Arc::new(Inner {
            path,
            events: EventLog::default(),
            pool: Mutex::new(Vec::new()),
            opened: AtomicU64::new(0),
        }))
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0.path
    }

    /// Request connections opened so far (the mirror's are not counted).
    pub fn connections_opened(&self) -> u64 {
        self.0.opened.load(Ordering::Relaxed)
    }

    /// Idle pooled request connections, at most [`POOL_SIZE`].
    pub fn idle_connections(&self) -> usize {
        self.pool().len()
    }

    fn pool(&self) -> std::sync::MutexGuard<'_, Vec<Idle>> {
        self.0
            .pool
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    async fn connect(&self) -> Result<Client> {
        self.0.opened.fetch_add(1, Ordering::Relaxed);
        Client::connect(&self.0.path).await
    }

    fn take(&self) -> Option<Client> {
        let mut pool = self.pool();
        pool.retain(|idle| idle.since.elapsed() < POOL_IDLE);
        pool.pop().map(|idle| idle.client)
    }

    fn give_back(&self, client: Client) {
        let mut pool = self.pool();
        if pool.len() < POOL_SIZE {
            pool.push(Idle {
                client,
                since: Instant::now(),
            });
        }
    }

    /// Run one request on a pooled or new connection. A failed connection is
    /// dropped. Only `retry` (read-only) requests are repeated on a fresh
    /// connection after a pooled one fails; actions never are.
    async fn call<T, F, Fut>(&self, retry: bool, request: F) -> Result<T>
    where
        F: Fn(Client) -> Fut,
        Fut: std::future::Future<Output = (Client, Result<T>)>,
    {
        if let Some(client) = self.take() {
            match request(client).await {
                (client, Ok(value)) => {
                    self.give_back(client);
                    return Ok(value);
                }
                (client, Err(error)) if !retry || is_refusal(&error) => {
                    if is_refusal(&error) {
                        self.give_back(client);
                    }
                    return Err(error);
                }
                (_, Err(_)) => {}
            }
        }
        let (client, result) = request(self.connect().await?).await;
        if result.as_ref().map_or_else(is_refusal, |_| true) {
            self.give_back(client);
        }
        result
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
        let mut client = Client::connect(&self.0.path).await?;
        // A restarted manager numbers its events from 1 again.
        if client.events(None, 1).await?.latest < self.0.events.latest() {
            self.0.events.reset();
        }
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
        pooled!(self, true, status())
    }

    pub async fn probe(&self, ready: bool) -> Result<()> {
        pooled!(self, true, probe(ready))
    }

    pub async fn config_view(&self) -> Result<ConfigView> {
        let redacted = false;
        pooled!(self, true, config(redacted))
    }

    pub async fn action(&self, action: Action) -> Result<()> {
        pooled!(self, false, action(action))
    }

    /// A console consumes its connection; it is never pooled.
    pub async fn console(&self) -> Result<UnixStream> {
        self.connect().await?.console().await
    }

    pub async fn agent_status(&self) -> Result<AgentStatus> {
        pooled!(self, true, agent_status())
    }

    pub async fn agent_freeze_status(&self) -> Result<FreezeState> {
        pooled!(self, true, agent_freeze_status())
    }

    pub async fn agent_freeze(&self, mountpoints: Option<Vec<String>>) -> Result<i64> {
        pooled!(self, false, agent_freeze(mountpoints))
    }

    pub async fn agent_thaw(&self) -> Result<i64> {
        pooled!(self, false, agent_thaw())
    }

    pub async fn agent_update_trigger(&self, args: Vec<String>) -> Result<UpdateStatus> {
        pooled!(self, false, agent_update_trigger(args))
    }

    pub async fn agent_update_status(&self) -> Result<UpdateStatus> {
        pooled!(self, true, agent_update_status())
    }
}

/// The manager answered with an error, so the connection is healthy.
/// `Client` reports other manager errors as "control request failed (...)".
fn is_refusal(error: &Error) -> bool {
    matches!(error, Error::ShuttingDown | Error::Config(_))
        || error.to_string().starts_with("control request failed")
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
