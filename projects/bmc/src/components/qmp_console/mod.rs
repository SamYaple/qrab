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

use bmc::types::CommandDto;

use command_form::CommandForm;
use command_picker::CommandPicker;
use queue_badge::QueueBadge;

use crate::components::store::use_bmc_store;

#[component]
pub fn QmpConsole(commands: Vec<CommandDto>) -> Element {
    let selected = use_signal(|| None::<usize>);
    let query = use_signal(String::new);
    let form_state = use_signal(|| Value::Object(serde_json::Map::new()));
    let errors = use_signal(HashMap::<String, String>::new);
    let response = use_signal(|| None::<Result<Value, String>>);
    let oob_toggle = use_signal(|| false);

    // Queue status flows from the SSE-backed store. No polling — the
    // server broadcasts a `Queue` frame after every `execute_qmp` call.
    let store = use_bmc_store();
    let queue_now = store.queue;

    let commands_value = commands;
    let load_error_value = String::new();
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

            if !load_error_value.is_empty() {
                div { class: "empty-state", "Failed to load commands: {load_error_value}" }
            } else if total == 0 {
                div { class: "empty-state", "Loading commands…" }
            } else {
                div { class: "qmp-console-body",
                    div { class: "qmp-picker-col",
                        div { class: "picker-summary",
                            "{total} commands"
                        }
                        CommandPicker {
                            commands: commands_value.clone(),
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
