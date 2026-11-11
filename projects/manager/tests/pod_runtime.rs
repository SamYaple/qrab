use manager::pod::{self, Config, Disk, DiskFormat, Phase};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, BufReader};

fn fixture_binary(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn config(runtime: &Path, binary: PathBuf, kernel: PathBuf) -> Config {
    serde_json::from_value(serde_json::json!({"name":"test-vm","qemu_binary":binary,"kernel":kernel,"runtime_dir":runtime,"startup_timeout_secs":5,"shutdown_grace_secs":0})).unwrap()
}

#[test]
fn arguments_separate_paths_from_qemu_option_syntax() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = dir.path().join("kernel");
    std::fs::write(&kernel, []).unwrap();
    let mut config = config(dir.path(), PathBuf::from("/bin/true"), kernel);
    let disk = dir.path().join("disk, file.raw");
    std::fs::write(&disk, []).unwrap();
    config.disk = Some(Disk {
        path: disk.clone(),
        format: DiskFormat::Raw,
        read_only: true,
    });
    config.validate().unwrap();
    let args = config.arguments(Path::new("/tmp/a,b/qmp.sock"));
    assert!(args.iter().any(|a| a.contains("path=/tmp/a,,b/qmp.sock")));
    let blockdev = args.windows(2).find(|a| a[0] == "-blockdev").unwrap();
    let value: serde_json::Value = serde_json::from_str(&blockdev[1]).unwrap();
    assert_eq!(value["filename"], disk.to_str().unwrap());
    assert_eq!(value["read-only"], true);
    assert!(args.windows(2).any(|a| a == ["-nic", "none"]));
    assert!(!args.iter().any(|a| a.contains("qrab-init")));
    assert!(!args.iter().any(|a| a == "-vnc"));
    // The guest agent port is on a private socket beside QMP, never TCP.
    assert!(args.windows(2).any(|a| a
        == [
            "-chardev",
            "socket,id=qga0,path=/tmp/a,,b/qga.sock,server=on,wait=off"
        ]));
    assert!(args.windows(2).any(|a| a
        == [
            "-device",
            "virtserialport,bus=qga-serial.0,chardev=qga0,name=org.qemu.guest_agent.0"
        ]));
    config.guest_agent = false;
    let args = config.arguments(Path::new("/tmp/private/qmp.sock"));
    assert!(!args
        .iter()
        .any(|a| a.contains("qga") || a.contains("virtio-serial")));
    config.guest_agent = true;
    config.console = true;
    let args = config.arguments(Path::new("/tmp/private/qmp.sock"));
    assert!(args
        .windows(2)
        .any(|a| a == ["-vnc", "unix:/tmp/private/vnc.sock,share=force-shared"]));
    assert!(args
        .windows(2)
        .any(|a| a == ["-device", "usb-tablet,bus=usb.0"]));
    assert!(args.windows(2).any(|a| a == ["-nic", "none"]));
}

#[test]
fn invalid_configuration_and_stale_readiness_are_refused() {
    assert!(serde_json::from_value::<Config>(
        serde_json::json!({"kernel":"/kernel","surprise":true})
    )
    .is_err());
    let dir = tempfile::tempdir().unwrap();
    let kernel = dir.path().join("kernel");
    std::fs::write(&kernel, []).unwrap();
    let mut config = config(dir.path(), PathBuf::from("/bin/true"), kernel);
    config.name = "injected,name=x".into();
    assert!(config.validate().is_err());
    config.name = "ok".into();
    config.cpus = 0;
    assert!(config.validate().is_err());
    assert!(pod::probe(&config, true).is_err());
}

#[tokio::test]
async fn early_child_failure_is_reported_and_status_is_not_ready() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = dir.path().join("kernel");
    std::fs::write(&kernel, []).unwrap();
    let config = config(dir.path(), PathBuf::from("/bin/false"), kernel);
    let result = pod::run(config.clone(), std::future::pending()).await;
    assert!(result.is_err());
    let status = pod::status(&config).unwrap();
    assert_eq!(status.phase, Phase::Failed);
    assert!(!status.ready);
    assert!(pod::probe(&config, false).is_err());
}

