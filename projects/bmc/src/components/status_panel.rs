use dioxus::prelude::*;

use bmc::types::{VmPhase, VmStatusResponse};

#[component]
pub fn StatusPanel(status: VmStatusResponse) -> Element {
    let phase_class = match &status.phase {
        VmPhase::Off => "phase-off",
        VmPhase::Running => "phase-running",
        VmPhase::Paused => "phase-paused",
        VmPhase::ShuttingDown => "phase-shutting-down",
    };

    rsx! {
        div { class: "panel status-panel",
            div { class: "panel-header",
                h2 { "Status" }
                span { class: "status-dot {phase_class}" }
            }

            div { class: "status-info",
                div { class: "status-row",
                    span { class: "label", "Phase" }
                    span { class: "value {phase_class}", "{status.phase}" }
                }
                if let Some(running) = status.running {
                    div { class: "status-row",
                        span { class: "label", "Running" }
                        span { class: "value",
                            if running { "Yes" } else { "No" }
                        }
                    }
                }
                if let Some(ref run_state) = status.status {
                    div { class: "status-row",
                        span { class: "label", "Run State" }
                        span { class: "value", "{run_state}" }
                    }
                }
                if let Some(ref version) = status.qemu_version {
                    div { class: "status-row",
                        span { class: "label", "QEMU" }
                        span { class: "value", "{version}" }
                    }
                }
            }
        }
    }
}
