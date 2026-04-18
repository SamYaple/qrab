use dioxus::prelude::*;

use bmc::server::qmp_exec::list_commands;
use bmc::server::vm_status::get_dashboard_state;
use bmc::types::{CommandDto, DashboardState};

use super::attestation_panel::AttestationPanel;
use super::config_panel::ConfigPanel;
use super::console::Console;
use super::cpu_info::CpuInfo;
use super::event_log::EventLog;
use super::layout::{DashboardLayout, PanelType};
use super::power_controls::PowerControls;
use super::qmp_console::QmpConsole;
use super::security_hero::SecurityHero;
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
    let layout = use_signal(DashboardLayout::default);

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

    // Fetch the schema-driven QMP command list once. Piggy-back on Dashboard's
    // working `use_resource` pattern — fetching it from QmpConsole directly
    // never fired on client under Dioxus 0.7 fullstack hydration.
    let commands_resource = use_resource(move || async move {
        let _ = refresh_tick();
        list_commands().await.ok().unwrap_or_default()
    });
    let commands: Vec<CommandDto> = commands_resource.read().clone().unwrap_or_default();

    let DashboardState {
        status,
        cpu_info,
        kvm_info,
        console_available,
        serial_tail,
        sev_status,
        sev_capabilities,
        tpm_status,
        attestation: _attestation,
    } = state_value;

    let current_phase = status.phase.clone();
    let layout_panels = layout.read().panels.clone();

    rsx! {
        div { class: "bmc-dashboard",
            header { class: "bmc-header",
                div { class: "header-left",
                    h1 { "qrab" }
                    span { class: "header-sep", "/" }
                    span { class: "header-subtitle", "Virtual BMC" }
                }
            }

            // Layout-driven panel rendering
            div { class: "dashboard-grid",
                for panel_cfg in layout_panels.iter() {
                    if !panel_cfg.collapsed {
                        div { class: "{panel_cfg.width.css_class()}",
                            match panel_cfg.panel {
                                PanelType::SecurityHero => rsx! {
                                    SecurityHero {
                                        sev_status: sev_status.clone(),
                                        tpm_status: tpm_status.clone(),
                                    }
                                },
                                PanelType::Attestation => rsx! {
                                    AttestationPanel {
                                        sev_status: sev_status.clone(),
                                    }
                                },
                                PanelType::PowerControls => rsx! {
                                    PowerControls {
                                        current_phase: current_phase.clone(),
                                        on_action: move |_| {
                                            refresh_tick += 1;
                                        },
                                    }
                                },
                                PanelType::Status => rsx! {
                                    StatusPanel { status: status.clone() }
                                },
                                PanelType::CpuInfo => rsx! {
                                    CpuInfo {
                                        current_phase: current_phase.clone(),
                                        cpu_info: cpu_info.clone(),
                                        kvm_info: kvm_info.clone(),
                                    }
                                },
                                PanelType::Config => rsx! {
                                    ConfigPanel {
                                        current_phase: current_phase.clone(),
                                        sev_status: sev_status.clone(),
                                        sev_capabilities: sev_capabilities.clone(),
                                        tpm_status: tpm_status.clone(),
                                        kvm_info: kvm_info.clone(),
                                    }
                                },
                                PanelType::Console => rsx! {
                                    Console { console_available }
                                },
                                PanelType::Serial => rsx! {
                                    SerialConsole {
                                        current_phase: current_phase.clone(),
                                        serial_tail: serial_tail.clone(),
                                    }
                                },
                                PanelType::Events => rsx! {
                                    EventLog {}
                                },
                                PanelType::QmpConsole => rsx! {
                                    QmpConsole { commands: commands.clone() }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
