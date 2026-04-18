//! Live queue-depth + OOB-availability badge.
//!
//! Self-contained: polls `qmp_queue_status` every 500ms so the dashboard
//! shows queue pressure even when no command has been submitted recently.

use dioxus::prelude::*;

use bmc::server::qmp_exec::qmp_queue_status;
use bmc::types::QueueStatus;

use crate::components::util::sleep_ms;

const POLL_INTERVAL_MS: u64 = 500;

#[component]
pub fn QueueBadge() -> Element {
    let mut status = use_signal(QueueStatus::default);
    use_future(move || async move {
        loop {
            if let Ok(s) = qmp_queue_status().await {
                status.set(s);
            }
            sleep_ms(POLL_INTERVAL_MS).await;
        }
    });

    let QueueStatus {
        depth,
        capacity,
        oob_supported,
    } = status.read().clone();

    // Color by depth: green <4, yellow 4-6, red 7-8.
    let depth_class = if depth >= 7 {
        "queue-red"
    } else if depth >= 4 {
        "queue-yellow"
    } else {
        "queue-green"
    };

    let oob_label = if oob_supported {
        "OOB"
    } else {
        "OOB unavailable"
    };
    let oob_class = if oob_supported {
        "queue-oob-ok"
    } else {
        "queue-oob-na"
    };

    rsx! {
        span { class: "queue-badge",
            span { class: "queue-depth {depth_class}",
                "Queue {depth}/{capacity}"
            }
            span { class: "queue-oob {oob_class}", " · {oob_label}" }
        }
    }
}
