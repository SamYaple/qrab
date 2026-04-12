use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;
use qapi_protocol::QmpEvent;

use crate::state::BmcState;

/// Create an Axum router with the SSE events endpoint.
pub fn events_router(state: Arc<BmcState>) -> axum::Router {
    axum::Router::new()
        .route("/api/events", axum::routing::get(sse_handler))
        .with_state(state)
}

async fn sse_handler(
    State(state): State<Arc<BmcState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.event_tx.subscribe();
    let stream = async_stream::stream! {
        while let Ok(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event) {
                yield Ok(Event::default().data(json));
            }
        }
    };
    Sse::new(stream)
}

/// Get event history as a server function (for initial page load).
pub async fn get_event_history(state: &BmcState) -> Vec<QmpEvent> {
    let history = state.event_history.lock().await;
    history.iter().cloned().collect()
}
