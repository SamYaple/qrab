use dioxus::prelude::*;

use bmc::server::vm_status::get_event_history;
use bmc::types::BmcEvent;

use super::util::sleep_ms;

#[component]
pub fn EventLog(refresh_tick: u64) -> Element {
    let mut poll_tick = use_signal(|| 0u64);
    let mut events = use_signal(Vec::<BmcEvent>::new);

    // Periodic ticker (every 1.5s). Task is scope-bound and auto-cancels on unmount.
    use_future(move || async move {
        loop {
            sleep_ms(1500).await;
            poll_tick += 1;
        }
    });

    // use_resource writes into `events` only on successful fetches so a
    // transient failure doesn't wipe the previous list.
    let _events_resource = use_resource(use_reactive!(|refresh_tick| async move {
        let _ = poll_tick();
        let _ = refresh_tick;
        if let Ok(new_events) = get_event_history().await {
            events.set(new_events);
        }
    }));

    let event_list = events.read();
    let count = event_list.len();

    rsx! {
        div { class: "panel event-panel",
            div { class: "panel-header",
                h2 { "Event Log" }
                if count > 0 {
                    span { class: "badge", "{count}" }
                }
            }

            if event_list.is_empty() {
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
                            for (i, event) in event_list.iter().rev().enumerate() {
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
