use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bmc::{auth, config::Config, session_store::BoundedStore, AppState};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tower::ServiceExt;
use tower_sessions::Session;

const SUBJECT: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
fn app() -> AppState {
    let metadata: openidconnect::core::CoreProviderMetadata = serde_json::from_value(serde_json::json!({
        "issuer":"https://idp.example", "authorization_endpoint":"https://idp.example/authorize", "token_endpoint":"https://idp.example/token", "jwks_uri":"https://idp.example/jwks", "response_types_supported":["code"], "subject_types_supported":["public"], "id_token_signing_alg_values_supported":["ES256"]
    })).unwrap();
    let client = openidconnect::core::CoreClient::from_provider_metadata(
        metadata,
        openidconnect::ClientId::new("qrab-bmc".into()),
        None,
    )
    .set_redirect_uri(
        openidconnect::RedirectUrl::new("https://vm.example/auth/callback".into()).unwrap(),
    );
    let allowed = [SUBJECT.to_string()].into_iter().collect();
    let config = Config {
        listen: "127.0.0.1:8080".parse().unwrap(),
        origin: "https://vm.example".into(),
        issuer: "https://idp.example".into(),
        client_id: "qrab-bmc".into(),
        allowed_subjects: allowed,
        assets_dir: std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"),
        ca_file: None,
    };
    let (control, _receiver) = manager::pod::control_channel();
    AppState {
        oidc: auth::Oidc {
            client: Arc::new(client),
            http: openidconnect::reqwest::Client::new(),
            allowed: config.allowed_subjects.clone(),
        },
        config,
        vm: serde_json::from_value(serde_json::json!({"kernel":"/unused", "console":true}))
            .unwrap(),
        control,
        sessions: BoundedStore::default(),
        consoles: Arc::new(Semaphore::new(4)),
    }
}
async fn cookie(store: BoundedStore, subject: &str, expires: u64) -> String {
    let session = Session::new(None, Arc::new(store), None);
    session
        .insert(
            "user",
            auth::User {
                subject: subject.into(),
                name: "operator".into(),
                csrf: "correct".into(),
                expires,
            },
        )
        .await
        .unwrap();
    session.save().await.unwrap();
    format!("__Host-qrab-session={}", session.id().unwrap())
}
fn req(path: &str, cookie: Option<&str>) -> axum::http::request::Builder {
    let mut request = Request::builder().uri(path);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    request
}
#[tokio::test]
async fn login_is_pkce_and_cookie_is_opaque_secure_and_host_only() {
    let response = bmc::router(app())
        .oneshot(req("/auth/login", None).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let url =
        openidconnect::url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
    let q: std::collections::HashMap<_, _> = url.query_pairs().collect();
    for key in ["nonce", "state", "code_challenge"] {
        assert!(q[key].len() >= 32);
    }
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["redirect_uri"], "https://vm.example/auth/callback");
    let value = response.headers()["set-cookie"].to_str().unwrap();
    assert!(value.starts_with("__Host-qrab-session="));
    for flag in ["Secure", "HttpOnly", "SameSite=Lax", "Path=/"] {
        assert!(value.contains(flag));
    }
    assert!(!value.contains("Domain="));
    assert!(!value.contains(q["state"].as_ref()));
}

