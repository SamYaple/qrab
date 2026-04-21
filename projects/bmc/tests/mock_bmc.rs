use std::collections::VecDeque;
use std::sync::Arc;

use qapi_protocol::QmpEvent;
use tokio::sync::{broadcast, Mutex};

// --- Phase 1 tests ---

/// Test that the event history ring buffer caps at the expected size.
#[tokio::test]
async fn event_history_ring_buffer_cap() {
    let cap = 1000;
    let history: Arc<Mutex<VecDeque<QmpEvent>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(cap)));
    let (tx, _rx) = broadcast::channel::<QmpEvent>(256);

    for i in 0..1200 {
        let event = QmpEvent {
            event: format!("TEST_{i}"),
            timestamp: qapi_protocol::Timestamp {
                seconds: i as i64,
                microseconds: 0,
            },
            data: serde_json::Value::Null,
        };
        let _ = tx.send(event.clone());
        let mut h = history.lock().await;
        h.push_back(event);
        if h.len() > cap {
            h.pop_front();
        }
    }

    let h = history.lock().await;
    assert_eq!(h.len(), cap);
    assert_eq!(h.front().unwrap().event, "TEST_200");
    assert_eq!(h.back().unwrap().event, "TEST_1199");
}

/// Test that QmpEvent can be serialized and cloned.
#[tokio::test]
async fn qmp_event_serialize_clone() {
    let event = QmpEvent {
        event: "STOP".into(),
        timestamp: qapi_protocol::Timestamp {
            seconds: 1234567890,
            microseconds: 123456,
        },
        data: serde_json::json!({}),
    };

    let cloned = event.clone();
    assert_eq!(cloned.event, "STOP");
    assert_eq!(cloned.timestamp.seconds, 1234567890);

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("STOP"));
    assert!(json.contains("1234567890"));

    let deserialized: QmpEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.event, "STOP");
}

/// Test that broadcast channel delivers events to multiple subscribers.
#[tokio::test]
async fn broadcast_multiple_subscribers() {
    let (tx, mut rx1) = broadcast::channel::<QmpEvent>(16);
    let mut rx2 = tx.subscribe();

    let event = QmpEvent {
        event: "RESET".into(),
        timestamp: qapi_protocol::Timestamp {
            seconds: 0,
            microseconds: 0,
        },
        data: serde_json::Value::Null,
    };

    tx.send(event).unwrap();

    let e1 = rx1.recv().await.unwrap();
    let e2 = rx2.recv().await.unwrap();
    assert_eq!(e1.event, "RESET");
    assert_eq!(e2.event, "RESET");
}

// --- Phase 2 tests ---

/// Test BmcEvent DTO serialization round-trip.
#[tokio::test]
async fn bmc_event_dto_roundtrip() {
    use bmc::types::BmcEvent;

    let event = BmcEvent {
        event: "SHUTDOWN".into(),
        timestamp_secs: 1000,
        timestamp_usecs: 500,
        data: serde_json::json!({"guest": true}),
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: BmcEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.event, "SHUTDOWN");
    assert_eq!(deserialized.timestamp_secs, 1000);
    assert_eq!(deserialized.timestamp_usecs, 500);
    assert_eq!(deserialized.data["guest"], true);
}

/// Test CpuInfoResponse DTO serialization.
#[tokio::test]
async fn cpu_info_response_dto() {
    use bmc::types::{CpuInfoResponse, GuestCpu};

    let info = CpuInfoResponse {
        guest_cpus: vec![
            GuestCpu {
                index: 0,
                thread_id: 12345,
                qom_path: "/machine/unattached/device[0]".into(),
            },
            GuestCpu {
                index: 1,
                thread_id: 12346,
                qom_path: "/machine/unattached/device[1]".into(),
            },
        ],
        host_threads: 8,
    };

    let json = serde_json::to_string(&info).unwrap();
    let deserialized: CpuInfoResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.guest_cpus.len(), 2);
    assert_eq!(deserialized.host_threads, 8);
    assert_eq!(deserialized.guest_cpus[0].thread_id, 12345);
}

/// Test KvmInfoResponse DTO serialization.
#[tokio::test]
async fn kvm_info_response_dto() {
    use bmc::types::KvmInfoResponse;

    let info = KvmInfoResponse {
        enabled: true,
        present: true,
    };

    let json = serde_json::to_string(&info).unwrap();
    let deserialized: KvmInfoResponse = serde_json::from_str(&json).unwrap();
    assert!(deserialized.enabled);
    assert!(deserialized.present);
}

