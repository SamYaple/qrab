use dioxus::prelude::*;

use bmc::server::vm_status::get_console_available;

use super::util::sleep_ms;

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

#[component]
pub fn Console() -> Element {
    let mut console_available = use_signal(|| false);
    let mut poll_tick = use_signal(|| 0u64);

    // Periodic ticker (every 2s). Task is scope-bound and auto-cancels on unmount.
    use_future(move || async move {
        loop {
            sleep_ms(2000).await;
            poll_tick += 1;
        }
    });

    // Poll console availability via use_resource. Writes into the
    // console_available signal, and disconnects noVNC on transitions to false.
    let _availability = use_resource(move || async move {
        let _ = poll_tick();
        if let Ok(available) = get_console_available().await {
            let prev = console_available();
            console_available.set(available);
            if prev && !available {
                let _ = document::eval(NOVNC_DISCONNECT_JS);
            }
        }
    });

    // Connect noVNC when console becomes available.
    use_effect(move || {
        if console_available() {
            spawn(async move {
                let _ = document::eval(NOVNC_LOADER_JS);
            });
        }
    });

    let available = console_available();

    rsx! {
        div { class: "panel console-panel",
            div { class: "panel-header",
                h2 { "Console" }
            }
            if available {
                div { id: "vnc-container", class: "vnc-container" }
            } else {
                div { class: "empty-state",
                    "Power on the VM to access the console"
                }
            }
        }
    }
}
