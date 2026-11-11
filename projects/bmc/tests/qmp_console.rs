//! Integration tests for the schema-driven QMP console.
//!
//! The plan asks for six test cases:
//!
//! 1. Validation rejects unknown command
//! 2. Validation rejects missing required field
//! 3. Validation accepts a flat-union command
//! 4. `query-status` (empty args) passes validation
//! 5. OOB path: envelope reaches the mock with `exec-oob`
//! 6. `qmp_queue_status` reflects live state
//!
//! Cases 1-4 exercise the validator directly via `validate_envelope`
//! (exported from the server module). Case 5 drives `Qmp::execute_raw_oob`
//! against a mock server and verifies the envelope shape the server
//! produces — the construction helper (`build_qmp_request`) is tested
//! in the module's unit tests; this test verifies the mock sees the
//! `exec-oob` verb. Case 6 calls the `qmp_queue_status` server fn
//! directly with the VM state in `Off`.
//!
//! Tests that require an actual `manager::vm::Vm` in `BmcState::Running`
//! aren't possible without a live `qemu-system-*` process; that part of
//! the flow is covered by the `manager` crate's own integration tests.

use bmc::server::qmp_exec::{
    build_qmp_request, qmp_queue_status, validate_envelope,
};
use qapi_protocol::server::QmpSession;
use qapi_protocol::Qmp;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::UnixListener;

static COUNTER: AtomicU64 = AtomicU64::new(1);
fn unique_socket() -> String {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("/tmp/qrab-bmc-qmp-{}-{}.sock", std::process::id(), id)
}

// -- Case 1: unknown command --
#[tokio::test]
async fn validation_rejects_unknown_command() {
    let env = serde_json::json!({"execute": "nonsense"});
    let err = validate_envelope(&env).expect_err("should reject");
    assert!(
        err.reason.contains("unknown command"),
        "expected unknown-command reason, got {err:?}"
    );
}

// -- Case 2: missing required field --
#[tokio::test]
async fn validation_rejects_missing_driver() {
    let env = serde_json::json!({
        "execute": "blockdev-add",
        "arguments": {}
    });
    let err = validate_envelope(&env).expect_err("should reject");
    assert!(
        err.path.ends_with("/driver"),
        "path should pinpoint /arguments/driver, got {err:?}"
    );
}

// -- Case 3: flat union accepted --
#[tokio::test]
async fn validation_accepts_flat_union() {
    let env = serde_json::json!({
        "execute": "blockdev-add",
        "arguments": {
            "driver": "file",
            "node-name": "n",
            "filename": "/tmp/x"
        }
    });
    validate_envelope(&env).expect("should accept");
}

// -- Case 4: empty args command --
#[tokio::test]
async fn validation_accepts_query_status() {
    let env = serde_json::json!({"execute": "query-status"});
    validate_envelope(&env).expect("should accept");
}

