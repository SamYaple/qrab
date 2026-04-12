use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
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

/// Bidirectional proxy: WebSocket binary frames <-> VNC Unix socket bytes.
///
/// On either direction completing (peer closed, error, etc.), the other
/// direction is aborted and joined so no half stays open holding the VNC
/// UnixStream or the WebSocket sink.
async fn handle_socket(ws: WebSocket, state: Arc<BmcState>) {
    let vnc_path = match state.vnc_sock().await {
        Some(p) => p,
        None => {
            tracing::debug!("console WS opened with no vnc_sock; closing");
            let _ = ws.split().0.close().await;
            return;
        }
    };

    let vnc_stream = match UnixStream::connect(&vnc_path).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(?vnc_path, error = %e, "console WS: VNC connect failed");
            let _ = ws.split().0.close().await;
            return;
        }
    };

    let (vnc_read, vnc_write) = tokio::io::split(vnc_stream);
    let (ws_tx, ws_rx) = ws.split();

    let mut ws_to_vnc = tokio::spawn(copy_ws_to_vnc(ws_rx, vnc_write));
    let mut vnc_to_ws = tokio::spawn(copy_vnc_to_ws(vnc_read, ws_tx));

    tokio::select! {
        _ = &mut ws_to_vnc => {
            vnc_to_ws.abort();
            let _ = vnc_to_ws.await;
        }
        _ = &mut vnc_to_ws => {
            ws_to_vnc.abort();
            let _ = ws_to_vnc.await;
        }
    }
}

/// Read binary frames from the WebSocket and forward bytes to the VNC socket.
/// On exit, shut down the VNC write half so the peer sees a clean close.
async fn copy_ws_to_vnc(
    mut ws_rx: SplitStream<WebSocket>,
    mut vnc_write: WriteHalf<UnixStream>,
) {
    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(Message::Binary(data)) => {
                if vnc_write.write_all(&data).await.is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(_) => {} // ignore Text/Ping/Pong
        }
    }
    let _ = vnc_write.shutdown().await;
}

/// Read bytes from the VNC socket and forward as binary frames to the WebSocket.
/// On exit, close the WebSocket sink so the client sees a clean close.
async fn copy_vnc_to_ws(
    mut vnc_read: ReadHalf<UnixStream>,
    mut ws_tx: SplitSink<WebSocket, Message>,
) {
    let mut buf = vec![0u8; 8192];
    loop {
        match vnc_read.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                if ws_tx
                    .send(Message::Binary(buf[..n].to_vec().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let _ = ws_tx.close().await;
}
