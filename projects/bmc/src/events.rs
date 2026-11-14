//! QMP event history for the dashboard; the BMC UI reads Redfish's
//! `LogServices/Events` and `EventService/SSE` instead.
use crate::{auth, AppState, Failure};
use axum::{
    extract::{Query, State},
    Json,
};
use manager::pod::EventPage;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Cursor {
    after: Option<u64>,
    limit: Option<usize>,
}

pub async fn page(
    State(app): State<AppState>,
    _: auth::Principal,
    Query(cursor): Query<Cursor>,
) -> Result<Json<EventPage>, Failure> {
    Ok(Json(app.control.events(
        cursor.after,
        cursor.limit.unwrap_or(manager::pod::EVENT_PAGE_LIMIT),
    )))
}
