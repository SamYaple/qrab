use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;

use crate::state::BmcState;
use crate::types::BmcEvent;

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
            // Convert the internal qapi-protocol QmpEvent into the frontend
            // DTO before serialization. This mirrors `get_event_history` in
            // vm_status.rs and keeps qapi-protocol types off the wire.
            let dto = BmcEvent {
                event: event.event,
                timestamp_secs: event.timestamp.seconds,
                timestamp_usecs: event.timestamp.microseconds,
                data: event.data,
            };
            if let Ok(json) = serde_json::to_string(&dto) {
                yield Ok(Event::default().data(json));
            }
        }
    };
    Sse::new(stream)
}
