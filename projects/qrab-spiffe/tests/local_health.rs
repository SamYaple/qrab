#![cfg(feature = "http")]
//! Probe transport stays local and cannot expose application routes.
use axum::{http::StatusCode, routing::get, Router};
use std::{net::SocketAddr, time::Duration};

#[tokio::test]
async fn only_loopback_fixed_health_paths_are_probed_and_application_paths_are_hidden() {
    for address in ["0.0.0.0:8080", "10.244.0.1:8080", "[::]:8080"] {
        assert!(qrab_spiffe::http::require_loopback(address.parse().unwrap()).is_err());
    }
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(|| async { StatusCode::SERVICE_UNAVAILABLE }))
        .route("/private", get(|| async { "secret application data" }))
        .layer(axum::middleware::from_fn(qrab_spiffe::http::health_only));
    let server = tokio::spawn(async move { axum::serve(socket, app).await });
    qrab_spiffe::http::probe_health(address, "/healthz")
        .await
        .unwrap();
    assert!(qrab_spiffe::http::probe_health(address, "/readyz")
        .await
        .is_err());
    assert!(qrab_spiffe::http::probe_health(address, "/private")
        .await
        .is_err());
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    for path in ["/private", "/healthz?private=1", "/health%7a"] {
        assert_eq!(
            client
                .get(format!("http://{address}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    server.abort();
}

#[tokio::test]
async fn dribbling_response_headers_cannot_extend_the_total_probe_budget() {
    use tokio::io::AsyncWriteExt;
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address: SocketAddr = socket.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = socket.accept().await.unwrap();
        for byte in b"HTTP/1.1 200 OK\r\n" {
            if stream.write_all(&[*byte]).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    });
    let began = std::time::Instant::now();
    assert!(qrab_spiffe::http::probe_health(address, "/healthz")
        .await
        .is_err());
    assert!(began.elapsed() < Duration::from_secs(3));
    server.abort();
}
