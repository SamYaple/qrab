#![allow(non_snake_case)]

mod components;

use dioxus::prelude::*;

use components::dashboard::Dashboard;

static MAIN_CSS: Asset = asset!("/assets/main.css");

#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[route("/")]
    Dashboard {},
}

fn main() {
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        // Initialize tracing subscriber so log calls in the BMC and manager
        // crates actually emit. Honors RUST_LOG / BMC_LOG env vars.
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_env("BMC_LOG")
                    .or_else(|_| tracing_subscriber::EnvFilter::try_from_default_env())
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("bmc=info,manager=info")),
            )
            .with_writer(std::io::stderr)
            .try_init();

        let state = bmc::state::bmc();

        Ok(dioxus::server::router(App)
            .nest_service(
                "/novnc",
                tower_http::services::ServeDir::new("assets/novnc"),
            )
            .nest_service(
                "/static/js",
                tower_http::services::ServeDir::new("assets/js"),
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
