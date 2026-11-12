//! QMP event history and live Server-Sent Events, for viewers and admins.
use crate::{auth, AppState, Failure};
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures::{Stream, StreamExt};
use manager::pod::{EventPage, VmEvent};
use serde::Deserialize;
use std::{collections::VecDeque, convert::Infallible, time::Duration};
use tokio::sync::OwnedSemaphorePermit;

const STREAM_BATCH: usize = 64;

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

struct Follow {
    app: AppState,
    principal: auth::Principal,
    cursor: u64,
    queue: VecDeque<VmEvent>,
    missed: u64,
    /// Last QMP-client dropped count the browser has (from its page read).
    dropped: u64,
    _permit: OwnedSemaphorePermit,
}

/// Same-origin EventSource requests carry no Origin header, and SameSite=Lax
/// withholds the cookie from cross-site subresources; a foreign Origin is
/// refused. Bearer requests carry no ambient credential and skip that check.
/// The stream ends at the credential's expiry, or when a session is revoked.
pub async fn stream(
    State(app): State<AppState>,
    principal: auth::Principal,
    headers: HeaderMap,
    Query(cursor): Query<Cursor>,
) -> Result<Response, Failure> {
    if !principal.is_bearer()
        && headers.contains_key("origin")
        && !crate::same_origin(&app, &headers)
    {
        return Err(Failure::forbidden());
    }
    let permit = app
        .event_streams
        .clone()
        .try_acquire_owned()
        .map_err(|_| Failure::unavailable("Too many live event streams are open."))?;
    // A reconnecting EventSource resumes from its last delivered sequence.
    let resume = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    let cursor = resume
        .or(cursor.after)
        .unwrap_or_else(|| app.control.events(None, 1).latest);
    tracing::info!(subject=%principal.user.subject, role=?principal.role, bearer=principal.is_bearer(), "VM event stream opened");
    let app_dropped = app.control.events(None, 1).qmp_dropped;
    let ended = {
        let (app, principal) = (app.clone(), principal.clone());
        async move { principal.ended(&app, auth::Role::Viewer).await }
    };
    let follow = Follow {
        app,
        principal,
        cursor,
        queue: VecDeque::new(),
        missed: 0,
        dropped: app_dropped,
        _permit: permit,
    };
    Ok(Sse::new(events(follow).take_until(ended))
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response())
}

fn events(follow: Follow) -> impl Stream<Item = Result<Event, Infallible>> {
    futures::stream::unfold(follow, |mut follow| async move {
        loop {
            if follow.missed > 0 {
                let missed = std::mem::take(&mut follow.missed);
                return Some((
                    Ok(Event::default().event("missed").data(missed.to_string())),
                    follow,
                ));
            }
            if let Some(event) = follow.queue.pop_front() {
                let data = serde_json::to_string(&event).unwrap_or_default();
                return Some((
                    Ok(Event::default()
                        .event("qmp")
                        .id(event.seq.to_string())
                        .data(data)),
                    follow,
                ));
            }
            // Authorization is rechecked before every batch and at least once
            // a second while idle; logout, expiry or delisting end the stream.
            if !follow
                .principal
                .still_current(&follow.app, auth::Role::Viewer)
                .await
            {
                tracing::info!(subject=%follow.principal.user.subject, "VM event stream closed");
                return None;
            }
            let page = follow.app.control.events(Some(follow.cursor), STREAM_BATCH);
            follow.cursor = page.next;
            follow.missed = page.missed;
            if follow.dropped != page.qmp_dropped {
                follow.dropped = page.qmp_dropped;
                return Some((
                    Ok(Event::default()
                        .event("qmp_dropped")
                        .data(page.qmp_dropped.to_string())),
                    follow_with(follow, page.events),
                ));
            }
            follow.queue.extend(page.events);
            if follow.queue.is_empty() && follow.missed == 0 {
                let _ = tokio::time::timeout(
                    Duration::from_secs(1),
                    follow.app.control.wait_events(follow.cursor),
                )
                .await;
            }
        }
    })
}

/// Keep a page's events queued behind a frame emitted first.
fn follow_with(mut follow: Follow, events: Vec<VmEvent>) -> Follow {
    follow.queue.extend(events);
    follow
}