#[tokio::test]
async fn vm_apis_reject_missing_expired_and_unlisted_sessions() {
    let app = app();
    for path in ["/api/session", "/api/status"] {
        for cookie in [
            None,
            Some(cookie(app.sessions.clone(), SUBJECT, auth::now()).await),
            Some(cookie(app.sessions.clone(), "unlisted", auth::now() + 60).await),
        ] {
            let response = bmc::router(app.clone())
                .oneshot(req(path, cookie.as_deref()).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }
    let response = bmc::router(app)
        .oneshot(
            req("/auth/callback?code=stolen&state=wrong", None)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn mutations_need_both_csrf_and_exact_origin_before_reaching_vm() {
    let app = app();
    let cookie = cookie(app.sessions.clone(), SUBJECT, auth::now() + 60).await;
    for (origin, csrf) in [
        ("https://evil.example", "correct"),
        ("null", "correct"),
        ("https://vm.example", "wrong"),
    ] {
        for (path, body) in [("/api/action", "\"reset\""), ("/auth/logout", "")] {
            let response = bmc::router(app.clone())
                .oneshot(
                    req(path, Some(&cookie))
                        .method("POST")
                        .header("content-type", "application/json")
                        .header("origin", origin)
                        .header("x-csrf-token", csrf)
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }
    let response = bmc::router(app)
        .oneshot(
            req("/api/action", Some(&cookie))
                .method("POST")
                .header("content-type", "application/json")
                .header("origin", "https://vm.example")
                .header("x-csrf-token", "correct")
                .body(Body::from("\"reset\""))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn logout_revokes_the_server_session() {
    let app = app();
    let cookie = cookie(app.sessions.clone(), SUBJECT, auth::now() + 60).await;
    let response = bmc::router(app.clone())
        .oneshot(
            req("/auth/logout", Some(&cookie))
                .method("POST")
                .header("origin", "https://vm.example")
                .header("x-csrf-token", "correct")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = bmc::router(app)
        .oneshot(
            req("/api/session", Some(&cookie))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn websocket_admission_checks_auth_and_origin_over_real_http() {
    let app = app();
    let cookie = cookie(app.sessions.clone(), SUBJECT, auth::now() + 60).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, bmc::router(app)).into_future());
    use std::future::IntoFuture;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    for (cookie, origin, expected) in [
        (None, "https://vm.example", StatusCode::UNAUTHORIZED),
        (
            Some(cookie.as_str()),
            "https://evil.example",
            StatusCode::FORBIDDEN,
        ),
        (
            Some(cookie.as_str()),
            "https://vm.example",
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let mut request = format!("ws://{address}/api/console")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("origin", origin.parse().unwrap());
        if let Some(cookie) = cookie {
            request
                .headers_mut()
                .insert("cookie", cookie.parse().unwrap());
        }
        match tokio_tungstenite::connect_async(request).await.unwrap_err() {
            tokio_tungstenite::tungstenite::Error::Http(response) => {
                assert_eq!(response.status(), expected)
            }
            error => panic!("unexpected error {error}"),
        }
    }
    server.abort();
}

#[test]
fn public_url_never_silently_enables_http_on_a_network_listener() {
    let local = "127.0.0.1:8080".parse().unwrap();
    let public = "0.0.0.0:8080".parse().unwrap();
    for url in [
        "http://vm.example",
        "https://user@vm.example",
        "https://vm.example/path",
        "https://vm.example?x=y",
        "https://vm.example#x",
    ] {
        assert!(bmc::config::public_origin(url, false, &local).is_err());
    }
    assert!(bmc::config::public_origin("http://localhost:8080", true, &public).is_err());
    assert_eq!(
        bmc::config::public_origin("http://localhost:8080", true, &local).unwrap(),
        "http://localhost:8080"
    );
    assert_eq!(
        bmc::config::public_origin("https://vm.example/", false, &public).unwrap(),
        "https://vm.example"
    );
}

#[tokio::test]
#[ignore = "requires QRAB_TEST_QEMU, QRAB_TEST_QEMU_DATA and QRAB_TEST_GUEST with console support"]
async fn real_vnc_streams_expire_and_are_revoked_without_browser_cooperation() {
    use futures::StreamExt;
    use std::future::IntoFuture;
    use std::time::Duration;
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
    use tower_sessions::SessionStore;
    let mut app = app();
    let runtime = tempfile::tempdir().unwrap();
    let guest = std::path::PathBuf::from(std::env::var_os("QRAB_TEST_GUEST").expect("guest path"));
    app.vm = serde_json::from_value(serde_json::json!({
        "name":"bmc-test", "console":true, "qemu_binary":std::env::var("QRAB_TEST_QEMU").unwrap(),
        "kernel":guest.join("vmlinuz"), "initrd":guest.join("initramfs.gz"),
        "firmware_dir":std::env::var("QRAB_TEST_QEMU_DATA").unwrap(),
        "kernel_args":"console=tty0 console=ttyS0 rdinit=/init quiet", "runtime_dir":runtime.path(), "shutdown_grace_secs":0
    })).unwrap();
    let (control, receiver) = manager::pod::control_channel();
    app.control = control;
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let vm = app.vm.clone();
    let runner = tokio::spawn(async move {
        manager::pod::run_controlled(
            vm,
            async {
                let _ = stopped.await;
            },
            receiver,
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        while manager::pod::probe(&app.vm, false).is_err() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, bmc::router(app.clone())).into_future());
    for revoke in [false, true] {
        let cookie = cookie(
            app.sessions.clone(),
            SUBJECT,
            auth::now() + if revoke { 60 } else { 2 },
        )
        .await;
        let mut request = format!("ws://{address}/api/console")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("origin", "https://vm.example".parse().unwrap());
        request
            .headers_mut()
            .insert("cookie", cookie.parse().unwrap());
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
        let banner = socket.next().await.unwrap().unwrap();
        assert!(matches!(banner, Message::Binary(ref bytes) if bytes.starts_with(b"RFB ")));
        if revoke {
            app.sessions
                .delete(&cookie.split_once('=').unwrap().1.parse().unwrap())
                .await
                .unwrap();
        }
        // No browser polling, logout handler or client disconnect helps this test.
        tokio::time::timeout(Duration::from_secs(4), async {
            while let Some(message) = socket.next().await {
                match message {
                    Err(_) | Ok(Message::Close(_)) => break,
                    _ => {}
                }
            }
        })
        .await
        .expect("server retained an expired/revoked VNC connection");
    }
    assert_eq!(app.consoles.available_permits(), 4);
    app.control
        .action(manager::pod::Action::Powerdown)
        .await
        .unwrap();
    let outcome = tokio::time::timeout(Duration::from_secs(5), runner)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(outcome.reason, "shutdown-requested");
    let _ = stop.send(());
    server.abort();
}
