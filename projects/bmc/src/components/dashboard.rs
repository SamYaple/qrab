use dioxus::prelude::*;

use bmc::server::vm_status::get_status;
use bmc::types::{VmPhase, VmStatusResponse};

use super::cpu_info::CpuInfo;
use super::event_log::EventLog;
use super::power_controls::PowerControls;
use super::status_panel::StatusPanel;
use super::util::sleep_ms;

#[component]
pub fn Dashboard() -> Element {
    let mut status = use_signal(|| VmStatusResponse {
        phase: VmPhase::Off,
        running: None,
        status: None,
        qemu_version: None,
    });
    let mut refresh_tick = use_signal(|| 0u64);

    // Poll status every 2s
    use_future(move || async move {
        loop {
            if let Ok(s) = get_status().await {
                status.set(s);
            }
            sleep_ms(2000).await;
        }
    });

    let current_phase = status.read().phase.clone();

    rsx! {
        div { class: "bmc-dashboard",
            header { class: "bmc-header",
                div { class: "header-left",
                    h1 { "qrab" }
                    span { class: "header-sep", "/" }
                    span { class: "header-subtitle", "Virtual BMC" }
                }
            }

            // Top section: 3-column — status | controls | system
            div { class: "top-section",
                StatusPanel { status: status.read().clone() }
                PowerControls {
                    current_phase: current_phase.clone(),
                    on_action: move |_| {
                        spawn(async move {
                            if let Ok(s) = get_status().await {
                                status.set(s);
                            }
                            refresh_tick += 1;
                        });
                    },
                }
                CpuInfo {
                    current_phase: current_phase.clone(),
                    refresh_tick: refresh_tick(),
                }
            }

            // Bottom section: full-width events
            EventLog { refresh_tick: refresh_tick() }
        }
    }
}
