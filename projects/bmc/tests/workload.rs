//! The real BMC router on the shared TLS listener and its Redfish mTLS bridge.
use axum::{extract::Request, routing::get, Json, Router};
use manager::pod;
use openidconnect::{core::CoreJsonWebKeySet, ClientId, IssuerUrl};
use qrab_spiffe::{
    fixture::{Authority, Workload},
    http::{install_tls13_provider, Listener},
    id, Identity,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::Semaphore;
const DASH: &str = "spiffe://yapplesauce.com/kubernetes/qrab/qrab-dashboard";
const BMC: &str = "spiffe://yapplesauce.com/kubernetes/qrab/vm/uid-a/bmc";
const RF: &str = "spiffe://yapplesauce.com/kubernetes/qrab/vm/uid-a/redfish";
async fn identity(ca: &Authority, name: &str) -> (Workload, Identity) {
    install_tls13_provider().unwrap();
    let w = Workload::serve(vec![ca.svid(name)]);
    let i = Identity::connect(&w.endpoint).await.unwrap();
    (w, i)
}
async fn serve(identity: &Identity, peer: &str, app: Router) -> (u16, tokio::task::JoinHandle<()>) {
    use axum::serve::Listener as _;
    let listener = Listener::bind(
        "127.0.0.1:0".parse().unwrap(),
        identity,
        &[id(peer).unwrap()],
    )
    .await
    .unwrap();
    let port = listener.local_addr().unwrap().port();
    (
        port,
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
    )
}
fn state(redfish: bmc::proxy::Upstream) -> bmc::AppState {
    let config = bmc::config::Config::from_vars(|key| match key {
        "BMC_BEARER_ISSUER" => Some("https://idp.example".into()),
        "BMC_BEARER_AUDIENCE" => Some("dashboard".into()),
        "BMC_ADMIN_SUBJECTS" => Some("6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b".into()),
        _ => None,
    })
    .unwrap();
    bmc::AppState {
        oidc: None,
        access: bmc::auth::Access::from_config(&config),
        bearer: Some(Arc::new(oidc_bearer::Verifier::new(
            IssuerUrl::new("https://idp.example".into()).unwrap(),
            ClientId::new("dashboard".into()),
            CoreJsonWebKeySet::new(vec![]),
        ))),
        config,
        control: pod::Backend::Socket(pod::Remote::new("/unused-mtls-control.sock".into())),
        sessions: Default::default(),
        consoles: Arc::new(Semaphore::new(4)),
        event_streams: Arc::new(Semaphore::new(16)),
        redfish,
    }
}
#[tokio::test]
async fn transport_identity_alone_does_not_sign_a_user_in() {
    let ca = Authority::new();
    let (_bw, bmc_identity) = identity(&ca, BMC).await;
    let (_dw, dash) = identity(&ca, DASH).await;
    let app = bmc::router(state(
        bmc::proxy::Upstream::new("http://127.0.0.1:9").unwrap(),
    ));
    let (port, task) = serve(&bmc_identity, DASH, app).await;
    let client = qrab_spiffe::http::client(&dash, id(BMC).unwrap()).unwrap();
    let response = client
        .get(format!("https://127.0.0.1:{port}/api/session"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
    task.abort();
}
#[tokio::test]
async fn explicit_credentials_cross_both_tls_hops_only_for_this_vms_redfish_id() {
    let ca = Authority::new();
    let (_bw, bmc_identity) = identity(&ca, BMC).await;
    let (_dw, dash) = identity(&ca, DASH).await;
    let (_rw, rf) = identity(&ca, RF).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let app = Router::new().route(
        "/redfish/v1/Systems/1",
        get(move |request: Request| async move {
            assert_eq!(
                request.headers().get("authorization").unwrap(),
                "Bearer explicit-user-token"
            );
            assert!(request.headers().get("cookie").is_none());
            seen.fetch_add(1, Ordering::SeqCst);
            Json(serde_json::json!({"PowerState":"On"}))
        }),
    );
    let (rfport, rftask) = serve(&rf, BMC, app).await;
    for expected in [
        "spiffe://yapplesauce.com/kubernetes/qrab/vm/uid-b/redfish",
        RF,
    ] {
        let upstream = bmc::proxy::Upstream::mtls(
            &format!("https://127.0.0.1:{rfport}"),
            &bmc_identity,
            id(expected).unwrap(),
        )
        .unwrap();
        let (port, task) = serve(&bmc_identity, DASH, bmc::router(state(upstream))).await;
        let client = qrab_spiffe::http::client(&dash, id(BMC).unwrap()).unwrap();
        let response = client
            .get(format!("https://127.0.0.1:{port}/bmc/redfish/v1/Systems/1"))
            .bearer_auth("explicit-user-token")
            .send()
            .await
            .unwrap();
        if expected == RF {
            assert_eq!(response.status(), 200);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        } else {
            assert_eq!(response.status(), 503);
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
        task.abort();
    }
    rftask.abort();
}
#[test]
fn unprotected_remote_or_default_webpki_origins_are_not_a_proxy_fallback() {
    for origin in [
        "http://10.244.0.1:8081",
        "http://rf.example:8081",
        "https://127.0.0.1:8444",
        "http://127.0.0.1:8081/path",
        "http://user:secret@127.0.0.1:8081",
    ] {
        assert!(bmc::proxy::Upstream::new(origin).is_err());
    }
}
