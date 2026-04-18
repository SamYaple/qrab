use dioxus::prelude::*;

use bmc::server::qmp_exec::list_commands;
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
use super::store::use_bmc_store;

#[component]
pub fn Dashboard() -> Element {
    let layout = use_signal(DashboardLayout::default);
    let store = use_bmc_store();

    // Dashboard state flows in from the SSE-backed store. No polling
    // loop; the server pushes a fresh `Dashboard` frame whenever the
    // underlying slice changes (power transitions, QMP lifecycle events,
    // config mutations).
    let state_value: DashboardState = store.dashboard.read().clone();

    // Schema-driven QMP command list is fetched once on mount. Commands
    // are immutable for the lifetime of the server process, so this is
    // pure cache warmup — not a poll.
    let commands_resource =
        use_resource(|| async move { list_commands().await.ok().unwrap_or_default() });
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
                                        // Server pushes a fresh Dashboard
                                        // frame after every power action,
                                        // so the callback is a no-op now.
                                        on_action: move |_| {},
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
