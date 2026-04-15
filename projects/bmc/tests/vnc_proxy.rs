//! End-to-end test: QEMU VNC → Unix socket → WebSocket proxy → WS client.
//! Verifies the proxy actually relays VNC protocol correctly.

use std::path::PathBuf;
use std::time::Duration;

use bmc::state::BmcState;
use bmc::types::{DiskConfig, DiskFormat, DiskInterface};
use manager::Arch;

/// RAII wrapper around a per-test cirros image copy. Removes the temp file
/// when dropped so parallel tests don't accumulate gigabytes in /tmp.
struct TestDisk {
    pub config: DiskConfig,
}

impl Drop for TestDisk {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.config.file);
    }
}

/// Make a per-test copy of the cirros image to avoid qcow2 write-lock
/// contention when tests run in parallel. Path uniqueness uses a process-wide
/// atomic counter so tests in the same binary don't collide on `process::id()`.
async fn test_disk(label: &str) -> TestDisk {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("target/test-fixtures");
    let src = cache_dir.join("cirros-0.6.2-x86_64-disk.img");
    if !src.exists() {
        let url = "https://download.cirros-cloud.net/0.6.2/cirros-0.6.2-x86_64-disk.img";
        let _ = tokio::fs::create_dir_all(&cache_dir).await;
        let out = tokio::process::Command::new("curl")
            .args(["-fSL", "-o", &src.display().to_string(), url])
            .output().await.expect("curl");
        assert!(out.status.success(), "cirros download failed");
    }
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dest = std::env::temp_dir().join(format!(
        "qrab-vnc-proxy-{label}-{}-{}.qcow2",
        std::process::id(),
        id
    ));
    tokio::fs::copy(&src, &dest).await.expect("copy cirros");
    TestDisk {
        config: DiskConfig {
            file: dest,
            format: DiskFormat::Qcow2,
            interface: DiskInterface::Virtio,
            read_only: false,
        },
    }
}

#[tokio::test]
async fn websocket_proxy_relays_vnc_handshake() {
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    // Use the full BmcState power-on path so vnc_sock comes from the VM handle.
    let state = BmcState::new();
    // Per-test cirros copy avoids file-lock contention with parallel tests.
    // Keep `_disk` bound to function scope so its Drop runs after force_off.
    let _disk = test_disk("vnc-proxy-test").await;
    {
        let mut config = state.config.lock().await;
        config.disks = vec![_disk.config.clone()];
    }
    state.power_on().await.expect("power_on");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Start minimal Axum server with just the console router
    let router = bmc::server::console::console_router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().unwrap();
    eprintln!("Test server at {addr}");

    tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });

    // 4. Connect WebSocket client
    let ws_url = format!("ws://{addr}/api/console");
    eprintln!("Connecting WS to {ws_url}");

    let (mut ws, _response) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .expect("ws connect");

    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;

    // Helper: read all available binary data from WS
    async fn read_ws_binary(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> Vec<u8> {
        match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
            Ok(Some(Ok(Message::Binary(data)))) => data.to_vec(),
            Ok(Some(Ok(other))) => panic!("Expected binary, got: {other:?}"),
            Ok(Some(Err(e))) => panic!("WS error: {e}"),
            Ok(None) => panic!("WS stream ended"),
            Err(_) => panic!("WS read timeout"),
        }
    }

    // 5. VNC handshake through WebSocket proxy
    // Read server version
    let version = read_ws_binary(&mut ws).await;
    let version_str = String::from_utf8_lossy(&version);
    eprintln!("Server version: {version_str:?}");
    assert!(version_str.starts_with("RFB "), "Expected VNC protocol version");

    // Send client version
    ws.send(Message::Binary(version.into())).await.expect("send version");

    // Read security types
    let sec = read_ws_binary(&mut ws).await;
    eprintln!("Security data: {sec:?}");
    let num_types = sec[0] as usize;
    assert!(sec[1..1 + num_types].contains(&1), "Expected None auth");

    // Select None auth
    ws.send(Message::Binary(vec![1].into())).await.expect("send auth");

    // Read security result
    let result = read_ws_binary(&mut ws).await;
    eprintln!("Auth result: {result:?}");
    assert_eq!(
        u32::from_be_bytes([result[0], result[1], result[2], result[3]]),
        0,
        "Auth should succeed"
    );

    // Send ClientInit (shared=1)
    ws.send(Message::Binary(vec![1].into())).await.expect("send clientinit");

    // Read ServerInit
    let init = read_ws_binary(&mut ws).await;
    eprintln!("ServerInit: {} bytes", init.len());
    assert!(init.len() >= 24, "ServerInit too short");

    let width = u16::from_be_bytes([init[0], init[1]]);
    let height = u16::from_be_bytes([init[2], init[3]]);
    let name_len = u32::from_be_bytes([init[20], init[21], init[22], init[23]]) as usize;
    let name = String::from_utf8_lossy(&init[24..24 + name_len]).to_string();

    eprintln!("\n=== VNC through WebSocket proxy ===");
    eprintln!("  Resolution: {width}x{height}");
    eprintln!("  Name: {name:?}");

    assert_eq!(name, "QEMU", "Should show graphic console");
    // 720x400 = VGA text mode 3 (SeaBIOS POST output via real VGA framebuffer).
    // 640x480 = VNC's "no graphic display device" placeholder (failure mode).
    assert_eq!(width, 720, "Should be 720x400 VGA text mode, not 640x480 placeholder");
    assert_eq!(height, 400, "Should be 720x400 VGA text mode, not 640x480 placeholder");

    // Cleanup
    let _ = ws.close(None).await;
    state.force_off().await.expect("force_off");
}

/// Verify the proxy closes the WebSocket immediately when no VM is running
/// (so vnc_sock() returns None).
#[tokio::test]
async fn vnc_proxy_no_sock_returns_immediately() {
    let state = BmcState::new();
    // VmState::Off → vnc_sock() returns None
    assert!(state.vnc_sock().await.is_none());

    let router = bmc::server::console::console_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.ok() });

    let ws_url = format!("ws://{addr}/api/console");
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    use futures::StreamExt;
    // Server should close the connection immediately
    let result = tokio::time::timeout(Duration::from_secs(2), ws.next()).await;
    match result {
        Ok(None) => {} // clean close
        Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_)))) => {} // close frame
        Ok(Some(Err(_))) => {} // error is fine
        Ok(Some(Ok(other))) => panic!("expected close, got: {other:?}"),
        Err(_) => panic!("server did not close WS within 2s"),
    }
}

/// Verify that closing the client WebSocket releases the underlying VNC
/// stream so the VM can be force-shut-down quickly.
#[tokio::test]
async fn vnc_proxy_client_disconnect_releases_vnc() {
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();
    let _disk = test_disk("vnc-proxy-disconnect").await;
    {
        let mut config = state.config.lock().await;
        config.disks = vec![_disk.config.clone()];
    }
    state.power_on().await.expect("power_on");
    tokio::time::sleep(Duration::from_millis(300)).await;

    let router = bmc::server::console::console_router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.ok() });

    let ws_url = format!("ws://{addr}/api/console");
    let (ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    // Drop the WS without doing the handshake — proxy should clean up
    drop(ws);

    // force_off should complete quickly (proxy held no lingering refs)
    let shutdown_start = std::time::Instant::now();
    state.force_off().await.expect("force_off");
    let elapsed = shutdown_start.elapsed();
    assert!(
        elapsed < Duration::from_secs(3),
        "force_off took {elapsed:?} — proxy may be holding the socket"
    );
}
