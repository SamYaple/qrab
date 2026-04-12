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
        VmPhase::Preconfig,
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
