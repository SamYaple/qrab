mod support;
use qapi_protocol::{ClientOptions, Error, Qmp};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, net::UnixStream};

#[tokio::test]
async fn oversized_unterminated_frame_fails_without_waiting_for_newline() {
    let options = ClientOptions {
        max_message_bytes: 512,
        ..ClientOptions::default()
    };
    let (client, _, mut reader, mut writer) = support::raw_qmp(options).await;
    let client = Arc::new(client);
    let other = client.clone();
    let call = tokio::spawn(async move { other.execute_raw(json!({"execute": "ping"})).await });
    support::read(&mut reader).await;
    writer.write_all(&vec![b'a'; 1024]).await.unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(1), call)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert!(matches!(&*client.closed().await, Error::Protocol(_)));
}

#[tokio::test]
async fn default_limit_accepts_realistic_large_responses() {
    let (client, _, mut reader, mut writer) = support::raw_qmp(ClientOptions::default()).await;
    let call = tokio::spawn(async move {
        let result = client
            .execute_raw(json!({"execute": "large"}))
            .await
            .unwrap();
        assert_eq!(result.as_str().unwrap().len(), 2 * 1024 * 1024);
        client.shutdown().await;
    });
    let request = support::read(&mut reader).await;
    support::write(
        &mut writer,
        json!({"return": "x".repeat(2 * 1024 * 1024), "id": request["id"]}),
    )
    .await;
    call.await.unwrap();
}

#[tokio::test]
async fn malformed_envelopes_fail_the_connection_instead_of_becoming_success() {
    for malformed in [
        json!({"id": 1}),
        json!({"id": 1, "return": {}, "error": {"class":"X","desc":"X"}}),
        json!({"id": 1, "error": {"desc":"missing class"}}),
        json!([1]),
    ] {
        let (client, _, mut reader, mut writer) = support::raw_qmp(ClientOptions::default()).await;
        let client = Arc::new(client);
        let other = client.clone();
        let call = tokio::spawn(async move { other.execute(&qapi::commands::Quit {}).await });
        support::read(&mut reader).await;
        support::write(&mut writer, malformed).await;
        assert!(tokio::time::timeout(Duration::from_secs(1), call)
            .await
            .unwrap()
            .unwrap()
            .is_err());
        assert!(client.is_closed());
    }
}

#[tokio::test]
async fn truncated_json_at_eof_and_invalid_utf8_are_errors() {
    for bytes in [b"{\"return\":{},\"id\":1}".as_slice(), b"\xff\n".as_slice()] {
        let (client, _, mut reader, mut writer) = support::raw_qmp(ClientOptions::default()).await;
        let call =
            tokio::spawn(async move { client.execute_raw(json!({"execute": "ping"})).await });
        support::read(&mut reader).await;
        writer.write_all(bytes).await.unwrap();
        drop(writer);
        assert!(call.await.unwrap().is_err());
    }
}

#[tokio::test]
async fn fragmented_read_survives_outbound_activity() {
    let (client, mut events, mut reader, mut writer) =
        support::raw_qmp(ClientOptions::default()).await;
    let client = Arc::new(client);
    writer.write_all(b"{\"event\":\"ST").await.unwrap();
    tokio::task::yield_now().await;
    let other = client.clone();
    let call = tokio::spawn(async move { other.execute_raw(json!({"execute": "ping"})).await });
    let request = support::read(&mut reader).await;
    writer
        .write_all(b"OP\",\"timestamp\":{\"seconds\":-1,\"microseconds\":-1}}\r\n")
        .await
        .unwrap();
    support::write(&mut writer, json!({"return": null, "id": request["id"]})).await;
    assert!(call.await.unwrap().unwrap().is_null());
    let event = events.recv().await.unwrap();
    assert_eq!(event.event, "STOP");
    assert_eq!(event.timestamp.microseconds, -1);
    client.shutdown().await;
}

#[tokio::test]
async fn invalid_raw_requests_fail_before_writing_and_do_not_poison_the_session() {
    let (client, _, _peer) = support::qmp(ClientOptions::default()).await;
    for value in [
        json!(null),
        json!({"execute":""}),
        json!({"execute":"ping", "id":7}),
        json!({"execute":"ping", "arguments":[]}),
        json!({"exec-oob":"query-yank"}),
    ] {
        assert!(matches!(
            client.execute_raw(value).await,
            Err(Error::InvalidRequest(_))
        ));
        assert!(!client.is_closed());
        assert_eq!(client.queue_depth(), 0);
    }
    client.shutdown().await;
}

#[tokio::test]
async fn handshake_requires_real_capability_ack_and_preserves_early_events() {
    for response in [
        json!({}),
        json!({"return":{},"id":99}),
        json!({"return":null,"id":0}),
    ] {
        let (client, peer) = UnixStream::pair().unwrap();
        let (read, mut write) = peer.into_split();
        let mut read = tokio::io::BufReader::new(read);
        let server = tokio::spawn(async move {
            support::write(&mut write, support::greeting()).await;
            support::read(&mut read).await;
            support::write(&mut write, response).await;
        });
        assert!(Qmp::from_stream(client).await.is_err());
        server.await.unwrap();
    }
    let (client, peer) = UnixStream::pair().unwrap();
    let server = tokio::spawn(async move {
        let mut peer = qapi_protocol::server::QmpSession::from_stream(peer);
        peer.send_greeting(11, 1, 3).await.unwrap();
        peer.send_event("STOP", json!({})).await.unwrap();
        peer.negotiate().await.unwrap();
        peer
    });
    let (client, mut events) = Qmp::from_stream(client).await.unwrap();
    let _peer = server.await.unwrap();
    assert_eq!(events.recv().await.unwrap().event, "STOP");
    assert_eq!(client.greeting().version.package, "");
    client.shutdown().await;
}

