use dioxus::prelude::*;

use bmc::server::vm_actions;
use bmc::types::VmPhase;

#[component]
pub fn PowerControls(current_phase: VmPhase, on_action: EventHandler<()>) -> Element {
    let error_msg = use_signal(|| Option::<String>::None);

    rsx! {
        div { class: "panel power-controls",
            div { class: "panel-header",
                h2 { "Controls" }
            }

            if let Some(err) = error_msg() {
                div { class: "error-banner",
                    "{err}"
                }
            }

            div { class: "button-group",
                if current_phase == VmPhase::Off {
                    ActionButton {
                        label: "Power On",
                        icon: "power",
                        action: "power_on",
                        error_msg,
                        on_action,
                    }
                }
                if current_phase == VmPhase::Running {
                    ActionButton {
                        label: "Pause",
                        icon: "pause",
                        action: "pause",
                        error_msg,
                        on_action,
                    }
                }
                if current_phase == VmPhase::Paused {
                    ActionButton {
                        label: "Resume",
                        icon: "play",
                        action: "resume",
                        error_msg,
                        on_action,
                    }
                }
                if current_phase == VmPhase::Running || current_phase == VmPhase::Paused {
                    ActionButton {
                        label: "Reset",
                        icon: "reset",
                        action: "reset",
                        error_msg,
                        on_action,
                    }
                    ActionButton {
                        label: "Shutdown",
                        icon: "shutdown",
                        action: "power_off",
                        error_msg,
                        on_action,
                    }
                }
                if current_phase != VmPhase::Off {
                    ActionButton {
                        label: "Force Off",
                        icon: "danger",
                        danger: true,
                        action: "force_off",
                        error_msg,
                        on_action,
                    }
                }
            }
        }
    }
}

#[component]
fn ActionButton(
    label: &'static str,
    icon: &'static str,
    #[props(default = false)] danger: bool,
    action: &'static str,
    error_msg: Signal<Option<String>>,
    on_action: EventHandler<()>,
) -> Element {
    let btn_class = if danger {
        "btn btn-danger"
    } else {
        "btn btn-action"
    };
    rsx! {
        button {
            class: btn_class,
            onclick: move |_| async move {
                error_msg.set(None);
                let result: Result<VmPhase, ServerFnError> = match action {
                    "power_on" => vm_actions::power_on().await,
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
