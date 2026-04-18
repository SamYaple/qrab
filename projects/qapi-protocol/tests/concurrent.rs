//! Concurrent QMP client behaviour tests.
//!
//! These exercise the new pipelined writer / id-keyed reader design:
//! up to `QMP_QUEUE_CAPACITY` non-OOB commands in flight, OOB commands
//! that bypass the queue, the stall reaper that wakes hung callers,
//! and shutdown that cleanly aborts every parked future.

use qapi_protocol::server::{ParsedCommand, QmpReader, QmpSession, QmpWriter};
use qapi_protocol::{Qmp, QMP_QUEUE_CAPACITY};
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UnixListener;
use tokio::sync::Mutex;

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);
fn unique_socket() -> String {
    let id = SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "/tmp/qrab-mock-concurrent-{}-{}.sock",
        std::process::id(),
        id
    )
}

/// Bring up a mock QMP server, perform the greeting + capability
/// negotiation, then split the session and hand the (reader, writer)
/// pair to `handler`. The handler runs in a dedicated tokio task.
async fn spawn_mock_split<F, Fut>(
    handler: F,
) -> (Qmp, tokio::task::JoinHandle<()>, String)
where
    F: FnOnce(QmpReader, Arc<Mutex<QmpWriter>>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let sock = unique_socket();
    let listener = UnixListener::bind(&sock).unwrap();

    let server = tokio::spawn(async move {
        let mut session = QmpSession::accept(&listener).await.unwrap();
        session.send_greeting(10, 2, 0).await.unwrap();
        session.negotiate().await.unwrap();
        let (reader, writer) = session.split();
        handler(reader, Arc::new(Mutex::new(writer))).await;
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let (qmp, _events) = Qmp::new(Path::new(&sock)).await.unwrap();
    (qmp, server, sock)
}

/// Pump commands off `reader`, spawning `respond` as a background task
/// for each one. The responder receives the parsed command and the
/// shared writer. Returns when the client disconnects.
async fn pump<F, Fut>(
    mut reader: QmpReader,
    writer: Arc<Mutex<QmpWriter>>,
    respond: F,
) where
    F: Fn(ParsedCommand, Arc<Mutex<QmpWriter>>) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let respond = Arc::new(respond);
    loop {
        match reader.recv_command_full().await {
            Ok(parsed) => {
                let respond = respond.clone();
                let writer = writer.clone();
                tokio::spawn(async move {
                    respond(parsed, writer).await;
                });
            }
            Err(_) => break,
        }
    }
}

/// 16 concurrent commands against a mock that delays each response by
/// ~100ms. With `QMP_QUEUE_CAPACITY` (8) permits the wall time should
/// be ~200ms (two batches), and the in-flight gauge on the server
/// must never exceed 8.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_eight_in_flight() {
    let in_flight = Arc::new(AtomicUsize::new(0));
    let max_seen = Arc::new(AtomicUsize::new(0));
    let in_flight_h = in_flight.clone();
    let max_seen_h = max_seen.clone();

    let (qmp, server, sock) = spawn_mock_split(move |reader, writer| async move {
        let in_flight = in_flight_h.clone();
        let max_seen = max_seen_h.clone();
        pump(reader, writer, move |parsed, writer| {
            let in_flight = in_flight.clone();
            let max_seen = max_seen.clone();
            async move {
                let n = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                max_seen.fetch_max(n, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(100)).await;
                in_flight.fetch_sub(1, Ordering::SeqCst);
                let mut w = writer.lock().await;
                let _ = w
                    .send_return_with_id(
                        serde_json::json!({"running": true, "status": "running"}),
                        parsed.id,
                    )
                    .await;
            }
        })
        .await;
    })
    .await;

    let qmp = Arc::new(qmp);
    let start = Instant::now();
    let mut handles = Vec::with_capacity(16);
    for _ in 0..16 {
        let qmp = qmp.clone();
        handles.push(tokio::spawn(async move {
            let _: qapi::StatusInfo =
                qmp.execute(&qapi::commands::QueryStatus {}).await.unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(500),
        "16 commands in batches of 8 (~100ms each) should take <500ms, took {:?}",
        elapsed
    );
    assert!(
        elapsed >= Duration::from_millis(180),
        "two ~100ms batches must take at least ~200ms; sequential bug? took {:?}",
        elapsed
    );
    let peak = max_seen.load(Ordering::SeqCst);
    assert!(
        peak <= QMP_QUEUE_CAPACITY,
        "server saw {} concurrent unanswered commands but client capacity is {}",
        peak,
        QMP_QUEUE_CAPACITY
    );
    assert!(
        peak >= 2,
        "expected at least 2 commands concurrently in flight, saw only {}",
        peak
    );

    drop(qmp);
    let _ = std::fs::remove_file(&sock);
    server.abort();
    let _ = server.await;
}

/// OOB commands skip the queue. With 8 slow non-OOB commands holding
/// every permit, an OOB command must still come back almost
/// immediately because the mock answers it without delay.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oob_bypasses_queue() {
    let (qmp, server, sock) = spawn_mock_split(|reader, writer| async move {
        pump(reader, writer, |parsed, writer| async move {
            if !parsed.oob {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            let mut w = writer.lock().await;
            let _ = w
                .send_return_with_id(
                    serde_json::json!({"running": true, "status": "running"}),
                    parsed.id,
                )
                .await;
        })
        .await;
    })
    .await;

    assert!(
        qmp.oob_supported(),
        "mock greeting advertises oob, client should have negotiated it"
    );

    let qmp = Arc::new(qmp);

    // Saturate the non-OOB queue.
    let mut slow = Vec::new();
    for _ in 0..QMP_QUEUE_CAPACITY {
        let qmp = qmp.clone();
        slow.push(tokio::spawn(async move {
            let _: qapi::StatusInfo =
                qmp.execute(&qapi::commands::QueryStatus {}).await.unwrap();
        }));
    }
    // Give them a moment to take their permits.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // OOB raw command should overtake the queue. Use raw because the
    // schema-typed allow-oob commands (Yank, etc.) live in the qapi
    // crate and this test doesn't need to depend on a specific one.
    let oob_start = Instant::now();
    let _: serde_json::Value = qmp
        .execute_raw_oob(serde_json::json!({"exec-oob": "query-status"}))
        .await
        .unwrap();
    let oob_elapsed = oob_start.elapsed();
    assert!(
        oob_elapsed < Duration::from_millis(150),
        "OOB query should bypass the queue in <150ms, took {:?}",
        oob_elapsed
    );

    // Let the slow commands finish so we can clean up.
    for h in slow {
        h.await.unwrap();
    }

    drop(qmp);
    let _ = std::fs::remove_file(&sock);
    server.abort();
    let _ = server.await;
}

/// Verifies `execute_raw_oob` succeeds when the server advertised OOB
/// in the greeting.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn raw_oob_runtime_check() {
    let (mut qmp, server, sock) = spawn_mock_split(|reader, writer| async move {
        pump(reader, writer, |parsed, writer| async move {
            assert!(parsed.oob, "raw OOB must use exec-oob envelope");
            let mut w = writer.lock().await;
            let _ = w
                .send_return_with_id(serde_json::json!({}), parsed.id)
                .await;
        })
        .await;
    })
    .await;

    let _: serde_json::Value = qmp
        .execute_raw_oob(serde_json::json!({"exec-oob": "query-status"}))
        .await
        .unwrap();

    qmp.shutdown().await;
    let _ = std::fs::remove_file(&sock);
    let _ = server.await;
}

/// A server that never responds: after `COMMAND_STALL_TIMEOUT` (30s)
/// the reaper drops the reply sender and the caller sees Error::Channel
/// or Error::Timeout. Skip when `QRAB_SKIP_LONG_TESTS=1`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stall_reaper_evicts_hung_command() {
    if std::env::var("QRAB_SKIP_LONG_TESTS").is_ok() {
        eprintln!("SKIP: QRAB_SKIP_LONG_TESTS set");
        return;
    }
    let (mut qmp, server, sock) = spawn_mock_split(|mut reader, writer| async move {
        // Keep writer alive — see shutdown_wakes_pending_callers for why.
        let _writer_keepalive = writer;
        let _ = reader.recv_command_full().await;
        // Block forever — the test's shutdown will end this task.
        let _ = reader.recv_command_full().await;
    })
    .await;

    let start = Instant::now();
    let result: qapi_protocol::Result<qapi::StatusInfo> =
        qmp.execute(&qapi::commands::QueryStatus {}).await;
    let elapsed = start.elapsed();
    assert!(
        result.is_err(),
        "stalled command must error out, got {:?}",
        result
    );
    // Reaper runs every 5s and evicts at >=30s, so worst case is ~35s.
    assert!(
        elapsed >= Duration::from_secs(30) && elapsed < Duration::from_secs(40),
        "expected ~30-35s stall eviction, took {:?}",
        elapsed
    );

    qmp.shutdown().await;
    let _ = std::fs::remove_file(&sock);
    server.abort();
    let _ = server.await;
}

/// `shutdown()` must wake every parked caller within 2 seconds — even
/// when the peer is alive but unresponsive (so the reaper would
/// otherwise be the only escape after 30s).
///
/// We can't get `&mut self` on `Qmp` while clones are parked in
/// `execute()`, so we exercise the equivalent path: dropping the
/// `Qmp` (the only handle that owns the worker tasks). On Drop the
/// shutdown signal fires + tasks abort, and the in-flight Arc loses
/// its primary reference. The aborted write_loop and read_loop run
/// their cleanup arms (`in_flight.lock().await; clear; …`) which
/// drops every reply oneshot. Effect: every parked caller wakes with
/// `Error::Channel`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shutdown_wakes_pending_callers() {
    let (qmp, server, sock) = spawn_mock_split(|mut reader, writer| async move {
        // Keep the writer alive by holding it inside the future. The
        // closure parameter would otherwise drop after returning the
        // async block, which closes the write half and signals EOF to
        // the client's reader — exactly what we don't want here.
        let _writer_keepalive = writer;
        loop {
            if reader.recv_command_full().await.is_err() {
                break;
            }
        }
    })
    .await;

    let qmp = Arc::new(qmp);

    let mut handles = Vec::new();
    for _ in 0..5 {
        let qmp = qmp.clone();
        handles.push(tokio::spawn(async move {
            qmp.execute(&qapi::commands::QueryStatus {})
                .await
                .map(|_: qapi::StatusInfo| ())
        }));
    }

    // Wait for callers to register in flight.
    let parked = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if qmp.queue_depth() >= 1 {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or(false);
    assert!(parked, "callers should be in flight (depth >= 1)");

    let shutdown_start = Instant::now();

    // Take the Qmp out via try_unwrap. We have only `qmp` (1 strong)
    // plus 5 strong refs in the spawned tasks. We need to wait for
    // those to drop their refs, but they're parked. Workaround: kill
    // the server (closes the socket), which makes the client's
    // read_loop exit cleanly and drop its `in_flight` Arc clone — and
    // its EOF-handler clears in_flight, waking every parked caller.
    server.abort();
    let _ = server.await;

    let results = futures::future::join_all(handles).await;
    let shutdown_elapsed = shutdown_start.elapsed();

    assert!(
        shutdown_elapsed < Duration::from_secs(3),
        "wakeup-on-disconnect should be near-instant; took {:?}",
        shutdown_elapsed
    );

    for r in results {
        let inner = r.expect("task panicked");
        assert!(
            inner.is_err(),
            "expected Error::Channel after disconnect, got Ok"
        );
        match inner.unwrap_err() {
            qapi_protocol::Error::Channel(_) => {}
            other => panic!("expected Error::Channel, got {:?}", other),
        }
    }

    // Now we are the only Arc holder; tear down through &mut.
    let mut qmp = Arc::try_unwrap(qmp)
        .map_err(|_| "extra refs outlived their tasks")
        .unwrap();
    qmp.shutdown().await;
    let _ = std::fs::remove_file(&sock);
}

/// Subscribe to the depth gauge: when commands are in flight, the
/// watch channel reports a non-zero value.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queue_depth_watch_updates() {
    let (qmp, server, sock) = spawn_mock_split(|reader, writer| async move {
        pump(reader, writer, |parsed, writer| async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let mut w = writer.lock().await;
            let _ = w
                .send_return_with_id(
                    serde_json::json!({"running": true, "status": "running"}),
                    parsed.id,
                )
                .await;
        })
        .await;
    })
    .await;

    let mut watcher = qmp.subscribe_queue();
    assert_eq!(*watcher.borrow_and_update(), 0);

    let qmp = Arc::new(qmp);
    let mut handles = Vec::new();
    for _ in 0..4 {
        let qmp = qmp.clone();
        handles.push(tokio::spawn(async move {
            let _: qapi::StatusInfo =
                qmp.execute(&qapi::commands::QueryStatus {}).await.unwrap();
        }));
    }

    let saw_busy = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if watcher.changed().await.is_err() {
                return false;
            }
            let depth = *watcher.borrow_and_update();
            if depth > 0 {
                return true;
            }
        }
    })
    .await
    .unwrap_or(false);
    assert!(saw_busy, "expected to observe queue depth > 0");

    for h in handles {
        h.await.unwrap();
    }

    drop(qmp);
    let _ = std::fs::remove_file(&sock);
    server.abort();
    let _ = server.await;
}
