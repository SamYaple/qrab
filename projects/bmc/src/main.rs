#![allow(non_snake_case)]

mod components;

use dioxus::prelude::*;

use components::dashboard::Dashboard;

/// Static stylesheet path. The `asset!` macro works under `dx serve` but
/// bakes a compile-time filesystem path into the URL under a plain
/// `cargo run`, 404-ing every load. Serve it via a predictable
/// `/styles/*` ServeDir mount instead (the Dioxus fullstack router
/// already reserves `/assets`). Both run modes resolve this path.
const MAIN_CSS_HREF: &str = "/styles/main.css";

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
    {
        // S7-21: resolve the auth posture BEFORE `dioxus::serve` — its
        // internal `fullstack_address_or_localhost` reads the `IP`/`PORT`
        // env directly, so we validate + export them up front. Failing
        // here (non-loopback without a token) is the whole point.
        //
        // Tracing isn't up yet; print the resolve error via `eprintln!`
        // so the operator sees it even without BMC_LOG set.
        let auth_cfg = match bmc::server::auth::resolve_from_env() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("BMC startup: {e:#}");
                std::process::exit(1);
            }
        };
        bmc::server::auth::export_bind_env(&auth_cfg);

        dioxus::serve(move || {
            let auth_cfg = auth_cfg.clone();
            async move {
                // Initialize tracing subscriber so log calls in the BMC and manager
                // crates actually emit. Honors BMC_LOG, then RUST_LOG, then the
                // baked-in DEFAULT_LOG_FILTER.
                let _ = tracing_subscriber::fmt()
                    .with_env_filter(
                        tracing_subscriber::EnvFilter::try_from_env("BMC_LOG")
                            .or_else(|_| tracing_subscriber::EnvFilter::try_from_default_env())
                            .unwrap_or_else(|_| {
                                tracing_subscriber::EnvFilter::new(DEFAULT_LOG_FILTER)
                            }),
                    )
                    .with_writer(std::io::stderr)
                    .try_init();

                bmc::server::auth::log_startup(&auth_cfg);

                let state = bmc::state::bmc();

                // Auto power on for SEV-SNP boot debugging. Security features are now
                // always on (see BmcState::power_on), so this just skips the browser
                // click-through.
                if std::env::var("QRAB_AUTO_SEV").is_ok() {
                    let s = state.clone();
                    tokio::spawn(async move {
                        tracing::info!("QRAB_AUTO_SEV: auto power_on");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        match s.power_on().await {
                            Ok(()) => tracing::info!("QRAB_AUTO_SEV: power_on succeeded"),
                            Err(e) => tracing::error!("QRAB_AUTO_SEV: power_on failed: {e}"),
                        }
                    });
                }

                let router = dioxus::server::router(App)
                    .nest_service(
                        "/styles",
                        tower_http::services::ServeDir::new(
                            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"),
                        ),
                    )
                    .nest_service(
                        "/novnc",
                        tower_http::services::ServeDir::new(
                            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                                .join("assets/novnc"),
                        ),
                    )
                    .nest_service(
                        "/static/js",
                        tower_http::services::ServeDir::new(
                            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/js"),
                        ),
                    )
                    .merge(bmc::server::events::events_router(state.clone()))
                    .merge(bmc::server::console::console_router(state));

                // S7-21: one middleware layer covers every route —
                // server fns, SSE, WS console, static assets. A dev
                // loopback bind with no token makes this a pass-through.
                Ok(router.layer(axum::middleware::from_fn_with_state(
                    auth_cfg,
                    bmc::server::auth::require_token,
                )))
            }
        });
    }

    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Install the central SSE-backed store before any child renders.
    // Every dashboard panel reads its data from this store instead of
    // running its own polling loop.
    let _store = components::store::provide_bmc_store();

    rsx! {
        document::Stylesheet { href: MAIN_CSS_HREF }
        Router::<Route> {}
    }
}