#[tokio::test]
async fn startup_timeout_and_signal_reap_the_owned_process() {
    for shutdown in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let kernel = dir.path().join("kernel");
        std::fs::write(&kernel, []).unwrap();
        let binary = fixture_binary("sleep.sh");
        let mut config = config(dir.path(), binary, kernel);
        config.startup_timeout_secs = 1;
        let result = pod::run(config.clone(), async {
            if shutdown {
                tokio::time::sleep(Duration::from_millis(100)).await;
            } else {
                std::future::pending::<()>().await;
            }
        })
        .await;
        assert_eq!(result.is_ok(), shutdown);
        let status = pod::status(&config).unwrap();
        assert!(!status.ready);
        assert!(
            !PathBuf::from(format!("/proc/{}", status.qemu_pid.unwrap())).exists(),
            "owned child was not reaped"
        );
    }
}

#[tokio::test]
#[ignore = "requires current QEMU and demo guest assets; set QRAB_TEST_QEMU, QRAB_TEST_QEMU_DATA, QRAB_TEST_GUEST"]
async fn real_cli_boot_probe_and_sigterm_shutdown() {
    let binary = PathBuf::from(std::env::var_os("QRAB_TEST_QEMU").expect("QEMU path"));
    let data = PathBuf::from(std::env::var_os("QRAB_TEST_QEMU_DATA").expect("QEMU data path"));
    let guest = PathBuf::from(std::env::var_os("QRAB_TEST_GUEST").expect("guest path"));
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path(), binary, guest.join("vmlinuz"));
    let disk = dir.path().join("disk, readonly.raw");
    std::fs::write(&disk, vec![0u8; 1024 * 1024]).unwrap();
    config.disk = Some(Disk {
        path: disk,
        format: DiskFormat::Raw,
        read_only: true,
    });
    config.firmware_dir = Some(data);
    config.initrd = Some(guest.join("initramfs.gz"));
    config.kernel_args = "console=ttyS0 rdinit=/init panic=0 quiet".into();
    let path = dir.path().join("config.json");
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_qrab-manager"))
        .args(["run", "--config"])
        .arg(&path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    tokio::time::timeout(Duration::from_secs(60), async {
        while let Some(line) = lines.next_line().await.unwrap() {
            if line.contains("QRAB_GUEST_BOOTED") {
                return;
            }
        }
        panic!("guest marker missing");
    })
    .await
    .unwrap();
    pod::probe(&config, true).unwrap();
    let before = pod::status(&config).unwrap();
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id().unwrap() as i32),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap();
    let exit = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .unwrap()
        .unwrap();
    assert!(exit.success());
    assert!(pod::probe(&config, true).is_err());
    assert_eq!(pod::status(&config).unwrap().phase, Phase::Exited);
    assert!(!PathBuf::from(format!("/proc/{}", before.qemu_pid.unwrap())).exists());
}

