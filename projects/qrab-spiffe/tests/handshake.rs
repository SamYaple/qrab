//! Exact-ID mutual TLS over real Workload API sources.
use qrab_spiffe::{
    fixture::{Authority, Workload},
    id, Identity,
};
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

const VM: &str = "spiffe://yapplesauce.com/kubernetes/qrab/vm/a";
const OTHER: &str = "spiffe://yapplesauce.com/kubernetes/qrab/vm/b";

async fn identity(ca: &Authority, spiffe_id: &str) -> (Workload, Identity) {
    let workload = Workload::serve(vec![ca.svid(spiffe_id)]);
    let identity = Identity::connect(&workload.endpoint).await.unwrap();
    (workload, identity)
}

#[tokio::test]
async fn an_exact_peer_completes_tls13_and_carries_data() {
    let ca = Authority::new();
    let (_a, server) = identity(&ca, VM).await;
    let (_b, client) = identity(&ca, VM).await;
    assert_eq!(server.id().unwrap(), id(VM).unwrap());
    let acceptor = server.acceptor(&[id(VM).unwrap()]).unwrap();
    let connector = client.connector(&[id(VM).unwrap()]).unwrap();
    let (left, right) = duplex(1 << 16);
    let accepted = tokio::spawn(async move {
        let mut stream = acceptor.accept(right).await.unwrap();
        let mut buffer = [0; 5];
        stream.read_exact(&mut buffer).await.unwrap();
        buffer
    });
    let mut stream = connector.connect(left).await.unwrap();
    stream.write_all(b"hello").await.unwrap();
    stream.flush().await.unwrap();
    assert_eq!(&accepted.await.unwrap(), b"hello");
}

#[tokio::test]
async fn the_server_refuses_another_id_in_the_trust_domain() {
    let ca = Authority::new();
    let (_a, server) = identity(&ca, VM).await;
    let (_b, client) = identity(&ca, OTHER).await;
    let acceptor = server.acceptor(&[id(VM).unwrap()]).unwrap();
    // The client would accept the server; only the server's check can fail.
    let connector = client.connector(&[id(VM).unwrap()]).unwrap();
    let (left, right) = duplex(1 << 16);
    let accepted = tokio::spawn(async move { acceptor.accept(right).await.map(|_| ()) });
    let connected = async {
        let mut stream = connector.connect(left).await?;
        // TLS 1.3 clients learn of a rejected certificate on first read.
        let mut buffer = [0; 1];
        stream.read_exact(&mut buffer).await?;
        Ok::<_, qrab_spiffe::Error>(())
    };
    assert!(connected.await.is_err());
    assert!(accepted.await.unwrap().is_err());
}

#[tokio::test]
async fn the_client_refuses_a_server_with_another_id() {
    let ca = Authority::new();
    let (_a, server) = identity(&ca, OTHER).await;
    let (_b, client) = identity(&ca, VM).await;
    let acceptor = server.acceptor(&[id(VM).unwrap()]).unwrap();
    let connector = client.connector(&[id(VM).unwrap()]).unwrap();
    let (left, right) = duplex(1 << 16);
    let accepted = tokio::spawn(async move { acceptor.accept(right).await.map(|_| ()) });
    assert!(connector.connect(left).await.is_err());
    assert!(accepted.await.unwrap().is_err());
}

#[tokio::test]
async fn a_different_trust_domain_is_refused_even_when_listed() {
    let ca = Authority::new();
    let foreign = "spiffe://elsewhere.example/kubernetes/qrab/vm/a";
    let (_a, server) = identity(&ca, VM).await;
    let (_b, client) = identity(&ca, foreign).await;
    let acceptor = server
        .acceptor(&[id(VM).unwrap(), id(foreign).unwrap()])
        .unwrap();
    let connector = client.connector(&[id(VM).unwrap()]).unwrap();
    let (left, right) = duplex(1 << 16);
    let accepted = tokio::spawn(async move { acceptor.accept(right).await.map(|_| ()) });
    let _ = connector.connect(left).await;
    assert!(accepted.await.unwrap().is_err());
}

#[tokio::test]
async fn only_unix_endpoints_are_accepted() {
    assert!(matches!(
        Identity::connect("tcp://127.0.0.1:1").await,
        Err(qrab_spiffe::Error::Endpoint)
    ));
}
