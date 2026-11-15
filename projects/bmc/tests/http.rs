use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use bmc::{
    admin_bucket::{self, Feature},
    auth,
    config::Config,
    session_store::BoundedStore,
    AppState,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tower::ServiceExt;
use tower_sessions::Session;

const SUBJECT: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const VIEWER: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const ADMIN_GROUP: &str = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const VIEWER_GROUP: &str = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
const PORTAL: &str = "192.0.2.10:3260";
const TARGET: &str = "iqn.2026-11.com.yapplesauce:qrab.test.disk";
const IMAGE: &str =
    "zot.yapplesauce.com/qrab/manager@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ISSUER: &str = "https://idm.example/oauth2/openid/qrab-dashboard";
const AUDIENCE: &str = "qrab-dashboard";
const UNLISTED: &str = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
const SIGNER: &str = "test-only-signing-key.pem";
/// No Redfish listens here.
const UNREACHABLE: &str = "http://127.0.0.1:1";

/// Redfish's test-only RSA keys; the BMC verifies tokens the same way.
fn key(name: &str) -> openidconnect::core::CoreRsaPrivateSigningKey {
    let pem = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../redfish/tests/fixtures")
            .join(name),
    )
    .unwrap();
    openidconnect::core::CoreRsaPrivateSigningKey::from_pem(
        &pem,
        Some(openidconnect::JsonWebKeyId::new("test".into())),
    )
    .unwrap()
}

