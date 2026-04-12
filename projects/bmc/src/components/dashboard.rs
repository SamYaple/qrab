use dioxus::prelude::*;

use bmc::server::vm_status::get_status;
use bmc::types::{VmPhase, VmStatusResponse};

use super::console::Console;
use super::cpu_info::CpuInfo;
use super::event_log::EventLog;
use super::power_controls::PowerControls;
use super::serial::SerialConsole;
use super::status_panel::StatusPanel;
use super::util::sleep_ms;

#[component]
pub fn Dashboard() -> Element {
    let default_status = VmStatusResponse {
        phase: VmPhase::Off,
        running: None,
        status: None,
        qemu_version: None,
    };
    let mut refresh_tick = use_signal(|| 0u64);
    let mut poll_tick = use_signal(|| 0u64);

    // Drive periodic polling. Task is tied to this component's scope and is
    // dropped automatically when the component unmounts.
    use_future(move || async move {
        loop {
            sleep_ms(2000).await;
            poll_tick += 1;
        }
    });

    // use_resource cancels any in-flight future when its dependencies change
    // and is cancelled on unmount, so there's no orphaned polling.
    let status_resource = use_resource(move || async move {
        // Subscribe to both the polling tick and any manual refresh bumps.
        let _ = poll_tick();
        let _ = refresh_tick();
        get_status().await.ok()
    });

    let status_value = status_resource
        .read()
        .clone()
        .flatten()
        .unwrap_or(default_status);
    let current_phase = status_value.phase.clone();

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
                StatusPanel { status: status_value.clone() }
                PowerControls {
                    current_phase: current_phase.clone(),
                    on_action: move |_| {
                        refresh_tick += 1;
                    },
                }
                CpuInfo {
                    current_phase: current_phase.clone(),
                    refresh_tick: refresh_tick(),
                }
            }

            // Consoles
            Console {}
            SerialConsole { current_phase: current_phase.clone() }

            // Bottom section: full-width events
            EventLog { refresh_tick: refresh_tick() }
        }
    }
}
