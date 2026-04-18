//! Live queue-depth + OOB-availability badge.
//!
//! Reads from the central SSE-backed store. No self-polling — before
//! this refactor the badge had its own 500 ms `qmp_queue_status` loop
//! that ran in parallel to the parent `QmpConsole`'s identical loop,
//! doubling HTTP load. Now both read the same `store.queue` signal.

use dioxus::prelude::*;

use bmc::types::QueueStatus;

use crate::components::store::use_bmc_store;

#[component]
pub fn QueueBadge() -> Element {
    let store = use_bmc_store();
    let QueueStatus {
        depth,
        capacity,
        oob_supported,
    } = store.queue.read().clone();

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
            span { class: "queue-oob {oob_class}", " \u{00b7} {oob_label}" }
        }
    }
}
