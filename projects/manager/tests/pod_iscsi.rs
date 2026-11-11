use manager::pod::{Config, IscsiBus};
use std::{os::unix::fs::PermissionsExt, path::Path};

const PASSWORD: &str = "test-only-secret,with:punctuation";
const TARGET: &str = "iqn.2026-11.com.yapplesauce:qrab.test.disk";
const INITIATOR: &str = "iqn.2026-11.com.yapplesauce:qrab.vm.test-uid";

fn fixture() -> (tempfile::TempDir, Config) {
    let directory = tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let kernel = directory.path().join("kernel");
    std::fs::write(&kernel, []).unwrap();
    let password = directory.path().join("password, with spaces");
    std::fs::write(&password, PASSWORD).unwrap();
    let config=serde_json::from_value(serde_json::json!({
        "kernel":kernel,"qemu_binary":"/bin/true","state_id":"test-uid","state_dir":directory.path().join("state"),"runtime_dir":directory.path().join("runtime"),
        "iscsi_disks":[{"id":"qrab-probe","portal":"127.0.0.1:3260","target":TARGET,"lun":1,"initiator_name":INITIATOR,"username":"qrab-test","password_file":password}]
    })).unwrap();
    (directory, config)
}

#[test]
fn native_block_graph_uses_only_file_backed_chap_and_preserves_guest_no_nic() {
    let (_directory, config) = fixture();
    config.validate().unwrap();
    let args = config.arguments(Path::new("/tmp/private/qmp.sock"));
    assert!(args.windows(2).any(|arg| arg == ["-nic", "none"]));
    assert!(!args
        .iter()
        .any(|arg| arg == "-netdev" || arg.starts_with("iscsi://") || arg.contains(PASSWORD)));
    assert!(!serde_json::to_string(&config).unwrap().contains(PASSWORD));
    assert!(!format!("{config:?}").contains(PASSWORD));
    let objects: Vec<serde_json::Value> = args
        .windows(2)
        .filter(|arg| arg[0] == "-object")
        .map(|arg| serde_json::from_str(&arg[1]).unwrap())
        .collect();
    let secret = objects
        .iter()
        .find(|object| object["qom-type"] == "secret")
        .unwrap();
    assert_eq!(
        secret["file"],
        config.iscsi_disks[0].password_file.to_str().unwrap()
    );
    assert_eq!(secret["format"], "raw");
    assert!(secret.get("data").is_none());
    let nodes: Vec<serde_json::Value> = args
        .windows(2)
        .filter(|arg| arg[0] == "-blockdev")
        .map(|arg| serde_json::from_str(&arg[1]).unwrap())
        .collect();
    let native = nodes.iter().find(|node| node["driver"] == "iscsi").unwrap();
    assert_eq!(native["password-secret"], secret["id"]);
    assert!(native.get("password").is_none() && native.get("filename").is_none());
    assert_eq!(native["transport"], "tcp");
    assert_eq!(native["target"], TARGET);
    assert_eq!(native["initiator-name"], INITIATOR);
    assert_eq!(native["lun"], 1);
    let raw = nodes.iter().find(|node| node["driver"] == "raw").unwrap();
    assert_eq!(raw["file"], native["node-name"]);
    assert_eq!(raw["read-only"], false);
    assert!(args
        .windows(2)
        .any(|arg| arg[0] == "-device" && arg[1].contains("serial=qrab-probe")));
}

