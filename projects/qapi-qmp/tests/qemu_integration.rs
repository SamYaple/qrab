use qapi::*;
use qapi_qmp::Qmp;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::{Child, Command};

struct QemuProcess {
    child: Child,
    socket_path: PathBuf,
}

impl QemuProcess {
    async fn start() -> Option<Self> {
        let tmpdir = std::env::temp_dir();
        let socket_path = tmpdir.join(format!("qrab-test-{}.sock", std::process::id()));

        let _ = tokio::fs::remove_file(&socket_path).await;

        let child = Command::new("qemu-system-x86_64")
            .args([
                "-machine",
                "none",
                "-nographic",
                "-nodefaults",
                "-qmp",
                &format!("unix:{},server=on,wait=off", socket_path.display()),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        // Wait for socket to appear
        for _ in 0..50 {
            if socket_path.exists() {
                return Some(QemuProcess {
                    child,
                    socket_path,
                });
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        None
    }
}

impl Drop for QemuProcess {
    fn drop(&mut self) {
        // Best-effort cleanup. The child may already be dead.
        let _ = self.child.start_kill();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[tokio::test]
#[ignore]
async fn test_qmp_negotiate_and_query_status() {
    let qemu = QemuProcess::start()
        .await
        .expect("Failed to start QEMU — is qemu-system-x86_64 in PATH?");

    let (mut qmp, _events) = Qmp::new(&qemu.socket_path)
        .await
        .expect("Failed to connect to QMP");

    // qemu_version should have been set during negotiation
    let version = qapi::qemu_version();
    println!("QEMU version: {}.{}.{}", version.0, version.1, version.2);
    assert!(version.0 > 0 || version.1 > 0, "Got zero version");

    // Query status
    let status: StatusInfo = qmp.execute(&QueryStatus {}).await.unwrap();
    println!(
        "Status: running={}, status={}",
        status.running,
        serde_json::to_string(&status.status).unwrap()
    );

    qmp.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_qmp_query_version() {
    let qemu = QemuProcess::start()
        .await
        .expect("Failed to start QEMU");

    let (mut qmp, _events) = Qmp::new(&qemu.socket_path).await.unwrap();

    let version: VersionInfo = qmp.execute(&QueryVersion {}).await.unwrap();
    println!(
        "Version: {}.{}.{}",
        version.qemu.major, version.qemu.minor, version.qemu.micro
    );

    let global = qapi::qemu_version();
    assert_eq!(version.qemu.major as u32, global.0);
    assert_eq!(version.qemu.minor as u32, global.1);

    qmp.shutdown().await;
}

#[tokio::test]
#[ignore]
async fn test_qmp_query_name() {
    let qemu = QemuProcess::start()
        .await
        .expect("Failed to start QEMU");

    let (mut qmp, _events) = Qmp::new(&qemu.socket_path).await.unwrap();

    let name: NameInfo = qmp.execute(&QueryName {}).await.unwrap();
    println!("Name: {:?}", name.name);

    qmp.shutdown().await;
}
