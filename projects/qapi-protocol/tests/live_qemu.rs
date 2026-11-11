//! Real QMP smoke test, independent of manager, KVM, guest images or systemd.
use qapi::{commands::*, QapiCommand};
use qapi_protocol::Qmp;
use std::{collections::BTreeSet, process::Stdio, time::Duration};

#[tokio::test]
#[ignore = "requires a QEMU binary; set QRAB_TEST_QEMU and run with --ignored"]
async fn real_qmp_handshake_schema_and_typed_commands() {
    for pretty in [false, true] {
        smoke(pretty).await;
    }
}

async fn smoke(pretty: bool) {
    let binary = std::env::var_os("QRAB_TEST_QEMU").unwrap_or_else(|| "qemu-system-x86_64".into());
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("qmp.sock");
    let mut child = tokio::process::Command::new(binary)
        .args([
            "-machine",
            "none",
            "-accel",
            "tcg",
            "-nodefaults",
            "-display",
            "none",
            "-S",
            "-chardev",
        ])
        .arg(format!(
            "socket,id=qmp,path={},server=on,wait=off",
            socket.display()
        ))
        .arg("-object")
        .arg(format!(
            "monitor-qmp,id=monitor,chardev=qmp,pretty={}",
            if pretty { "on" } else { "off" }
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("failed to launch test QEMU");
    tokio::time::timeout(Duration::from_secs(10), async {
        while !socket.exists() {
            assert!(
                child.try_wait().unwrap().is_none(),
                "QEMU exited before opening QMP"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("QEMU did not open QMP socket");
    let (qmp, _) = Qmp::new(&socket).await.unwrap();
    let version = qmp.execute(&QueryVersion {}).await.unwrap();
    assert_eq!(
        qmp.version(),
        (
            version.qemu.major as u32,
            version.qemu.minor as u32,
            version.qemu.micro as u32
        )
    );
    eprintln!(
        "Validated QMP peer version: {:?}, pretty={pretty}",
        qmp.version()
    );
    let status = qmp.execute(&QueryStatus {}).await.unwrap();
    assert!(!status.running);
    let commands = qmp.execute(&QueryCommands {}).await.unwrap();
    let known: BTreeSet<_> = qapi::metadata::COMMANDS
        .iter()
        .map(|c| c.wire_name)
        .collect();
    let missing: Vec<_> = commands
        .iter()
        .filter(|c| !known.contains(c.name.as_str()))
        .map(|c| &c.name)
        .collect();
    assert!(
        missing.is_empty(),
        "Live QEMU commands missing from generated metadata: {missing:?}"
    );
    let schema = qmp
        .execute(&QueryQmpSchema { arguments: None })
        .await
        .unwrap();
    assert!(schema.iter().any(|s| s.name == QueryStatus::NAME));
    // Real pipelining and OOB execution against the current QEMU monitor.
    let replies = tokio::join!(
        qmp.execute(&QueryVersion {}),
        qmp.execute(&QueryStatus {}),
        qmp.execute(&QueryName {}),
        qmp.execute_oob(&QueryYank {})
    );
    replies.0.unwrap();
    replies.1.unwrap();
    replies.2.unwrap();
    replies.3.unwrap();
    assert!(matches!(
        qmp.execute_raw(serde_json::json!({"execute": "qrab-no-such-command"}))
            .await,
        Err(qapi_protocol::Error::Qmp(_))
    ));
    qmp.execute(&QueryStatus {}).await.unwrap();
    qmp.execute(&Quit {}).await.unwrap();
    qmp.shutdown().await;
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(status.success());
}
