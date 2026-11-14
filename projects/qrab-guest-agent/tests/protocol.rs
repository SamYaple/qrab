//! Drive the agent with the real qapi-protocol QGA client over a socketpair.
use qapi::qga::{commands::*, GuestFsfreezeStatus};
use qapi_protocol::{qga::Qga, ClientOptions, Error};
use qrab_guest_agent::{
    proto::{QrabUpdateStatus, QrabUpdateTrigger, UpdateState},
    sys::{Mount, Platform, ShutdownMode},
    Agent, Config, HookPolicy,
};
use std::{
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
struct Fake {
    mounts: Vec<Mount>,
    firmware: Option<qrab_guest_agent::proto::FirmwareState>,
    secure_databases: Option<qrab_guest_agent::secure_boot::SecureBootDatabases>,
    log: Mutex<Vec<String>>,
    frozen: Mutex<Vec<PathBuf>>,
    fail_freeze: Option<PathBuf>,
}

impl Fake {
    fn with_mounts(dirs: &[&str]) -> Self {
        Self {
            mounts: dirs
                .iter()
                .enumerate()
                .map(|(i, d)| Mount {
                    dir: d.into(),
                    fstype: "ext4".into(),
                    device: (253, i as u32),
                })
                .collect(),
            ..Self::default()
        }
    }
    fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

impl Platform for Fake {
    fn secure_boot_databases(
        &self,
    ) -> std::io::Result<qrab_guest_agent::secure_boot::SecureBootDatabases> {
        Ok(self.secure_databases.clone().unwrap_or_default())
    }
    fn secure_boot_update(
        &self,
        request: qrab_guest_agent::secure_boot::QrabSecureBootUpdate,
    ) -> std::io::Result<qrab_guest_agent::secure_boot::SecureBootDatabases> {
        self.log.lock().unwrap().push(format!(
            "authenticated {} append={}",
            request.database_id, request.append
        ));
        self.secure_boot_databases()
    }

    fn firmware_state(&self) -> std::io::Result<qrab_guest_agent::proto::FirmwareState> {
        Ok(self.firmware.clone().unwrap_or_default())
    }

    fn network_interfaces(
        &self,
    ) -> std::io::Result<Vec<qrab_guest_agent::proto::NetworkInterface>> {
        use qrab_guest_agent::proto::{NetworkAddress, NetworkInterface, NetworkStatistics};
        Ok(vec![NetworkInterface {
            name: "eth0".into(),
            hardware_address: Some("02:aa:bb:cc:dd:ee".into()),
            ip_addresses: vec![
                NetworkAddress {
                    address: "192.0.2.8".into(),
                    prefix: 24,
                },
                NetworkAddress {
                    address: "2001:db8::8".into(),
                    prefix: 64,
                },
            ],
            interface_enabled: Some(true),
            link_up: Some(false),
            statistics: Some(NetworkStatistics {
                rx_bytes: 123,
                tx_packets: 456,
                ..Default::default()
            }),
        }])
    }

    fn mounts(&self) -> std::io::Result<Vec<Mount>> {
        Ok(self.mounts.clone())
    }
    fn freeze(&self, mount: &Mount) -> std::io::Result<()> {
        if self.fail_freeze.as_ref() == Some(&mount.dir) {
            return Err(std::io::Error::from_raw_os_error(libc::EIO));
        }
        self.log
            .lock()
            .unwrap()
            .push(format!("freeze {}", mount.dir.display()));
        self.frozen.lock().unwrap().push(mount.dir.clone());
        Ok(())
    }
    fn thaw(&self, mount: &Mount) -> std::io::Result<()> {
        let mut frozen = self.frozen.lock().unwrap();
        let index = frozen
            .iter()
            .position(|d| d == &mount.dir)
            .ok_or_else(|| std::io::Error::from_raw_os_error(libc::EINVAL))?;
        frozen.remove(index);
        self.log
            .lock()
            .unwrap()
            .push(format!("thaw {}", mount.dir.display()));
        Ok(())
    }
    fn trim(&self, mount: &Mount, minimum: u64) -> std::io::Result<(u64, u64)> {
        if mount.dir == Path::new("/") {
            Ok((4096, minimum.max(512)))
        } else {
            Err(std::io::Error::from_raw_os_error(libc::EOPNOTSUPP))
        }
    }
    fn shutdown(&self, mode: ShutdownMode) -> std::io::Result<()> {
        self.log.lock().unwrap().push(format!("shutdown {mode:?}"));
        Ok(())
    }
}

fn config(dir: &Path) -> Config {
    Config {
        freeze_timeout: Duration::from_secs(60),
        update: HookPolicy {
            path: dir.join("update-hook"),
            require_root: false,
            timeout: Duration::from_secs(10),
        },
        os_release: vec![dir.join("os-release")],
    }
}

/// Serve the agent on one end of a socketpair in a thread.
fn spawn_agent(agent: Arc<Agent>) -> tokio::net::UnixStream {
    let (host, guest) = UnixStream::pair().unwrap();
    std::thread::spawn(move || {
        let writer = guest.try_clone().unwrap();
        let _ = agent.serve(&guest, &writer);
    });
    host.set_nonblocking(true).unwrap();
    tokio::net::UnixStream::from_std(host).unwrap()
}

async fn connect(agent: Arc<Agent>) -> Qga {
    Qga::from_stream(spawn_agent(agent)).await.unwrap()
}

fn agent_with(platform: Fake, config: Config) -> (Arc<Agent>, Arc<Fake>) {
    let platform = Arc::new(platform);
    (Agent::new(config, platform.clone()), platform)
}

#[tokio::test]
async fn sync_info_ping_and_identity() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("os-release"),
        "ID=qrab\nPRETTY_NAME=\"qrab demo\"\n",
    )
    .unwrap();
    let (agent, _) = agent_with(Fake::default(), config(dir.path()));
    let mut qga = connect(agent).await;
    assert_eq!(qga.version(), (11, 1, 3));
    qga.execute(&GuestPing {}).await.unwrap();
    assert_eq!(qga.execute(&GuestSync { id: -5 }).await.unwrap(), -5);

    let info = qga.execute(&GuestInfo {}).await.unwrap();
    let mut names: Vec<_> = info
        .supported_commands
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "guest-fsfreeze-freeze",
            "guest-fsfreeze-freeze-list",
            "guest-fsfreeze-status",
            "guest-fsfreeze-thaw",
            "guest-fstrim",
            "guest-get-host-name",
            "guest-get-osinfo",
            "guest-get-time",
            "guest-info",
            "guest-network-get-interfaces",
            "guest-ping",
            "guest-shutdown",
            "guest-sync",
            "guest-sync-delimited",
            "qrab-boot-next",
            "qrab-boot-state",
            "qrab-firmware-state",
            "qrab-network-get-interfaces",
            "qrab-secure-boot-databases",
            "qrab-secure-boot-update",
            "qrab-update-status",
            "qrab-update-trigger",
        ]
    );
    for command in &info.supported_commands {
        // No hook is installed in this directory.
        assert_eq!(
            command.enabled,
            command.name != "qrab-update-trigger",
            "{}",
            command.name
        );
        assert_eq!(
            command.success_response,
            Some(command.name != "guest-shutdown")
        );
    }

    let os = qga.execute(&GuestGetOsinfo {}).await.unwrap();
    assert_eq!(os.id.as_deref(), Some("qrab"));
    assert_eq!(os.pretty_name.as_deref(), Some("qrab demo"));
    assert!(os.kernel_release.is_some_and(|r| !r.is_empty()));
    assert!(!qga
        .execute(&GuestGetHostName {})
        .await
        .unwrap()
        .host_name
        .is_empty());
    let time = qga.execute(&GuestGetTime {}).await.unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as i64;
    assert!((now - time).abs() < 60_000_000_000);
}

