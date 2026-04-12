use dioxus::prelude::*;

use bmc::server::vm_status::get_dashboard_state;
use bmc::types::{
    BmcEvent, CpuInfoResponse, DashboardState, KvmInfoResponse, VmPhase, VmStatusResponse,
};

use super::console::Console;
use super::cpu_info::CpuInfo;
use super::event_log::EventLog;
use super::power_controls::PowerControls;
use super::serial::SerialConsole;
use super::status_panel::StatusPanel;
use super::util::sleep_ms;

/// Default DashboardState used before the first fetch resolves (or if a
/// fetch fails). Matches an Off VM with no history.
fn empty_dashboard_state() -> DashboardState {
    DashboardState {
        status: VmStatusResponse {
            phase: VmPhase::Off,
            running: None,
            status: None,
            qemu_version: None,
        },
        cpu_info: CpuInfoResponse {
            guest_cpus: Vec::new(),
            host_threads: 0,
        },
        kvm_info: KvmInfoResponse {
            enabled: false,
            present: false,
        },
        console_available: false,
        serial_tail: String::new(),
        events: Vec::<BmcEvent>::new(),
    }
}

#[component]
pub fn Dashboard() -> Element {
    let mut refresh_tick = use_signal(|| 0u64);
    let mut poll_tick = use_signal(|| 0u64);

    // Single poll loop at 1s — the tightest of the original per-component
    // rates (serial output was 1s). Task is tied to this component's scope
    // and is dropped automatically when the component unmounts.
    use_future(move || async move {
        loop {
            sleep_ms(1000).await;
            poll_tick += 1;
        }
    });

    // One use_resource drives every dashboard slice. use_resource cancels any
    // in-flight future when its dependencies change and is cancelled on
    // unmount, so there's no orphaned polling.
    let state_resource = use_resource(move || async move {
        // Subscribe to both the polling tick and any manual refresh bumps.
        let _ = poll_tick();
        let _ = refresh_tick();
        get_dashboard_state().await.ok()
    });

    let state_value = state_resource
        .read()
        .clone()
        .flatten()
        .unwrap_or_else(empty_dashboard_state);

    let DashboardState {
        status,
        cpu_info,
        kvm_info,
        console_available,
        serial_tail,
        events,
    } = state_value;

    let current_phase = status.phase.clone();

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
                StatusPanel { status: status.clone() }
                PowerControls {
                    current_phase: current_phase.clone(),
                    on_action: move |_| {
                        refresh_tick += 1;
                    },
                }
                CpuInfo {
                    current_phase: current_phase.clone(),
                    cpu_info: cpu_info.clone(),
                    kvm_info: kvm_info.clone(),
                }
            }

            // Consoles
            Console { console_available }
            SerialConsole {
                current_phase: current_phase.clone(),
                serial_tail: serial_tail.clone(),
            }

            // Bottom section: full-width events
            EventLog { events: events.clone() }
        }
    }
}