#[test]
fn scsi_and_virtio_disks_have_distinct_graphs_serials_and_safe_boot_priorities() {
    let (_directory, mut config) = fixture();
    config.iscsi_disks[0].read_only = true;
    config.iscsi_disks[0].boot_index = Some(0);
    let mut second = config.iscsi_disks[0].clone();
    second.id = "second".into();
    second.lun = 2;
    second.bus = IscsiBus::Scsi;
    second.boot_index = Some(2);
    config.iscsi_disks.push(second);
    config.validate().unwrap();
    let args = config.arguments(Path::new("/tmp/private/qmp.sock"));
    assert_eq!(
        args.iter()
            .filter(|arg| arg.as_str() == "virtio-scsi-pci,id=iscsi-controller")
            .count(),
        1
    );
    assert!(args
        .iter()
        .any(|arg| arg.starts_with("virtio-blk-pci,")
            && arg.contains("serial=qrab-probe,bootindex=0")));
    assert!(args
        .iter()
        .any(|arg| arg.starts_with("scsi-hd,") && arg.contains("serial=second,bootindex=2")));
    let nodes: Vec<serde_json::Value> = args
        .windows(2)
        .filter(|arg| arg[0] == "-blockdev")
        .map(|arg| serde_json::from_str(&arg[1]).unwrap())
        .collect();
    assert!(nodes
        .iter()
        .all(|node| node["read-only"] == true && node["node-name"].as_str().unwrap().len() <= 31));
    assert_eq!(
        nodes
            .iter()
            .filter_map(|node| node["node-name"].as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
}

#[test]
fn ambiguous_or_invalid_disk_endpoints_ids_and_boot_slots_fail_validation() {
    let (_directory, config) = fixture();
    for id in [
        "",
        "UPPER",
        "has,comma",
        "-start",
        "end-",
        "this-id-is-far-too-long",
    ] {
        let mut invalid = config.clone();
        invalid.iscsi_disks[0].id = id.into();
        assert!(invalid.validate().is_err());
    }
    for portal in [
        "iscsi://user:must-not-leak@127.0.0.1/target/1",
        "storage.example:3260",
        "127.0.0.1",
        "127.0.0.1:0",
        "0.0.0.0:3260",
        "224.0.0.1:3260",
        "255.255.255.255:3260",
    ] {
        let mut invalid = config.clone();
        invalid.iscsi_disks[0].portal = portal.into();
        let error = invalid.validate().unwrap_err().to_string();
        assert!(!error.contains("must-not-leak"));
    }
    for iqn in [
        "",
        "iqn.2026-00.com.example:disk",
        "iqn.2026-13.com.example:disk",
        "iqn.2026-11.example:disk",
        "iscsi://example/disk",
        "iqn.2026-11.com.example:",
        "iqn.2026-11.com.example:UPPER",
    ] {
        let mut invalid = config.clone();
        invalid.iscsi_disks[0].target = iqn.into();
        assert!(invalid.validate().is_err());
    }
    let mut invalid = config.clone();
    invalid.iscsi_disks.push(invalid.iscsi_disks[0].clone());
    assert!(invalid.validate().is_err());
    invalid.iscsi_disks[1].id = "second".into();
    assert!(
        invalid.validate().is_err(),
        "duplicate target/LUN accepted under another ID"
    );
    invalid.iscsi_disks[1].lun = 2;
    invalid.iscsi_disks[1].initiator_name = "iqn.2026-11.com.example:other".into();
    assert!(invalid.validate().is_err());
    invalid.iscsi_disks[1].initiator_name = INITIATOR.into();
    invalid.iscsi_disks[0].boot_index = Some(2);
    invalid.iscsi_disks[1].boot_index = Some(2);
    assert!(invalid.validate().is_err());
    let mut invalid = config.clone();
    invalid.iscsi_disks[0].lun = u32::MAX;
    assert!(invalid.validate().is_err());
    invalid.iscsi_disks[0].lun = 1;
    invalid.iscsi_disks[0].boot_index = Some(1);
    assert!(invalid.validate().is_err());
    let mut ipv6 = config;
    ipv6.iscsi_disks[0].portal = "[fd00::1]:3260".into();
    ipv6.validate().unwrap();
}

#[test]
fn secret_files_fail_closed_without_echoing_data_or_accepting_inline_passwords() {
    let (directory, config) = fixture();
    for content in [
        b"".as_slice(),
        b"do-not-leak\n",
        b"do-not-leak\0",
        &[0xff],
        &vec![b'x'; 4097],
    ] {
        std::fs::write(&config.iscsi_disks[0].password_file, content).unwrap();
        let error = config.validate().unwrap_err().to_string();
        assert!(!error.contains("do-not-leak"));
    }
    let mut missing = config.clone();
    missing.iscsi_disks[0].password_file = directory.path().join("missing");
    assert!(missing.validate().is_err());
    missing.iscsi_disks[0].password_file = "relative".into();
    assert!(missing.validate().is_err());
    missing.iscsi_disks[0].password_file = directory.path().into();
    assert!(missing.validate().is_err());
    let fifo = directory.path().join("fifo");
    nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::S_IRUSR).unwrap();
    missing.iscsi_disks[0].password_file = fifo;
    assert!(
        missing.validate().is_err(),
        "a FIFO must be rejected without blocking"
    );
    let mut inline = serde_json::to_value(&config).unwrap();
    inline["iscsi_disks"][0]["password"] = "do-not-leak".into();
    assert!(serde_json::from_value::<Config>(inline).is_err());
    std::fs::write(&config.iscsi_disks[0].password_file, PASSWORD).unwrap();
    let alias = directory.path().join("projected-key");
    std::os::unix::fs::symlink(&config.iscsi_disks[0].password_file, &alias).unwrap();
    missing.iscsi_disks[0].password_file = alias;
    missing.validate().unwrap();
}

#[tokio::test]
async fn forced_native_child_shutdown_is_not_reported_as_clean_release() {
    let (directory, mut config) = fixture();
    let binary = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ignore-term.sh");
    config.qemu_binary = binary;
    let result = manager::pod::run(config.clone(), async {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    })
    .await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("clean disk release is not proven"));
    let status = manager::pod::status(&config).unwrap();
    assert_eq!(status.phase, manager::pod::Phase::Failed);
    assert!(!std::path::PathBuf::from(format!("/proc/{}", status.qemu_pid.unwrap())).exists());
    assert!(!serde_json::to_string(&status).unwrap().contains(PASSWORD));
    // Preserve the existing no-native-disks lifecycle rather than changing its
    // restart policy as a side effect of the storage release contract.
    config.iscsi_disks.clear();
    config.runtime_dir = directory.path().join("legacy-runtime");
    assert!(manager::pod::run(config, async {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    })
    .await
    .is_ok());
}
