mod support;
use qapi_protocol::{qga::Qga, ClientOptions, Error};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

#[tokio::test]
async fn timed_out_response_closes_session_and_cannot_be_reused() {
    let options = ClientOptions {
        command_timeout: Duration::from_millis(20),
        ..ClientOptions::default()
    };
    let (mut client, mut peer) = support::qga(options).await;
    let (result, _) = tokio::join!(
        client.execute(&qapi::qga::commands::GuestPing {}),
        peer.recv_command()
    );
    assert!(matches!(result, Err(Error::Timeout(_))));
    assert!(client.is_closed());
    assert!(matches!(
        client.execute(&qapi::qga::commands::GuestPing {}).await,
        Err(Error::Closed)
    ));
    assert!(peer.recv_command().await.is_err());
}

#[tokio::test]
async fn cancelling_a_partially_read_response_closes_session() {
    let (client, peer) = UnixStream::pair().unwrap();
    let server = tokio::spawn(async move {
        let (read, mut write) = peer.into_split();
        let mut read = BufReader::new(read);
        assert_eq!(read.read_u8().await.unwrap(), 0xff);
        let sync = support::read(&mut read).await;
        write.write_all(&[0xff]).await.unwrap();
        support::write(&mut write, json!({"return":sync["arguments"]["id"]})).await;
        support::read(&mut read).await;
        support::write(
            &mut write,
            json!({"return":{"version":"11.1.3","supported_commands":[]}}),
        )
        .await;
        support::read(&mut read).await;
        write.write_all(b"{\"return\":").await.unwrap();
        let mut byte = [0u8];
        assert_eq!(read.read(&mut byte).await.unwrap(), 0);
    });
    let mut client = Qga::from_stream(client).await.unwrap();
    assert!(tokio::time::timeout(
        Duration::from_millis(30),
        client.execute(&qapi::qga::commands::GuestPing {})
    )
    .await
    .is_err());
    assert!(client.is_closed());
    server.await.unwrap();
}

#[tokio::test]
async fn sync_ignores_stale_replies_and_resets_at_each_sentinel() {
    let (client, peer) = UnixStream::pair().unwrap();
    let server = tokio::spawn(async move {
        let (read, mut write) = peer.into_split();
        let mut read = BufReader::new(read);
        assert_eq!(read.read_u8().await.unwrap(), 0xff);
        let sync = support::read(&mut read).await;
        let nonce = sync["arguments"]["id"].as_i64().unwrap();
        write
            .write_all(b"stale without newline\xffbroken json\n\xff{\"return\":\"wrong type\"}\n")
            .await
            .unwrap();
        write.write_all(&[0xff]).await.unwrap();
        support::write(&mut write, json!({"return":nonce.wrapping_add(1)})).await;
        write
            .write_all(b"\xff{\"return\":garbage\xff")
            .await
            .unwrap();
        support::write(&mut write, json!({"return":nonce})).await;
        assert_eq!(support::read(&mut read).await["execute"], "guest-info");
        support::write(
            &mut write,
            json!({"return":{"version":"11.1.3","supported_commands":[]}}),
        )
        .await;
        // Keep both socket halves alive until the client explicitly closes.
        let mut byte = [0];
        assert_eq!(read.read(&mut byte).await.unwrap(), 0);
    });
    let mut client = Qga::from_stream(client).await.unwrap();
    assert_eq!(client.version(), (11, 1, 3));
    client.shutdown();
    server.await.unwrap();
}

#[tokio::test]
async fn error_responses_preserve_unknown_classes_and_allow_reuse() {
    let (mut client, mut peer) = support::qga(ClientOptions::default()).await;
    let (result, ()) = tokio::join!(client.execute(&qapi::qga::commands::GuestPing {}), async {
        peer.recv_command().await.unwrap();
        peer.send_error("NewAgentError", "exact").await.unwrap();
    });
    match result.unwrap_err() {
        Error::Qmp(e) => assert_eq!(e.class, "NewAgentError"),
        e => panic!("{e:?}"),
    }
    assert!(!client.is_closed());
    let (result, ()) = tokio::join!(client.execute(&qapi::qga::commands::GuestPing {}), async {
        peer.recv_command().await.unwrap();
        peer.send_return(json!({})).await.unwrap();
    });
    result.unwrap();
}

#[tokio::test]
async fn malformed_reply_is_not_success_and_invalid_arguments_are_not_sent() {
    let (mut client, mut peer) = support::qga(ClientOptions::default()).await;
    assert!(matches!(
        client.execute_raw::<Value>("guest-ping", json!([])).await,
        Err(Error::InvalidRequest(_))
    ));
    assert!(!client.is_closed());
    // Empty object cannot be silently converted into Option<Required> = None.
    #[derive(serde::Deserialize)]
    struct Required {
        _required: String,
    }
    let (result, ()) = tokio::join!(
        client.execute_raw::<Option<Required>>("test", json!({})),
        async {
            peer.recv_command().await.unwrap();
            peer.send_return(json!({})).await.unwrap();
        }
    );
    assert!(matches!(result, Err(Error::Json(_))));
    assert!(
        !client.is_closed(),
        "a typed decoding failure does not desynchronize the wire"
    );
}

#[tokio::test]
async fn response_less_shutdown_is_an_explicit_delivery_only_operation() {
    let (mut client, mut peer) = support::qga(ClientOptions::default()).await;
    let command = qapi::qga::commands::GuestShutdown { mode: None };
    assert!(matches!(
        client.execute(&command).await,
        Err(Error::InvalidRequest(_))
    ));
    client.send_no_response(&command).await.unwrap();
    assert!(client.is_closed());
    assert_eq!(peer.recv_command().await.unwrap().0, "guest-shutdown");
    assert!(peer.recv_command().await.is_err());
}

#[tokio::test]
async fn write_backpressure_is_inside_the_command_deadline() {
    let options = ClientOptions {
        command_timeout: Duration::from_millis(30),
        ..ClientOptions::default()
    };
    let (mut client, _peer) = support::qga(options).await;
    let result = client
        .execute_raw::<Value>("big", json!({"data":"x".repeat(4*1024*1024)}))
        .await;
    assert!(matches!(result, Err(Error::Timeout(_))));
    assert!(client.is_closed());
}
