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
#[component]
pub fn Console(console_available: bool) -> Element {
    // Track the previous availability across renders so we can react to
    // true->false transitions (disconnect) and false->true transitions
    // (connect) without re-running the loader on every re-render.
    let mut was_available = use_signal(|| false);

    use_effect(move || {
        let prev = was_available();
        if console_available && !prev {
            was_available.set(true);
            spawn(async move {
                let _ = document::eval(NOVNC_LOADER_JS);
            });
        } else if !console_available && prev {
            was_available.set(false);
            let _ = document::eval(NOVNC_DISCONNECT_JS);
        }
    });

    rsx! {
        div { class: "panel console-panel",
            div { class: "panel-header",
                h2 { "Console" }
            }
            if console_available {
                div { id: "vnc-container", class: "vnc-container" }
            } else {
                div { class: "empty-state",
                    "Power on the VM to access the console"
                }
            }
        }
    }
}