/// Test VmStatusResponse DTO serialization with all phases.
#[tokio::test]
async fn vm_status_response_all_phases() {
    use bmc::types::{VmPhase, VmStatusResponse};

    for phase in [
        VmPhase::Off,
        VmPhase::Initializing,
        VmPhase::Running,
        VmPhase::Paused,
        VmPhase::ShuttingDown,
    ] {
        let resp = VmStatusResponse {
            phase: phase.clone(),
            running: Some(true),
            status: Some("inmigrate".into()),
            qemu_version: Some("9.0.0".into()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: VmStatusResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.phase, phase);
    }
}

// --- Attestation tests ---

/// Test AttestationState default is NotAvailable.
#[tokio::test]
async fn attestation_state_default_not_available() {
    use bmc::types::{AttestationState, AttestationStatus};

    let state = AttestationState::default();
    assert_eq!(state.status, AttestationStatus::NotAvailable);
    assert!(state.launch_measure.is_none());
    assert!(state.sev_capabilities.is_none());
    assert!(state.last_attestation_report.is_none());
}

/// Test AttestationState lifecycle: Pending -> Measured -> Verified.
#[tokio::test]
async fn attestation_state_lifecycle() {
    use bmc::types::{
        AttestationReportResponse, AttestationState, AttestationStatus, SevCapabilitiesResponse,
    };

    let mut state = AttestationState::default();

    // Start: not available
    assert_eq!(state.status, AttestationStatus::NotAvailable);

    // SEV detected -> Pending
    state.status = AttestationStatus::Pending;
    assert_eq!(state.status, AttestationStatus::Pending);

    // Launch measurement captured -> Measured
    state.launch_measure = Some("dGVzdC1tZWFzdXJlbWVudA==".into());
    state.sev_capabilities = Some(SevCapabilitiesResponse {
        pdh: "cGRoLWtleQ==".into(),
        cert_chain: "Y2VydC1jaGFpbg==".into(),
        cbitpos: 51,
        reduced_phys_bits: 1,
    });
    state.status = AttestationStatus::Measured;
    assert_eq!(state.status, AttestationStatus::Measured);
    assert!(state.launch_measure.is_some());
    assert!(state.sev_capabilities.is_some());

    // Attestation report requested -> Verified
    state.last_attestation_report = Some(AttestationReportResponse {
        data: "cmVwb3J0LWRhdGE=".into(),
        mnonce: "bm9uY2U=".into(),
        timestamp: "1713400000Z".into(),
    });
    state.status = AttestationStatus::Verified;
    assert_eq!(state.status, AttestationStatus::Verified);
    assert!(state.last_attestation_report.is_some());

    // Serialize round-trip
    let json = serde_json::to_string(&state).unwrap();
    let deserialized: AttestationState = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.status, AttestationStatus::Verified);
    assert_eq!(
        deserialized.launch_measure.as_deref(),
        Some("dGVzdC1tZWFzdXJlbWVudA==")
    );
    assert_eq!(deserialized.sev_capabilities.unwrap().cbitpos, 51);
    assert_eq!(
        deserialized.last_attestation_report.unwrap().mnonce,
        "bm9uY2U="
    );
}

/// Test AttestationStatus::Failed carries error message.
#[tokio::test]
async fn attestation_status_failed_carries_message() {
    use bmc::types::AttestationStatus;

    let status = AttestationStatus::Failed("SEV not initialized".into());
    let json = serde_json::to_string(&status).unwrap();
    let deserialized: AttestationStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, AttestationStatus::Failed("SEV not initialized".into()));
}

/// Test SevCapabilitiesResponse serialization round-trip.
#[tokio::test]
async fn sev_capabilities_response_roundtrip() {
    use bmc::types::SevCapabilitiesResponse;

    let caps = SevCapabilitiesResponse {
        pdh: "cGRo".into(),
        cert_chain: "Y2VydA==".into(),
        cbitpos: 51,
        reduced_phys_bits: 1,
    };
    let json = serde_json::to_string(&caps).unwrap();
    let deserialized: SevCapabilitiesResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.pdh, "cGRo");
    assert_eq!(deserialized.cbitpos, 51);
    assert_eq!(deserialized.reduced_phys_bits, 1);
}

/// Test AttestationReportResponse serialization round-trip.
#[tokio::test]
async fn attestation_report_response_roundtrip() {
    use bmc::types::AttestationReportResponse;

    let report = AttestationReportResponse {
        data: "cmVwb3J0".into(),
        mnonce: "bm9uY2U=".into(),
        timestamp: "1713400000Z".into(),
    };
    let json = serde_json::to_string(&report).unwrap();
    let deserialized: AttestationReportResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.data, "cmVwb3J0");
    assert_eq!(deserialized.mnonce, "bm9uY2U=");
    assert_eq!(deserialized.timestamp, "1713400000Z");
}

