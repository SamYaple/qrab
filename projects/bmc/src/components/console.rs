use dioxus::prelude::*;

// Small loader that ensures /static/js/novnc-init.js is fetched and then
// invokes window._novncInit(). The real noVNC wiring lives in that JS file.
const NOVNC_LOADER_JS: &str = r#"
(async () => {
    try {
        if (typeof window._novncInit !== 'function') {
            console.log('noVNC: loading novnc-init.js');
            await new Promise((resolve, reject) => {
                const existing = document.getElementById('novnc-init-script');
                if (existing) { existing.addEventListener('load', resolve); existing.addEventListener('error', reject); return; }
                const s = document.createElement('script');
                s.id = 'novnc-init-script';
                s.type = 'module';
                s.src = '/static/js/novnc-init.js';
                s.addEventListener('load', resolve);
                s.addEventListener('error', reject);
                document.head.appendChild(s);
            });
        }
        if (typeof window._novncInit === 'function') {
            window._novncInit();
        } else {
            console.error('noVNC: _novncInit not defined after load');
        }
    } catch (e) {
        console.error('noVNC: loader EXCEPTION:', e.message, e.stack);
    }
})();
"#;

const NOVNC_DISCONNECT_JS: &str = r#"
if (window._rfb) {
    window._rfb.disconnect();
    window._rfb = null;
}
"#;

/// Graphical (VNC) console panel.
///
/// Pure props-driven for availability — the parent Dashboard polls
/// `get_dashboard_state()` and passes `console_available` down. A
/// use_effect watches that prop and drives the noVNC connect/disconnect
/// side-effects so the RFB session is torn down when the VM leaves the
/// Running state.
///
/// **Reactivity gotcha**: `console_available` is a non-reactive prop, not a
/// signal. Effects only subscribe to signal reads by default — a plain
/// prop captured by value in the closure would NOT re-run the effect when
/// the parent re-renders with a new value. The reactive-dependency macro
/// is what tells Dioxus to treat the listed value as a dependency.
/// Dropping this wrapper (round 2 cleanup regression, 2026-04-13) left the
/// VNC console blank because the loader only ever ran with the initial
/// `false` value. A source-level regression guard in `tests/integration.rs`
/// asserts that the wrapper is still present below.
#[component]
pub fn Console(console_available: bool) -> Element {
    // Manual connect — user must click "Connect" to start VNC.
    // Auto-connect during SNP boot causes GHCB faults: the VNC client
    // triggers VGA MMIO reads before the guest firmware has registered
    // GHCB pages with the PSP.
    let mut was_available = use_signal(|| false);
    let mut user_wants_vnc = use_signal(|| false);

    // Disconnect when console becomes unavailable (VM shutdown).
    use_effect(use_reactive!(|console_available| {
        if !console_available && was_available() {
            was_available.set(false);
            user_wants_vnc.set(false);
            let _ = document::eval(NOVNC_DISCONNECT_JS);
        }
    }));

    // Connect only when user clicks and console is available.
    use_effect(use_reactive!(|console_available| {
        if console_available && user_wants_vnc() && !was_available() {
            was_available.set(true);
            spawn(async move {
                let _ = document::eval(NOVNC_LOADER_JS);
            });
        }
    }));

    rsx! {
        div { class: "panel console-panel",
            div { class: "panel-header",
                h2 { "Console" }
            }
            if console_available && was_available() {
                div { id: "vnc-container", class: "vnc-container" }
            } else if console_available {
                div { class: "empty-state",
                    button {
                        class: "btn",
                        onclick: move |_| {
                            user_wants_vnc.set(true);
                        },
                        "Connect Console"
                    }
                }
            } else {
                div { class: "empty-state",
                    "Power on the VM to access the console"
                }
            }
        }
    }
}

/// Pure state-transition for the VNC console side-effects.
/// Extracted so the connect/disconnect decision is unit-testable without a
/// browser or Dioxus runtime. The live component is a thin wrapper around
/// this function plus `document::eval` calls.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ConsoleTransition {
    /// No side effect — either both false (VM off) or both true (still
    /// connected) since the last render.
    Idle,
    /// Availability went false → true: load noVNC and connect.
    Connect,
    /// Availability went true → false: disconnect the RFB session.
    Disconnect,
}

pub(super) fn console_transition(prev: bool, current: bool) -> ConsoleTransition {
    match (prev, current) {
        (false, true) => ConsoleTransition::Connect,
        (true, false) => ConsoleTransition::Disconnect,
        _ => ConsoleTransition::Idle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_idle_when_both_off() {
        assert_eq!(console_transition(false, false), ConsoleTransition::Idle);
    }

    #[test]
    fn transition_idle_when_still_connected() {
        assert_eq!(console_transition(true, true), ConsoleTransition::Idle);
    }

    #[test]
    fn transition_connect_on_availability_rising_edge() {
        assert_eq!(console_transition(false, true), ConsoleTransition::Connect);
    }

    #[test]
    fn transition_disconnect_on_availability_falling_edge() {
        assert_eq!(
            console_transition(true, false),
            ConsoleTransition::Disconnect
        );
    }
}
