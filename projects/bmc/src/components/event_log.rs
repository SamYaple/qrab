use dioxus::prelude::*;

use bmc::server::vm_status::get_event_history;
use bmc::types::BmcEvent;

use super::util::sleep_ms;

#[component]
pub fn EventLog(refresh_tick: u64) -> Element {
    let mut events = use_signal(Vec::<BmcEvent>::new);
    let mut event_count = use_signal(|| 0usize);

    // Poll events every 1.5s
    use_future(move || async move {
        loop {
            if let Ok(new_events) = get_event_history().await {
                let new_count = new_events.len();
                events.set(new_events);
                event_count.set(new_count);
            }
            sleep_ms(1500).await;
        }
    });

    // Also refresh on tick
    use_effect(move || {
        let _tick = refresh_tick;
        spawn(async move {
            if let Ok(new_events) = get_event_history().await {
                events.set(new_events);
            }
        });
    });

    let event_list = events.read();
    let count = event_list.len();

    rsx! {
        div { class: "panel event-log",
            div { class: "panel-header",
                h2 { "Events" }
                if count > 0 {
                    span { class: "badge", "{count}" }
                }
            }
            div { class: "event-list",
                if event_list.is_empty() {
                    div { class: "empty-state", "No events yet" }
                } else {
                    for (i, event) in event_list.iter().rev().enumerate() {
                        EventEntry {
                            key: "{event.timestamp_secs}-{event.timestamp_usecs}-{event.event}",
                            event: event.clone(),
                            is_new: i == 0,
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn EventEntry(event: BmcEvent, is_new: bool) -> Element {
    let has_data = !event.data.is_null()
        && event.data != serde_json::Value::Object(serde_json::Map::new());
    let new_class = if is_new { " event-new" } else { "" };

    rsx! {
        div { class: "event-entry{new_class}",
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