#[tokio::test]
async fn unknown_disabled_and_malformed_requests_fail_without_desync() {
    let dir = tempfile::tempdir().unwrap();
    let (agent, _) = agent_with(Fake::default(), config(dir.path()));
    let mut qga = connect(agent).await;
    for (name, class) in [
        ("guest-exec", "CommandNotFound"),
        ("guest-file-open", "CommandNotFound"),
        ("guest-set-user-password", "CommandNotFound"),
        ("guest-network-set-interfaces", "CommandNotFound"),
        ("qrab-update-trigger", "CommandNotFound"),
    ] {
        match qga
            .execute_raw::<serde_json::Value>(name, serde_json::json!({}))
            .await
        {
            Err(Error::Qmp(error)) => assert_eq!(error.class, class, "{name}: {}", error.desc),
            other => panic!("{name}: {other:?}"),
        }
    }
    match qga
        .execute_raw::<serde_json::Value>("guest-ping", serde_json::json!({"surprise": 1}))
        .await
    {
        Err(Error::Qmp(error)) => assert!(error.desc.contains("surprise")),
        other => panic!("{other:?}"),
    }
    match qga
        .execute_raw::<i64>("guest-sync", serde_json::json!({"id": "x"}))
        .await
    {
        Err(Error::Qmp(error)) => assert_eq!(error.class, "GenericError"),
        other => panic!("{other:?}"),
    }
    // The session stays aligned after agent errors.
    qga.execute(&GuestPing {}).await.unwrap();
}

