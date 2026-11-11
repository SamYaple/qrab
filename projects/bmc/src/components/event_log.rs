use dioxus::prelude::*;

use bmc::types::BmcEvent;

use crate::components::store::use_bmc_store;

/// Event log panel.
///
/// Reads from the central SSE-backed store. Before this refactor the
/// component opened its own `EventSource` and seeded via a
/// `get_event_history()` `#[server]` call; now the global store owns
/// the single SSE connection and every QMP event arrives as a
/// `BmcPush::QmpEvent` frame, dispatched into `store.events`.
#[component]
pub fn EventLog() -> Element {
    let store = use_bmc_store();
    let events = store.events;
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

    let time = if event.timestamp_secs < 0 || event.timestamp_usecs < 0 {
        "unavailable".to_owned()
    } else {
        format!("{}.{:06}", event.timestamp_secs, event.timestamp_usecs)
    };

    rsx! {
        tr { class: row_class,
            td { class: "cell-time",
                "{time}"
            }
            td { class: "cell-event",
                span { class: "event-tag", "{event.event}" }
            }
            td { class: "cell-data",
                if has_data {
                    code { "{data_str}" }
                } else {
                    span { class: "no-data", "\u{2014}" }
                }
            }
        }
    }
}
