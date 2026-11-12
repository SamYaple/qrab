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

/// Admission authenticates before the upgrade. Cookie sessions need the exact
/// Origin; a bearer token is not ambient browser state and needs none.
pub async fn upgrade(
    State(app): State<AppState>,
    principal: auth::Principal,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, Failure> {
    let principal = principal.admin()?;
    if !principal.is_bearer() && !crate::same_origin(&app, &headers) {
        return Err(Failure::forbidden());
    }
    let permit = app
        .consoles
        .clone()
        .try_acquire_owned()
        .map_err(|_| Failure::unavailable("Too many consoles are open for this VM."))?;
    let stream = app
        .control
        .console()
        .await
        .map_err(|error| Failure::vm(&error, "The VM console is unavailable."))?;
    tracing::info!(subject=%principal.user.subject, bearer=principal.is_bearer(), "VM console connected");
    Ok(ws
        .protocols(["binary"])
        .max_message_size(64 * 1024)
        .max_frame_size(64 * 1024)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            bridge(socket, stream, app, principal).await;
        }))
}

/// Ends at logout, expiry or role loss for sessions, and at the token's
/// expiry for bearer admission.
async fn bridge(ws: WebSocket, stream: UnixStream, app: AppState, principal: auth::Principal) {
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
    tokio::select! {
        _=to_vnc => {}, _=to_ws => {},
        _=principal.ended(&app, auth::Role::Admin) => {},
    }
    tracing::info!(subject=%principal.user.subject, "VM console disconnected");
}
