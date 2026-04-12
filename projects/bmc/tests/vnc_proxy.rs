//! End-to-end test: QEMU VNC → Unix socket → WebSocket proxy → WS client.
//! Verifies the proxy actually relays VNC protocol correctly.

use std::sync::Arc;
use std::time::Duration;

use bmc::state::BmcState;
use manager::arch::{Arch, MachineConfig};
use manager::vm::VmBuilder;
use tokio::sync::Mutex;

#[tokio::test]
async fn websocket_proxy_relays_vnc_handshake() {
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    // 1. Spawn QEMU with VNC display directly via VmBuilder
    let preconfig = VmBuilder::new(
        Arch::X86_64,
        MachineConfig::Bios { memory: "256M".into() },
    )
    .display()
    .spawn()
    .await
    .expect("spawn");

    let vnc_sock = preconfig.vnc_sock().expect("vnc_sock").clone();
    eprintln!("VNC socket: {}", vnc_sock.display());

    // Start machine so VGA device is fully initialized
    let vm = preconfig.start().await.expect("start");
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 2. Set up BmcState with the VNC socket path
    let state = BmcState::new();
    *state.vnc_sock.lock().await = Some(vnc_sock.clone());

    // 3. Start minimal Axum server with just the console router
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
    assert_eq!(height, 480, "640x480 = VGA graphic console (not 384 = text console)");

    // Cleanup
    let _ = ws.close(None).await;
    vm.shutdown().await;
}