// -- Case 5: OOB path routes through exec-oob verb --
#[tokio::test]
async fn oob_envelope_reaches_mock() {
    let sock = unique_socket();
    let listener = UnixListener::bind(&sock).unwrap();

    // Server: advertise OOB, negotiate, then receive an `exec-oob`
    // verb and reply. If any `execute` / non-OOB envelope arrives the
    // test panics.
    let server = tokio::spawn(async move {
        let mut session = QmpSession::accept(&listener).await.unwrap();
        session.send_greeting(10, 2, 0).await.unwrap();
        session.negotiate().await.unwrap();

        let parsed = session.recv_command_full().await.unwrap();
        assert!(parsed.oob, "mock expected exec-oob verb, got execute");
        assert_eq!(parsed.name, "quit");
        session.send_return(serde_json::json!({})).await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let (qmp, _events) = Qmp::new(std::path::Path::new(&sock)).await.unwrap();
    assert!(qmp.oob_supported(), "handshake should negotiate OOB");

    // Build the envelope using the real server-side helper — what the
    // `execute_qmp` server fn would have constructed.
    let envelope = serde_json::json!({
        "execute": "quit",
        "arguments": {}
    });
    let wire = build_qmp_request(&envelope, true);
    assert_eq!(wire["exec-oob"], "quit");

    qmp.execute_raw_oob(wire).await.expect("oob dispatch succeeds");

    drop(qmp);
    server.await.unwrap();
    let _ = std::fs::remove_file(&sock);
}

// -- Case 6: queue status reflects state.vm == Off --
#[tokio::test]
async fn queue_status_defaults_when_vm_off() {
    // A stale legacy process default must not masquerade as a connected VM.
    let _ = qapi::set_qemu_version(99, 0, 0);
    assert_eq!(bmc::server::qmp_exec::current_qemu_version().await.unwrap(), None);
    let status = qmp_queue_status().await.expect("server fn returns");
    assert_eq!(status.depth, 0, "no vm -> zero depth");
    assert_eq!(status.capacity, 8, "default capacity matches Qmp::queue_capacity");
    assert!(
        !status.oob_supported,
        "without a peer there is nothing to advertise"
    );
}

// -- Bonus: non-OOB request round-trips through the mock --
#[tokio::test]
async fn non_oob_envelope_reaches_mock() {
    let sock = unique_socket();
    let listener = UnixListener::bind(&sock).unwrap();

    let server = tokio::spawn(async move {
        let mut session = QmpSession::accept(&listener).await.unwrap();
        session.send_greeting(10, 2, 0).await.unwrap();
        session.negotiate().await.unwrap();

        let parsed = session.recv_command_full().await.unwrap();
        assert!(!parsed.oob, "expected regular execute, got exec-oob");
        assert_eq!(parsed.name, "query-status");
        session
            .send_return(serde_json::json!({"running": true, "status": "running"}))
            .await
            .unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let (qmp, _events) = Qmp::new(std::path::Path::new(&sock)).await.unwrap();

    let envelope = serde_json::json!({"execute": "query-status"});
    let wire = build_qmp_request(&envelope, false);
    assert_eq!(wire["execute"], "query-status");
    assert!(wire.get("exec-oob").is_none());

    let resp = qmp.execute_raw(wire).await.expect("dispatch");
    assert_eq!(resp["running"], true);

    drop(qmp);
    server.await.unwrap();
    let _ = std::fs::remove_file(&sock);
}

// Note: the hand-rolled scorer's ordering is covered by unit tests inside
// `components/qmp_console/search.rs`. Exposing it here would require making
// `components` a public module of the `bmc` library, which is out of scope
// for this feature.

// --- Push helper tests ---
//
// BmcState's push_* helpers replace the per-component polling loops.
// These tests verify that subscribers to push_tx see a frame after each
// helper runs, which is the contract the SSE handler and client store
// rely on. They do NOT require a running VM — with VmState::Off the
// helpers emit default slices.

/// `push_config` must broadcast the mutated config so every open SSE
/// client re-renders without a refetch.
#[tokio::test]
async fn push_config_emits_frame() {
    use bmc::state::BmcState;
    use bmc::types::{BmcConfig, BmcPush};

    let state = BmcState::new();
    let mut rx = state.push_tx.subscribe();

    // Mutate config, then push.
    state.config.lock().await.memory = "2G".into();
    state.push_config().await;

    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("push_config should broadcast within 1s")
        .expect("broadcast recv");
    match frame {
        BmcPush::Config(BmcConfig { memory, .. }) => {
            assert_eq!(memory, "2G", "pushed config should carry mutated memory");
        }
        other => panic!("expected Config frame, got {other:?}"),
    }
}

/// `push_dashboard` must emit a Dashboard frame even when the VM is
/// off — the client renders that as the "powered off" state.
#[tokio::test]
async fn push_dashboard_emits_frame_when_off() {
    use bmc::state::BmcState;
    use bmc::types::{BmcPush, VmPhase};

    let state = BmcState::new();
    let mut rx = state.push_tx.subscribe();

    state.push_dashboard().await;

    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("push_dashboard should broadcast within 1s")
        .expect("broadcast recv");
    match frame {
        BmcPush::Dashboard(d) => {
            assert_eq!(d.status.phase, VmPhase::Off);
        }
        other => panic!("expected Dashboard frame, got {other:?}"),
    }
}

/// `push_queue` broadcasts the current QMP queue depth. With the VM
/// off the defaults flow through.
#[tokio::test]
async fn push_queue_emits_default_when_off() {
    use bmc::state::BmcState;
    use bmc::types::BmcPush;

    let state = BmcState::new();
    let mut rx = state.push_tx.subscribe();

    state.push_queue().await;

    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("push_queue should broadcast within 1s")
        .expect("broadcast recv");
    match frame {
        BmcPush::Queue(q) => {
            assert_eq!(q.depth, 0);
            assert!(!q.oob_supported);
        }
        other => panic!("expected Queue frame, got {other:?}"),
    }
}

/// `snapshot` returns the first-frame seed that the SSE handler sends
/// on connect. With an idle state every slice should be its default.
#[tokio::test]
async fn snapshot_returns_defaults_when_off() {
    use bmc::state::BmcState;
    use bmc::types::VmPhase;

    let state = BmcState::new();
    let snap = state.snapshot().await;
    assert_eq!(snap.dashboard.status.phase, VmPhase::Off);
    assert_eq!(snap.queue.depth, 0);
    assert!(snap.events.is_empty());
    assert_eq!(snap.config.memory, "1G");
}
