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
fn admin_bucket_audio_adds_a_playback_device_on_the_vnc_audio_capture() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path(), PathBuf::from("/bin/true"), dir.path().join("k"));
    config.console = true;
    let socket = Path::new("/tmp/private/qmp.sock");
    let plain = config.arguments(socket);
    config.admin_bucket = pod::AdminBucket {
        audio: false,
        usb_redirect: true,
    };
    assert_eq!(config.arguments(socket), plain);
    assert!(!plain.iter().any(|a| a.contains("audio")));
    config.admin_bucket.audio = true;
    let args = config.arguments(socket);
    for pair in [
        ["-audiodev", "none,id=snd0"],
        ["-device", "intel-hda,id=hda"],
        ["-device", "hda-output,audiodev=snd0"],
        [
            "-vnc",
            "unix:/tmp/private/vnc.sock,share=force-shared,audiodev=snd0",
        ],
    ] {
        assert!(args.windows(2).any(|a| a == pair), "{pair:?} in {args:?}");
    }
    // Playback only: no input (microphone) device or second audiodev.
    assert!(!args
        .iter()
        .any(|a| a.contains("hda-duplex") || a.contains("hda-micro")));
    assert_eq!(args.iter().filter(|a| *a == "-audiodev").count(), 1);
    assert_eq!(args.len(), plain.len() + 6);
    // Without the console there is nothing to carry the audio.
    config.console = false;
    assert!(!config.arguments(socket).iter().any(|a| a.contains("audio")));
}

#[test]
fn guest_nics_replace_nic_none_with_stream_netdevs_to_their_relays() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = dir.path().join("kernel");
    std::fs::write(&kernel, []).unwrap();
    let mut config = config(dir.path(), PathBuf::from("/bin/true"), kernel);
    config.nics = serde_json::from_value(serde_json::json!([
        {"id": "lan", "mac": "02:12:34:56:78:9a", "host": "10.0.3.7", "port": 7700},
        {"id": "storage", "mac": "06:00:00:00:00:02", "host": "fd00::7", "port": 7701}
    ]))
    .unwrap();
    config.validate().unwrap();
    let args = config.arguments(Path::new("/tmp/private/qmp.sock"));
    assert!(!args.iter().any(|a| a == "-nic"));
    let netdevs: Vec<serde_json::Value> = args
        .windows(2)
        .filter(|a| a[0] == "-netdev")
        .map(|a| serde_json::from_str(&a[1]).unwrap())
        .collect();
    assert_eq!(
        netdevs,
        [
            serde_json::json!({"type":"stream","id":"nic-lan","server":false,
                "addr":{"type":"inet","host":"10.0.3.7","port":"7700"},"reconnect-ms":1000}),
            serde_json::json!({"type":"stream","id":"nic-storage","server":false,
                "addr":{"type":"inet","host":"fd00::7","port":"7701"},"reconnect-ms":1000}),
        ]
    );
    assert!(args.windows(2).any(|a| a
        == [
            "-device",
            "virtio-net-pci,id=nic-lan,netdev=nic-lan,mac=02:12:34:56:78:9a,romfile="
        ]));
    assert!(args.windows(2).any(|a| a
        == [
            "-device",
            "virtio-net-pci,id=nic-storage,netdev=nic-storage,mac=06:00:00:00:00:02,romfile="
        ]));
    config.nics.clear();
    let args = config.arguments(Path::new("/tmp/private/qmp.sock"));
    assert!(args.windows(2).any(|a| a == ["-nic", "none"]));
    assert!(!args.iter().any(|a| a == "-netdev"));
}

