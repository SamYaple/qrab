use dioxus::prelude::*;

use bmc::server::vm_status::get_serial_output;
use bmc::types::VmPhase;

use super::util::sleep_ms;

#[component]
pub fn SerialConsole(current_phase: VmPhase) -> Element {
    let mut output = use_signal(String::new);
    let is_on = current_phase != VmPhase::Off;

    use_future(move || async move {
        loop {
            if let Ok(text) = get_serial_output().await {
                if !text.is_empty() {
                    output.set(text);
                }
            }
            sleep_ms(1000).await;
        }
    });

    // Auto-scroll to bottom when output changes
    use_effect(move || {
        if !output.read().is_empty() {
            spawn(async move {
                let _ = document::eval(
                    r#"
                    const el = document.getElementById('serial-output');
                    if (el) el.scrollTop = el.scrollHeight;
                    "#,
                );
            });
        }
    });

    let text = output.read().clone();

    rsx! {
        div { class: "panel serial-panel",
            div { class: "panel-header",
                h2 { "Serial Console" }
            }
            if is_on && !text.is_empty() {
                pre { id: "serial-output", class: "serial-output", "{text}" }
            } else if is_on {
                div { class: "empty-state", "Waiting for serial output..." }
            } else {
                div { class: "empty-state", "Power on the VM to see serial output" }
            }
        }
    }
}
