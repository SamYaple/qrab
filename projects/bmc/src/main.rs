#![allow(non_snake_case)]

mod components;

use dioxus::prelude::*;

use components::dashboard::Dashboard;

static MAIN_CSS: Asset = asset!("/assets/main.css");

/// Default tracing filter applied when neither `BMC_LOG` nor `RUST_LOG` is
/// set. Lets the BMC and manager crates log at info, silences the rest of
/// the framework stack at warn — Dioxus/Axum/hyper/tower would otherwise
/// log per request, which gets noisy under dashboard polling load.
#[cfg(feature = "server")]
const DEFAULT_LOG_FILTER: &str = "bmc=info,manager=info,\
                                  dioxus=warn,dioxus_server=warn,\
                                  dioxus_fullstack=warn,dioxus_cli=warn,\
                                  hyper=warn,hyper_util=warn,\
                                  axum=warn,tower=warn,tower_http=warn,\
                                  tokio=warn";

#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[route("/")]
    Dashboard {},
}

fn main() {
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        // Initialize tracing subscriber so log calls in the BMC and manager
        // crates actually emit. Honors BMC_LOG, then RUST_LOG, then the
        // baked-in DEFAULT_LOG_FILTER.
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_env("BMC_LOG")
                    .or_else(|_| tracing_subscriber::EnvFilter::try_from_default_env())
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER)),
            )
            .with_writer(std::io::stderr)
            .try_init();

        let state = bmc::state::bmc();

        // Auto-enable SEV-SNP and power on for debugging.
        if std::env::var("QRAB_AUTO_SEV").is_ok() {
            let s = state.clone();
            tokio::spawn(async move {
                tracing::info!("QRAB_AUTO_SEV: auto-enabling SEV-SNP and powering on");
                {
                    let mut config = s.config.lock().await;
                    config.kvm = true;
                    config.sev_snp = Some(Default::default());
                    config.display = true;
                    config.machine = bmc::types::BmcMachine::Uefi;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                match s.power_on().await {
                    Ok(()) => tracing::info!("QRAB_AUTO_SEV: power_on succeeded"),
                    Err(e) => tracing::error!("QRAB_AUTO_SEV: power_on failed: {e}"),
                }
            });
        }

        Ok(dioxus::server::router(App)
            .nest_service(
                "/novnc",
                tower_http::services::ServeDir::new(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/novnc"),
                ),
            )
            .nest_service(
                "/static/js",
                tower_http::services::ServeDir::new(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/js"),
                ),
            )
            .merge(bmc::server::events::events_router(state.clone()))
            .merge(bmc::server::console::console_router(state)))
    });

    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        Router::<Route> {}
    }
}
