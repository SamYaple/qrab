use dioxus::prelude::*;

use bmc::server::vm_actions;
use bmc::server::vm_status::get_status;
use bmc::types::{VmPhase, VmStatusResponse};

#[component]
pub fn Dashboard() -> Element {
    let status = use_resource(get_status);
    let error_msg = use_signal(|| Option::<String>::None);

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

            if let Some(err) = error_msg() {
                div { class: "error-banner",
                    "{err}"
                }
            }

            div { class: "status-panel",
                h2 { "VM Status" }
                match status.read().as_ref() {
                    Some(Ok(s)) => rsx! { StatusDisplay { status: s.clone() } },
                    Some(Err(e)) => rsx! { p { class: "error", "Error: {e}" } },
                    None => rsx! { p { "Loading..." } },
                }
            }

            div { class: "power-controls",
                h2 { "Controls" }
                ActionButton {
                    label: "Power On",
                    disabled: current_phase != VmPhase::Off,
                    action: "power_on",
                    error_msg,
                    status,
                }
                ActionButton {
                    label: "Start VM",
                    disabled: current_phase != VmPhase::Preconfig,
                    action: "start_vm",
                    error_msg,
                    status,
                }
                ActionButton {
                    label: "Pause",
                    disabled: current_phase != VmPhase::Running,
                    action: "pause",
                    error_msg,
                    status,
                }
                ActionButton {
                    label: "Resume",
                    disabled: current_phase != VmPhase::Paused,
                    action: "resume",
                    error_msg,
                    status,
                }
                ActionButton {
                    label: "Reset",
                    disabled: current_phase != VmPhase::Running && current_phase != VmPhase::Paused,
                    action: "reset",
                    error_msg,
                    status,
                }
                ActionButton {
                    label: "ACPI Powerdown",
                    disabled: current_phase != VmPhase::Running,
                    action: "power_off",
                    error_msg,
                    status,
                }
                ActionButton {
                    label: "Force Off",
                    disabled: current_phase == VmPhase::Off,
                    danger: true,
                    action: "force_off",
                    error_msg,
                    status,
                }
            }
        }
    }
}

#[component]
fn ActionButton(
    label: &'static str,
    disabled: bool,
    #[props(default = false)] danger: bool,
    action: &'static str,
    error_msg: Signal<Option<String>>,
    status: Resource<Result<VmStatusResponse, ServerFnError>>,
) -> Element {
    let class = if danger { "danger" } else { "" };
    rsx! {
        button {
            class,
            disabled,
            onclick: move |_| async move {
                error_msg.set(None);
                let result: Result<VmPhase, ServerFnError> = match action {
                    "power_on" => vm_actions::power_on().await,
                    "start_vm" => vm_actions::start_vm().await,
                    "pause" => vm_actions::pause().await,
                    "resume" => vm_actions::resume().await,
                    "reset" => vm_actions::reset().await,
                    "power_off" => vm_actions::power_off().await,
                    "force_off" => vm_actions::force_off().await,
                    _ => Err(ServerFnError::new(format!("Unknown action: {action}"))),
                };
                match result {
                    Ok(_) => status.restart(),
                    Err(e) => error_msg.set(Some(e.to_string())),
                }
            },
            "{label}"
        }
    }
}

#[component]
fn StatusDisplay(status: VmStatusResponse) -> Element {
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
