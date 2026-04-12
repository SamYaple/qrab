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

        // Run WebSocket console proxy on separate port to bypass Dioxus middleware
        let console_state = state.clone();
        tokio::spawn(async move {
            let router = bmc::server::console::console_router(console_state);
            let listener = tokio::net::TcpListener::bind("0.0.0.0:9090")
                .await
                .expect("bind console port 9090");
            eprintln!("VNC WebSocket proxy on port 9090");
            axum::serve(listener, router).await.ok();
        });

        Ok(dioxus::server::router(App)
            .nest_service(
                "/novnc",
                tower_http::services::ServeDir::new("assets/novnc"),
            )
            .merge(bmc::server::events::events_router(state)))
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
