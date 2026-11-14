//! Live migration of a UEFI + swtpm guest between two real managers.
//!
//! Run inside the manager image (QEMU 11.1, swtpm, OVMF and the demo guest):
//! see docs/migration.md. The SPIFFE identity comes from the qrab-spiffe
//! fixture Workload API, so no SPIRE is needed.
use manager::pod::{self, Config};
use qrab_spiffe::fixture::{Authority, Workload};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::mpsc,
};

const VM: &str = "spiffe://yapplesauce.com/kubernetes/qrab/vm/migration-test";

fn spawn(config: &Config, file: &Path) -> (Child, mpsc::UnboundedReceiver<String>) {
    std::fs::write(file, serde_json::to_vec(config).unwrap()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_qrab-manager"))
        .args(["run", "--config"])
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let (sender, receiver) = mpsc::unbounded_channel();
    let label = config.name.clone();
    for (stream, kind) in [
        (
            Box::new(child.stdout.take().unwrap()) as Box<dyn tokio::io::AsyncRead + Unpin + Send>,
            "serial",
        ),
        (Box::new(child.stderr.take().unwrap()), "manager"),
    ] {
        let sender = sender.clone();
        let label = label.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("[{label} {kind}] {line}");
                let _ = sender.send(line);
            }
        });
    }
    (child, receiver)
}

async fn until(lines: &mut mpsc::UnboundedReceiver<String>, marker: &str, limit: u64) -> String {
    tokio::time::timeout(Duration::from_secs(limit), async {
        loop {
            let line = lines.recv().await.expect("manager output ended");
            if line.contains(marker) {
                return line;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("no {marker} within {limit}s"))
}

fn ek(line: &str) -> String {
    let index = line.find("QRAB_TPM_EK_SHA256=").unwrap() + "QRAB_TPM_EK_SHA256=".len();
    line[index..]
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .collect()
}

fn config(directory: &Path, name: &str, workload: &Workload, port: u16) -> Config {
    let qemu = std::env::var("QRAB_TEST_QEMU")
        .unwrap_or_else(|_| "/opt/qemu/bin/qemu-system-x86_64".into());
    serde_json::from_value(serde_json::json!({
        "name":name, "state_id":"migration-test", "qemu_binary":qemu, "memory_mib":512, "accelerator":"tcg",
        "console":false, "control_socket":true, "uefi":{}, "tpm":{},
        "runtime_dir":directory.join(name).join("run"), "state_dir":directory.join(name).join("state"),
        "startup_timeout_secs":30, "shutdown_grace_secs":5,
        "migration":{"port":port,"workload_api":workload.endpoint},
    }))
    .unwrap()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn ping(config: &Config, file: &Path) -> bool {
    std::fs::write(file, serde_json::to_vec(config).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_qrab-manager"))
        .args(["agent", "ping", "--config"])
        .arg(file)
        .status()
        .await
        .is_ok_and(|status| status.success())
}

#[tokio::test]
#[ignore = "requires the manager image: QEMU 11.1, swtpm, OVMF and the demo guest"]
async fn a_uefi_tpm_guest_moves_between_managers_and_keeps_its_identity() {
    let directory = tempfile::tempdir().unwrap();
    let base: PathBuf = directory.path().into();
    let ca = Authority::new();
    let source_identity = Workload::serve(vec![ca.svid(VM)]);
    let target_identity = Workload::serve(vec![ca.svid(VM)]);
    let port = free_port();

    let source = config(&base, "source", &source_identity, port);
    let (mut source_process, mut source_lines) = spawn(&source, &base.join("source.json"));
    let booted = until(&mut source_lines, "QRAB_TPM_EK_SHA256=", 120).await;
    let identity = ek(&booted);
    assert_eq!(identity.len(), 64);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_err()
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "migration listener never opened"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    // The guest agent answers before migration.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while !ping(&source, &base.join("ping-source.json")).await {
        assert!(
            tokio::time::Instant::now() < deadline,
            "source guest agent never answered"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    let mut target = config(&base, "target", &target_identity, free_port());
    target.incoming = serde_json::from_value(serde_json::json!({
        "migration":"m-test","source":format!("127.0.0.1:{port}"),"workload_api":target_identity.endpoint})).unwrap();
    let (mut target_process, mut target_lines) = spawn(&target, &base.join("target.json"));
    until(&mut target_lines, "migration_received", 180).await;
    let exited = tokio::time::timeout(Duration::from_secs(30), source_process.wait())
        .await
        .expect("source manager did not exit after migration")
        .unwrap();
    assert!(exited.success(), "source manager failed: {exited}");
    until(&mut source_lines, "\"reason\":\"migrated\"", 5).await;

    // The same guest, not a reboot: it answers without booting again.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while pod::probe(&target, true).is_err() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "target never became ready"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while !ping(&target, &base.join("ping-target.json")).await {
        assert!(
            tokio::time::Instant::now() < deadline,
            "migrated guest agent did not answer"
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let mut rebooted = false;
    while let Ok(line) = target_lines.try_recv() {
        rebooted |= line.contains("QRAB_UEFI_BOOTED");
    }
    assert!(
        !rebooted,
        "the target booted the guest instead of resuming it"
    );

    // The TPM state reached the target's own state directory: a cold boot
    // from it reports the source's endorsement key.
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(target_process.id().unwrap() as i32),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(20), target_process.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    let mut cold = target.clone();
    cold.incoming = None;
    cold.runtime_dir = base.join("target").join("cold");
    let (_cold_process, mut cold_lines) = spawn(&cold, &base.join("cold.json"));
    let rebooted = until(&mut cold_lines, "QRAB_TPM_EK_SHA256=", 120).await;
    assert_eq!(
        ek(&rebooted),
        identity,
        "the migrated TPM identity was not persisted"
    );
}
