//! Real TLS/HTTP and WebSocket clients with Workload API fixture SVIDs.
use axum::{
    Json, Router,
    extract::{
        Request,
        ws::{Message, WebSocketUpgrade},
    },
    http::StatusCode,
    routing::get,
};
use futures::{SinkExt, StreamExt};
use qrab_dashboard::{
    bmc::Bmc,
    redfish::{Redfish, RedfishError, SYSTEM},
};
use qrab_operator::api::VirtualMachine;
use qrab_spiffe::{
    Identity,
    fixture::{Authority, Workload},
    http::{Listener, install_tls13_provider},
};
use std::{
    net::IpAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
const DASH: &str = "spiffe://yapplesauce.com/kubernetes/qrab/qrab-dashboard";
const TOKEN: &str = "verified-user-id-token";
async fn identity(ca: &Authority, id: &str) -> (Workload, Identity) {
    install_tls13_provider().unwrap();
    let workload = Workload::serve(vec![ca.svid(id)]);
    let source = Identity::connect(&workload.endpoint).await.unwrap();
    (workload, source)
}
fn vm(uid: &str) -> VirtualMachine {
    serde_json::from_value(serde_json::json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine","metadata":{"name":"test","namespace":"qrab","uid":uid},"spec":{"stateVolumeClaim":"test-state"}})).unwrap()
}
fn service(uid: &str, component: &str) -> String {
    format!("spiffe://yapplesauce.com/kubernetes/qrab/vm/{uid}/{component}")
}
async fn server(identity: &Identity, router: Router) -> (u16, tokio::task::JoinHandle<()>) {
    use axum::serve::Listener as _;
    let listener = Listener::bind(
        "127.0.0.1:0".parse().unwrap(),
        identity,
        &[qrab_spiffe::id(DASH).unwrap()],
    )
    .await
    .unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (port, task)
}
#[tokio::test]
async fn redfish_bearer_is_sent_only_to_the_exact_vms_authenticated_service() {
    let ca = Authority::new();
    let (_dw, dash) = identity(&ca, DASH).await;
    let (_rw, redfish) = identity(&ca, &service("uid-a", "redfish")).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let app = Router::new().route(
        SYSTEM,
        get(move |request: Request| async move {
            seen.fetch_add(1, Ordering::SeqCst);
            assert_eq!(
                request.headers().get("authorization").unwrap(),
                format!("Bearer {TOKEN}").as_str()
            );
            assert!(request.headers().get("cookie").is_none());
            Json(serde_json::json!({"PowerState":"On"}))
        }),
    );
    let (port, task) = server(&redfish, app).await;
    let client = Redfish::new(dash, port);
    let ip: IpAddr = "127.0.0.1".parse().unwrap();
    assert!(
        client
            .for_vm(&vm("uid-b"), ip)
            .unwrap()
            .get::<serde_json::Value>(SYSTEM, TOKEN, 1024)
            .await
            .is_err()
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "wrong VM must not receive the user token"
    );
    let result = client
        .for_vm(&vm("uid-a"), ip)
        .unwrap()
        .get::<serde_json::Value>(SYSTEM, TOKEN, 1024)
        .await
        .unwrap();
    assert_eq!(result["PowerState"], "On");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    task.abort();
}
#[tokio::test]
async fn an_allowed_workload_still_observes_application_refusals_and_no_redirects() {
    let ca = Authority::new();
    let (_dw, dash) = identity(&ca, DASH).await;
    let (_rw, redfish) = identity(&ca, &service("uid-a", "redfish")).await;
    let (port, task) = server(
        &redfish,
        Router::new().route(SYSTEM, get(|| async { StatusCode::UNAUTHORIZED })),
    )
    .await;
    let client = Redfish::new(dash.clone(), port)
        .for_vm(&vm("uid-a"), "127.0.0.1".parse().unwrap())
        .unwrap();
    assert_eq!(
        client
            .get::<serde_json::Value>(SYSTEM, TOKEN, 1024)
            .await
            .unwrap_err(),
        RedfishError::Status(401)
    );
    task.abort();
    let (port, task) = server(
        &redfish,
        Router::new().route(
            SYSTEM,
            get(|| async { (StatusCode::FOUND, [("location", "http://127.0.0.1:9/leak")]) }),
        ),
    )
    .await;
    let client = Redfish::new(dash, port)
        .for_vm(&vm("uid-a"), "127.0.0.1".parse().unwrap())
        .unwrap();
    assert_eq!(
        client
            .get::<serde_json::Value>(SYSTEM, TOKEN, 1024)
            .await
            .unwrap_err(),
        RedfishError::Status(302)
    );
    task.abort();
}
#[tokio::test]
async fn console_uses_wss_with_user_bearer_and_the_bmc_role_id_of_this_vm() {
    let ca = Authority::new();
    let (_dw, dash) = identity(&ca, DASH).await;
    let (_bw, bmc) = identity(&ca, &service("uid-a", "bmc")).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let app = Router::new().route(
        "/api/console",
        get(
            move |headers: axum::http::HeaderMap, ws: WebSocketUpgrade| async move {
                assert_eq!(
                    headers.get("authorization").unwrap(),
                    format!("Bearer {TOKEN}").as_str()
                );
                assert!(headers.get("cookie").is_none());
                seen.fetch_add(1, Ordering::SeqCst);
                ws.on_upgrade(|mut socket| async move {
                    if let Some(Ok(Message::Binary(bytes))) = socket.recv().await {
                        socket.send(Message::Binary(bytes)).await.unwrap();
                    }
                })
            },
        ),
    );
    let (port, task) = server(&bmc, app).await;
    let client = Bmc::new(dash, port);
    let ip = "127.0.0.1".parse().unwrap();
    assert!(client.connect(&vm("uid-b"), ip, TOKEN, &[]).await.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let mut socket = client.connect(&vm("uid-a"), ip, TOKEN, &[]).await.unwrap();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Binary(
            b"RFB pixels".to_vec().into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        socket.next().await.unwrap().unwrap().into_data(),
        b"RFB pixels".as_slice()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    task.abort();
}
#[tokio::test]
async fn redfish_has_no_plaintext_fallback() {
    let ca = Authority::new();
    let (_dw, dash) = identity(&ca, DASH).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 4096];
        let n = socket.read(&mut bytes).await.unwrap();
        assert_eq!(bytes[0], 22, "must start TLS rather than HTTP");
        assert!(
            !bytes[..n]
                .windows(TOKEN.len())
                .any(|b| b == TOKEN.as_bytes())
        );
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
            .await
            .unwrap();
    });
    let client = Redfish::new(dash, port)
        .for_vm(&vm("uid-a"), "127.0.0.1".parse().unwrap())
        .unwrap();
    assert!(
        client
            .get::<serde_json::Value>(SYSTEM, TOKEN, 1024)
            .await
            .is_err()
    );
    task.await.unwrap();
}

#[tokio::test]
async fn an_authenticated_tls12_only_server_cannot_receive_the_bearer() {
    use tokio::io::AsyncReadExt;
    install_tls13_provider().unwrap();
    let ca = Authority::new();
    let (_dw, dash) = identity(&ca, DASH).await;
    let issued = ca.svid(&service("uid-a", "redfish"));
    let work = Workload::serve(vec![issued.clone()]);
    let source = Identity::connect(&work.endpoint).await.unwrap();
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS12])
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        source
            .source()
            .svid()
            .unwrap()
            .cert_chain()
            .iter()
            .map(|cert| rustls::pki_types::CertificateDer::from(cert.as_ref().to_vec()))
            .collect(),
        rustls::pki_types::PrivateKeyDer::Pkcs8(issued.x509_svid_key.into()),
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let result = tokio_rustls::TlsAcceptor::from(Arc::new(config))
            .accept(tcp)
            .await;
        if let Ok(mut stream) = result {
            let mut byte = [0; 1];
            let _ = stream.read_exact(&mut byte).await;
            panic!("TLS1.2 must not negotiate");
        }
    });
    let client = Redfish::new(dash, port)
        .for_vm(&vm("uid-a"), "127.0.0.1".parse().unwrap())
        .unwrap();
    assert!(
        client
            .get::<serde_json::Value>(SYSTEM, TOKEN, 1024)
            .await
            .is_err()
    );
    task.await.unwrap();
}