#[tokio::test]
async fn cancellation_reaps_child_and_concurrent_owners_are_refused() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let kernel = dir.path().join("kernel");
    std::fs::write(&kernel, []).unwrap();
    let binary = fixture_binary("sleep.sh");
    let config = config(dir.path(), binary, kernel);
    let first = config.clone();
    let task = tokio::spawn(async move { pod::run(first, std::future::pending()).await });
    let pid = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Ok(s) = pod::status(&config) {
                if let Some(pid) = s.qemu_pid {
                    break pid;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let session = std::fs::read_dir(&config.runtime_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("session-")
        })
        .unwrap();
    assert_eq!(
        std::fs::metadata(session).unwrap().permissions().mode() & 0o777,
        0o700,
        "another UID must not be able to traverse the QMP/VNC directory"
    );
    let duplicate = pod::run(config.clone(), std::future::pending()).await;
    assert!(duplicate
        .unwrap_err()
        .to_string()
        .contains("another manager"));
    task.abort();
    let _ = task.await;
    tokio::time::timeout(Duration::from_secs(2), async {
        while PathBuf::from(format!("/proc/{pid}")).exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut next = config.clone();
    next.qemu_binary = "/bin/false".into();
    let error = pod::run(next, std::future::pending())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        !error.contains("another manager"),
        "ownership lock survived cancellation"
    );
}

#[test]
fn uefi_and_tpm_arguments_keep_boot_data_and_control_paths_separate() {
    let directory = tempfile::tempdir().unwrap();
    for name in ["code", "vars", "boot", "data"] {
        std::fs::write(directory.path().join(name), vec![0; 128]).unwrap();
    }
    let config: Config=serde_json::from_value(serde_json::json!({
        "qemu_binary":"/bin/true", "console":true, "state_dir":directory.path().join("state"),
        "uefi":{"code":directory.path().join("code"),"vars_template":directory.path().join("vars"),"boot_disk":directory.path().join("boot")},
        "tpm":{"swtpm_binary":"/bin/true"},"disk":{"path":directory.path().join("data")}
    })).unwrap();
    config.validate().unwrap();
    let args = config.arguments(Path::new("/tmp/private/qmp.sock"));
    assert!(!args
        .iter()
        .any(|arg| matches!(arg.as_str(), "-kernel" | "-initrd" | "-append" | "-netdev")));
    assert!(args.windows(2).any(|arg| arg == ["-nic", "none"]));
    assert!(args
        .windows(2)
        .any(|arg| arg == ["-device", "tpm-tis,tpmdev=tpm0"]));
    assert!(args
        .windows(2)
        .any(|arg| arg == ["-chardev", "socket,id=tpm,path=/tmp/private/swtpm.sock"]));
    assert!(args
        .windows(2)
        .any(|arg| arg == ["-device", "virtio-blk-pci,drive=boot,bootindex=1"]));
    assert!(args
        .windows(2)
        .any(|arg| arg == ["-device", "virtio-blk-pci,drive=disk"]));
    let nodes: Vec<serde_json::Value> = args
        .windows(2)
        .filter(|arg| arg[0] == "-blockdev")
        .map(|arg| serde_json::from_str(&arg[1]).unwrap())
        .collect();
    let vars = nodes
        .iter()
        .find(|node| node["node-name"] == "pflash-vars-file")
        .unwrap();
    assert_eq!(
        vars["filename"],
        config
            .state_dir
            .join("identity/OVMF_VARS.fd")
            .to_str()
            .unwrap()
    );
    assert_eq!(vars["read-only"], false);
    let boot = nodes
        .iter()
        .find(|node| node["node-name"] == "boot-file")
        .unwrap();
    assert_eq!(boot["read-only"], true);
}

#[tokio::test]
async fn swtpm_startup_failure_timeout_and_shutdown_reap_the_tpm_before_qemu() {
    for mode in ["exit", "timeout", "shutdown"] {
        let directory = tempfile::tempdir().unwrap();
        let kernel = directory.path().join("kernel");
        std::fs::write(&kernel, []).unwrap();
        let binary = if mode == "exit" {
            PathBuf::from("/bin/false")
        } else {
            fixture_binary("sleep.sh")
        };
        let config: Config=serde_json::from_value(serde_json::json!({
            "kernel":kernel,"qemu_binary":"/bin/false", "tpm":{"swtpm_binary":binary},
            "state_dir":directory.path().join("state"),"runtime_dir":directory.path().join("runtime"),"startup_timeout_secs":1
        })).unwrap();
        let outcome = pod::run(config.clone(), async {
            if mode == "shutdown" {
                tokio::time::sleep(Duration::from_millis(100)).await;
            } else {
                std::future::pending::<()>().await;
            }
        })
        .await;
        match mode {
            "exit" => assert!(outcome
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("swtpm exited during startup")),
            "timeout" => assert!(outcome
                .as_ref()
                .unwrap_err()
                .to_string()
                .contains("swtpm startup timed out")),
            "shutdown" => assert_eq!(
                outcome.as_ref().unwrap().reason,
                "shutdown-during-tpm-startup"
            ),
            _ => unreachable!(),
        }
        let status = pod::status(&config).unwrap();
        let pid = status.swtpm_pid.unwrap_or_else(|| {
            panic!(
                "swtpm PID missing in mode {mode}; outcome={outcome:?}; detail={:?}",
                status.detail
            )
        });
        assert!(status.qemu_pid.is_none());
        assert!(!status.ready);
        assert!(
            !PathBuf::from(format!("/proc/{pid}")).exists(),
            "TPM process leaked after {mode}"
        );
    }
}
