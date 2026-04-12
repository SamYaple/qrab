use dioxus::prelude::*;

use bmc::server::vm_status::{get_console_available, probe_vnc_socket};

use super::util::sleep_ms;

#[component]
pub fn Console() -> Element {
    let mut console_available = use_signal(|| false);
    let mut vnc_probe = use_signal(|| String::new());

    // Poll console availability
    use_future(move || async move {
        loop {
            if let Ok(available) = get_console_available().await {
                // Probe VNC socket when available
                if available {
                    if let Ok(info) = probe_vnc_socket().await {
                        vnc_probe.set(info);
                    }
                }
                let prev = console_available();
                console_available.set(available);

                // Disconnect noVNC when console goes away
                if prev && !available {
                    let _ = document::eval(
                        r#"
                        if (window._rfb) {
                            window._rfb.disconnect();
                            window._rfb = null;
                        }
                        "#,
                    );
                }
            }
            sleep_ms(2000).await;
        }
    });

    // Connect noVNC when console becomes available
    use_effect(move || {
        if console_available() {
            spawn(async move {
                let _ = document::eval(
                    r#"
                    (async () => {
                        try {
                            if (window._rfb) { console.log('noVNC: already connected'); return; }
                            const container = document.getElementById('vnc-container');
                            if (!container) { console.error('noVNC: #vnc-container not in DOM'); return; }
                            console.log('noVNC: importing rfb.js...');
                            const { default: RFB } = await import('/novnc/core/rfb.js');
                            console.log('noVNC: import OK');
                            const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
                            const url = `${proto}//${window.location.hostname}:9090/api/console`;
                            console.log('noVNC: connecting to', url);
                            const rfb = new RFB(container, url, { scaleViewport: true });
                            window._rfb = rfb;
                            rfb.addEventListener('connect', () => {
                                console.log('noVNC: connected, resolution', rfb._fbWidth + 'x' + rfb._fbHeight);
                            });
                            rfb.addEventListener('disconnect', (e) => {
                                console.log('noVNC: disconnect', e.detail);
                                window._rfb = null;
                            });
                            rfb.addEventListener('securityfailure', (e) => {
                                console.error('noVNC: security failure', e.detail);
                            });
                            console.log('noVNC: RFB created, waiting for connect event...');
                        } catch (e) {
                            console.error('noVNC: EXCEPTION:', e.message, e.stack);
                        }
                    })();
                    "#,
                );
            });
        }
    });

    let available = console_available();

    rsx! {
        div { class: "panel console-panel",
            div { class: "panel-header",
                h2 { "Console" }
                if !vnc_probe.read().is_empty() {
                    span { class: "badge", "{vnc_probe}" }
                }
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
