use dioxus::prelude::*;

use bmc::server::vm_status::get_status;
use bmc::types::VmPhase;

use super::cpu_info::CpuInfo;
use super::event_log::EventLog;
use super::power_controls::PowerControls;
use super::status_panel::StatusPanel;

#[component]
pub fn Dashboard() -> Element {
    let mut status = use_resource(get_status);

    let current_phase = status
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|s| s.phase.clone())
        .unwrap_or(VmPhase::Off);

    rsx! {
        div { class: "bmc-dashboard",
            header { class: "bmc-header",
                h1 { "qrab Virtual BMC" }
            }

            div { class: "bmc-grid",
                div { class: "bmc-top-row",
                    StatusPanel {}
                    PowerControls {
                        current_phase: current_phase.clone(),
                        on_action: move |_| {
                            status.restart();
                        },
                    }
                }
                div { class: "bmc-bottom-row",
                    CpuInfo { current_phase: current_phase.clone() }
                    EventLog {}
                }
            }
        }
    }
}
