//! Rendered form for a selected command. Owns the submit button and
//! the OOB toggle; validation errors come back from the server fn and
//! propagate into `errors` so field widgets can highlight them.

use std::collections::HashMap;

use dioxus::prelude::*;
use serde_json::Value;

use bmc::server::qmp_exec::execute_qmp;
use bmc::types::CommandDto;

use super::assemble::FormPath;
use super::struct_body::StructBody;

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}

fn doc_first_line(doc: &str) -> String {
    doc.lines()
        .find(|l| {
            let t = l.trim_start_matches('#').trim();
            !t.is_empty()
        })
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .unwrap_or_default()
}

#[component]
pub fn CommandForm(
    cmd: CommandDto,
    form_state: Signal<Value>,
    errors: Signal<HashMap<String, String>>,
    response: Signal<Option<Result<Value, String>>>,
    oob: Signal<bool>,
    oob_supported: bool,
) -> Element {
    let first_line = doc_first_line(&cmd.doc);
    let doc_short = truncate(&first_line, 100);

    let oob_available = oob_supported && cmd.allow_oob;
    let oob_now = *oob.read() && oob_available;

    let badges = rsx! {
        span { class: "cmd-badges",
            if cmd.allow_oob { span { class: "badge badge-oob", "OOB" } }
            if cmd.deprecated { span { class: "badge badge-deprecated", "deprecated" } }
            if cmd.unstable { span { class: "badge badge-unstable", "unstable" } }
            if cmd.until.is_some() { span { class: "badge badge-removed", "removed" } }
        }
    };

    let path: FormPath = Vec::new();

    // Submit handler pulls the current form_state and posts the whole
    // envelope; errors flow back into `errors` + `response`.
    let cmd_name = cmd.wire_name.clone();
    let execute_handler = move |_| {
        let envelope = serde_json::json!({
            "execute": cmd_name.clone(),
            "arguments": form_state.read().clone(),
        });
        let oob = oob_now;
        spawn(async move {
            errors.write().clear();
            let result = execute_qmp(envelope, oob).await;
            match result {
                Ok(v) => response.set(Some(Ok(v))),
                Err(e) => {
                    let msg = e.to_string();
                    // Parse out path: reason format and feed into errors.
                    if let Some((path, reason)) = msg.split_once(": ") {
                        errors.write().insert(path.to_string(), reason.to_string());
                    }
                    response.set(Some(Err(msg)));
                }
            }
        });
    };

    rsx! {
        div { class: "command-form",
            div { class: "cmd-header",
                h3 { "{cmd.wire_name}" }
                {badges}
            }
            div { class: "cmd-subtitle",
                "{cmd.rust_name} · since {cmd.since.0}.{cmd.since.1}.{cmd.since.2}"
                if let Some(until) = cmd.until {
                    span { class: "cmd-until",
                        " · removed in {until.0}.{until.1}.{until.2}"
                    }
                }
            }
            if !doc_short.is_empty() {
                details { class: "cmd-doc",
                    summary { "{doc_short}" }
                    pre { class: "cmd-doc-full", "{cmd.doc}" }
                }
            }

            StructBody {
                meta: cmd.args.clone(),
                path: path,
                form_state: form_state,
                errors: errors,
            }

            div { class: "cmd-submit-bar",
                label { class: "oob-toggle",
                    input {
                        r#type: "checkbox",
                        disabled: !oob_available,
                        checked: oob_now,
                        onchange: move |e| oob.set(e.value() == "true"),
                    }
                    span { "Execute OOB" }
                }
                button {
                    r#type: "button",
                    class: "cmd-execute",
                    onclick: execute_handler,
                    "Execute"
                }
            }
        }
    }
}
