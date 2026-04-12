use std::collections::VecDeque;

use dioxus::prelude::*;

use bmc::server::vm_status::get_event_history;
use bmc::types::BmcEvent;

/// Maximum number of events to retain in the client-side ring buffer.
/// Mirrors the server-side `EVENT_HISTORY_CAP` in state.rs so the UI can
/// never hold more history than the server would have emitted.
const EVENT_HISTORY_CAP: usize = 1000;

/// Event log panel. Self-managed: fetches the current history on mount via
/// `get_event_history()` to seed the UI, then opens an EventSource on
/// `/api/events` and appends new events as they arrive. No polling, no
/// props — the dashboard renders `EventLog {}` with no arguments.
#[component]
pub fn EventLog() -> Element {
    // VecDeque bounded at EVENT_HISTORY_CAP. Rendering walks it in reverse
    // (newest first), matching the previous behavior.
    let mut events = use_signal(VecDeque::<BmcEvent>::new);

    // Seed with existing history on mount. Done exactly once via use_future
    // (no deps → runs to completion; the spawned task for SSE is separate).
    use_future(move || async move {
        if let Ok(history) = get_event_history().await {
            let mut buf = VecDeque::with_capacity(history.len().min(EVENT_HISTORY_CAP));
            for ev in history.into_iter().rev().take(EVENT_HISTORY_CAP).rev() {
                buf.push_back(ev);
            }
            events.set(buf);
        }

        // Open the SSE stream. JS parses each event frame as JSON and pushes
        // it back to Rust via dioxus.send(); Rust receives it as a serde_json
        // Value and deserializes to BmcEvent. Typing on the JS side would
        // require stringifying again, so we keep it as a raw Value and let
        // serde handle the conversion in Rust.
        //
        // Dioxus 0.7 `Eval` is Copy + !Send. `recv::<T>()` takes &mut self
        // and deserializes the JSON pushed via `dioxus.send(...)`. The eval
        // handle is kept alive for the lifetime of this task; when the
        // component unmounts, the task is cancelled and the EventSource
        // closes via the `beforeunload`/page lifecycle.
        let mut eval_handle = document::eval(
            r#"
            (function() {
                if (window._bmcEventSource) {
                    try { window._bmcEventSource.close(); } catch (e) {}
                    window._bmcEventSource = null;
                }
                const es = new EventSource('/api/events');
                window._bmcEventSource = es;
                es.onmessage = (e) => {
                    try {
                        const obj = JSON.parse(e.data);
                        dioxus.send(obj);
                    } catch (err) {
                        console.error('SSE parse error:', err);
                    }
                };
                es.onerror = (err) => {
                    console.warn('SSE error (browser will auto-reconnect):', err);
                };
            })();
            "#,
        );

        loop {
            match eval_handle.recv::<BmcEvent>().await {
                Ok(event) => {
                    // Scope the write guard so it's dropped before the next
                    // await. Dioxus signals are RefCell-backed; holding the
                    // write across an await would panic if the render path
                    // tries to read concurrently.
                    {
                        let mut buf = events.write();
                        buf.push_back(event);
                        while buf.len() > EVENT_HISTORY_CAP {
                            buf.pop_front();
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("SSE recv error: {e}");
                    break;
                }
            }
        }
    });

    let events_ref = events.read();
    let count = events_ref.len();

    rsx! {
        div { class: "panel event-panel",
            div { class: "panel-header",
                h2 { "Event Log" }
                if count > 0 {
                    span { class: "badge", "{count}" }
                }
            }

            if events_ref.is_empty() {
                div { class: "empty-state", "Waiting for events..." }
            } else {
                div { class: "event-table-wrap",
                    table { class: "event-table",
                        thead {
                            tr {
                                th { class: "col-time", "Timestamp" }
                                th { class: "col-event", "Event" }
                                th { class: "col-data", "Data" }
                            }
                        }
                        tbody {
                            for (i, event) in events_ref.iter().rev().enumerate() {
                                EventRow {
                                    key: "{event.timestamp_secs}-{event.timestamp_usecs}-{event.event}",
                                    event: event.clone(),
                                    is_newest: i == 0,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn EventRow(event: BmcEvent, is_newest: bool) -> Element {
    let has_data = !event.data.is_null()
        && event.data != serde_json::Value::Object(serde_json::Map::new());
    let row_class = if is_newest { "event-row newest" } else { "event-row" };

    let data_str = if has_data {
        serde_json::to_string_pretty(&event.data).unwrap_or_default()
    } else {
        String::new()
    };

    rsx! {
        tr { class: row_class,
            td { class: "cell-time",
                "{event.timestamp_secs}.{event.timestamp_usecs:06}"
            }
            td { class: "cell-event",
                span { class: "event-tag", "{event.event}" }
            }
            td { class: "cell-data",
                if has_data {
                    code { "{data_str}" }
                } else {
                    span { class: "no-data", "—" }
                }
            }
        }
    }
}