#[test]
fn raw_framing_resets_at_sentinel_and_echoes_ids() {
    let dir = tempfile::tempdir().unwrap();
    let (agent, _) = agent_with(Fake::default(), config(dir.path()));
    let (mut host, guest) = UnixStream::pair().unwrap();
    std::thread::spawn(move || {
        let writer = guest.try_clone().unwrap();
        let _ = agent.serve(&guest, &writer);
    });
    host.write_all(b"{\"execute\":\"guest-pi\xff\xff{\"execute\":\"guest-sync-delimited\",\"arguments\":{\"id\":9}}")
        .unwrap();
    host.write_all(b"{\"execute\":\"guest-ping\",\"id\":\"abc\"}not-json\n")
        .unwrap();
    host.write_all(&[b' '; 70 * 1024]).unwrap();
    host.write_all(b"{\"execute\":\"guest-ping\"}").unwrap();
    host.shutdown(std::net::Shutdown::Write).unwrap();
    let mut out = Vec::new();
    host.read_to_end(&mut out).unwrap();
    let mut lines = out.split(|b| *b == b'\n');
    assert_eq!(lines.next().unwrap(), b"\xff{\"return\":9}");
    let ping: serde_json::Value = serde_json::from_slice(lines.next().unwrap()).unwrap();
    assert_eq!(ping, serde_json::json!({"return": {}, "id": "abc"}));
    let junk: serde_json::Value = serde_json::from_slice(lines.next().unwrap()).unwrap();
    assert_eq!(junk["error"]["class"], "GenericError");
    // Whitespace between requests is not buffered toward the size limit.
    let last: serde_json::Value = serde_json::from_slice(lines.next().unwrap()).unwrap();
    assert_eq!(last, serde_json::json!({"return": {}}));
}

#[tokio::test]
async fn client_resynchronizes_past_stale_replies_after_reconnect() {
    let dir = tempfile::tempdir().unwrap();
    let (agent, _) = agent_with(Fake::default(), config(dir.path()));
    let mut stream = spawn_agent(agent);
    // A previous host session left a request whose reply is still unread.
    use tokio::io::AsyncWriteExt;
    stream
        .write_all(b"{\"execute\":\"guest-sync-delimited\",\"arguments\":{\"id\":1}}{\"execute\":\"guest-info\"}")
        .await
        .unwrap();
    let mut qga = Qga::from_stream(stream).await.unwrap();
    qga.execute(&GuestPing {}).await.unwrap();
}

