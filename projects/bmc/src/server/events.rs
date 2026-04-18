use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;

use crate::state::BmcState;
use crate::types::BmcPush;

/// Create an Axum router with the SSE events endpoint.
pub fn events_router(state: Arc<BmcState>) -> axum::Router {
    axum::Router::new()
        .route("/api/events", axum::routing::get(sse_handler))
        .with_state(state)
}

/// Server-Sent Events endpoint.
///
/// Frame sequence:
/// 1. `BmcPush::Snapshot` — full initial state (dashboard + queue +
///    config + event history). Replaces the old "seed via several
///    `#[server]` fetches" pattern so the client can render everything
///    after a single network round-trip.
/// 2. `BmcPush::{Dashboard,Queue,Config,QmpEvent,Heartbeat}` — every
///    subsequent server-side change is pushed as a typed frame. The
///    client keeps a single `EventSource` open and updates its store as
///    frames arrive.
///
/// Broadcast lag handling: if a slow client falls behind the 256-slot
/// ring, `rx.recv()` returns `Lagged`. We drop the stream — browsers
/// auto-reconnect, and the next connection starts with a fresh Snapshot
/// so no state is lost.
async fn sse_handler(
    State(state): State<Arc<BmcState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Subscribe BEFORE building the snapshot so mutations concurrent
    // with our snapshot build are buffered on the broadcast and replayed
    // into the stream — no lost push between snapshot and first recv.
    let mut rx = state.push_tx.subscribe();
    let snapshot = state.snapshot().await;

    let stream = async_stream::stream! {
        // Frame 1: initial snapshot.
        if let Ok(json) = serde_json::to_string(&BmcPush::Snapshot(snapshot)) {
            yield Ok(Event::default().data(json));
        }
        // Frames 2..: live broadcast.
        loop {
            match rx.recv().await {
                Ok(push) => {
                    if let Ok(json) = serde_json::to_string(&push) {
                        yield Ok(Event::default().data(json));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("SSE client lagged by {n} frames, dropping stream");
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Sse::new(stream)
}
