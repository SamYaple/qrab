mod support;
use qapi_protocol::{ClientOptions, Error, QMP_QUEUE_CAPACITY};
use serde_json::json;
use std::{sync::Arc, time::Duration};

#[tokio::test]
async fn pipelined_replies_are_routed_out_of_order() {
    let (client, _, mut peer) = support::qmp(ClientOptions::default()).await;
    let client = Arc::new(client);
    let mut tasks = Vec::new();
    for index in 0..8 {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            let response = client
                .execute_raw(json!({"execute": "echo", "arguments": {"index": index}}))
                .await
                .unwrap();
            assert_eq!(response, json!(index));
        }));
    }
    let mut requests = Vec::new();
    for _ in 0..8 {
        requests.push(peer.recv_command_full().await.unwrap());
    }
    for request in requests.into_iter().rev() {
        peer.send_return_with_id(request.arguments["index"].clone(), request.id)
            .await
            .unwrap();
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(client.queue_depth(), 0);
    client.shutdown().await;
}

#[tokio::test]
async fn cancelled_sent_commands_keep_remote_queue_slots() {
    let (client, _, mut peer) = support::qmp(ClientOptions::default()).await;
    let client = Arc::new(client);
    let mut tasks = Vec::new();
    for _ in 0..QMP_QUEUE_CAPACITY {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            client.execute_raw(json!({"execute": "slow"})).await
        }));
    }
    let mut requests = Vec::new();
    for _ in 0..8 {
        requests.push(peer.recv_command_full().await.unwrap());
    }
    for task in tasks {
        task.abort();
        let _ = task.await;
    }
    let other = client.clone();
    let ninth = tokio::spawn(async move { other.execute_raw(json!({"execute": "ninth"})).await });
    assert!(
        tokio::time::timeout(Duration::from_millis(50), peer.recv_command_full())
            .await
            .is_err()
    );
    assert_eq!(client.queue_depth(), 8);
    let first = requests.remove(0);
    peer.send_return_with_id(json!({}), first.id).await.unwrap();
    let request = peer.recv_command_full().await.unwrap();
    assert_eq!(request.name, "ninth");
    for request in requests {
        peer.send_return_with_id(json!({}), request.id)
            .await
            .unwrap();
    }
    peer.send_return_with_id(json!({}), request.id)
        .await
        .unwrap();
    ninth.await.unwrap().unwrap();
    assert_eq!(client.queue_depth(), 0);
    client.shutdown().await;
}

#[tokio::test]
async fn oob_bypasses_saturated_normal_admission_and_shutdown_wakes_everyone() {
    let (client, _, mut peer) = support::qmp(ClientOptions::default()).await;
    let client = Arc::new(client);
    let mut tasks = Vec::new();
    for _ in 0..9 {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            client.execute_raw(json!({"execute": "slow"})).await
        }));
    }
    for _ in 0..8 {
        peer.recv_command_full().await.unwrap();
    }
    let other = client.clone();
    let oob = tokio::spawn(async move {
        other
            .execute_raw_oob(json!({"exec-oob": "query-yank"}))
            .await
    });
    let request = peer.recv_command_full().await.unwrap();
    assert!(request.oob);
    peer.send_return_with_id(json!([]), request.id)
        .await
        .unwrap();
    assert_eq!(oob.await.unwrap().unwrap(), json!([]));
    // This is real client shutdown, not killing the mock server as a workaround.
    client.shutdown().await;
    for task in tasks {
        assert!(matches!(task.await.unwrap(), Err(Error::Disconnected(_))));
    }
    assert_eq!(client.queue_depth(), 0);
    assert!(peer.recv_command().await.is_err());
}

#[tokio::test]
async fn command_deadline_includes_waiting_for_admission() {
    let (client, _, mut peer) = support::qmp(ClientOptions::default()).await;
    let client = Arc::new(client);
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let client = client.clone();
        tasks.push(tokio::spawn(async move {
            client.execute_raw(json!({"execute": "slow"})).await
        }));
    }
    for _ in 0..8 {
        peer.recv_command_full().await.unwrap();
    }
    let result = client
        .execute_raw_with_timeout(json!({"execute": "never-send"}), Duration::from_millis(20))
        .await;
    assert!(matches!(result, Err(Error::Timeout(_))));
    assert!(
        !client.is_closed(),
        "timing out before dispatch must not close the session"
    );
    client.shutdown().await;
    for task in tasks {
        assert!(task.await.unwrap().is_err());
    }
}

#[tokio::test]
async fn cancelled_unanswered_request_still_has_a_background_deadline() {
    let options = ClientOptions {
        command_timeout: Duration::from_secs(2),
        ..ClientOptions::default()
    };
    let (client, _, mut peer) = support::qmp(options).await;
    let client = Arc::new(client);
    let other = client.clone();
    let task = tokio::spawn(async move { other.execute_raw(json!({"execute": "slow"})).await });
    peer.recv_command().await.unwrap();
    task.abort();
    let _ = task.await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(3)).await;
    let reason = client.closed().await;
    assert!(matches!(&*reason, Error::Timeout(_)));
    assert_eq!(client.queue_depth(), 0);
    assert!(client
        .execute_raw(json!({"execute": "query-status"}))
        .await
        .is_err());
}

#[tokio::test]
async fn event_overflow_and_dropped_receivers_do_not_break_commands() {
    let options = ClientOptions {
        event_capacity: 2,
        ..ClientOptions::default()
    };
    let (client, events, mut peer) = support::qmp(options).await;
    let client = Arc::new(client);
    for _ in 0..10 {
        peer.send_event("STOP", json!({})).await.unwrap();
    }
    let other = client.clone();
    let call = tokio::spawn(async move { other.execute_raw(json!({"execute": "ping"})).await });
    peer.recv_command().await.unwrap();
    peer.send_return(json!({})).await.unwrap();
    call.await.unwrap().unwrap();
    assert_eq!(client.dropped_events(), 8);
    drop(events);
    peer.send_event("STOP", json!({})).await.unwrap();
    let other = client.clone();
    let call = tokio::spawn(async move { other.execute_raw(json!({"execute": "ping"})).await });
    peer.recv_command().await.unwrap();
    peer.send_return(json!({})).await.unwrap();
    call.await.unwrap().unwrap();
    client.shutdown().await;
}

#[tokio::test]
async fn oversized_events_are_not_retained_in_the_event_queue() {
    let options = ClientOptions {
        max_event_bytes: 256,
        ..ClientOptions::default()
    };
    let (client, mut events, mut peer) = support::qmp(options).await;
    let client = Arc::new(client);
    peer.send_event("LARGE", json!({"data":"x".repeat(1024)}))
        .await
        .unwrap();
    let other = client.clone();
    let call = tokio::spawn(async move { other.execute_raw(json!({"execute":"ping"})).await });
    peer.recv_command().await.unwrap();
    peer.send_return(json!({})).await.unwrap();
    call.await.unwrap().unwrap();
    assert_eq!(client.dropped_events(), 1);
    assert!(events.try_recv().is_err());
    client.shutdown().await;
}
