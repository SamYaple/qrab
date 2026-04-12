use dioxus::prelude::*;

use bmc::server::vm_status::get_status;
use bmc::types::{VmPhase, VmStatusResponse};

#[component]
pub fn StatusPanel() -> Element {
    let status = use_resource(get_status);

    rsx! {
        div { class: "status-panel",
            h2 { "VM Status" }
            match status.read().as_ref() {
                Some(Ok(s)) => rsx! { StatusInfo { status: s.clone() } },
                Some(Err(e)) => rsx! { p { class: "error", "Error: {e}" } },
                None => rsx! { p { "Loading..." } },
            }
        }
    }
}

#[component]
fn StatusInfo(status: VmStatusResponse) -> Element {
    let phase_class = match &status.phase {
        VmPhase::Off => "phase-off",
        VmPhase::Preconfig => "phase-preconfig",
        VmPhase::Running => "phase-running",
        VmPhase::Paused => "phase-paused",
        VmPhase::ShuttingDown => "phase-shutting-down",
    };

    rsx! {
        div { class: "status-info",
            div { class: "status-row",
                span { class: "label", "Phase:" }
                span { class: "value {phase_class}", "{status.phase}" }
            }
            if let Some(running) = status.running {
                div { class: "status-row",
                    span { class: "label", "Running:" }
                    span { class: "value", "{running}" }
                }
            }
            if let Some(ref run_state) = status.status {
                div { class: "status-row",
                    span { class: "label", "Run State:" }
                    span { class: "value", "{run_state}" }
                }
            }
            if let Some(ref version) = status.qemu_version {
                div { class: "status-row",
                    span { class: "label", "QEMU Version:" }
                    span { class: "value", "{version}" }
                }
            }
        }
    }
}
