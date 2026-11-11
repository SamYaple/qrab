//! Version gating belongs to each protocol connection, including QGA peers
//! whose version differs from the host's QEMU version.
use qapi_protocol::{
    qga::Qga,
    server::{QgaSession, QmpSession},
    Qmp,
};
use tokio::net::UnixListener;

#[qapi::qapi(command, name = "test-version", added = "1.0.0", returns = "bool")]
struct TestVersion {
    #[qapi(added = "11.0.0")]
    new_field: Option<bool>,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multiple_qmp_versions_serialize_independently() {
    let dir = tempfile::tempdir().unwrap();
    let mut clients = Vec::new();
    let mut servers = Vec::new();
    for major in [10, 11] {
        let path = dir.path().join(format!("qmp-{major}"));
        let listener = UnixListener::bind(&path).unwrap();
        servers.push(tokio::spawn(async move {
            let mut session = QmpSession::accept(&listener).await.unwrap();
            session.send_greeting(major, 1, 0).await.unwrap();
            session.negotiate().await.unwrap();
            for _ in 0..3 {
                let (name, args) = session.recv_command().await.unwrap();
                assert_eq!(name, "test-version");
                assert_eq!(args.get("new_field").is_some(), major >= 11);
                session.send_return(serde_json::json!(true)).await.unwrap();
            }
        }));
        let (client, _) = Qmp::new(&path).await.unwrap();
        assert_eq!(client.version(), (major, 1, 0));
        clients.push(client);
    }
    let tasks: Vec<_> = clients
        .into_iter()
        .map(|client| {
            tokio::spawn(async move {
                for _ in 0..3 {
                    assert!(client
                        .execute(&TestVersion {
                            new_field: Some(true)
                        })
                        .await
                        .unwrap());
                    tokio::task::yield_now().await;
                }
                client.shutdown().await;
            })
        })
        .collect();
    for task in tasks {
        task.await.unwrap();
    }
    for server in servers {
        server.await.unwrap();
    }
}

#[tokio::test]
async fn qga_uses_guest_version_instead_of_host_default() {
    // This deliberately disagrees with both agents below.
    let _ = qapi::set_qemu_version(1, 0, 0);
    let dir = tempfile::tempdir().unwrap();
    let mut clients = Vec::new();
    let mut servers = Vec::new();
    for major in [10, 11] {
        let path = dir.path().join(format!("qga-{major}"));
        let listener = UnixListener::bind(&path).unwrap();
        servers.push(tokio::spawn(async move {
            let mut session = QgaSession::accept(&listener).await.unwrap();
            session.handle_sync().await.unwrap();
            assert_eq!(session.recv_command().await.unwrap().0, "guest-info");
            session.send_return(serde_json::json!({ "version": format!("{major}.1.0"), "supported_commands": [] })).await.unwrap();
            let (name, args) = session.recv_command().await.unwrap();
            assert_eq!(name, "test-version");
            assert_eq!(args.get("new_field").is_some(), major >= 11);
            session.send_return(serde_json::json!(true)).await.unwrap();
        }));
        let client = Qga::new(&path).await.unwrap();
        assert_eq!(client.version(), (major, 1, 0));
        clients.push(client);
    }
    for mut client in clients {
        assert!(client
            .execute(&TestVersion {
                new_field: Some(true)
            })
            .await
            .unwrap());
    }
    for server in servers {
        server.await.unwrap();
    }
}

#[tokio::test]
async fn typed_guest_file_read_decodes_response_not_request() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("qga");
    let listener = UnixListener::bind(&path).unwrap();
    let server = tokio::spawn(async move {
        let mut session = QgaSession::accept(&listener).await.unwrap();
        session.handle_sync().await.unwrap();
        assert_eq!(session.recv_command().await.unwrap().0, "guest-info");
        session
            .send_return(serde_json::json!({ "version": "11.1.3", "supported_commands": [] }))
            .await
            .unwrap();
        let (name, args) = session.recv_command().await.unwrap();
        assert_eq!(name, "guest-file-read");
        assert_eq!(args, serde_json::json!({"handle": 7, "count": 3}));
        session
            .send_return(serde_json::json!({"count": 3, "buf-b64": "YWJj", "eof": true}))
            .await
            .unwrap();
    });
    let mut client = Qga::new(&path).await.unwrap();
    let reply: qapi::qga::GuestFileRead = client
        .execute(&qapi::qga::commands::GuestFileReadCommand {
            handle: 7,
            count: Some(3),
        })
        .await
        .unwrap();
    assert_eq!(reply.count, 3);
    assert_eq!(reply.buf_b64, "YWJj");
    assert!(reply.eof);
    server.await.unwrap();
}
