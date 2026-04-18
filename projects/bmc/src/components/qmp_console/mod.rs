//! Schema-driven QMP command console.
//!
//! Layout:
//! ```
//! +-- QmpConsole --------------------+
//! | header                            |
//! |   "QMP Console"                   |
//! |   QueueBadge (poll)               |
//! | CommandPicker (search + filter)   |
//! | CommandForm (when selected)       |
//! |   StructBody                      |
//! |   [OOB] [Execute]                 |
//! | ResponsePane                      |
//! +-----------------------------------+
//! ```
//!
//! Everything the UI needs — command list, struct shapes, enum variants
//! — is fetched from server fns that mirror `qapi::metadata::*` onto
//! wasm-safe DTOs. The wasm bundle never links `qapi` directly; it holds
//! only what the server has sent.

pub mod assemble;
pub mod command_form;
pub mod command_picker;
pub mod field_input;
pub mod queue_badge;
pub mod search;
pub mod struct_body;

use std::collections::HashMap;

use dioxus::prelude::*;
use serde_json::Value;

use bmc::server::qmp_exec::{list_commands, qmp_queue_status};
use bmc::types::{CommandDto, QueueStatus};

use command_form::CommandForm;
use command_picker::CommandPicker;
use queue_badge::QueueBadge;

#[component]
pub fn QmpConsole() -> Element {
    let mut commands = use_signal(Vec::<CommandDto>::new);
    let mut load_error = use_signal(String::new);
    let selected = use_signal(|| None::<usize>);
    let query = use_signal(String::new);
    let form_state = use_signal(|| Value::Object(serde_json::Map::new()));
    let errors = use_signal(HashMap::<String, String>::new);
    let response = use_signal(|| None::<Result<Value, String>>);
    let oob_toggle = use_signal(|| false);

    // Fetch the command list once on mount.
    use_future(move || async move {
        match list_commands().await {
            Ok(v) => commands.set(v),
            Err(e) => load_error.set(e.to_string()),
        }
    });

    // Separately poll queue status for `oob_supported` so the Execute OOB
    // checkbox can enable/disable immediately when the VM powers on.
    let mut queue_now = use_signal(QueueStatus::default);
    use_future(move || async move {
        loop {
            if let Ok(s) = qmp_queue_status().await {
                queue_now.set(s);
            }
            crate::components::util::sleep_ms(500).await;
        }
    });

    let commands_value = commands.read().clone();
    let selected_idx = *selected.read();
    let selected_cmd = selected_idx.and_then(|i| commands_value.get(i).cloned());
    let total = commands_value.len();
    let oob_supported = queue_now.read().oob_supported;

    rsx! {
        div { class: "panel qmp-console-panel",
            div { class: "panel-header",
                h2 { "QMP Console" }
                QueueBadge {}
            }

            if !load_error.read().is_empty() {
                div { class: "empty-state", "Failed to load commands: {load_error}" }
            } else if total == 0 {
                div { class: "empty-state", "Loading commands…" }
            } else {
                div { class: "qmp-console-body",
                    div { class: "qmp-picker-col",
                        div { class: "picker-summary",
                            "{total} commands"
                        }
                        CommandPicker {
                            commands: commands,
                            selected: selected,
                            query: query,
                        }
                    }
                    div { class: "qmp-form-col",
                        match selected_cmd {
                            Some(cmd) => rsx! {
                                CommandForm {
                                    cmd: cmd,
                                    form_state: form_state,
                                    errors: errors,
                                    response: response,
                                    oob: oob_toggle,
                                    oob_supported: oob_supported,
                                }
                                ResponsePane { response: response }
                            },
                            None => rsx! {
                                div { class: "no-selection",
                                    "Pick a command from the list to get started."
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ResponsePane(response: Signal<Option<Result<Value, String>>>) -> Element {
    let current = response.read().clone();
    match current {
        None => rsx! { div { class: "response-pane empty", "No response yet." } },
        Some(Ok(v)) => {
            let pretty = serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into());
            rsx! {
                div { class: "response-pane ok",
                    div { class: "pane-header", "Response" }
                    pre { class: "response-body", "{pretty}" }
                }
            }
        }
        Some(Err(e)) => rsx! {
            div { class: "response-pane err",
                div { class: "pane-header", "Error" }
                pre { class: "response-body", "{e}" }
            }
        },
    }
}
