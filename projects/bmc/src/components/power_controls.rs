use dioxus::prelude::*;

use bmc::server::vm_actions;
use bmc::types::VmPhase;

#[component]
pub fn PowerControls(
    current_phase: VmPhase,
    on_action: EventHandler<()>,
) -> Element {
    let error_msg = use_signal(|| Option::<String>::None);

    rsx! {
        div { class: "power-controls",
            h2 { "Controls" }

            if let Some(err) = error_msg() {
                div { class: "error-banner",
                    "{err}"
                }
            }

            div { class: "button-row",
                ActionButton {
                    label: "Power On",
                    disabled: current_phase != VmPhase::Off,
                    action: "power_on",
                    error_msg,
                    on_action,
                }
                ActionButton {
                    label: "Start VM",
                    disabled: current_phase != VmPhase::Preconfig,
                    action: "start_vm",
                    error_msg,
                    on_action,
                }
                ActionButton {
                    label: "Pause",
                    disabled: current_phase != VmPhase::Running,
                    action: "pause",
                    error_msg,
                    on_action,
                }
                ActionButton {
                    label: "Resume",
                    disabled: current_phase != VmPhase::Paused,
                    action: "resume",
                    error_msg,
                    on_action,
                }
                ActionButton {
                    label: "Reset",
                    disabled: current_phase != VmPhase::Running && current_phase != VmPhase::Paused,
                    action: "reset",
                    error_msg,
                    on_action,
                }
                ActionButton {
                    label: "ACPI Powerdown",
                    disabled: current_phase != VmPhase::Running,
                    action: "power_off",
                    error_msg,
                    on_action,
                }
                ActionButton {
                    label: "Force Off",
                    disabled: current_phase == VmPhase::Off,
                    danger: true,
                    action: "force_off",
                    error_msg,
                    on_action,
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
    on_action: EventHandler<()>,
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
                    Ok(_) => on_action.call(()),
                    Err(e) => error_msg.set(Some(e.to_string())),
                }
            },
            "{label}"
        }
    }
}
