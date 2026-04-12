use dioxus::prelude::*;

use bmc::types::BmcEvent;

/// Event log panel. Pure props-driven: the parent Dashboard polls
/// `get_dashboard_state()` and passes the event slice down.
#[component]
pub fn EventLog(events: Vec<BmcEvent>) -> Element {
    let count = events.len();

    rsx! {
        div { class: "panel event-panel",
            div { class: "panel-header",
                h2 { "Event Log" }
                if count > 0 {
                    span { class: "badge", "{count}" }
                }
            }

            if events.is_empty() {
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
                            for (i, event) in events.iter().rev().enumerate() {
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
