//! The console WebSocket: browser ⇄ dashboard ⇄ the VM's BMC `/api/console`.
use crate::{AppState, Failure, access::Role, auth};
use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::HeaderMap,
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_tungstenite::tungstenite::Message as Upstream;
use tower_sessions::Session;

const FRAME: usize = 64 * 1024;
/// Consoles one subject may hold open at once, within the global limit.
pub const PER_SUBJECT: usize = 4;

pub type SubjectCounts = Arc<Mutex<HashMap<String, usize>>>;

/// One open console counted against its subject until dropped.
struct SubjectSlot {
    counts: SubjectCounts,
    subject: String,
}

impl SubjectSlot {
    fn acquire(counts: &SubjectCounts, subject: &str) -> Option<Self> {
        let mut map = counts.lock().unwrap_or_else(|e| e.into_inner());
        let count = map.entry(subject.to_owned()).or_default();
        if *count >= PER_SUBJECT {
            return None;
        }
        *count += 1;
        Some(Self {
            counts: counts.clone(),
            subject: subject.to_owned(),
        })
    }
}

impl Drop for SubjectSlot {
    fn drop(&mut self) {
        let mut map = self.counts.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(count) = map.get_mut(&self.subject) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                map.remove(&self.subject);
            }
        }
    }
}
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
/// How often the VM's grants are re-read while a console is open.
const ROLE_RECHECK: Duration = Duration::from_secs(30);

pub async fn upgrade(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((namespace, name)): Path<(String, String)>,
    ws: WebSocketUpgrade,
) -> Result<Response, Failure> {
    let user = auth::user(&session).await?;
    if !auth::origin_matches(&headers, &app.origin) {
        return Err(Failure::forbidden());
    }
    let (vm, role) = crate::visible_vm(&app, &user, &namespace, &name).await?;
    // The BMC refuses viewers at console admission; so does the dashboard.
    if role != Role::Admin || !vm.spec.components.bmc.enabled {
        return Err(Failure::forbidden());
    }
    let ip = crate::pod_ip(&app, &vm)
        .await?
        .ok_or_else(|| Failure::unavailable("The VM's Pod is not running."))?;
    let subject_slot = SubjectSlot::acquire(&app.console_subjects, &user.subject)
        .ok_or_else(|| Failure::unavailable("You have too many consoles open."))?;
    let permit = app
        .consoles
        .clone()
        .try_acquire_owned()
        .map_err(|_| Failure::unavailable("Too many consoles are open."))?;
    let request = app
        .bmc
        .console_request(ip, &user.id_token)
        .map_err(|_| Failure::internal())?;
    let (upstream, _) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(request),
    )
    .await
    .map_err(|_| Failure::unavailable("The VM console is unavailable."))?
    .map_err(|error| {
        tracing::warn!(%error, vm=%name, "BMC console connection failed");
        Failure::unavailable("The VM console is unavailable.")
    })?;
    let id = session.id().ok_or_else(Failure::unauthorized)?;
    tracing::info!(subject=%user.subject, vm=%name, "VM console connected");
    Ok(ws
        .max_message_size(FRAME)
        .max_frame_size(FRAME)
        .on_upgrade(move |socket| async move {
            let _permits = (permit, subject_slot);
            bridge(socket, upstream, app, id, user, name).await;
        }))
}

async fn bridge(
    browser: WebSocket,
    upstream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    app: AppState,
    id: tower_sessions::session::Id,
    user: auth::User,
    name: String,
) {
    let (mut to_browser, mut from_browser) = browser.split();
    let (mut to_bmc, mut from_bmc) = upstream.split();
    let up = async {
        while let Some(Ok(message)) = from_browser.next().await {
            let forward = match message {
                Message::Binary(bytes) => Upstream::Binary(bytes),
                Message::Ping(_) | Message::Pong(_) => continue,
                _ => break,
            };
            match tokio::time::timeout(WRITE_TIMEOUT, to_bmc.send(forward)).await {
                Ok(Ok(())) => {}
                _ => break,
            }
        }
        let _ = to_bmc.close().await;
    };
    let down = async {
        while let Some(Ok(message)) = from_bmc.next().await {
            let forward = match message {
                Upstream::Binary(bytes) => Message::Binary(bytes),
                Upstream::Ping(_) | Upstream::Pong(_) | Upstream::Frame(_) => continue,
                _ => break,
            };
            match tokio::time::timeout(WRITE_TIMEOUT, to_browser.send(forward)).await {
                Ok(Ok(())) => {}
                _ => break,
            }
        }
    };
    // Logout or expiry ends the stream within a second; a lost grant within
    // ROLE_RECHECK.
    let revoked = async {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        let mut last_role_check = tokio::time::Instant::now();
        loop {
            tick.tick().await;
            if !auth::still_current(&app.sessions, &id, &user).await {
                break;
            }
            if last_role_check.elapsed() >= ROLE_RECHECK {
                last_role_check = tokio::time::Instant::now();
                let ns = app.namespace.clone();
                match crate::visible_vm(&app, &user, &ns, &name).await {
                    Ok((_, Role::Admin)) => {}
                    _ => break,
                }
            }
        }
    };
    let expiry = tokio::time::sleep(Duration::from_secs(
        user.expires.saturating_sub(auth::now()),
    ));
    tokio::select! {
        _ = up => {}, _ = down => {}, _ = revoked => {}, _ = expiry => {},
    }
    tracing::info!(subject=%user.subject, vm=%name, "VM console disconnected");
}
