mod support;
use qapi_protocol::{ClientOptions, Error};
use serde_json::json;

#[qapi::qapi(command, returns = "()", allow_oob(added = "11.0.0"))]
struct NewOob {}

#[qapi::qapi(command, returns = "()", no_success_response(added = "11.0.0"))]
struct ChangedReply {}

#[tokio::test]
async fn qmp_uses_the_peer_release_for_oob_and_reply_flags() {
    let (old, _, mut old_peer) = support::qmp_version(ClientOptions::default(), (10, 2, 0)).await;
    let (new, _, mut new_peer) = support::qmp_version(ClientOptions::default(), (11, 1, 3)).await;
    assert!(matches!(
        old.execute_oob(&NewOob {}).await,
        Err(Error::InvalidRequest(_))
    ));
    let (result, ()) = tokio::join!(new.execute_oob(&NewOob {}), async {
        let request = new_peer.recv_command_full().await.unwrap();
        assert!(request.oob);
        new_peer
            .send_return_with_id(json!({}), request.id)
            .await
            .unwrap();
    });
    result.unwrap();
    assert!(matches!(
        new.execute(&ChangedReply {}).await,
        Err(Error::InvalidRequest(_))
    ));
    let (result, ()) = tokio::join!(old.execute(&ChangedReply {}), async {
        let request = old_peer.recv_command_full().await.unwrap();
        assert!(!request.oob);
        old_peer
            .send_return_with_id(json!({}), request.id)
            .await
            .unwrap();
    });
    result.unwrap();
    old.shutdown().await;
    new.shutdown().await;
}

#[tokio::test]
async fn qga_selects_reply_or_delivery_only_for_each_peer_release() {
    let (mut old, mut old_peer) = support::qga_version(ClientOptions::default(), "10.2.0").await;
    let (mut new, mut new_peer) = support::qga_version(ClientOptions::default(), "11.1.3").await;
    assert!(matches!(
        old.send_no_response(&ChangedReply {}).await,
        Err(Error::InvalidRequest(_))
    ));
    let (result, ()) = tokio::join!(old.execute(&ChangedReply {}), async {
        old_peer.recv_command().await.unwrap();
        old_peer.send_return(json!({})).await.unwrap();
    });
    result.unwrap();
    assert!(matches!(
        new.execute(&ChangedReply {}).await,
        Err(Error::InvalidRequest(_))
    ));
    new.send_no_response(&ChangedReply {}).await.unwrap();
    new_peer.recv_command().await.unwrap();
    assert!(new.is_closed());
    assert!(new_peer.recv_command().await.is_err());
}