#[tokio::test]
async fn unknown_error_classes_are_preserved_and_valid_errors_allow_next_command() {
    let (client, _, mut peer) = support::qmp(ClientOptions::default()).await;
    let client = Arc::new(client);
    let other = client.clone();
    let call = tokio::spawn(async move { other.execute_raw(json!({"execute":"ping"})).await });
    peer.recv_command().await.unwrap();
    peer.send_error("FutureError", "exact description")
        .await
        .unwrap();
    match call.await.unwrap().unwrap_err() {
        Error::Qmp(error) => {
            assert_eq!(error.class, "FutureError");
            assert_eq!(error.desc, "exact description");
            assert!(error.known_class().is_none());
        }
        other => panic!("{other:?}"),
    }
    assert!(!client.is_closed());
    client.shutdown().await;
}

#[tokio::test]
async fn large_upload_does_not_block_reading_an_earlier_large_reply() {
    use tokio::io::AsyncBufReadExt;
    let (client, _, mut reader, mut writer) = support::raw_qmp(ClientOptions::default()).await;
    let client = Arc::new(client);
    let other = client.clone();
    let download =
        tokio::spawn(async move { other.execute_raw(json!({"execute":"download"})).await });
    let first = support::read(&mut reader).await;
    let other = client.clone();
    let upload = tokio::spawn(async move {
        other
            .execute_raw(json!({"execute":"upload","arguments":{"data":"y".repeat(4*1024*1024)}}))
            .await
    });
    // The second command has begun writing; the socket cannot buffer it all.
    assert!(!reader.fill_buf().await.unwrap().is_empty());
    tokio::time::timeout(Duration::from_secs(2), async {
        support::write(
            &mut writer,
            json!({"return":"x".repeat(2*1024*1024),"id":first["id"]}),
        )
        .await;
        let second = support::read(&mut reader).await;
        assert_eq!(
            second["arguments"]["data"].as_str().unwrap().len(),
            4 * 1024 * 1024
        );
        support::write(&mut writer, json!({"return":{},"id":second["id"]})).await;
        assert_eq!(
            download.await.unwrap().unwrap().as_str().unwrap().len(),
            2 * 1024 * 1024
        );
        upload.await.unwrap().unwrap();
    })
    .await
    .expect("full-duplex read/write deadlocked under backpressure");
    client.shutdown().await;
}

#[tokio::test]
async fn pretty_printed_json_and_escaped_delimiters_are_framed_as_one_message() {
    let (client, _, mut reader, mut writer) = support::raw_qmp(ClientOptions::default()).await;
    let call = tokio::spawn(async move { client.execute_raw(json!({"execute":"pretty"})).await });
    let request = support::read(&mut reader).await;
    let payload = json!([{"text":"quoted \"}]}\" and newline\n inside a string"}]);
    let mut bytes =
        serde_json::to_vec_pretty(&json!({"return":payload,"id":request["id"]})).unwrap();
    bytes.extend_from_slice(b"\r\n");
    // Split in the middle of an escaped string to exercise persistent lexer state.
    for chunk in bytes.chunks(7) {
        writer.write_all(chunk).await.unwrap();
        tokio::task::yield_now().await;
    }
    assert_eq!(call.await.unwrap().unwrap(), payload);
}

#[tokio::test]
async fn shutdown_cancels_a_blocked_write_and_wakes_its_caller() {
    use tokio::io::AsyncBufReadExt;
    let (client, mut events, mut reader, _writer) =
        support::raw_qmp(ClientOptions::default()).await;
    let client = Arc::new(client);
    let other = client.clone();
    let call = tokio::spawn(async move {
        other
            .execute_raw(json!({"execute":"upload","arguments":{"data":"x".repeat(4*1024*1024)}}))
            .await
    });
    assert!(!reader.fill_buf().await.unwrap().is_empty());
    tokio::time::timeout(Duration::from_secs(1), client.shutdown())
        .await
        .unwrap();
    assert!(matches!(call.await.unwrap(), Err(Error::Disconnected(_))));
    assert_eq!(client.queue_depth(), 0);
    assert!(events.recv().await.is_none());
}

#[tokio::test]
async fn unknown_ids_of_any_json_type_are_dropped_without_harming_routing() {
    let (client, _, mut reader, mut writer) = support::raw_qmp(ClientOptions::default()).await;
    let call = tokio::spawn(async move { client.execute_raw(json!({"execute":"ping"})).await });
    let request = support::read(&mut reader).await;
    for id in [
        json!(null),
        json!("stale"),
        json!({"old":true}),
        json!([1]),
        json!(999),
    ] {
        support::write(&mut writer, json!({"return":"wrong","id":id})).await;
    }
    support::write(&mut writer, json!({"return":"correct","id":request["id"]})).await;
    assert_eq!(call.await.unwrap().unwrap(), json!("correct"));
}

#[tokio::test]
async fn idless_server_error_closes_connection_and_preserves_the_cause() {
    let (client, _, mut reader, mut writer) = support::raw_qmp(ClientOptions::default()).await;
    let client = Arc::new(client);
    let other = client.clone();
    let call = tokio::spawn(async move { other.execute_raw(json!({"execute":"ping"})).await });
    support::read(&mut reader).await;
    support::write(
        &mut writer,
        json!({"error":{"class":"GenericError","desc":"parser rejected input"}}),
    )
    .await;
    match call.await.unwrap().unwrap_err() {
        Error::Disconnected(cause) => match &*cause {
            Error::Qmp(error) => assert_eq!(error.desc, "parser rejected input"),
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
    client.shutdown().await;
}
