use std::collections::VecDeque;
use std::sync::Arc;

use qapi_protocol::QmpEvent;
use tokio::sync::{broadcast, Mutex};

/// Test that the event history ring buffer caps at the expected size.
#[tokio::test]
async fn event_history_ring_buffer_cap() {
    let cap = 1000;
    let history: Arc<Mutex<VecDeque<QmpEvent>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(cap)));
    let (tx, _rx) = broadcast::channel::<QmpEvent>(256);

    // Push more events than the cap
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
    // Oldest should be TEST_200 (first 200 were evicted)
    assert_eq!(h.front().unwrap().event, "TEST_200");
    assert_eq!(h.back().unwrap().event, "TEST_1199");
}

/// Test that QmpEvent can be serialized and cloned (the derives we added).
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

    // Clone
    let cloned = event.clone();
    assert_eq!(cloned.event, "STOP");
    assert_eq!(cloned.timestamp.seconds, 1234567890);

    // Serialize
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("STOP"));
    assert!(json.contains("1234567890"));

    // Deserialize back
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
