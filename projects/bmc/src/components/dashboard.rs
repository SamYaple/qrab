use dioxus::prelude::*;

use bmc::server::vm_status::get_dashboard_state;
use bmc::types::DashboardState;

use super::config_panel::ConfigPanel;
use super::console::Console;
use super::cpu_info::CpuInfo;
use super::event_log::EventLog;
use super::power_controls::PowerControls;
use super::security_panel::SecurityPanel;
use super::serial::SerialConsole;
use super::status_panel::StatusPanel;
use super::util::sleep_ms;

/// Dashboard polling interval. Was 1s after Phase 2C consolidation —
/// too aggressive given that the combined endpoint issues a QMP
/// `query-status` and a 64 KiB serial-tail read each tick. Serial output
/// stays roughly live at 3s and the dashboard feels responsive. Manual
/// refresh via `PowerControls` bumps `refresh_tick` to force an immediate
/// refetch outside the periodic cycle.
const POLL_INTERVAL_MS: u64 = 3000;

#[component]
pub fn Dashboard() -> Element {
    let mut refresh_tick = use_signal(|| 0u64);
    let mut poll_tick = use_signal(|| 0u64);

    // Single poll loop at POLL_INTERVAL_MS. Task is tied to this component's
    // scope and is dropped automatically when the component unmounts.
    use_future(move || async move {
        loop {
            sleep_ms(POLL_INTERVAL_MS).await;
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
        .unwrap_or_default();

    let DashboardState {
        status,
        cpu_info,
        kvm_info,
        console_available,
        serial_tail,
        sev_status,
        tpm_status,
        attestation: _attestation,
        capabilities,
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
                SecurityPanel {
                    sev_status: sev_status.clone(),
                    tpm_status: tpm_status.clone(),
                    capabilities: capabilities.clone(),
                }
                ConfigPanel {
                    current_phase: current_phase.clone(),
                    capabilities: capabilities.clone(),
                }
            }

            // Consoles
            Console { console_available }
            SerialConsole {
                current_phase: current_phase.clone(),
                serial_tail: serial_tail.clone(),
            }

            // Bottom section: full-width events. EventLog is self-managed:
            // it fetches initial history via `get_event_history()` and then
            // streams new events over SSE (`/api/events`). No props needed.
            EventLog {}
        }
    }
}
