use crate::{
    admin_bucket::{self, Feature},
    auth,
    rfb_filter::RfbFilter,
    AppState, Failure,
};
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
    let subject = &principal.user.subject;
    // Neither direction is detached. Dropping either copy future also drops
    // its stream half, including during backpressure or shutdown cancellation.
    // Every browser byte passes the RFB filter, which enforces audio admission.
    let to_vnc = async {
        let mut filter = RfbFilter::new();
        let mut filtered = Vec::new();
        while let Some(Ok(message)) = rx.next().await {
            match message {
                Message::Binary(bytes) => {
                    filtered.clear();
                    let audio = grants.is_active(Feature::Audio);
                    if let Err(violation) = filter.feed(&bytes, audio, &mut filtered) {
                        tracing::warn!(%subject, %violation, "console closed by the RFB filter");
                        break;
                    }
                    match tokio::time::timeout(Duration::from_secs(5), write.write_all(&filtered))
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
        // Stopping audio already in flight would mean parsing the server's
        // stream; closing the console is the simple, complete revocation.
        _=lost(&grants, Feature::Audio) => {
            tracing::info!(%subject, "console closed: audio admission ended");
        },
    }
    grants.revoke_all(&principal.user.subject, "console closed");
    tracing::info!(subject=%principal.user.subject, "VM console disconnected");
}

/// Resolves when `feature`, admitted on this connection, is withdrawn; never
/// for a feature the connection was not admitted.
async fn lost(grants: &admin_bucket::Grants, feature: Feature) {
    if grants.is_active(feature) {
        grants.revoked(feature).await;
    } else {
        std::future::pending().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn only_an_admitted_feature_can_be_lost() {
        let timeout = |grants: admin_bucket::Grants| async move {
            tokio::time::timeout(Duration::from_millis(50), lost(&grants, Feature::Audio))
                .await
                .is_ok()
        };
        assert!(!timeout(admin_bucket::Grants::none()).await);
        let grants = admin_bucket::Grants::new([Feature::Audio].into());
        assert!(!timeout(grants.clone()).await);
        let waiter = tokio::spawn(timeout(grants.clone()));
        tokio::task::yield_now().await;
        grants.revoke_all("subject", "test");
        assert!(waiter.await.unwrap());
    }
}
