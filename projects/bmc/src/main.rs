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
