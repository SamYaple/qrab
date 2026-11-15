//! Real-router browser checks; all identities, Pods and upstreams are in-memory
//! fixtures. No test login route or production authentication bypass is added.
use super::*;
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

async fn browser_session(
    store: &session_store::BoundedStore,
    subject: &str,
    token: &str,
    expires: u64,
    launcher: bool,
) -> String {
    let session = Session::new(None, Arc::new(store.clone()), None);
    session
        .insert(
            "user",
            auth::User {
                subject: subject.into(),
                name: if launcher {
                    "fixture-admin"
                } else {
                    "fixture-viewer"
                }
                .into(),
                groups: if launcher {
                    vec![LAUNCHERS.into()]
                } else {
                    vec![]
                },
                csrf: "correct".into(),
                expires,
                id_token: token.into(),
            },
        )
        .await
        .unwrap();
    session.save().await.unwrap();
    session.id().unwrap().to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Requires Python Playwright and an installed browser; loopback fixtures only"]
async fn actual_router_browser() {
    let mut h = harness().await;
    h.cluster.vms.lock().unwrap()[0]
        .spec
        .console
        .as_mut()
        .unwrap()
        .viewer_subjects
        .push(BOB.into());
    let admitted = Arc::new(AtomicUsize::new(0));
    let connections = admitted.clone();
    let console = move |headers: HeaderMap, ws: WebSocketUpgrade| {
        let connections = connections.clone();
        async move {
            if headers.get("authorization").and_then(|v| v.to_str().ok())
                != Some("Bearer id-token-of-alice")
                || headers.contains_key("cookie")
                || headers.contains_key("origin")
            {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            ws.on_upgrade(move |mut socket| async move {
                connections.fetch_add(1, Ordering::SeqCst);
                // Hold native RFB negotiation: the UI creates its real canvas,
                // and its authenticated proxy must close at logout/expiry.
                while socket.recv().await.is_some() {}
            })
        }
    };
    let events = |headers: HeaderMap| async move {
        if !matches!(
            headers.get("authorization").and_then(|v| v.to_str().ok()),
            Some("Bearer id-token-of-alice" | "Bearer id-token-of-stale")
        ) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        let stream = futures::stream::unfold((), |_| async {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            Some((Ok::<_, std::io::Error>(": heartbeat\n\n"), ()))
        });
        (
            [("content-type", "text/event-stream")],
            Body::from_stream(stream),
        )
            .into_response()
    };
    let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = upstream.local_addr().unwrap().port();
    serve(
        upstream,
        Router::new()
            .route("/api/console", axum::routing::get(console))
            .route("/redfish/v1/EventService/SSE", axum::routing::get(events))
            .fallback_service(fake_bmc("browser-vm", h.bmc_a.clone())),
    )
    .await;
    h.state.bmc = bmc::Bmc::new(port);
    h.state.redfish = redfish::Redfish::new(port).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    h.state.origin = origin.clone();
    let seconds: u64 = std::env::var("QRAB_UI_SESSION_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60)
        .clamp(30, 900);
    let expires = auth::now() + seconds;
    let admin = browser_session(&h.state.sessions, ALICE, "id-token-of-alice", expires, true).await;
    let expiry =
        browser_session(&h.state.sessions, ALICE, "id-token-of-alice", expires, true).await;
    let viewer = browser_session(&h.state.sessions, BOB, "id-token-of-stale", expires, false).await;
    serve(listener, router(h.state.clone())).await;
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contrib/tests/router-browser.py");
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = std::env::var("QRAB_UI_PYTHON").unwrap_or_else(|_| "python3".into());
    let engine = std::env::var("QRAB_UI_ENGINE").unwrap_or_else(|_| "chromium".into());
    let output = std::env::var("QRAB_UI_OUTPUT").unwrap_or_else(|_| "target/ui-router".into());
    let config = serde_json::json!({"origin":origin,"admin":admin,"expiry":expiry,"viewer":viewer,"expires":expires});
    let status = tokio::task::spawn_blocking(move || {
        let mut child = Command::new(python)
            .current_dir(repository)
            .arg(script)
            .arg("--engine")
            .arg(engine)
            .arg("--output")
            .arg(output)
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(config.to_string().as_bytes())
            .unwrap();
        child.wait().unwrap()
    })
    .await
    .unwrap();
    assert!(status.success(), "actual-router browser check failed");
    assert!(
        admitted.load(Ordering::SeqCst) >= 2,
        "browser never reached the authenticated console proxy"
    );
    assert_eq!(h.cluster.mutations(), 0);
    assert!(h.kanidm.lock().unwrap().is_empty());
}
