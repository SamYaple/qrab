use dioxus::prelude::*;

use bmc::server::vm_status::get_event_history;
use bmc::types::BmcEvent;

#[component]
pub fn EventLog() -> Element {
    let history = use_resource(get_event_history);

    rsx! {
        div { class: "event-log",
            h2 { "QMP Events" }
            div { class: "event-list",
                match history.read().as_ref() {
                    Some(Ok(events)) if !events.is_empty() => rsx! {
                        for event in events.iter().rev() {
                            EventEntry { event: event.clone() }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        p { class: "muted", "No events yet." }
                    },
                    Some(Err(e)) => rsx! {
                        p { class: "error", "Error loading events: {e}" }
                    },
                    None => rsx! {
                        p { "Loading..." }
                    },
                }
            }
        }
    }
}

#[component]
fn EventEntry(event: BmcEvent) -> Element {
    let has_data = !event.data.is_null()
        && event.data != serde_json::Value::Object(serde_json::Map::new());

    rsx! {
        div { class: "event-entry",
            div { class: "event-header",
                span { class: "event-name", "{event.event}" }
                span { class: "event-time",
                    "{event.timestamp_secs}.{event.timestamp_usecs:06}"
                }
            }
            if has_data {
                pre { class: "event-data",
                    "{event.data}"
                }
            }
        }
    }
}
