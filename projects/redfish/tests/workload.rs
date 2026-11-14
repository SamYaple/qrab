//! Independent workload admission and application authentication over real TLS.
use axum::serve::Listener as _;
use openidconnect::{core::CoreJsonWebKeySet, ClientId, IssuerUrl};
use qrab_redfish::{auth::Authenticator, workload::Listener, AppState};
use qrab_spiffe::{
    fixture::{Authority, Workload},
    id, Identity,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::Semaphore,
};

const SERVER: &str = "spiffe://example.org/redfish";
const CLIENT: &str = "spiffe://example.org/validator";
const OTHER: &str = "spiffe://example.org/unlisted";

async fn identity(ca: &Authority, name: &str) -> (Workload, Identity) {
    let workload = Workload::serve(vec![ca.svid(name)]);
    let identity = Identity::connect(&workload.endpoint).await.unwrap();
    (workload, identity)
}

async fn server(ca: &Authority) -> (Workload, SocketAddr, tokio::task::JoinHandle<()>) {
    let (workload, identity) = identity(ca, SERVER).await;
    let listener = Listener::bind(
        "127.0.0.1:0".parse().unwrap(),
        &identity,
        &[id(CLIENT).unwrap()],
    )
    .await
    .unwrap();
    let address = listener.local_addr().unwrap();
    let state = AppState {
        auth: Arc::new(Authenticator::new(
            IssuerUrl::new("https://idp.example/".into()).unwrap(),
            ClientId::new("redfish".into()),
            CoreJsonWebKeySet::new(vec![]),
            ["admin".into()].into_iter().collect(),
            Default::default(),
        )),
        control: manager::pod::Backend::Socket(manager::pod::Remote::new(
            "/unused-workload-test.sock".into(),
        )),
        streams: Arc::new(Semaphore::new(8)),
        requests: Arc::new(Semaphore::new(32)),
        tasks: Default::default(),
        telemetry: Default::default(),
        triggers: Default::default(),
        subscriptions: Default::default(),
        exporter: Default::default(),
    };
    let task = tokio::spawn(async move {
        axum::serve(listener, qrab_redfish::router(state))
            .await
            .unwrap();
    });
    (workload, address, task)
}

async fn get<IO: AsyncRead + AsyncWrite + Unpin>(
    mut io: IO,
    path: &str,
) -> std::io::Result<String> {
    // Even a spoofed identity header cannot authorize the application request.
    io.write_all(format!("GET {path} HTTP/1.1\r\nHost: redfish\r\nX-Forwarded-Spiffe-Id: {CLIENT}\r\nConnection: close\r\n\r\n").as_bytes()).await?;
    let mut output = String::new();
    io.read_to_string(&mut output).await?;
    Ok(output)
}

#[tokio::test]
async fn accepted_workload_still_needs_application_credentials() {
    let ca = Authority::new();
    let (_server_api, address, task) = server(&ca).await;
    let (_client_api, client) = identity(&ca, CLIENT).await;
    let connector = client.connector(&[id(SERVER).unwrap()]).unwrap();
    for (path, expected) in [
        ("/healthz", "HTTP/1.1 200"),
        ("/redfish/v1/Systems/1", "HTTP/1.1 401"),
    ] {
        let socket = TcpStream::connect(address).await.unwrap();
        let tls = connector.connect(socket).await.unwrap();
        assert!(get(tls, path).await.unwrap().starts_with(expected));
    }
    task.abort();
}

#[tokio::test]
async fn unlisted_workload_never_reaches_http_even_with_a_claimed_header() {
    let ca = Authority::new();
    let (_server_api, address, task) = server(&ca).await;
    let (_client_api, client) = identity(&ca, OTHER).await;
    let connector = client.connector(&[id(SERVER).unwrap()]).unwrap();
    let result = async {
        let tls = connector
            .connect(TcpStream::connect(address).await?)
            .await?;
        get(tls, "/healthz").await.map_err(qrab_spiffe::Error::from)
    }
    .await;
    assert!(result.is_err());
    task.abort();
}

#[derive(Debug)]
struct NoCertificate;
impl rustls::client::ResolvesClientCert for NoCertificate {
    fn resolve(
        &self,
        _: &[&[u8]],
        _: &[rustls::SignatureScheme],
    ) -> Option<Arc<rustls::sign::CertifiedKey>> {
        None
    }
    fn has_certs(&self) -> bool {
        false
    }
}

#[tokio::test]
async fn tls_client_without_certificate_is_rejected() {
    let ca = Authority::new();
    let (_server_api, address, task) = server(&ca).await;
    let (_client_api, client) = identity(&ca, CLIENT).await;
    // Keep the genuine exact-server verifier; remove only our client certificate.
    let mut config = client.client_config(&[id(SERVER).unwrap()], &[]).unwrap();
    config.client_auth_cert_resolver = Arc::new(NoCertificate);
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let result = async {
        let tls = connector
            .connect(
                "spiffe.invalid".try_into().unwrap(),
                TcpStream::connect(address).await?,
            )
            .await?;
        get(tls, "/healthz").await
    }
    .await;
    assert!(result.is_err());
    task.abort();
}

#[tokio::test]
async fn a_stalled_handshake_does_not_serialize_an_authorized_client() {
    let ca = Authority::new();
    let (_server_api, address, task) = server(&ca).await;
    let _stalled = TcpStream::connect(address).await.unwrap();
    let (_client_api, client) = identity(&ca, CLIENT).await;
    let connector = client.connector(&[id(SERVER).unwrap()]).unwrap();
    let response = tokio::time::timeout(Duration::from_secs(2), async {
        let tls = connector
            .connect(TcpStream::connect(address).await.unwrap())
            .await
            .unwrap();
        get(tls, "/healthz").await.unwrap()
    })
    .await
    .expect("another client's stalled handshake must not block admission");
    assert!(response.starts_with("HTTP/1.1 200"));
    task.abort();
}

#[tokio::test]
async fn established_connections_hold_admission_until_their_io_is_dropped() {
    let ca = Authority::new();
    let (_server_api, address, task) = server(&ca).await;
    let (_client_api, client) = identity(&ca, CLIENT).await;
    let connector = client.connector(&[id(SERVER).unwrap()]).unwrap();
    let mut held = Vec::new();
    for _ in 0..qrab_redfish::workload::MAX_CONNECTIONS {
        held.push(
            connector
                .connect(TcpStream::connect(address).await.unwrap())
                .await
                .unwrap(),
        );
    }
    let waiting = connector.connect(TcpStream::connect(address).await.unwrap());
    tokio::pin!(waiting);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut waiting)
            .await
            .is_err()
    );
    drop(held.pop());
    let tls = tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .unwrap()
        .unwrap();
    assert!(get(tls, "/healthz")
        .await
        .unwrap()
        .starts_with("HTTP/1.1 200"));
    task.abort();
}
