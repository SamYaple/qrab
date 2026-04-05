mod common;

use qapi_protocol::server::{QmpSession, QgaSession};
use qapi_protocol::Qmp;
use qapi_protocol::qga::Qga;
use tokio::net::UnixListener;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);
fn unique_socket() -> String {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("/tmp/qrab-mock-{}-{}.sock", std::process::id(), id)
}

// ── Mock QMP Server Tests ──

#[tokio::test]
async fn mock_qmp_negotiate() {
    let sock = unique_socket();
    let listener = UnixListener::bind(&sock).unwrap();

    let server = tokio::spawn(async move {
        let mut session = QmpSession::accept(&listener).await.unwrap();
        session.send_greeting(10, 2, 0).await.unwrap();
        session.negotiate().await.unwrap();

        // Receive query-status
        let (name, _args) = session.recv_command().await.unwrap();
        assert_eq!(name, "query-status");
        session.send_return(serde_json::json!({
            "running": true,
            "status": "running"
        })).await.unwrap();
    });

    // Give server a moment to bind
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let (mut qmp, _events) = Qmp::new(std::path::Path::new(&sock)).await.unwrap();
    assert_eq!(qapi::qemu_version(), (10, 2, 0));

    let status: qapi::StatusInfo = qmp.execute(&qapi::QueryStatus {}).await.unwrap();
    assert!(status.running);

    qmp.shutdown().await;
    server.await.unwrap();
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn mock_qmp_error_response() {
    let sock = unique_socket();
    let listener = UnixListener::bind(&sock).unwrap();

    let server = tokio::spawn(async move {
        let mut session = QmpSession::accept(&listener).await.unwrap();
        session.send_greeting(10, 2, 0).await.unwrap();
        session.negotiate().await.unwrap();

        let (name, _args) = session.recv_command().await.unwrap();
        assert_eq!(name, "query-status");
        session.send_error("GenericError", "something went wrong").await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let (mut qmp, _events) = Qmp::new(std::path::Path::new(&sock)).await.unwrap();
    let result: Result<qapi::StatusInfo, _> = qmp.execute(&qapi::QueryStatus {}).await;
    match result {
        Err(e) => assert!(e.to_string().contains("something went wrong"), "Got: {}", e),
        Ok(_) => panic!("Expected error"),
    }

    qmp.shutdown().await;
    server.await.unwrap();
    let _ = std::fs::remove_file(&sock);
}

#[tokio::test]
async fn mock_qmp_event() {
    let sock = unique_socket();
    let listener = UnixListener::bind(&sock).unwrap();

    let server = tokio::spawn(async move {
        let mut session = QmpSession::accept(&listener).await.unwrap();
        session.send_greeting(10, 2, 0).await.unwrap();
        session.negotiate().await.unwrap();

        // Send an event then a command response
        session.send_event("STOP", serde_json::json!({})).await.unwrap();

        let (name, _) = session.recv_command().await.unwrap();
        assert_eq!(name, "query-status");
        session.send_return(serde_json::json!({"running": false, "status": "paused"})).await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let (mut qmp, mut events) = Qmp::new(std::path::Path::new(&sock)).await.unwrap();

    let status: qapi::StatusInfo = qmp.execute(&qapi::QueryStatus {}).await.unwrap();
    assert!(!status.running);

    // The event should have been received
    let evt = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        events.recv(),
    ).await.unwrap().unwrap();
    assert_eq!(evt.event, "STOP");

    qmp.shutdown().await;
    server.await.unwrap();
    let _ = std::fs::remove_file(&sock);
}

// ── Mock QGA Server Tests ──

#[tokio::test]
async fn mock_qga_sync_and_ping() {
    let sock = unique_socket();
    let listener = UnixListener::bind(&sock).unwrap();

    let server = tokio::spawn(async move {
        let mut session = QgaSession::accept(&listener).await.unwrap();
        let sync_id = session.handle_sync().await.unwrap();
        assert!(sync_id != 0, "Sync ID should be non-zero");

        // Expect guest-info (auto-sent by Qga::new)
        let (name, _args) = session.recv_command().await.unwrap();
        assert_eq!(name, "guest-info");
        session.send_return(serde_json::json!({
            "version": "10.2.0",
            "supported_commands": [
                {"name": "guest-ping", "enabled": true, "success-response": true},
                {"name": "guest-info", "enabled": true, "success-response": true}
            ]
        })).await.unwrap();

        // Expect guest-ping
        let (name, _args) = session.recv_command().await.unwrap();
        assert_eq!(name, "guest-ping");
        session.send_return(serde_json::json!({})).await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut qga = Qga::new(std::path::Path::new(&sock)).await.unwrap();
    assert_eq!(qapi::qga_version(), "10.2.0");

    // Execute guest-ping
    qga.execute(&qapi::qga::GuestPing {}).await.unwrap();

    server.await.unwrap();
    let _ = std::fs::remove_file(&sock);
}
