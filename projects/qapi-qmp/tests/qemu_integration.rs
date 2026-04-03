use qapi::*;
use qapi_qmp::QmpConnection;
use std::process::{Child, Command};

struct QemuProcess {
    child: Child,
    socket_path: String,
}

impl QemuProcess {
    fn start() -> Option<Self> {
        let tmpdir = std::env::temp_dir();
        let socket_path = tmpdir
            .join(format!("qrab-test-{}.sock", std::process::id()))
            .to_string_lossy()
            .to_string();

        // Clean up any stale socket
        let _ = std::fs::remove_file(&socket_path);

        let child = Command::new("qemu-system-x86_64")
            .args([
                "-machine",
                "none",
                "-nographic",
                "-nodefaults",
                "-qmp",
                &format!("unix:{},server=on,wait=off", socket_path),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;

        // Wait for socket to appear
        for _ in 0..50 {
            if std::path::Path::new(&socket_path).exists() {
                return Some(QemuProcess {
                    child,
                    socket_path,
                });
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        None
    }
}

impl Drop for QemuProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[test]
#[ignore]
fn test_qmp_negotiate() {
    let qemu = QemuProcess::start().expect("Failed to start QEMU — is qemu-system-x86_64 in PATH?");
    let mut conn = QmpConnection::connect(&qemu.socket_path).expect("Failed to connect");
    let (major, minor, micro) = conn.negotiate().expect("Failed to negotiate");
    println!("QEMU version: {}.{}.{}", major, minor, micro);
    assert!(major > 0 || minor > 0, "Got zero version");

    // Set the version for runtime gating
    set_qemu_version(major, minor, micro);
}

#[test]
#[ignore]
fn test_qmp_query_status() {
    let qemu = QemuProcess::start().expect("Failed to start QEMU");
    let mut conn = QmpConnection::connect(&qemu.socket_path).unwrap();
    let (major, minor, micro) = conn.negotiate().unwrap();
    set_qemu_version(major, minor, micro);

    let cmd = QueryStatus {};
    let status: StatusInfo = conn.execute(&cmd).unwrap();
    println!("Status: running={}, status={:?}", status.running, serde_json::to_string(&status.status).unwrap());
    // With -machine none, QEMU starts in prelaunch or running state
    assert!(!status.running || matches!(status.status, RunState::Running));
}

#[test]
#[ignore]
fn test_qmp_query_version() {
    let qemu = QemuProcess::start().expect("Failed to start QEMU");
    let mut conn = QmpConnection::connect(&qemu.socket_path).unwrap();
    let (major, minor, micro) = conn.negotiate().unwrap();
    set_qemu_version(major, minor, micro);

    let cmd = QueryVersion {};
    let version: VersionInfo = conn.execute(&cmd).unwrap();
    assert_eq!(version.qemu.major as u32, major);
    assert_eq!(version.qemu.minor as u32, minor);
    assert_eq!(version.qemu.micro as u32, micro);
    println!("Version: {}.{}.{}", version.qemu.major, version.qemu.minor, version.qemu.micro);
}

#[test]
#[ignore]
fn test_qmp_query_name() {
    let qemu = QemuProcess::start().expect("Failed to start QEMU");
    let mut conn = QmpConnection::connect(&qemu.socket_path).unwrap();
    let (major, minor, micro) = conn.negotiate().unwrap();
    set_qemu_version(major, minor, micro);

    let cmd = QueryName {};
    let name: NameInfo = conn.execute(&cmd).unwrap();
    // Name is optional and typically unset
    println!("Name: {:?}", name.name);
}
