use dioxus::prelude::*;

use bmc::server::vm_status::get_serial_output;
use bmc::types::VmPhase;

use super::util::sleep_ms;

#[component]
pub fn SerialConsole(current_phase: VmPhase) -> Element {
    let mut output = use_signal(String::new);
    let mut poll_tick = use_signal(|| 0u64);
    let is_on = current_phase != VmPhase::Off;

    // Periodic ticker (every 1s). Task is scope-bound and auto-cancels on unmount.
    use_future(move || async move {
        loop {
            sleep_ms(1000).await;
            poll_tick += 1;
        }
    });

    // use_resource updates `output` only when new text is available. It is
    // reactive on `poll_tick` and its task is cancelled on unmount.
    let _serial_resource = use_resource(move || async move {
        let _ = poll_tick();
        if let Ok(text) = get_serial_output().await {
            if !text.is_empty() {
                output.set(text);
            }
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
