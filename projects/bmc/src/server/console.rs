use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::state::BmcState;

/// Create an Axum router with the WebSocket-to-VNC proxy endpoint.
pub fn console_router(state: Arc<BmcState>) -> axum::Router {
    axum::Router::new()
        .route("/api/console", axum::routing::get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<BmcState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(ws: WebSocket, state: Arc<BmcState>) {
    let vnc_path = {
        let guard = state.vnc_sock.lock().await;
        match guard.clone() {
            Some(p) => p,
            None => return,
        }
    };

    let vnc_stream = match UnixStream::connect(&vnc_path).await {
        Ok(s) => s,
        Err(_) => return,
    };

    let (vnc_read, mut vnc_write) = tokio::io::split(vnc_stream);
    let (mut ws_tx, mut ws_rx) = ws.split();

    use futures::SinkExt;
    use futures::StreamExt;

    // WS -> VNC
    let ws_to_vnc = async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Binary(data) => {
                    if vnc_write.write_all(&data).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    };

    // VNC -> WS
    let vnc_to_ws = async move {
        let mut vnc_read = vnc_read;
        let mut buf = vec![0u8; 8192];
        loop {
            match vnc_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if ws_tx.send(Message::Binary(buf[..n].to_vec().into())).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    tokio::select! {
        _ = ws_to_vnc => {}
        _ = vnc_to_ws => {}
    };
}
