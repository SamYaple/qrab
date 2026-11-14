use crate::{admin_bucket, auth, AppState, Failure};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
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
/// `?features=` asks for admin-bucket features; each must pass
/// [`admin_bucket::admit`] or the whole admission is refused.
pub async fn upgrade(
    State(app): State<AppState>,
    principal: auth::Principal,
    Query(query): Query<Vec<(String, String)>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, Failure> {
    let requested = admin_bucket::requested(&query)?;
    if !principal.is_bearer() && !crate::same_origin(&app, &headers) {
        return Err(Failure::forbidden());
    }
    let grants = admin_bucket::admission(&app, &principal, &requested)?;
    let principal = principal.admin()?;
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
    let features: Vec<_> = grants.active().into_iter().map(|f| f.name()).collect();
    tracing::info!(subject=%principal.user.subject, bearer=principal.is_bearer(), ?features, "VM console connected");
    Ok(ws
        .protocols(["binary"])
        .max_message_size(64 * 1024)
        .max_frame_size(64 * 1024)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            bridge(socket, stream, app, principal, grants).await;
        }))
}

/// Ends at logout, expiry or role loss for sessions, and at the token's
/// expiry for bearer admission. `grants` is the connection's admin-bucket
/// state: feature channels consult it, and it is rechecked independently of
/// the console and revoked when the console closes.
async fn bridge(
    ws: WebSocket,
    stream: UnixStream,
    app: AppState,
    principal: auth::Principal,
    grants: admin_bucket::Grants,
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
    tokio::select! {
        _=to_vnc => {}, _=to_ws => {},
        _=principal.ended(&app, auth::Role::Admin) => {},
        _=grants.supervise(&app, &principal) => {},
    }
    grants.revoke_all(&principal.user.subject, "console closed");
    tracing::info!(subject=%principal.user.subject, "VM console disconnected");
}
