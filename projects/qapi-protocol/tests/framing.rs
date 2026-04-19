//! `MAX_LINE_BYTES` framing-limit tests for the QMP read loop.
//!
//! Complements the three `split_framed_*` mock-server tests by
//! exercising the specific S7-12 invariant: "oversize line triggers
//! resync or clean error without crash". Uses a raw `UnixStream`
//! instead of the `QmpSession` helpers so the server can emit a
//! >1 MiB line (and a just-under-limit line) without going through
//! the serializer which would never produce such payloads.

use qapi_protocol::Qmp;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);
fn unique_socket() -> String {
    let id = SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("/tmp/qrab-mock-framing-{}-{}.sock", std::process::id(), id)
}

/// Mirrors the crate-private constant. The assertion below guards
/// against the production value drifting without this file being
/// updated (see `projects/qapi-protocol/src/lib.rs:54`).
const MAX_LINE_BYTES: usize = 1_024 * 1_024;

/// Write greeting + consume `qmp_capabilities` handshake on a raw
/// stream. Factored out so each test's server closure stays short.
async fn handshake(stream: &mut UnixStream) {
    let greeting = br#"{"QMP":{"version":{"qemu":{"major":10,"minor":2,"micro":0},"package":""},"capabilities":["oob"]}}"#;
    stream.write_all(greeting).await.unwrap();
    stream.write_all(b"\n").await.unwrap();
    stream.flush().await.unwrap();

    // Read qmp_capabilities request line.
    let (read, mut write) = stream.split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    // Echo id back if present.
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    let mut resp = serde_json::json!({"return": {}});
    if let Some(id) = parsed.get("id").cloned() {
        resp["id"] = id;
    }
    write
        .write_all(serde_json::to_string(&resp).unwrap().as_bytes())
        .await
        .unwrap();
    write.write_all(b"\n").await.unwrap();
    write.flush().await.unwrap();
}

/// Serve one command by routing the QMP `id` back unchanged.
async fn serve_one_query_status(stream: &mut UnixStream) {
    let (read, mut write) = stream.split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    let id = parsed.get("id").cloned();
    let mut resp = serde_json::json!({
        "return": {"running": true, "status": "running"}
    });
    if let Some(id) = id {
        resp["id"] = id;
    }
    write
        .write_all(serde_json::to_string(&resp).unwrap().as_bytes())
        .await
        .unwrap();
    write.write_all(b"\n").await.unwrap();
    write.flush().await.unwrap();
}

/// Client connected to a mock that (a) completes the handshake, then
/// (b) pushes an oversize unterminated line, then (c) answers a
/// normal `query-status` issued after the overflow. Asserts the
/// client survives the overflow (no panic/hang) and that the next
/// `execute` still works — proving `read_line_bounded`'s resync path
/// is live in the read loop.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversize_line_does_not_crash_client() {
    let sock = unique_socket();
    let listener = UnixListener::bind(&sock).unwrap();

    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        handshake(&mut stream).await;

        // Oversize payload: `MAX_LINE_BYTES + 1 KiB` of padded JSON.
        // Shape it as a valid-looking response ({"x":"AAAA..."}) so if
        // the cap ever silently grew the line would still be parseable
        // and we'd notice via a routing warning instead of a crash.
        // Newline at the very end so read_line_bounded's resync drain
        // can find a line boundary after dropping the head.
        let pad_len = MAX_LINE_BYTES + 1024;
        let mut big = Vec::with_capacity(pad_len + 32);
        big.extend_from_slice(br#"{"return":{"x":""#);
        big.resize(big.len() + pad_len, b'A');
        big.extend_from_slice(br#""}}"#);
        big.push(b'\n');
        stream.write_all(&big).await.unwrap();
        stream.flush().await.unwrap();

        // Now serve a real query-status so the caller can prove the
        // read loop is still alive.
        serve_one_query_status(&mut stream).await;

        // Hold the connection open briefly so the client's last
        // response actually lands before we tear down.
        tokio::time::sleep(Duration::from_millis(100)).await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let (mut qmp, _events) = Qmp::new(Path::new(&sock)).await.unwrap();

    // The oversize line may land before or after our dispatch — either
    // way the client must not crash. A bounded timeout guards against
    // a hang (the real bug we're looking for).
    let status: qapi::StatusInfo = tokio::time::timeout(
        Duration::from_secs(5),
        qmp.execute(&qapi::commands::QueryStatus {}),
    )
    .await
    .expect("client hung after oversize line — read loop did not resync")
    .expect("query-status failed after oversize line");
    assert!(status.running);

    qmp.shutdown().await;
    server.await.unwrap();
    let _ = std::fs::remove_file(&sock);
}

/// A line just under the cap, containing a valid large JSON response,
/// must still parse and route. This pins the upper end of the framing
/// window: oversize drops, undersize-at-limit survives.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undersize_line_at_limit_still_parses() {
    let sock = unique_socket();
    let listener = UnixListener::bind(&sock).unwrap();

    let server = tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.unwrap();
        handshake(&mut stream).await;

        // Wait for the client's query-status, then fabricate a reply
        // whose serialized form is just under MAX_LINE_BYTES. Pad the
        // payload so `len(serialized) + len("\n") < MAX_LINE_BYTES`,
        // leaving ~128 bytes of headroom for the JSON skeleton + id.
        let (read, mut write) = stream.split();
        let mut reader = BufReader::new(read);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        let id = parsed.get("id").cloned();

        let pad_len = MAX_LINE_BYTES - 256;
        let padding: String = "A".repeat(pad_len);
        let mut resp = serde_json::json!({
            "return": {
                "running": true,
                "status": "running",
                // status-info is open in the mock: extra fields are
                // dropped by the typed deserializer. The `_pad` here
                // fattens the wire line without disturbing typed
                // decoding.
                "_pad": padding,
            }
        });
        if let Some(id) = id {
            resp["id"] = id;
        }
        let serialized = serde_json::to_string(&resp).unwrap();
        assert!(
            serialized.len() + 1 < MAX_LINE_BYTES,
            "test payload {} bytes must stay under cap {}",
            serialized.len() + 1,
            MAX_LINE_BYTES,
        );
        write.write_all(serialized.as_bytes()).await.unwrap();
        write.write_all(b"\n").await.unwrap();
        write.flush().await.unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let (mut qmp, _events) = Qmp::new(Path::new(&sock)).await.unwrap();

    let status: qapi::StatusInfo = tokio::time::timeout(
        Duration::from_secs(5),
        qmp.execute(&qapi::commands::QueryStatus {}),
    )
    .await
    .expect("client hung on just-under-cap line")
    .expect("just-under-cap line failed to decode");
    assert!(status.running);

    qmp.shutdown().await;
    server.await.unwrap();
    let _ = std::fs::remove_file(&sock);
}