#[tokio::test]
async fn freeze_status_thaw_and_frozen_command_gating() {
    let dir = tempfile::tempdir().unwrap();
    let (agent, platform) = agent_with(Fake::with_mounts(&["/", "/var"]), config(dir.path()));
    let mut qga = connect(agent).await;
    assert!(!frozen(&mut qga).await);
    assert_eq!(qga.execute(&GuestFsfreezeFreeze {}).await.unwrap(), 2);
    assert_eq!(
        platform.log(),
        ["freeze /var", "freeze /"],
        "children freeze first"
    );
    assert!(frozen(&mut qga).await);

    let info = qga.execute(&GuestInfo {}).await.unwrap();
    let enabled: Vec<_> = info
        .supported_commands
        .iter()
        .filter(|c| c.enabled)
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(
        enabled,
        [
            "guest-sync",
            "guest-sync-delimited",
            "guest-ping",
            "guest-info",
            "guest-fsfreeze-status",
            "guest-fsfreeze-thaw",
            "qrab-update-status",
        ]
    );
    for blocked in [
        "guest-fsfreeze-freeze",
        "guest-fstrim",
        "guest-get-time",
        "guest-shutdown",
    ] {
        match qga
            .execute_raw::<serde_json::Value>(blocked, serde_json::json!({}))
            .await
        {
            Err(Error::Qmp(error)) => {
                assert_eq!(error.class, "CommandNotFound");
                assert!(error.desc.contains("frozen state"), "{}", error.desc);
            }
            other => panic!("{blocked}: {other:?}"),
        }
    }
    assert_eq!(qga.execute(&GuestFsfreezeThaw {}).await.unwrap(), 2);
    assert!(!frozen(&mut qga).await);
    assert_eq!(qga.execute(&GuestFsfreezeThaw {}).await.unwrap(), 0);

    // A listed freeze touches only the named filesystem; unknown names fail.
    let list = GuestFsfreezeFreezeList {
        mountpoints: Some(vec!["/var".into()]),
    };
    assert_eq!(qga.execute(&list).await.unwrap(), 1);
    assert_eq!(qga.execute(&GuestFsfreezeThaw {}).await.unwrap(), 1);
    let missing = GuestFsfreezeFreezeList {
        mountpoints: Some(vec!["/nope".into()]),
    };
    assert!(matches!(qga.execute(&missing).await, Err(Error::Qmp(_))));
    assert!(!frozen(&mut qga).await);
}

#[tokio::test]
async fn failed_freeze_thaws_what_was_frozen() {
    let dir = tempfile::tempdir().unwrap();
    let mut fake = Fake::with_mounts(&["/", "/var", "/srv"]);
    fake.fail_freeze = Some("/var".into());
    let (agent, platform) = agent_with(fake, config(dir.path()));
    let mut qga = connect(agent).await;
    match qga.execute(&GuestFsfreezeFreeze {}).await {
        Err(Error::Qmp(error)) => assert!(error.desc.contains("/var")),
        other => panic!("{other:?}"),
    }
    assert_eq!(platform.log(), ["freeze /srv", "thaw /srv"]);
    assert!(!frozen(&mut qga).await);
}

#[tokio::test]
async fn watchdog_auto_thaws_an_abandoned_freeze() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(dir.path());
    config.freeze_timeout = Duration::from_millis(300);
    let (agent, platform) = agent_with(Fake::with_mounts(&["/"]), config);
    let mut qga = connect(agent).await;
    assert_eq!(qga.execute(&GuestFsfreezeFreeze {}).await.unwrap(), 1);
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert!(!frozen(&mut qga).await);
    assert_eq!(platform.log(), ["freeze /", "thaw /"]);
    // A thaw-then-refreeze is covered by its own fresh deadline.
    assert_eq!(qga.execute(&GuestFsfreezeFreeze {}).await.unwrap(), 1);
    assert!(frozen(&mut qga).await);
    qga.execute(&GuestFsfreezeThaw {}).await.unwrap();
}

