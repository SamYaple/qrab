#![cfg(feature = "agent")]
use qrab_netd::{
    ct::Export,
    peer_tls::{node_for_host, node_id, Transport},
};
use qrab_spiffe::{
    fixture::{Authority, Workload},
    Identity,
};
use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

const DOMAIN: &str = "yapplesauce.com";
const GUEST: Ipv4Addr = Ipv4Addr::new(10, 244, 128, 7);

async fn identity(ca: &Authority, node: &str) -> (Workload, Identity) {
    let workload = Workload::serve(vec![ca.svid(&node_id(DOMAIN, node).unwrap().to_string())]);
    let identity = Identity::connect(&workload.endpoint).await.unwrap();
    (workload, identity)
}
fn export() -> Export {
    Export {
        now: 42,
        entries: (0..300)
            .map(|i| ('t', format!("{i:028x}"), "00".repeat(56)))
            .collect(),
        truncated: false,
    }
}
async fn serve(
    transport: Transport,
    peer: &'static str,
    expected: Export,
) -> (SocketAddr, tokio::task::JoinHandle<io::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await?;
        transport
            .serve(tcp, peer, move |address| async move {
                assert_eq!(address, GUEST);
                Ok(expected)
            })
            .await
    });
    (address, task)
}
#[tokio::test]
async fn exact_node_svids_transfer_a_bounded_export_and_refusals() {
    let ca = Authority::new();
    let (_sw, server) = identity(&ca, "kube01").await;
    let (_cw, client) = identity(&ca, "kube02").await;
    let source = Transport::new(server.clone(), "kube01").unwrap();
    let target = Transport::new(client, "kube02").unwrap();
    let (address, task) = serve(source.clone(), "kube02", export()).await;
    assert_eq!(
        target
            .fetch(TcpStream::connect(address).await.unwrap(), "kube01", GUEST)
            .await
            .unwrap(),
        export()
    );
    task.await.unwrap().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        source
            .serve(tcp, "kube02", |_| async {
                Err("same-VM/NIC authorization refused".into())
            })
            .await
    });
    let error = target
        .fetch(TcpStream::connect(address).await.unwrap(), "kube01", GUEST)
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("authorization refused"),
        "{error}"
    );
    task.await.unwrap().unwrap();
    assert!(
        Transport::new(server, "kube02").is_err(),
        "wrong local node binding"
    );
}
#[tokio::test]
async fn wrong_client_or_server_node_and_untrusted_ca_never_transfer_entries() {
    for mode in 0..3 {
        let ca = Authority::new();
        let alien = Authority::new();
        let (_sw, server) = identity(&ca, "kube01").await;
        let (_cw, client) = identity(if mode == 2 { &alien } else { &ca }, "kube02").await;
        let source = Transport::new(server, "kube01").unwrap();
        let target = Transport::new(client, "kube02").unwrap();
        let called = Arc::new(AtomicBool::new(false));
        let seen = called.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            source
                .serve(
                    tcp,
                    if mode == 0 { "kube03" } else { "kube02" },
                    |_| async move {
                        seen.store(true, Ordering::SeqCst);
                        Ok(export())
                    },
                )
                .await
        });
        let result = target
            .fetch(
                TcpStream::connect(address).await.unwrap(),
                if mode == 1 { "kube03" } else { "kube01" },
                GUEST,
            )
            .await;
        assert!(result.is_err(), "mode {mode} admitted");
        assert!(task.await.unwrap().is_err());
        assert!(!called.load(Ordering::SeqCst));
    }
}
#[tokio::test]
async fn plaintext_and_tls_without_client_certificate_are_refused_before_export() {
    let ca = Authority::new();
    let (_sw, server) = identity(&ca, "kube01").await;
    for no_certificate in [false, true] {
        let source = Transport::new(server.clone(), "kube01").unwrap();
        let called = Arc::new(AtomicBool::new(false));
        let seen = called.clone();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            source
                .serve(tcp, "kube02", |_| async move {
                    seen.store(true, Ordering::SeqCst);
                    Ok(export())
                })
                .await
        });
        let tcp = TcpStream::connect(address).await.unwrap();
        if no_certificate {
            #[derive(Debug)]
            struct NoCert;
            impl rustls::client::ResolvesClientCert for NoCert {
                fn resolve(
                    &self,
                    _: &[&[u8]],
                    _: &[rustls::SignatureScheme],
                ) -> Option<Arc<rustls::sign::CertifiedKey>> {
                    None
                }
                fn has_certs(&self) -> bool {
                    false
                }
            }
            let (_cw, client) = identity(&ca, "kube02").await;
            let mut config = client
                .client_config(&[node_id(DOMAIN, "kube01").unwrap()], &[])
                .unwrap();
            config.client_auth_cert_resolver = Arc::new(NoCert);
            let result = tokio_rustls::TlsConnector::from(Arc::new(config))
                .connect("spiffe.invalid".try_into().unwrap(), tcp)
                .await;
            if let Ok(mut tls) = result {
                let mut byte = [0; 1];
                assert!(tls.read_exact(&mut byte).await.is_err());
            }
        } else {
            let mut tcp = tcp;
            tcp.write_all(b"{\"address\":\"10.244.128.7\"}\n")
                .await
                .unwrap();
            tcp.shutdown().await.unwrap();
            let mut response = Vec::new();
            let _ = tcp.read_to_end(&mut response).await;
            assert!(!response.windows(9).any(|w| w == b"conntrack"));
        }
        assert!(task.await.unwrap().is_err());
        assert!(!called.load(Ordering::SeqCst));
    }
}
#[tokio::test]
async fn stalled_handshake_expires_and_does_not_prevent_a_good_connection() {
    let ca = Authority::new();
    let (_sw, server) = identity(&ca, "kube01").await;
    let (_cw, client) = identity(&ca, "kube02").await;
    let source = Transport::new(server, "kube01").unwrap();
    let target = Transport::new(client, "kube02").unwrap();
    let (address, stalled) = serve(source.clone(), "kube02", export()).await;
    let _idle = TcpStream::connect(address).await.unwrap();
    let (address, good) = serve(source, "kube02", export()).await;
    assert_eq!(
        target
            .fetch(TcpStream::connect(address).await.unwrap(), "kube01", GUEST)
            .await
            .unwrap(),
        export()
    );
    good.await.unwrap().unwrap();
    assert_eq!(
        stalled.await.unwrap().unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
}
#[test]
fn host_mapping_requires_live_unambiguous_anchor_metadata_and_valid_node_names() {
    use k8s_openapi::api::core::v1::Pod;
    let pod = |node: &str| -> Pod {
        serde_json::from_value(serde_json::json!({
        "metadata":{"labels":{"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm-nic","qrab.yapplesauce.com/owner-uid":"vm","qrab.yapplesauce.com/nic":"lan"},"annotations":{"qrab.yapplesauce.com/nic-datapath":"tap"}},
        "spec":{"nodeName":node,"containers":[]},"status":{"hostIP":"10.203.0.11"}
    })).unwrap()
    };
    let a = pod("kube01");
    let b = pod("kube02");
    let host = "10.203.0.11".parse().unwrap();
    assert_eq!(node_for_host([&a], host).unwrap(), "kube01");
    assert!(node_for_host([&a, &b], host).is_err());
    assert!(node_for_host([&a], "10.203.0.12".parse().unwrap()).is_err());
    let mut gone = a.clone();
    gone.metadata.deletion_timestamp = Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(
        k8s_openapi::jiff::Timestamp::now(),
    ));
    assert!(node_for_host([&gone], host).is_err());
    let mut stranger = a.clone();
    stranger.metadata.annotations = None;
    assert!(node_for_host([&stranger], host).is_err());
    for bad in [
        "",
        "Kube01",
        "../kube01",
        "kube/01",
        "kube%2f01",
        "-kube",
        "kube-",
        "a..b",
    ] {
        assert!(node_id(DOMAIN, bad).is_err(), "{bad}");
    }
}