#[test]
fn guest_nics_need_unique_labels_local_unicast_macs_and_ip_relays() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = dir.path().join("kernel");
    std::fs::write(&kernel, []).unwrap();
    let base = config(dir.path(), PathBuf::from("/bin/true"), kernel);
    let nic = |id: &str, mac: &str, host: &str, port: u16| pod::Nic {
        id: id.into(),
        mac: mac.into(),
        host: host.into(),
        port,
    };
    for (nics, problem) in [
        (
            vec![nic("Lan", "02:00:00:00:00:01", "10.0.0.1", 7700)],
            "id",
        ),
        (
            vec![nic("lan,x", "02:00:00:00:00:01", "10.0.0.1", 7700)],
            "id",
        ),
        (
            vec![
                nic("lan", "02:00:00:00:00:01", "10.0.0.1", 7700),
                nic("lan", "02:00:00:00:00:02", "10.0.0.2", 7700),
            ],
            "duplicate id",
        ),
        (
            vec![nic("lan", "00:16:3e:00:00:01", "10.0.0.1", 7700)],
            "vendor MAC",
        ),
        (
            vec![nic("lan", "03:00:00:00:00:01", "10.0.0.1", 7700)],
            "multicast MAC",
        ),
        (
            vec![nic("lan", "02:00:00:00:00", "10.0.0.1", 7700)],
            "short MAC",
        ),
        (
            vec![nic("lan", "02:00:00:00:00:01,romfile=x", "10.0.0.1", 7700)],
            "MAC injection",
        ),
        (
            vec![
                nic("a", "02:00:00:00:00:01", "10.0.0.1", 7700),
                nic("b", "02:00:00:00:00:01", "10.0.0.2", 7700),
            ],
            "duplicate MAC",
        ),
        (
            vec![nic("lan", "02:00:00:00:00:01", "relay.example", 7700)],
            "hostname",
        ),
        (
            vec![nic("lan", "02:00:00:00:00:01", "0.0.0.0", 7700)],
            "unspecified",
        ),
        (
            vec![nic("lan", "02:00:00:00:00:01", "224.0.0.1", 7700)],
            "multicast",
        ),
        (vec![nic("lan", "02:00:00:00:00:01", "10.0.0.1", 0)], "port"),
        (
            (0..=pod::NIC_LIMIT)
                .map(|i| {
                    nic(
                        &format!("n{i}"),
                        &format!("02:00:00:00:00:{i:02x}"),
                        "10.0.0.1",
                        7700,
                    )
                })
                .collect(),
            "too many",
        ),
    ] {
        let mut config = base.clone();
        config.nics = nics;
        assert!(config.validate().is_err(), "{problem} accepted");
    }
    assert_eq!(
        pod::parse_guest_mac("0A:BB:cc:00:00:01"),
        Some([0x0a, 0xbb, 0xcc, 0, 0, 1])
    );
}

#[test]
fn seed_disk_is_attached_read_only_without_boot_index_only_when_configured() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = dir.path().join("kernel");
    std::fs::write(&kernel, []).unwrap();
    let mut config = config(dir.path(), PathBuf::from("/bin/true"), kernel);
    let socket = Path::new("/run/qrab/session-x/qmp.sock");
    let plain = config.arguments(socket);
    assert!(!plain.iter().any(|a| a.contains("seed")));

    config.seed = Some(pod::Seed {
        meta_data: "instance-id: \"uid\"\n".into(),
        user_data: None,
        network_config: None,
    });
    config.validate().unwrap();
    let args = config.arguments(socket);
    // The seed adds exactly its own two nodes and device after the rest.
    assert_eq!(&args[..plain.len()], &plain[..]);
    let seed: Vec<_> = args[plain.len()..].iter().map(String::as_str).collect();
    assert_eq!(seed.len(), 6);
    let file: serde_json::Value = serde_json::from_str(seed[1]).unwrap();
    let raw: serde_json::Value = serde_json::from_str(seed[3]).unwrap();
    assert_eq!(
        (seed[0], seed[2], seed[4]),
        ("-blockdev", "-blockdev", "-device")
    );
    assert_eq!(
        file,
        serde_json::json!({"driver":"file","filename":"/run/qrab/session-x/seed.img","node-name":"seed-file","read-only":true})
    );
    assert_eq!(
        raw,
        serde_json::json!({"driver":"raw","file":"seed-file","node-name":"seed","read-only":true})
    );
    assert_eq!(seed[5], "virtio-blk-pci,drive=seed,id=seed");

    config.seed.as_mut().unwrap().user_data = Some("x".repeat(pod::SEED_INPUT_LIMIT));
    assert!(matches!(
        config.validate(),
        Err(manager::Error::Config(message)) if message.contains("seed")
    ));
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

#[test]
fn secure_capable_firmware_explicitly_requires_smm_and_protected_pflash() {
    let config: Config =
        serde_json::from_value(serde_json::json!({"uefi":{"secure_boot_capable":true}})).unwrap();
    let args = config.arguments(Path::new("/tmp/qmp.sock"));
    assert!(args
        .windows(2)
        .any(|a| a[0] == "-machine" && a[1].contains("smm=on")));
    assert!(args
        .windows(2)
        .any(|a| a == ["-global", "driver=cfi.pflash01,property=secure,value=on"]));
    let config: Config = serde_json::from_value(serde_json::json!({"uefi":{}})).unwrap();
    assert!(!config
        .arguments(Path::new("/tmp/qmp.sock"))
        .iter()
        .any(|a| a.contains("property=secure")));
}
