use crate::{auth, AppState, Failure};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::HeaderMap,
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};
use tower_sessions::{Session, SessionStore};

pub async fn upgrade(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, Failure> {
    let user = auth::user(&app, &session).await?;
    if !crate::same_origin(&app, &headers) {
        return Err(Failure::forbidden());
    }
    let permit = app
        .consoles
        .clone()
        .try_acquire_owned()
        .map_err(|_| Failure::unavailable())?;
    let stream = app
        .control
        .console()
        .await
        .map_err(|_| Failure::unavailable())?;
    let id = session.id().ok_or_else(Failure::unauthorized)?;
    tracing::info!(subject=%user.subject, "VM console connected");
    Ok(ws
        .protocols(["binary"])
        .max_message_size(64 * 1024)
        .max_frame_size(64 * 1024)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            bridge(socket, stream, app, id, user).await;
        }))
}

async fn bridge(
    ws: WebSocket,
    stream: UnixStream,
    app: AppState,
    id: tower_sessions::session::Id,
    user: auth::User,
) {
    let (mut tx, mut rx) = ws.split();
    let (mut read, mut write) = stream.into_split();
    // Neither direction is detached. Dropping either copy future also drops
    // its stream half, including during backpressure or shutdown cancellation.
    let to_vnc = async {
        while let Some(Ok(message)) = rx.next().await {
            match message {
                Message::Binary(bytes) => {
                    match tokio::time::timeout(Duration::from_secs(5), write.write_all(&bytes))
                        .await
                    {
                        Ok(Ok(())) => {}
                        _ => break,
                    }
                }
                Message::Ping(_) | Message::Pong(_) => {}
                _ => break,
            }
        }
    };
    let to_ws = async {
        let mut bytes = vec![0; 16 * 1024];
        loop {
            let n = match read.read(&mut bytes).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            match tokio::time::timeout(
                Duration::from_secs(5),
                tx.send(Message::Binary(bytes[..n].to_vec().into())),
            )
            .await
            {
                Ok(Ok(())) => {}
                _ => break,
            }
        }
    };
    let revoked = async {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let stored = app.sessions.load(&id).await.ok().flatten();
            let current: Option<auth::User> = stored
                .and_then(|r| r.data.get("user").cloned())
                .and_then(|v| serde_json::from_value(v).ok());
            if !current.is_some_and(|u| {
                u.subject == user.subject && u.csrf == user.csrf && auth::authorized(&app.oidc, &u)
            }) {
                break;
            }
        }
    };
    tokio::select! {
        _=to_vnc => {}, _=to_ws => {}, _=revoked => {},
        _=tokio::time::sleep(Duration::from_secs(user.expires.saturating_sub(auth::now()))) => {},
    }
    tracing::info!(subject=%user.subject, "VM console disconnected");
}
