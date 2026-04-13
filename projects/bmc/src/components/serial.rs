use dioxus::prelude::*;

use bmc::types::VmPhase;

/// Serial console panel. Pure props-driven: the parent Dashboard polls
/// `get_dashboard_state()` and passes the serial tail down.
#[component]
pub fn SerialConsole(current_phase: VmPhase, serial_tail: String) -> Element {
    let is_on = current_phase != VmPhase::Off;
    let has_text = !serial_tail.is_empty();

    // Auto-scroll to bottom whenever the serial tail changes. `serial_tail`
    // is a non-reactive prop — without `use_reactive!` the effect would
    // only fire on the initial render (see Dioxus 0.7 "non-reactive
    // dependencies" docs; same regression category that blanked the VNC
    // console in round 2).
    use_effect(use_reactive!(|serial_tail| {
        if !serial_tail.is_empty() {
            spawn(async move {
                let _ = document::eval(
                    r#"
                    const el = document.getElementById('serial-output');
                    if (el) el.scrollTop = el.scrollHeight;
                    "#,
                );
            });
        }
    }));

    rsx! {
        div { class: "panel serial-panel",
            div { class: "panel-header",
                h2 { "Serial Console" }
            }
            if is_on && has_text {
                pre { id: "serial-output", class: "serial-output", "{serial_tail}" }
            } else if is_on {
                div { class: "empty-state", "Waiting for serial output..." }
            } else {
                div { class: "empty-state", "Power on the VM to see serial output" }
            }
        }
    }
}