/// Test that event history preserves ordering.
#[tokio::test]
async fn event_history_ordering() {
    let history: Arc<Mutex<VecDeque<QmpEvent>>> = Arc::new(Mutex::new(VecDeque::new()));

    let events = ["STOP", "RESUME", "RESET", "SHUTDOWN"];
    for (i, name) in events.iter().enumerate() {
        let event = QmpEvent {
            event: name.to_string(),
            timestamp: qapi_protocol::Timestamp {
                seconds: i as i64,
                microseconds: 0,
            },
            data: serde_json::Value::Null,
        };
        history.lock().await.push_back(event);
    }

    let h = history.lock().await;
    let names: Vec<&str> = h.iter().map(|e| e.event.as_str()).collect();
    assert_eq!(names, events);
}

// --- BmcPush SSE envelope tests ---
//
// These exercise the serde representation only; the runtime
// broadcast/snapshot wiring is covered by the compile and by the
// state-level unit tests that run under `--features server`.

/// Adjacent tagging (tag="kind", content="data") must round-trip every
/// variant. The client SSE dispatcher matches on `kind`, so if this
/// format ever changes the browser silently drops frames.
#[test]
fn bmc_push_snapshot_roundtrip() {
    use bmc::types::{BmcPush, BmcSnapshot, BmcConfig, DashboardState, QueueStatus};

    let orig = BmcPush::Snapshot(BmcSnapshot {
        dashboard: DashboardState::default(),
        queue: QueueStatus::default(),
        config: BmcConfig::default(),
        events: Vec::new(),
    });
    let json = serde_json::to_string(&orig).unwrap();
    assert!(json.contains("\"kind\":\"snapshot\""), "tag missing: {json}");
    assert!(json.contains("\"data\""), "content missing: {json}");
    let back: BmcPush = serde_json::from_str(&json).unwrap();
    assert_eq!(back, orig);
}

#[test]
fn bmc_push_heartbeat_roundtrip() {
    use bmc::types::BmcPush;
    let json = serde_json::to_string(&BmcPush::Heartbeat).unwrap();
    assert!(
        json.contains("\"kind\":\"heartbeat\""),
        "heartbeat tag: {json}"
    );
    let back: BmcPush = serde_json::from_str(&json).unwrap();
    assert_eq!(back, BmcPush::Heartbeat);
}

#[test]
fn bmc_push_config_roundtrip() {
    use bmc::types::{BmcConfig, BmcPush};
    let cfg = BmcConfig {
        memory: "2G".into(),
        cpus: Some(4),
    };
    let orig = BmcPush::Config(cfg.clone());
    let json = serde_json::to_string(&orig).unwrap();
    assert!(json.contains("\"kind\":\"config\""));
    assert!(json.contains("\"2G\""));
    let back: BmcPush = serde_json::from_str(&json).unwrap();
    assert_eq!(back, BmcPush::Config(cfg));
}

#[test]
fn bmc_push_queue_roundtrip() {
    use bmc::types::{BmcPush, QueueStatus};
    let q = QueueStatus {
        depth: 3,
        capacity: 8,
        oob_supported: true,
    };
    let orig = BmcPush::Queue(q.clone());
    let json = serde_json::to_string(&orig).unwrap();
    assert!(json.contains("\"kind\":\"queue\""));
    let back: BmcPush = serde_json::from_str(&json).unwrap();
    assert_eq!(back, BmcPush::Queue(q));
}

#[test]
fn bmc_push_qmp_event_roundtrip() {
    use bmc::types::{BmcEvent, BmcPush};
    let ev = BmcEvent {
        event: "STOP".into(),
        timestamp_secs: 100,
        timestamp_usecs: 5,
        data: serde_json::json!({"foo": "bar"}),
    };
    let orig = BmcPush::QmpEvent(ev.clone());
    let json = serde_json::to_string(&orig).unwrap();
    assert!(json.contains("\"kind\":\"qmp_event\""));
    let back: BmcPush = serde_json::from_str(&json).unwrap();
    assert_eq!(back, BmcPush::QmpEvent(ev));
}

#[test]
fn bmc_push_dashboard_roundtrip() {
    use bmc::types::{BmcPush, DashboardState};
    let d = DashboardState::default();
    let orig = BmcPush::Dashboard(d.clone());
    let json = serde_json::to_string(&orig).unwrap();
    assert!(json.contains("\"kind\":\"dashboard\""));
    let back: BmcPush = serde_json::from_str(&json).unwrap();
    assert_eq!(back, BmcPush::Dashboard(d));
}