#[tokio::test]
async fn authenticated_but_malformed_or_oversized_exports_are_rejected() {
    let ca = Authority::new();
    let (_sw, server) = identity(&ca, "kube01").await;
    let (_cw, client) = identity(&ca, "kube02").await;
    let target = Transport::new(client, "kube02").unwrap();
    for bytes in [
        b"{\"conntrack\":{\"now\":1,\"entries\":[[\"t\",\"bad\",\"00\"]],\"truncated\":false}}\n"
            .to_vec(),
        b"{\"conntrack\":{\"now\":1,\"entries\":[]},\"error\":\"ambiguous\"}\n".to_vec(),
        vec![b'x'; qrab_netd::wire::PAYLOAD_LIMIT + 1],
    ] {
        let acceptor = server
            .acceptor(&[node_id(DOMAIN, "kube02").unwrap()])
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut tls = acceptor.accept(tcp).await.unwrap();
            let mut request = Vec::new();
            while request.last() != Some(&b'\n') {
                request.push(tls.read_u8().await.unwrap());
            }
            let _ = tls.write_all(&bytes).await;
            let _ = tls.shutdown().await;
        });
        assert!(target
            .fetch(TcpStream::connect(address).await.unwrap(), "kube01", GUEST)
            .await
            .is_err());
        task.await.unwrap();
    }
}

#[tokio::test]
async fn credentials_rotate_without_rebuilding_the_transport() {
    let ca = Authority::new();
    let (sw, server) = identity(&ca, "kube01").await;
    let (cw, client) = identity(&ca, "kube02").await;
    let old = server.source().svid().unwrap().cert_chain()[0]
        .as_ref()
        .to_vec();
    let old_client = client.source().svid().unwrap().cert_chain()[0]
        .as_ref()
        .to_vec();
    let source = Transport::new(server.clone(), "kube01").unwrap();
    let target = Transport::new(client.clone(), "kube02").unwrap();
    let fresh = ca.svid(&node_id(DOMAIN, "kube01").unwrap().to_string());
    sw.replace(vec![fresh]);
    cw.replace(vec![
        ca.svid(&node_id(DOMAIN, "kube02").unwrap().to_string())
    ]);
    // Wait for both real Workload API streams to install their new leaves.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let current = server.source().svid().unwrap();
            let current_client = client.source().svid().unwrap();
            if current.cert_chain()[0].as_ref() != old.as_slice()
                && current_client.cert_chain()[0].as_ref() != old_client.as_slice()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let (address, task) = serve(source, "kube02", export()).await;
    assert_eq!(
        target
            .fetch(TcpStream::connect(address).await.unwrap(), "kube01", GUEST)
            .await
            .unwrap(),
        export()
    );
    task.await.unwrap().unwrap();
}