fn verifier() -> oidc_bearer::Verifier {
    use openidconnect::PrivateSigningKey;
    oidc_bearer::Verifier::new(
        openidconnect::IssuerUrl::new(ISSUER.into()).unwrap(),
        openidconnect::ClientId::new(AUDIENCE.into()),
        openidconnect::core::CoreJsonWebKeySet::new(vec![key(SIGNER).as_verification_key()]),
    )
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Groups {
    groups: Vec<String>,
}
impl openidconnect::AdditionalClaims for Groups {}

/// A signed ID token, as the dashboard forwards it.
fn signed(
    signer: &str,
    issuer: &str,
    audience: &str,
    subject: &str,
    groups: &[&str],
    expires_in: i64,
) -> String {
    use openidconnect::core::{
        CoreGenderClaim, CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm,
    };
    let now = auth::now() as i64;
    let claims: openidconnect::IdTokenClaims<Groups, CoreGenderClaim> =
        serde_json::from_value(serde_json::json!({
            "iss": issuer, "aud": [audience], "sub": subject, "iat": now - 5,
            "exp": now + expires_in, "preferred_username": "dashboard-user", "groups": groups
        }))
        .unwrap();
    openidconnect::IdToken::<
        Groups,
        CoreGenderClaim,
        CoreJweContentEncryptionAlgorithm,
        CoreJwsSigningAlgorithm,
    >::new(
        claims,
        &key(signer),
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        None,
        None,
    )
    .unwrap()
    .to_string()
}

fn bearer(subject: &str) -> String {
    signed(SIGNER, ISSUER, AUDIENCE, subject, &[], 300)
}

fn grouped(subject: &str, groups: &[&str]) -> String {
    signed(SIGNER, ISSUER, AUDIENCE, subject, groups, 300)
}

fn breq(path: &str, token: &str) -> axum::http::request::Builder {
    Request::builder()
        .uri(browser_path(path))
        .header("authorization", format!("Bearer {token}"))
}

async fn send(app: &AppState, request: Request<Body>) -> (StatusCode, String) {
    let response = bmc::router(app.clone()).oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

fn app() -> AppState {
    app_with_receiver().0
}

/// No `BMC_PUBLIC_URL`: only the dashboard's bearer tokens get in.
fn bearer_only() -> AppState {
    let mut app = app();
    app.oidc = None;
    app.config.browser = None;
    app
}
fn app_with_receiver() -> (AppState, manager::pod::ControlReceiver) {
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
    let config = Config {
        listen: "127.0.0.1:8080".parse().unwrap(),
        browser: Some(bmc::config::Browser {
            origin: "https://vm.example".into(),
            issuer: "https://idp.example".into(),
            client_id: "qrab-bmc".into(),
        }),
        bearer: Some(bmc::config::Bearer {
            issuer: ISSUER.into(),
            audience: AUDIENCE.into(),
        }),
        admin_subjects: [SUBJECT.to_string()].into_iter().collect(),
        viewer_subjects: [VIEWER.to_string()].into_iter().collect(),
        admin_groups: [ADMIN_GROUP.to_string()].into_iter().collect(),
        viewer_groups: [VIEWER_GROUP.to_string()].into_iter().collect(),
        assets_dir: std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"),
        ca_file: None,
        // Audio is enabled for this VM; USB redirection is not.
        admin_bucket: [Feature::Audio].into(),
        redfish_url: UNREACHABLE.into(),
    };
    let vm: manager::pod::Config = serde_json::from_value(serde_json::json!({
        "kernel":"/unused", "console":true,
        "hotplug":{"max_cpus":4,"max_memory_mib":4096,"cpu_overhead_millis":500,"memory_overhead_mib":256},
        "iscsi_disks":[{
            "id":"root","portal":PORTAL,"target":TARGET,"initiator_name":"iqn.2026-11.com.yapplesauce:qrab.vm.x",
            "username":"chap-user","password_file":"/run/qrab-auth/root/password"
        }]
    }))
    .unwrap();
    let deployment = manager::pod::Deployment {
        node: Some("node-a".into()),
        image: Some(IMAGE.into()),
        ..Default::default()
    };
    let (control, receiver) = manager::pod::control_channel_for(&vm, deployment);
    let app = AppState {
        oidc: Some(auth::Oidc {
            client: Arc::new(client),
            http: openidconnect::reqwest::Client::new(),
        }),
        access: auth::Access::from_config(&config),
        bearer: Some(Arc::new(verifier())),
        config,
        control: manager::pod::Backend::Local(control),
        sessions: BoundedStore::default(),
        consoles: Arc::new(Semaphore::new(4)),
        event_streams: Arc::new(Semaphore::new(2)),
        redfish: bmc::proxy::Upstream::new(UNREACHABLE).unwrap(),
    };
    (app, receiver)
}
async fn cookie(store: BoundedStore, subject: &str, expires: u64) -> String {
    group_cookie(store, subject, &[], expires).await
}
async fn group_cookie(store: BoundedStore, subject: &str, groups: &[&str], expires: u64) -> String {
    let session = Session::new(None, Arc::new(store), None);
    session
        .insert(
            "user",
            auth::User {
                subject: subject.into(),
                groups: groups.iter().map(|g| g.to_string()).collect(),
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
// Match the browser's mapping of native Redfish-advertised links onto its
// private cookie bridge. The Recording sidecar continues to see native URIs.
fn browser_path(path: &str) -> String {
    if path == "/redfish" || path.starts_with("/redfish/") {
        format!("/bmc{path}")
    } else {
        path.to_owned()
    }
}
fn req(path: &str, cookie: Option<&str>) -> axum::http::request::Builder {
    let mut request = Request::builder().uri(browser_path(path));
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
    assert_eq!(q["scope"], "openid profile groups_uuid");
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
    for path in ["/api/session", "/api/console"] {
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
        for (path, body) in [("/auth/logout", "")] {
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
    // The session survived every refused logout.
    assert_eq!(get(&app, "/api/session", &cookie).await.0, StatusCode::OK);
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
                assert_eq!(response.status(), expected);
                if expected == StatusCode::SERVICE_UNAVAILABLE {
                    assert_eq!(
                        response.body().as_deref(),
                        Some(&b"The VM console is unavailable."[..])
                    );
                }
            }
            error => panic!("unexpected error {error}"),
        }
    }
    server.abort();
}

async fn get(app: &AppState, path: &str, cookie: &str) -> (StatusCode, String) {
    let response = bmc::router(app.clone())
        .oneshot(req(path, Some(cookie)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// The configuration's viewer redaction is Redfish's (`Systems/1`
/// `Oem.Qrab.Configuration`); the BMC reports only the session's role.
#[tokio::test]
async fn cookie_sessions_report_the_listed_role() {
    let app = app();
    for (subject, role) in [(VIEWER, "viewer"), (SUBJECT, "admin")] {
        let session = cookie(app.sessions.clone(), subject, auth::now() + 60).await;
        let (status, body) = get(&app, "/api/session", &session).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap()["role"],
            role
        );
    }
}

#[tokio::test]
async fn viewers_are_refused_at_console_websocket_admission() {
    let app = app();
    let viewer = cookie(app.sessions.clone(), VIEWER, auth::now() + 60).await;
    let unlisted = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
    let group_viewer = group_cookie(
        app.sessions.clone(),
        unlisted,
        &[VIEWER_GROUP],
        auth::now() + 60,
    )
    .await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    use std::future::IntoFuture;
    let server = tokio::spawn(axum::serve(listener, bmc::router(app)).into_future());
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    for viewer in [viewer, group_viewer] {
        let mut request = format!("ws://{address}/api/console")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("origin", "https://vm.example".parse().unwrap());
        request
            .headers_mut()
            .insert("cookie", viewer.parse().unwrap());
        match tokio_tungstenite::connect_async(request).await.unwrap_err() {
            tokio_tungstenite::tungstenite::Error::Http(response) => {
                assert_eq!(response.status(), StatusCode::FORBIDDEN)
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
    // The BMC runs as a sidecar: console and actions cross the control socket.
    let vm: manager::pod::Config = serde_json::from_value(serde_json::json!({
        "name":"bmc-test", "console":true, "qemu_binary":std::env::var("QRAB_TEST_QEMU").unwrap(),
        "kernel":guest.join("vmlinuz"), "initrd":guest.join("initramfs.gz"),
        "firmware_dir":std::env::var("QRAB_TEST_QEMU_DATA").unwrap(),
        "kernel_args":"console=tty0 console=ttyS0 rdinit=/init quiet", "runtime_dir":runtime.path(), "shutdown_grace_secs":0,
        "control_socket":true
    })).unwrap();
    let (_control, receiver) = manager::pod::control_channel();
    let remote = manager::pod::Remote::new(manager::pod::socket::socket_path(&vm));
    app.control = manager::pod::Backend::Socket(remote);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let probe = vm.clone();
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
        while manager::pod::probe(&probe, false).is_err() {
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
    // A graceful shutdown leaves the manager serving a powered-off VM; Pod
    // shutdown ends it.
    app.control
        .action(manager::pod::Action::Powerdown)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while app.control.status().await.unwrap().phase != manager::pod::Phase::Off {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let _ = stop.send(());
    let outcome = tokio::time::timeout(Duration::from_secs(5), runner)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(outcome.reason, "shutdown-requested");
    server.abort();
}

fn post(path: &str, cookie: &str, csrf: &str, body: &str) -> Request<Body> {
    req(path, Some(cookie))
        .method("POST")
        .header("content-type", "application/json")
        .header("origin", "https://vm.example")
        .header("x-csrf-token", csrf)
        .body(Body::from(body.to_owned()))
        .unwrap()
}

/// Management lives in Redfish: the BMC serves only `/api/session` and
/// `/api/console` besides the login, probes, assets and the `/redfish` proxy.
#[tokio::test]
async fn duplicate_management_routes_are_gone() {
    let app = app();
    let admin = cookie(app.sessions.clone(), SUBJECT, auth::now() + 60).await;
    for (method, path) in [
        ("GET", "/api/status"),
        ("GET", "/api/config"),
        ("GET", "/api/events"),
        ("GET", "/api/agent"),
        ("POST", "/api/action"),
        ("GET", "/api/resources"),
        ("GET", "/api/events/stream"),
        ("POST", "/api/agent/freeze"),
        ("POST", "/api/agent/thaw"),
        ("POST", "/api/agent/update"),
    ] {
        let request = req(path, Some(&admin))
            .method(method)
            .header("content-type", "application/json")
            .header("origin", "https://vm.example")
            .header("x-csrf-token", "correct")
            .body(Body::from("{}"))
            .unwrap();
        assert_eq!(send(&app, request).await.0, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn configured_groups_grant_roles_and_are_requested_only_when_configured() {
    let app = app();
    let unlisted = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
    for (groups, expected) in [
        (vec![VIEWER_GROUP], Some("viewer")),
        (vec![ADMIN_GROUP], Some("admin")),
        (vec![VIEWER_GROUP, ADMIN_GROUP], Some("admin")),
        (vec![], None),
        (vec!["ffffffff-ffff-4fff-8fff-ffffffffffff"], None),
    ] {
        let cookie = group_cookie(app.sessions.clone(), unlisted, &groups, auth::now() + 60).await;
        let (status, body) = get(&app, "/api/session", &cookie).await;
        match expected {
            Some(role) => {
                assert_eq!(status, StatusCode::OK, "{groups:?}");
                let body: serde_json::Value = serde_json::from_str(&body).unwrap();
                assert_eq!(body["role"], role, "{groups:?}");
            }
            None => assert_eq!(status, StatusCode::UNAUTHORIZED, "{groups:?}"),
        }
    }
    // Only configured groups from a verified token are kept in the session.
    use base64::Engine;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::json!({"groups":[VIEWER_GROUP.to_uppercase(),"ffffffff-ffff-4fff-8fff-ffffffffffff","admins@idm.example"]})
            .to_string(),
    );
    assert_eq!(
        app.access.configured_groups(&format!("h.{payload}.s")),
        [VIEWER_GROUP]
    );

    // Without configured groups the login asks for no group claim.
    let mut plain = app.clone();
    plain.access.admin_groups.clear();
    plain.access.viewer_groups.clear();
    let response = bmc::router(plain)
        .oneshot(req("/auth/login", None).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let url =
        openidconnect::url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
    let scope = url.query_pairs().find(|(k, _)| k == "scope").unwrap().1;
    assert_eq!(scope, "openid profile");
}

#[tokio::test]
async fn bearer_tokens_are_fully_verified_on_every_api_route() {
    let app = app();
    let cookie = cookie(app.sessions.clone(), SUBJECT, auth::now() + 60).await;
    let expired = signed(SIGNER, ISSUER, AUDIENCE, SUBJECT, &[], -60);
    let rejected = [
        String::new(),
        "not-a-jwt".to_string(),
        "a".repeat(oidc_bearer::MAX_TOKEN_BYTES + 1),
        expired.clone(),
        signed(SIGNER, ISSUER, "qrab-bmc", SUBJECT, &[], 300),
        signed(
            SIGNER,
            "https://evil.example/oauth2/openid/qrab-dashboard",
            AUDIENCE,
            SUBJECT,
            &[],
            300,
        ),
        signed(
            "test-only-foreign-key.pem",
            ISSUER,
            AUDIENCE,
            SUBJECT,
            &[],
            300,
        ),
    ];
    // Admission authenticates before the console's WebSocket upgrade.
    for path in ["/api/session", "/api/console"] {
        for token in &rejected {
            // A bearer request never falls back to the cookie session it carries.
            let request = breq(path, token)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap();
            assert_eq!(
                send(&app, request).await.0,
                StatusCode::UNAUTHORIZED,
                "{path} {}",
                &token[..token.len().min(40)]
            );
        }
        // A valid token whose subject and groups grant nothing is forbidden.
        let unlisted = grouped(UNLISTED, &["ffffffff-ffff-4fff-8fff-ffffffffffff"]);
        let request = breq(path, &unlisted).body(Body::empty()).unwrap();
        assert_eq!(send(&app, request).await.0, StatusCode::FORBIDDEN, "{path}");
    }
}

#[tokio::test]
async fn bearer_roles_come_from_subjects_and_groups_and_need_no_csrf() {
    let app = app();
    for (token, expected) in [
        (bearer(SUBJECT), "admin"),
        (bearer(VIEWER), "viewer"),
        (grouped(UNLISTED, &[VIEWER_GROUP]), "viewer"),
        (grouped(UNLISTED, &[ADMIN_GROUP]), "admin"),
        (grouped(VIEWER, &[ADMIN_GROUP]), "admin"),
        (grouped(UNLISTED, &[VIEWER_GROUP, ADMIN_GROUP]), "admin"),
    ] {
        let request = breq("/api/session", &token).body(Body::empty()).unwrap();
        let (status, body) = send(&app, request).await;
        assert_eq!(status, StatusCode::OK);
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["role"], expected);
        assert_eq!(body["user"]["name"], "dashboard-user");
    }
}

#[tokio::test]
async fn authorization_headers_grant_nothing_without_bearer_mode() {
    let mut app = app();
    app.bearer = None;
    for path in ["/api/session", "/api/console"] {
        let request = breq(path, &bearer(SUBJECT)).body(Body::empty()).unwrap();
        assert_eq!(
            send(&app, request).await.0,
            StatusCode::UNAUTHORIZED,
            "{path}"
        );
    }
    // The cookie session still decides, whatever the header says.
    let viewer = cookie(app.sessions.clone(), VIEWER, auth::now() + 60).await;
    let request = breq("/api/session", &bearer(SUBJECT))
        .header("cookie", &viewer)
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"role\":\"viewer\""), "{body}");
    // In bearer mode, other schemes also leave the cookie session in charge.
    app.bearer = Some(Arc::new(verifier()));
    let request = req("/api/session", Some(&viewer))
        .header("authorization", "Basic YWRtaW46cGFzc3dvcmQ=")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"role\":\"viewer\""), "{body}");
}

#[tokio::test]
async fn bearer_websocket_admission_needs_an_admin_token_and_no_origin() {
    let app = app();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    use std::future::IntoFuture;
    let server = tokio::spawn(axum::serve(listener, bmc::router(app)).into_future());
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    for (token, origin, expected) in [
        ("not-a-jwt".to_string(), None, StatusCode::UNAUTHORIZED),
        (
            signed(SIGNER, ISSUER, AUDIENCE, SUBJECT, &[], -60),
            None,
            StatusCode::UNAUTHORIZED,
        ),
        (bearer(UNLISTED), None, StatusCode::FORBIDDEN),
        (bearer(VIEWER), None, StatusCode::FORBIDDEN),
        (
            grouped(UNLISTED, &[VIEWER_GROUP]),
            None,
            StatusCode::FORBIDDEN,
        ),
        // Admission passed; no VM console is running here.
        (bearer(SUBJECT), None, StatusCode::SERVICE_UNAVAILABLE),
        (
            grouped(UNLISTED, &[ADMIN_GROUP]),
            Some("https://evil.example"),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let mut request = format!("ws://{address}/api/console")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        if let Some(origin) = origin {
            request
                .headers_mut()
                .insert("origin", origin.parse().unwrap());
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

#[tokio::test]
async fn bearer_admission_lasts_until_the_token_expires() {
    use axum::extract::FromRequestParts;
    use std::time::{Duration, Instant};
    let app = app();
    let token = signed(SIGNER, ISSUER, AUDIENCE, SUBJECT, &[], 2);
    let (mut parts, ()) = breq("/api/console", &token).body(()).unwrap().into_parts();
    let Ok(principal) = auth::Principal::from_request_parts(&mut parts, &app).await else {
        panic!("a valid admin token was refused");
    };
    assert!(principal.is_bearer() && principal.role == auth::Role::Admin);
    assert!(principal.still_current(&app, auth::Role::Admin).await);
    // The console bridge and the event stream both end on this future.
    let started = Instant::now();
    tokio::time::timeout(
        Duration::from_secs(5),
        principal.ended(&app, auth::Role::Admin),
    )
    .await
    .expect("bearer admission outlived its token");
    assert!(started.elapsed() >= Duration::from_millis(900));
    assert!(!principal.still_current(&app, auth::Role::Viewer).await);
}

#[tokio::test]
async fn bearer_only_bmc_serves_session_console_and_proxy_but_no_login_or_ui() {
    let app = bearer_only();
    for (method, path) in [
        ("GET", "/"),
        ("GET", "/auth/login"),
        ("GET", "/auth/callback?code=x&state=y"),
        ("POST", "/auth/logout"),
        ("GET", "/assets/novnc/core/rfb.js"),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(&app, request).await.0, StatusCode::NOT_FOUND, "{path}");
    }
    let request = breq("/api/session", &bearer(SUBJECT))
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(&app, request).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"role\":\"admin\""));
    for path in ["/api/session", "/api/console"] {
        let request = req(path, None).body(Body::empty()).unwrap();
        assert_eq!(
            send(&app, request).await.0,
            StatusCode::UNAUTHORIZED,
            "{path}"
        );
    }
    let request = req("/healthz", None).body(Body::empty()).unwrap();
    assert_eq!(send(&app, request).await.0, StatusCode::OK);
}

#[tokio::test]
async fn admin_bucket_features_need_this_vm_an_admin_and_an_explicit_request() {
    let app = app();
    let admin = cookie(app.sessions.clone(), SUBJECT, auth::now() + 60).await;
    let viewer = cookie(app.sessions.clone(), VIEWER, auth::now() + 60).await;
    let group_admin = group_cookie(
        app.sessions.clone(),
        UNLISTED,
        &[ADMIN_GROUP],
        auth::now() + 60,
    )
    .await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    use std::future::IntoFuture;
    let server = tokio::spawn(axum::serve(listener, bmc::router(app)).into_future());
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    const ORIGIN: Option<&str> = Some("https://vm.example");
    // 503 means admission passed; no VM console is running here.
    let unavailable = StatusCode::SERVICE_UNAVAILABLE;
    /// Query, cookie, bearer token, Origin and the expected admission status.
    type Case<'a> = (
        &'a str,
        Option<&'a str>,
        Option<String>,
        Option<&'a str>,
        StatusCode,
    );
    let cases: Vec<Case> = vec![
        ("", Some(&admin), None, ORIGIN, unavailable),
        ("?features=audio", Some(&admin), None, ORIGIN, unavailable),
        (
            "?features=audio&features=",
            Some(&group_admin),
            None,
            ORIGIN,
            unavailable,
        ),
        (
            "?features=audio",
            None,
            Some(bearer(SUBJECT)),
            None,
            unavailable,
        ),
        // Disabled for this VM, alone or beside an enabled feature.
        (
            "?features=usb-redirect",
            Some(&admin),
            None,
            ORIGIN,
            StatusCode::FORBIDDEN,
        ),
        (
            "?features=audio,usb-redirect",
            Some(&admin),
            None,
            ORIGIN,
            StatusCode::FORBIDDEN,
        ),
        // Viewers are refused at admission, by cookie or bearer.
        (
            "?features=audio",
            Some(&viewer),
            None,
            ORIGIN,
            StatusCode::FORBIDDEN,
        ),
        (
            "?features=audio",
            None,
            Some(bearer(VIEWER)),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            "?features=audio",
            Some(&admin),
            None,
            Some("https://evil.example"),
            StatusCode::FORBIDDEN,
        ),
        (
            "?features=microphone",
            Some(&admin),
            None,
            ORIGIN,
            StatusCode::BAD_REQUEST,
        ),
        (
            "?features=audio",
            None,
            None,
            ORIGIN,
            StatusCode::UNAUTHORIZED,
        ),
    ];
    for (query, cookie, token, origin, expected) in cases {
        let mut request = format!("ws://{address}/api/console{query}")
            .into_client_request()
            .unwrap();
        let headers = request.headers_mut();
        if let Some(cookie) = cookie {
            headers.insert("cookie", cookie.parse().unwrap());
        }
        if let Some(token) = token {
            headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
        }
        if let Some(origin) = origin {
            headers.insert("origin", origin.parse().unwrap());
        }
        match tokio_tungstenite::connect_async(request).await.unwrap_err() {
            tokio_tungstenite::tungstenite::Error::Http(response) => {
                assert_eq!(response.status(), expected, "{query} {cookie:?} {origin:?}")
            }
            error => panic!("unexpected error {error}"),
        }
    }
    server.abort();
}

/// Log lines written while `f` runs on this thread.
fn captured_logs(f: impl FnOnce()) -> String {
    #[derive(Clone, Default)]
    struct Buffer(Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Buffer {
        type Writer = Self;
        fn make_writer(&'a self) -> Self {
            self.clone()
        }
    }
    let buffer = Buffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(buffer.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::with_default(subscriber, f);
    let bytes = buffer.0.lock().unwrap().clone();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn admin_bucket_admissions_and_denials_are_logged_with_subject_feature_and_outcome() {
    let app = app();
    let principal = |subject: &str, role| auth::Principal {
        user: auth::User {
            subject: subject.into(),
            groups: Vec::new(),
            name: "operator".into(),
            csrf: String::new(),
            expires: auth::now() + 60,
        },
        role,
        credential: auth::Credential::Bearer,
    };
    let admin = principal(SUBJECT, auth::Role::Admin);
    let viewer = principal(VIEWER, auth::Role::Viewer);
    let both = [Feature::Audio, Feature::UsbRedirect].into();
    let mut outcomes = Vec::new();
    let logs = captured_logs(|| {
        for (feature, who, requested) in [
            (Feature::Audio, &admin, &both),
            (Feature::UsbRedirect, &admin, &both),
            (Feature::Audio, &viewer, &both),
            (Feature::Audio, &admin, &Default::default()),
        ] {
            outcomes.push(admin_bucket::admit(&app, feature, who, requested).is_ok());
        }
    });
    assert_eq!(outcomes, [true, false, false, false]);
    let lines: Vec<&str> = logs.lines().collect();
    assert_eq!(lines.len(), 4, "{logs}");
    for (line, (subject, feature, outcome, reason)) in lines.iter().zip([
        (SUBJECT, "audio", "admitted", ""),
        (SUBJECT, "usb-redirect", "denied", "not enabled for this VM"),
        (VIEWER, "audio", "denied", "not an admin"),
        (
            SUBJECT,
            "audio",
            "denied",
            "not requested on this connection",
        ),
    ]) {
        assert!(line.contains(" INFO "), "{line}");
        assert!(line.contains(&format!("subject={subject}")), "{line}");
        assert!(line.contains(&format!("feature={feature} ")), "{line}");
        assert!(line.contains(outcome) && line.contains(reason), "{line}");
    }
}

#[tokio::test]
async fn losing_admin_revokes_admin_bucket_features_within_two_seconds() {
    use std::time::Duration;
    use tower_sessions::SessionStore;
    let app = app();
    for removal in ["logout", "group removal"] {
        let cookie = group_cookie(
            app.sessions.clone(),
            UNLISTED,
            &[ADMIN_GROUP],
            auth::now() + 60,
        )
        .await;
        let id: tower_sessions::session::Id = cookie.split_once('=').unwrap().1.parse().unwrap();
        let session = Session::new(Some(id), Arc::new(app.sessions.clone()), None);
        let Ok(principal) = auth::user(&app, &session).await else {
            panic!("the admin session was refused");
        };
        let Ok(grants) = admin_bucket::admission(&app, &principal, &[Feature::Audio].into()) else {
            panic!("an admin was refused an enabled, requested feature");
        };
        assert!(grants.is_active(Feature::Audio) && !grants.is_active(Feature::UsbRedirect));
        let supervisor = tokio::spawn({
            let (app, grants, principal) = (app.clone(), grants.clone(), principal.clone());
            async move { grants.supervise(&app, &principal).await }
        });
        // Passing rechecks keep the feature.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(grants.is_active(Feature::Audio), "{removal}");
        match removal {
            "logout" => app.sessions.delete(&id).await.unwrap(),
            _ => {
                let mut record = app.sessions.load(&id).await.unwrap().unwrap();
                record.data.get_mut("user").unwrap()["groups"] = serde_json::json!([]);
                app.sessions.save(&record).await.unwrap();
            }
        }
        tokio::time::timeout(Duration::from_secs(2), grants.revoked(Feature::Audio))
            .await
            .unwrap_or_else(|_| panic!("{removal} left the feature admitted"));
        assert!(grants.active().is_empty());
        // The supervisor keeps running: the console follows its own rules.
        assert!(!supervisor.is_finished());
        supervisor.abort();
    }
    // Closing the console revokes its features too.
    let grants = admin_bucket::admission(
        &app,
        &auth::Principal {
            user: auth::User {
                subject: SUBJECT.into(),
                groups: Vec::new(),
                name: "operator".into(),
                csrf: String::new(),
                expires: auth::now() + 60,
            },
            role: auth::Role::Admin,
            credential: auth::Credential::Bearer,
        },
        &[Feature::Audio].into(),
    )
    .unwrap_or_else(|_| panic!("refused"));
    grants.revoke_all(SUBJECT, "console closed");
    assert!(!grants.is_active(Feature::Audio));
    assert!(admin_bucket::Grants::none().active().is_empty());
}

#[tokio::test]
async fn only_admins_see_the_enabled_admin_bucket_in_their_session() {
    let app = app();
    let admin = cookie(app.sessions.clone(), SUBJECT, auth::now() + 60).await;
    let viewer = cookie(app.sessions.clone(), VIEWER, auth::now() + 60).await;
    let (status, body) = get(&app, "/api/session", &admin).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"admin_bucket\":[\"audio\"]"), "{body}");
    let (status, body) = get(&app, "/api/session", &viewer).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("admin_bucket"), "{body}");
    let request = breq("/api/session", &bearer(SUBJECT))
        .body(Body::empty())
        .unwrap();
    assert!(send(&app, request)
        .await
        .1
        .contains("\"admin_bucket\":[\"audio\"]"));
}

const SESSION_TOKEN: &str = "session.id-token.never-shown";
const RESET: &str = "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset";
const SIGNED_UPDATE: &str =
    "/redfish/v1/Systems/1/SecureBoot/Actions/Oem/QrabSecureBoot.UpdateAuthenticatedDatabase";

/// A browser session whose login stored `token` server-side.
async fn token_cookie(store: BoundedStore, subject: &str, token: &str) -> String {
    let session = Session::new(None, Arc::new(store), None);
    session
        .insert(
            "user",
            auth::User {
                subject: subject.into(),
                groups: vec![],
                name: "operator".into(),
                csrf: "correct".into(),
                expires: auth::now() + 60,
            },
        )
        .await
        .unwrap();
    session.insert(auth::ID_TOKEN, token).await.unwrap();
    session.save().await.unwrap();
    format!("__Host-qrab-session={}", session.id().unwrap())
}

#[derive(Debug, Clone)]
struct Seen {
    method: String,
    uri: String,
    headers: axum::http::HeaderMap,
    body: Vec<u8>,
}

/// A fake in-pod Redfish that records every connection and request.
struct Recording {
    url: String,
    connections: Arc<std::sync::atomic::AtomicUsize>,
    seen: Arc<std::sync::Mutex<Vec<Seen>>>,
    /// Live SSE responses; each decrements when its stream is dropped.
    streams: Arc<std::sync::atomic::AtomicUsize>,
    server: tokio::task::JoinHandle<()>,
}

struct Open(Arc<std::sync::atomic::AtomicUsize>);
impl Drop for Open {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Recording {
    async fn start() -> Self {
        use axum::{
            response::{sse, IntoResponse},
            routing::{get, post},
        };
        use std::sync::atomic::{AtomicUsize, Ordering};
        let connections = Arc::new(AtomicUsize::new(0));
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let streams = Arc::new(AtomicUsize::new(0));
        let open = streams.clone();
        let sse = move || {
            let open = open.clone();
            async move {
                open.fetch_add(1, Ordering::SeqCst);
                let guard = Open(open);
                let first = futures::stream::once(async {
                    Ok::<_, std::convert::Infallible>(
                        sse::Event::default()
                            .id("7")
                            .data(r#"{"Events":[{"EventId":"7"}]}"#),
                    )
                });
                let rest = futures::stream::unfold(guard, |guard| async move {
                    std::future::pending::<()>().await;
                    Some((Ok(sse::Event::default()), guard))
                });
                sse::Sse::new(futures::StreamExt::chain(first, rest))
            }
        };
        let resource = || async {
            (
                [
                    ("odata-version", "4.0"),
                    ("set-cookie", "upstream=leak"),
                    ("x-upstream-secret", "leak"),
                ],
                axum::Json(serde_json::json!({
                    "@odata.id": "/redfish/v1/Systems/1", "PowerState": "On"
                })),
            )
        };
        let recorder = seen.clone();
        let app = axum::Router::new()
            .route(
                "/redfish/v1/Systems/1",
                get(resource).options(|| async { StatusCode::NO_CONTENT }),
            )
            .route("/redfish/v1/", get(resource))
            .route(
                "/redfish/v1/SessionService/Sessions",
                post(|| async {
                    let mut response = (
                        StatusCode::CREATED,
                        axum::Json(serde_json::json!({"Id":"session-resource-id"})),
                    )
                        .into_response();
                    response
                        .headers_mut()
                        .insert("x-auth-token", "redfish-session-secret".parse().unwrap());
                    response.headers_mut().insert(
                        "location",
                        "/redfish/v1/SessionService/Sessions/session-resource-id"
                            .parse()
                            .unwrap(),
                    );
                    response
                        .headers_mut()
                        .insert("set-cookie", "upstream=must-not-leak".parse().unwrap());
                    response
                }),
            )
            .route(
                "/redfish/v1/Challenges",
                get(|| async {
                    let mut response =
                        (StatusCode::UNAUTHORIZED, "credentials refused").into_response();
                    response.headers_mut().append(
                        "www-authenticate",
                        "Bearer realm=\"qrab-redfish\"".parse().unwrap(),
                    );
                    response.headers_mut().append(
                        "www-authenticate",
                        "Basic realm=\"qrab-redfish\", charset=\"UTF-8\""
                            .parse()
                            .unwrap(),
                    );
                    response
                }),
            )
            .route(RESET, post(|| async { StatusCode::NO_CONTENT }))
            .route(SIGNED_UPDATE, post(|| async { StatusCode::NO_CONTENT }))
            .route("/redfish/v1/EventService/SSE", get(sse))
            .route(
                "/redfish/v1/Refused",
                get(|| async { (StatusCode::UNAUTHORIZED, "token refused") }),
            )
            .layer(axum::middleware::from_fn(
                |request: axum::extract::Request, next: axum::middleware::Next| async move {
                    // The fixture records transport, not credential validity.
                    // Discovery and password session bootstrap are anonymous;
                    // all other resources require an explicit credential.
                    let anonymous = request.uri().path() == "/redfish/v1/"
                        || request.uri().path() == "/redfish/v1/SessionService/Sessions";
                    if !anonymous
                        && !request.headers().contains_key("authorization")
                        && !request.headers().contains_key("x-auth-token")
                    {
                        return (
                            StatusCode::UNAUTHORIZED,
                            [("www-authenticate", "Bearer realm=\"qrab-redfish\"")],
                        )
                            .into_response();
                    }
                    next.run(request).await
                },
            ))
            .layer(axum::middleware::from_fn(
                move |request: axum::extract::Request, next: axum::middleware::Next| {
                    let recorder = recorder.clone();
                    async move {
                        let (parts, body) = request.into_parts();
                        let body = axum::body::to_bytes(body, 1 << 20).await.unwrap();
                        recorder.lock().unwrap().push(Seen {
                            method: parts.method.to_string(),
                            uri: parts.uri.to_string(),
                            headers: parts.headers.clone(),
                            body: body.to_vec(),
                        });
                        next.run(axum::extract::Request::from_parts(parts, Body::from(body)))
                            .await
                    }
                },
            ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let counter = connections.clone();
        use axum::serve::ListenerExt;
        let listener = listener.tap_io(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url,
            connections,
            seen,
            streams,
            server,
        }
    }

    fn connections(&self) -> usize {
        self.connections.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn proxied() -> (AppState, Recording) {
    let upstream = Recording::start().await;
    let mut app = app();
    app.redfish = bmc::proxy::Upstream::new(&upstream.url).unwrap();
    (app, upstream)
}

/// Only these headers may reach Redfish.
fn assert_forwarded_only_allowed_headers(seen: &Seen) {
    for (name, _) in &seen.headers {
        assert!(
            [
                "authorization",
                "x-auth-token",
                "odata-version",
                "odata-maxversion",
                "content-type",
                "accept",
                "last-event-id",
                "host",
                "content-length"
            ]
            .contains(&name.as_str()),
            "{name} was forwarded to Redfish: {seen:?}"
        );
    }
}

/// Every response header and body, to search for leaked secrets.
async fn everything(response: axum::response::Response) -> (StatusCode, String) {
    let status = response.status();
    let mut text = format!("{:?}", response.headers());
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    text.push_str(std::str::from_utf8(&body).unwrap());
    (status, text)
}

#[tokio::test]
async fn redfish_proxy_forwards_the_session_token_or_the_request_bearer_and_nothing_else() {
    let (app, upstream) = proxied().await;
    let admin = token_cookie(app.sessions.clone(), SUBJECT, SESSION_TOKEN).await;
    let viewer = token_cookie(app.sessions.clone(), VIEWER, "viewer.id-token").await;
    for (cookie, token) in [(&admin, SESSION_TOKEN), (&viewer, "viewer.id-token")] {
        let response = bmc::router(app.clone())
            .oneshot(
                req("/redfish/v1/Systems/1", Some(cookie))
                    .header("accept", "application/json")
                    .header("x-forwarded-for", "203.0.113.9")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()["odata-version"], "4.0");
        assert!(response.headers().get("x-upstream-secret").is_none());
        assert!(response
            .headers()
            .get_all("set-cookie")
            .iter()
            .all(|value| !value.to_str().unwrap().contains("upstream")));
        let (status, text) = everything(response).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        assert!(text.contains("\"PowerState\":\"On\""), "{text}");
        assert!(!text.contains(token), "the ID token left the BMC: {text}");
        let seen = upstream.seen().pop().unwrap();
        assert_eq!(seen.uri, "/redfish/v1/Systems/1");
        assert_eq!(seen.headers["authorization"], format!("Bearer {token}"));
        assert_eq!(seen.headers["accept"], "application/json");
        assert_forwarded_only_allowed_headers(&seen);
    }
    // The session's own view never includes the token either.
    let response = bmc::router(app.clone())
        .oneshot(
            req("/api/session", Some(&admin))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let (status, text) = everything(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!text.contains(SESSION_TOKEN), "{text}");

    // A bearer request is forwarded with its own token, never the session's
    // or the cookie, and needs no CSRF token or Origin.
    let token = bearer(SUBJECT);
    for (method, path) in [("GET", "/redfish/v1/Systems/1"), ("POST", RESET)] {
        let response = bmc::router(app.clone())
            .oneshot(
                breq(path, &token)
                    .method(method)
                    .header("cookie", &admin)
                    .header("origin", "https://evil.example")
                    .header("content-type", "application/json")
                    .body(Body::from(if method == "POST" {
                        r#"{"ResetType":"Pause"}"#
                    } else {
                        ""
                    }))
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, text) = everything(response).await;
        assert!(status.is_success(), "{method} {path}: {status} {text}");
        let seen = upstream.seen().pop().unwrap();
        assert_eq!(seen.headers["authorization"], format!("Bearer {token}"));
        assert_forwarded_only_allowed_headers(&seen);
    }

    // Without either credential Redfish itself answers 401; an expired
    // session's cookie is not forwarded.
    let expired = cookie(app.sessions.clone(), SUBJECT, auth::now()).await;
    for cookie in [None, Some(expired.as_str())] {
        let response = bmc::router(app.clone())
            .oneshot(
                req("/redfish/v1/Systems/1", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers()["www-authenticate"]
            .to_str()
            .unwrap()
            .starts_with("Bearer"));
    }
    assert!(upstream
        .seen()
        .iter()
        .all(|seen| !seen.headers.contains_key("cookie")));

    // Redfish refusing a session the BMC accepted is a gateway fault, not a
    // signed-out browser.
    let (status, body) = get(&app, "/redfish/v1/Refused", &admin).await;
    assert_eq!(
        (status, body.as_str()),
        (
            StatusCode::BAD_GATEWAY,
            "The VM's Redfish service did not accept this sign-in."
        )
    );
}

#[tokio::test]
async fn cookie_mutations_need_origin_and_csrf_before_any_byte_reaches_redfish() {
    let (app, upstream) = proxied().await;
    let admin = token_cookie(app.sessions.clone(), SUBJECT, SESSION_TOKEN).await;
    let body = r#"{"ResetType":"Pause"}"#;
    for (origin, csrf) in [
        (None, Some("correct")),
        (Some("https://evil.example"), Some("correct")),
        (Some("null"), Some("correct")),
        (Some("https://vm.example"), None),
        (Some("https://vm.example"), Some("wrong")),
    ] {
        for method in ["POST", "PATCH", "DELETE"] {
            let mut request = req(RESET, Some(&admin))
                .method(method)
                .header("content-type", "application/json");
            if let Some(origin) = origin {
                request = request.header("origin", origin);
            }
            if let Some(csrf) = csrf {
                request = request.header("x-csrf-token", csrf);
            }
            let (status, _) = send(&app, request.body(Body::from(body)).unwrap()).await;
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "{method} {origin:?} {csrf:?}"
            );
        }
    }
    // A cross-origin read with the cookie is refused too.
    let request = req("/redfish/v1/Systems/1", Some(&admin))
        .header("origin", "https://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, request).await.0, StatusCode::FORBIDDEN);
    // Bodies over Redfish's bounds are refused here.
    let large = format!(r#"{{"ResetType":"{}"}}"#, "x".repeat(4096));
    let (status, _) = send(&app, post(RESET, &admin, "correct", &large)).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let (status, _) = send(
        &app,
        post(SIGNED_UPDATE, &admin, "correct", &"x".repeat(193 * 1024)),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    // Path traversal never reaches another upstream route.
    let request = req("/redfish/v1/../../readyz", Some(&admin))
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, request).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        upstream.connections(),
        0,
        "refused requests reached Redfish"
    );
    assert!(upstream.seen().is_empty());

    let (status, _) = send(&app, post(RESET, &admin, "correct", body)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let seen = upstream.seen().pop().unwrap();
    assert_eq!((seen.method.as_str(), seen.uri.as_str()), ("POST", RESET));
    assert_eq!(seen.body, body.as_bytes());
    assert_eq!(seen.headers["content-type"], "application/json");
    assert_eq!(
        seen.headers["authorization"],
        format!("Bearer {SESSION_TOKEN}")
    );
    assert_forwarded_only_allowed_headers(&seen);
    // Signed EFI updates have Redfish's larger bound.
    let (status, _) = send(
        &app,
        post(SIGNED_UPDATE, &admin, "correct", &"x".repeat(100 * 1024)),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(upstream.seen().pop().unwrap().body.len(), 100 * 1024);
}

#[tokio::test]
async fn proxied_event_streams_close_within_two_seconds_of_logout() {
    use futures::StreamExt;
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    let (app, upstream) = proxied().await;
    let viewer = token_cookie(app.sessions.clone(), VIEWER, SESSION_TOKEN).await;
    let response = bmc::router(app.clone())
        .oneshot(
            req("/redfish/v1/EventService/SSE?after=5", Some(&viewer))
                .header("accept", "text/event-stream")
                .header("last-event-id", "6")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    let mut body = response.into_body().into_data_stream();
    let mut seen = String::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !seen.contains("EventId") {
            let chunk = body.next().await.expect("stream ended").unwrap();
            seen.push_str(std::str::from_utf8(&chunk).unwrap());
        }
    })
    .await
    .unwrap_or_else(|_| panic!("event not streamed: {seen}"));
    assert!(seen.contains("id: 7"), "{seen}");
    let request = upstream.seen().pop().unwrap();
    // The page's history cursor reaches Redfish with the reconnect header.
    assert_eq!(request.uri, "/redfish/v1/EventService/SSE?after=5");
    assert_eq!(request.headers["last-event-id"], "6");
    assert_eq!(
        request.headers["authorization"],
        format!("Bearer {SESSION_TOKEN}")
    );
    assert!(!request.headers.contains_key("cookie"));
    assert_forwarded_only_allowed_headers(&request);
    assert_eq!(upstream.streams.load(Ordering::SeqCst), 1);
    assert_eq!(app.event_streams.available_permits(), 1);
    let drain = tokio::spawn(async move { while body.next().await.is_some() {} });
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(!drain.is_finished(), "the stream ended while signed in");
    let request = req("/auth/logout", Some(&viewer))
        .method("POST")
        .header("origin", "https://vm.example")
        .header("x-csrf-token", "correct")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, request).await.0, StatusCode::NO_CONTENT);
    let logged_out = Instant::now();
    tokio::time::timeout(Duration::from_secs(2), drain)
        .await
        .expect("the proxied stream outlived the session")
        .unwrap();
    assert!(logged_out.elapsed() < Duration::from_secs(2));
    assert_eq!(app.event_streams.available_permits(), 2);
    // The upstream stream is dropped with it.
    tokio::time::timeout(Duration::from_secs(2), async {
        while upstream.streams.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the upstream Redfish stream stayed open");
}

#[tokio::test]
async fn unreachable_redfish_is_a_named_503() {
    let app = app();
    let admin = token_cookie(app.sessions.clone(), SUBJECT, SESSION_TOKEN).await;
    let (status, body) = get(&app, "/redfish/v1/Systems/1", &admin).await;
    assert_eq!(
        (status, body.as_str()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "The VM's Redfish service is unavailable."
        )
    );
}

/// The manager's control socket with a fake QEMU VNC server behind its
/// console: it acknowledges -259 only when SetEncodings asks for it, answers
/// audio enable with audio-begin and one data message, and answers a
/// FramebufferUpdateRequest with a Bell. Like QEMU it drops a connection that
/// sends a 255/1 audio message without negotiating -259. Each console reports
/// the client messages it received when it ends.
fn fake_vnc_manager(path: &std::path::Path) -> tokio::sync::mpsc::UnboundedReceiver<Vec<Vec<u8>>> {
    let listener = tokio::net::UnixListener::bind(path).unwrap();
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let sender = sender.clone();
            tokio::spawn(async move {
                let mut received = Vec::new();
                let _ = fake_vnc(stream, &mut received).await;
                let _ = sender.send(received);
            });
        }
    });
    receiver
}

async fn fake_vnc(
    mut stream: tokio::net::UnixStream,
    received: &mut Vec<Vec<u8>>,
) -> std::io::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn read(stream: &mut tokio::net::UnixStream, n: usize) -> std::io::Result<Vec<u8>> {
        let mut bytes = vec![0; n];
        stream.read_exact(&mut bytes).await?;
        Ok(bytes)
    }
    stream
        .write_all(b"{\"protocol\":\"qrab-control\",\"version\":1,\"vm\":\"fake\",\"operations\":[\"console\"]}\n")
        .await?;
    let mut line = Vec::new();
    loop {
        match stream.read_u8().await? {
            b'\n' => break,
            byte => line.push(byte),
        }
    }
    let request: serde_json::Value = serde_json::from_slice(&line).unwrap();
    assert_eq!(request["op"], "console");
    let reply = format!("{{\"v\":1,\"id\":{},\"result\":null}}\n", request["id"]);
    stream.write_all(reply.as_bytes()).await?;
    // RFB 3.8 with the None security type, as QEMU serves the private socket.
    stream.write_all(b"RFB 003.008\n\x01\x01").await?;
    assert_eq!(read(&mut stream, 12).await?, b"RFB 003.008\n");
    assert_eq!(read(&mut stream, 1).await?, [1]);
    stream.write_all(&[0, 0, 0, 0]).await?;
    read(&mut stream, 1).await?;
    stream
        .write_all(&[
            2, 128, 1, 224, 32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 16, 8, 0, 0, 0, 0, 0, 0, 0, 4,
            b'f', b'a', b'k', b'e',
        ])
        .await?;
    let mut audio = false;
    loop {
        let mut message = vec![stream.read_u8().await?];
        match message[0] {
            2 => {
                message.extend(read(&mut stream, 3).await?);
                let count = u16::from_be_bytes([message[2], message[3]]);
                message.extend(read(&mut stream, 4 * usize::from(count)).await?);
                if message[4..].chunks(4).any(|e| e == (-259i32).to_be_bytes()) {
                    audio = true;
                    stream
                        .write_all(&[0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xfe, 0xfd])
                        .await?;
                }
            }
            3 => {
                message.extend(read(&mut stream, 9).await?);
                stream.write_all(&[2]).await?;
            }
            255 => {
                message.push(stream.read_u8().await?);
                if message[1] == 0 {
                    message.extend(read(&mut stream, 10).await?);
                } else {
                    message.extend(read(&mut stream, 2).await?);
                    if !audio {
                        received.push(message);
                        return Ok(());
                    }
                    match message[3] {
                        0 => {
                            stream.write_all(&[255, 1, 0, 1]).await?;
                            stream
                                .write_all(&[255, 1, 0, 2, 0, 0, 0, 4, 1, 2, 3, 4])
                                .await?;
                        }
                        2 => message.extend(read(&mut stream, 6).await?),
                        _ => {}
                    }
                }
            }
            _ => return Ok(()),
        }
        received.push(message);
    }
}

async fn closes_within(
    ws: &mut (impl futures::Stream<
        Item = tokio_tungstenite::tungstenite::Result<tokio_tungstenite::tungstenite::Message>,
    > + Unpin),
    what: &str,
) {
    use futures::StreamExt;
    use tokio_tungstenite::tungstenite::Message;
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        while let Some(Ok(message)) = ws.next().await {
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} left the console open"));
}

#[tokio::test]
async fn unadmitted_consoles_never_negotiate_or_receive_audio() {
    use futures::{SinkExt, StreamExt};
    use std::future::IntoFuture;
    use std::time::Duration;
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
    use tower_sessions::SessionStore;
    let runtime = tempfile::tempdir().unwrap();
    let socket = runtime.path().join("control.sock");
    let mut reports = fake_vnc_manager(&socket);
    let mut app = app();
    app.control = manager::pod::Backend::Socket(manager::pod::Remote::new(socket));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(axum::serve(listener, bmc::router(app.clone())).into_future());
    let bearer_console = |query: &str| {
        let mut request = format!("ws://{address}/api/console{query}")
            .into_client_request()
            .unwrap();
        request.headers_mut().insert(
            "authorization",
            format!("Bearer {}", bearer(SUBJECT)).parse().unwrap(),
        );
        request
    };
    let encodings = |list: &[i32]| {
        let mut message = vec![2, 0];
        message.extend((list.len() as u16).to_be_bytes());
        list.iter().for_each(|e| message.extend(e.to_be_bytes()));
        message
    };
    let extended_key = [255, 0, 0, 1, 0, 0, 0, 0x61, 0, 0, 0, 0x1e].to_vec();
    let set_format = [255, 1, 0, 2, 3, 2, 0, 0, 0xac, 0x44].to_vec();
    let enable = [255, 1, 0, 0].to_vec();
    let update_request = [3, 1, 0, 0, 0, 0, 2, 128, 1, 224].to_vec();
    // The server's handshake: version, security types, result, ServerInit.
    const PREFIX: usize = 12 + 2 + 4 + 28;
    let ack = [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xfe, 0xfd];
    let begin_and_data = [255, 1, 0, 1, 255, 1, 0, 2, 0, 0, 0, 4, 1, 2, 3, 4];
    let admitted_tail = [&ack[..], &begin_and_data, &[2]].concat();
    for (query, expected_tail, expected_received) in [
        // Unadmitted: only the Bell answers; QEMU saw no -259 and no 255/1.
        (
            "",
            vec![2],
            vec![
                encodings(&[0, -258]),
                extended_key.clone(),
                update_request.clone(),
            ],
        ),
        (
            "?features=audio",
            admitted_tail,
            vec![
                encodings(&[0, -258, -259]),
                extended_key.clone(),
                set_format.clone(),
                enable.clone(),
                update_request.clone(),
            ],
        ),
    ] {
        let (mut ws, _) = tokio_tungstenite::connect_async(bearer_console(query))
            .await
            .unwrap();
        // Handshake and SetEncodings coalesced in one frame, the rest split
        // across frames mid-message.
        let first = [&b"RFB 003.008\n\x01\x01"[..], &encodings(&[0, -258, -259])].concat();
        ws.send(Message::Binary(first.into())).await.unwrap();
        let rest = [&extended_key[..], &set_format, &enable, &update_request].concat();
        for chunk in rest.chunks(3) {
            ws.send(Message::Binary(chunk.to_vec().into()))
                .await
                .unwrap();
        }
        let mut bytes = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), async {
            while bytes.len() < PREFIX + expected_tail.len() {
                match ws.next().await {
                    Some(Ok(Message::Binary(chunk))) => bytes.extend_from_slice(&chunk),
                    other => panic!("console ended early: {other:?}"),
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("{query}: {bytes:?}"));
        assert_eq!(&bytes[PREFIX..], expected_tail, "{query}");
        ws.close(None).await.unwrap();
        let received = tokio::time::timeout(Duration::from_secs(5), reports.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(received, expected_received, "{query}");
    }
    // Losing the audio grant (here: the admin session ends) closes the
    // console without client cooperation.
    let cookie = cookie(app.sessions.clone(), SUBJECT, auth::now() + 60).await;
    let mut request = format!("ws://{address}/api/console?features=audio")
        .into_client_request()
        .unwrap();
    let headers = request.headers_mut();
    headers.insert("origin", "https://vm.example".parse().unwrap());
    headers.insert("cookie", cookie.parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let banner = ws.next().await.unwrap().unwrap();
    assert!(matches!(banner, Message::Binary(ref bytes) if bytes.starts_with(b"RFB ")));
    app.sessions
        .delete(&cookie.split_once('=').unwrap().1.parse().unwrap())
        .await
        .unwrap();
    closes_within(&mut ws, "losing the audio grant").await;
    // An unknown client message closes the console before it reaches QEMU.
    let (mut ws, _) = tokio_tungstenite::connect_async(bearer_console(""))
        .await
        .unwrap();
    let violation = [&b"RFB 003.008\n\x01\x01"[..], &[9]].concat();
    ws.send(Message::Binary(violation.into())).await.unwrap();
    closes_within(&mut ws, "an unknown client message").await;
    reports.recv().await.unwrap();
    let report = tokio::time::timeout(Duration::from_secs(5), reports.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(report.is_empty(), "{report:?}");
    server.abort();
}

#[tokio::test]
async fn redfish_explicit_credentials_and_protocol_headers_are_transparent_without_cookie_fallback()
{
    let (app, upstream) = proxied().await;
    let browser = token_cookie(app.sessions.clone(), SUBJECT, SESSION_TOKEN).await;
    for (name, value) in [
        ("authorization", "Basic dmFsaWRhdG9yOnBhc3N3b3Jk"),
        ("x-auth-token", "explicit-redfish-session"),
    ] {
        let response = bmc::router(app.clone())
            .oneshot(
                req("/redfish/v1/Systems/1", Some(&browser))
                    .header(name, value)
                    .header("odata-version", "4.0")
                    .header("odata-maxversion", "4.0")
                    .header("origin", "https://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let seen = upstream.seen().pop().unwrap();
        assert_eq!(seen.headers[name], value);
        assert_eq!(seen.headers["odata-version"], "4.0");
        assert_eq!(seen.headers["odata-maxversion"], "4.0");
        assert!(!seen.headers.contains_key("cookie"));
        assert!(!seen.headers.values().any(|v| v
            .as_bytes()
            .windows(SESSION_TOKEN.len())
            .any(|w| w == SESSION_TOKEN.as_bytes())));
        if name == "x-auth-token" {
            assert!(!seen.headers.contains_key("authorization"));
        }
        assert_forwarded_only_allowed_headers(&seen);
    }
    // Even empty/malformed/ambiguous credentials are Redfish's decision. They
    // cannot silently select the ambient browser's powerful login.
    for (name, value) in [
        ("authorization", ""),
        ("authorization", "garbage"),
        ("authorization", "Basic !!!"),
        ("x-auth-token", ""),
        ("x-auth-token", "invalid-session"),
    ] {
        let response = bmc::router(app.clone())
            .oneshot(
                req("/redfish/v1/Refused", Some(&browser))
                    .header(name, value)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(upstream.seen().pop().unwrap().headers[name], value);
    }
    let response = bmc::router(app.clone())
        .oneshot(
            req("/redfish/v1/Refused", Some(&browser))
                .header("authorization", "Basic first")
                .header("authorization", "Bearer second")
                .header("x-auth-token", "first-session")
                .header("x-auth-token", "second-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let seen = upstream.seen().pop().unwrap();
    assert_eq!(
        seen.headers
            .get_all("authorization")
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect::<Vec<_>>(),
        ["Basic first", "Bearer second"]
    );
    assert_eq!(
        seen.headers
            .get_all("x-auth-token")
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect::<Vec<_>>(),
        ["first-session", "second-session"]
    );
    assert!(!seen.headers.contains_key("cookie"));
}

#[tokio::test]
async fn redfish_session_bootstrap_response_and_challenges_survive_the_bmc_proxy() {
    let (app, upstream) = proxied().await;
    let path = "/redfish/v1/SessionService/Sessions";
    let body = r#"{"UserName":"validator","Password":"test-only-password"}"#;
    let response = bmc::router(app.clone())
        .oneshot(
            req(path, None)
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let secret = &response.headers()["x-auth-token"];
    assert_eq!(secret, "redfish-session-secret");
    assert!(secret.is_sensitive());
    assert_eq!(
        response.headers()["location"],
        "/redfish/v1/SessionService/Sessions/session-resource-id"
    );
    assert!(response
        .headers()
        .get_all("set-cookie")
        .iter()
        .all(|v| !v.to_str().unwrap().contains("upstream")));
    let seen = upstream.seen().pop().unwrap();
    assert_eq!(seen.body, body.as_bytes());
    assert!(!seen.headers.contains_key("authorization"));
    assert!(!seen.headers.contains_key("cookie"));
    let response = bmc::router(app.clone())
        .oneshot(
            req("/redfish/v1/Challenges", None)
                .header("x-auth-token", "invalid-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get_all("www-authenticate")
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "Bearer realm=\"qrab-redfish\"",
            "Basic realm=\"qrab-redfish\", charset=\"UTF-8\""
        ]
    );
    // Public discovery reaches Redfish without a cookie or manufactured token.
    let response = bmc::router(app.clone())
        .oneshot(req("/redfish/v1/", None).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let seen = upstream.seen().pop().unwrap();
    assert!(!seen.headers.contains_key("cookie"));
    assert!(!seen.headers.contains_key("authorization"));
}

#[tokio::test]
async fn browser_password_session_bootstrap_keeps_csrf_checks_without_injecting_oidc() {
    let (app, upstream) = proxied().await;
    let browser = token_cookie(app.sessions.clone(), SUBJECT, SESSION_TOKEN).await;
    let path = "/redfish/v1/SessionService/Sessions";
    let body = r#"{"UserName":"validator","Password":"test-only-password"}"#;
    let response = bmc::router(app.clone())
        .oneshot(
            req(path, Some(&browser))
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(upstream.seen().is_empty());
    let response = bmc::router(app.clone())
        .oneshot(post(path, &browser, "correct", body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let seen = upstream.seen().pop().unwrap();
    assert_eq!(seen.body, body.as_bytes());
    assert!(!seen.headers.contains_key("authorization"));
    assert!(!seen.headers.contains_key("cookie"));
}

#[tokio::test]
async fn unsupported_redfish_methods_are_decided_by_the_sidecar_not_the_bmc() {
    let (app, upstream) = proxied().await;
    let response = bmc::router(app.clone())
        .oneshot(
            req("/redfish/v1/Systems/1", None)
                .method("PUT")
                .header("x-auth-token", "explicit-redfish-session")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(response.headers()["allow"]
        .to_str()
        .unwrap()
        .contains("GET"));
    let seen = upstream.seen().pop().unwrap();
    assert_eq!(seen.method, "PUT");
    assert_eq!(seen.body, b"{}");
}

#[tokio::test]
async fn public_redfish_is_not_a_bmc_route_and_browser_bridge_strips_its_prefix() {
    let (app, upstream) = proxied().await;
    let browser = token_cookie(app.sessions.clone(), SUBJECT, SESSION_TOKEN).await;
    for path in [
        "/redfish",
        "/redfish/",
        "/redfish/v1/",
        "/redfish/v1/Systems/1",
    ] {
        // Deliberately bypass the browser_path helper: these are public URLs.
        let response = bmc::router(app.clone())
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("cookie", &browser)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
    assert!(upstream.seen().is_empty());
    let response = bmc::router(app.clone())
        .oneshot(
            Request::builder()
                .uri("/bmc/redfish/v1/Systems/1?test=preserved")
                .header("cookie", &browser)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let seen = upstream.seen().pop().unwrap();
    assert_eq!(seen.uri, "/redfish/v1/Systems/1?test=preserved");
    assert_eq!(
        seen.headers["authorization"],
        format!("Bearer {SESSION_TOKEN}")
    );
    assert!(!seen.headers.contains_key("cookie"));
}