#[tokio::test]
async fn fstrim_reports_each_filesystem() {
    let dir = tempfile::tempdir().unwrap();
    let (agent, _) = agent_with(Fake::with_mounts(&["/", "/boot"]), config(dir.path()));
    let mut qga = connect(agent).await;
    let reply = qga
        .execute(&GuestFstrim {
            minimum: Some(1024),
        })
        .await
        .unwrap();
    assert_eq!(reply.paths.len(), 2);
    assert_eq!(reply.paths[0].trimmed, Some(4096));
    assert_eq!(reply.paths[0].minimum, Some(1024));
    assert!(reply.paths[1].error.is_some());
}

#[tokio::test]
async fn shutdown_sends_no_reply_and_validates_mode() {
    let dir = tempfile::tempdir().unwrap();
    let (agent, platform) = agent_with(Fake::default(), config(dir.path()));
    let mut qga = connect(agent.clone()).await;
    match qga
        .execute_raw::<serde_json::Value>("guest-shutdown", serde_json::json!({"mode": "explode"}))
        .await
    {
        Err(Error::Qmp(error)) => assert!(error.desc.contains("mode")),
        other => panic!("{other:?}"),
    }
    qga.send_no_response(&GuestShutdown {
        mode: Some("reboot".into()),
    })
    .await
    .unwrap();
    for _ in 0..50 {
        if !platform.log().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(platform.log(), ["shutdown Reboot"]);
}

async fn frozen(qga: &mut Qga) -> bool {
    matches!(
        qga.execute(&GuestFsfreezeStatusCommand {}).await.unwrap(),
        GuestFsfreezeStatus::Frozen
    )
}

fn install_hook(dir: &Path, body: &str) {
    let hook = dir.join("update-hook");
    std::fs::write(&hook, body).unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
}

async fn wait_finished(qga: &mut Qga) -> qrab_guest_agent::proto::UpdateStatus {
    for _ in 0..200 {
        let status = qga.execute(&QrabUpdateStatus {}).await.unwrap();
        if status.state != UpdateState::Running {
            return status;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("update did not finish");
}

#[tokio::test]
async fn update_trigger_runs_only_the_fixed_hook_with_validated_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("ran");
    install_hook(
        dir.path(),
        &format!(
            "#!/bin/sh\necho \"args=$*\"\necho \"run=$QRAB_UPDATE_RUN_ID home=${{HOME:-unset}}\" >&2\nprintf x >> {}\n[ \"$1\" != fail ] || exit 3\nsleep 0.2\n",
            marker.display()
        ),
    );
    let (agent, _) = agent_with(Fake::with_mounts(&["/"]), config(dir.path()));
    let mut qga = connect(agent).await;
    let info = qga.execute(&GuestInfo {}).await.unwrap();
    assert!(info
        .supported_commands
        .iter()
        .any(|c| c.name == "qrab-update-trigger" && c.enabled));
    assert_eq!(
        qga.execute(&QrabUpdateStatus {}).await.unwrap().state,
        UpdateState::Idle
    );

    for bad in [
        vec!["-rf".to_string()],
        vec!["a;b".into()],
        vec!["x".into(); 17],
    ] {
        assert!(matches!(
            qga.execute(&QrabUpdateTrigger { args: bad }).await,
            Err(Error::Qmp(_))
        ));
    }
    match qga
        .execute_raw::<serde_json::Value>(
            "qrab-update-trigger",
            serde_json::json!({"path": "/bin/sh"}),
        )
        .await
    {
        Err(Error::Qmp(error)) => assert!(error.desc.contains("path")),
        other => panic!("{other:?}"),
    }
    assert!(!marker.exists(), "rejected requests never start the hook");

    let started = qga
        .execute(&QrabUpdateTrigger {
            args: vec!["channel=stable".into(), "v2".into()],
        })
        .await
        .unwrap();
    assert_eq!((started.state, started.run_id), (UpdateState::Running, 1));
    // One run at a time, and filesystems cannot be frozen under it.
    assert!(qga.execute(&QrabUpdateTrigger::default()).await.is_err());
    assert!(qga.execute(&GuestFsfreezeFreeze {}).await.is_err());
    let done = wait_finished(&mut qga).await;
    assert_eq!(done.state, UpdateState::Succeeded);
    assert_eq!(done.exit_code, Some(0));
    assert_eq!(done.args, ["channel=stable", "v2"]);
    assert!(
        done.output.contains("args=channel=stable v2"),
        "{}",
        done.output
    );
    assert!(done.output.contains("run=1 home=unset"), "{}", done.output);
    assert!(done.finished_unix_ms >= done.started_unix_ms);

    qga.execute(&QrabUpdateTrigger {
        args: vec!["fail".into()],
    })
    .await
    .unwrap();
    let failed = wait_finished(&mut qga).await;
    assert_eq!(
        (failed.state, failed.run_id, failed.exit_code),
        (UpdateState::Failed, 2, Some(3))
    );
    assert!(failed.error.is_some());

    // While frozen the trigger is disabled; status remains readable.
    qga.execute(&GuestFsfreezeFreeze {}).await.unwrap();
    match qga.execute(&QrabUpdateTrigger::default()).await {
        Err(Error::Qmp(error)) => assert_eq!(error.class, "CommandNotFound"),
        other => panic!("{other:?}"),
    }
    qga.execute(&QrabUpdateStatus {}).await.unwrap();
    qga.execute(&GuestFsfreezeThaw {}).await.unwrap();
    assert_eq!(std::fs::read_to_string(&marker).unwrap(), "xx");
}

#[tokio::test]
async fn update_hook_timeout_kills_the_run() {
    let dir = tempfile::tempdir().unwrap();
    install_hook(dir.path(), "#!/bin/sh\necho started\nsleep 30 &\nwait\n");
    let mut config = config(dir.path());
    config.update.timeout = Duration::from_millis(300);
    let (agent, _) = agent_with(Fake::default(), config);
    let options = ClientOptions::default();
    let mut qga = Qga::from_stream_with_options(spawn_agent(agent), options)
        .await
        .unwrap();
    qga.execute(&QrabUpdateTrigger::default()).await.unwrap();
    let status = wait_finished(&mut qga).await;
    assert_eq!(status.state, UpdateState::Failed);
    assert_eq!(status.signal, Some(libc::SIGKILL));
    assert!(status.error.unwrap().contains("timed out"));
    assert!(status.output.contains("started"));
}

#[tokio::test]
async fn network_inventory_is_read_only_typed_and_reports_actual_flags() {
    let dir = tempfile::tempdir().unwrap();
    let (agent, _) = agent_with(Fake::default(), config(dir.path()));
    let mut qga = connect(agent).await;
    let interfaces = qga.execute(&GuestNetworkGetInterfaces {}).await.unwrap();
    assert_eq!(interfaces.len(), 1);
    assert_eq!(interfaces[0].name, "eth0");
    assert_eq!(
        interfaces[0].hardware_address.as_deref(),
        Some("02:aa:bb:cc:dd:ee")
    );
    let ips = interfaces[0].ip_addresses.as_ref().unwrap();
    assert_eq!(ips[0].ip_address, "192.0.2.8");
    assert_eq!(ips[1].prefix, 64);
    assert_eq!(interfaces[0].statistics.as_ref().unwrap().rx_bytes, 123);
    let extended = qga
        .execute(&qrab_guest_agent::proto::QrabNetworkGetInterfaces {})
        .await
        .unwrap();
    assert_eq!(extended[0].interface_enabled, Some(true));
    assert_eq!(extended[0].link_up, Some(false));
    assert_eq!(extended[0].statistics.as_ref().unwrap().tx_packets, 456);
    qga.execute(&GuestFsfreezeFreeze {}).await.unwrap();
    assert!(qga.execute(&GuestNetworkGetInterfaces {}).await.is_err());
    qga.execute(&GuestFsfreezeThaw {}).await.unwrap();
    assert!(qga.execute(&GuestNetworkGetInterfaces {}).await.is_ok());
}

#[tokio::test]
async fn missing_guest_efi_interfaces_remain_unknown_on_the_wire() {
    let dir = tempfile::tempdir().unwrap();
    let (agent, _) = agent_with(Fake::default(), config(dir.path()));
    let mut qga = connect(agent).await;
    let state = qga
        .execute(&qrab_guest_agent::proto::QrabFirmwareState {})
        .await
        .unwrap();
    assert!(!state.efi_runtime_available);
    assert_eq!(state.secure_boot, None);
    assert_eq!(state.setup_mode, None);
}

#[tokio::test]
async fn firmware_flags_are_typed_on_the_real_guest_agent_wire() {
    use qrab_guest_agent::proto::{FirmwareState, QrabFirmwareState};
    let dir = tempfile::tempdir().unwrap();
    let expected = FirmwareState {
        efi_runtime_available: true,
        secure_boot: Some(true),
        setup_mode: Some(false),
        audit_mode: Some(false),
        deployed_mode: Some(true),
    };
    let (agent, _) = agent_with(
        Fake {
            firmware: Some(expected.clone()),
            ..Default::default()
        },
        config(dir.path()),
    );
    let mut qga = connect(agent).await;
    assert_eq!(qga.execute(&QrabFirmwareState {}).await.unwrap(), expected);
}

#[tokio::test]
async fn secure_boot_database_inventory_and_signed_update_admission_use_the_typed_wire() {
    use qrab_guest_agent::secure_boot::{
        EfiSignature, QrabSecureBootDatabases, QrabSecureBootUpdate, SecureBootDatabase,
        SecureBootDatabases,
    };
    let dir = tempfile::tempdir().unwrap();
    let expected = SecureBootDatabases {
        efi_runtime_available: true,
        databases: vec![SecureBootDatabase {
            id: "db".into(),
            present: Some(true),
            attributes: Some(0x27),
            signatures: vec![EfiSignature {
                signature_type_guid: "c1c41626-504c-4092-aca9-41f936934328".into(),
                owner_guid: "12345678-1234-5678-90ab-cdef12345678".into(),
                data_hex: "03".repeat(32),
            }],
            ..Default::default()
        }],
    };
    let (agent, platform) = agent_with(
        Fake {
            secure_databases: Some(expected.clone()),
            ..Default::default()
        },
        config(dir.path()),
    );
    let mut qga = connect(agent).await;
    assert_eq!(
        qga.execute(&QrabSecureBootDatabases {}).await.unwrap(),
        expected
    );
    let cms = [
        0x30, 0x12, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02, 0xa0, 0x05,
        0x30, 0x03, 0x02, 0x01, 0x01,
    ];
    let mut payload = vec![0xea, 0x07, 11, 14, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    payload.extend_from_slice(&(24u32 + cms.len() as u32).to_le_bytes());
    payload.extend_from_slice(&[0, 2, 0xf1, 0x0e]);
    payload.extend_from_slice(&[
        0x9d, 0xd2, 0xaf, 0x4a, 0xdf, 0x68, 0xee, 0x49, 0x8a, 0xa9, 0x34, 0x7d, 0x37, 0x56, 0x65,
        0xa7,
    ]);
    payload.extend_from_slice(&cms);
    let valid = QrabSecureBootUpdate {
        database_id: "db".into(),
        payload_hex: payload.iter().map(|b| format!("{b:02x}")).collect(),
        append: false,
    };
    let invalid = QrabSecureBootUpdate {
        payload_hex: "private-key-marker".into(),
        ..valid.clone()
    };
    assert!(qga.execute(&invalid).await.is_err());
    assert!(platform.log().is_empty());
    // This fake Platform acknowledges admission only; firmware cryptographic
    // authorization is intentionally not simulated by a protocol fixture.
    assert_eq!(qga.execute(&valid).await.unwrap(), expected);
    assert_eq!(platform.log(), vec!["authenticated db append=false"]);
    qga.execute(&GuestFsfreezeFreeze {}).await.unwrap();
    assert!(qga.execute(&valid).await.is_err());
    assert_eq!(platform.log().len(), 1);
    qga.execute(&GuestFsfreezeThaw {}).await.unwrap();
}
