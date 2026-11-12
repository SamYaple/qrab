//! Supervisor event capture, Control and the control socket against the
//! qapi-protocol mock QMP server. The fixture "QEMU" only sleeps; the test
//! binds the supervisor's private QMP socket itself.
use manager::pod::{
    self,
    socket::{self, Client, FollowFrame},
    Action, Config, Deployment, DiskView,
};
use qapi_protocol::server::QmpSession;
use serde_json::{json, Value};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::mpsc,
};

const PORTAL: &str = "192.0.2.10:3260";
const TARGET: &str = "iqn.2026-11.com.yapplesauce:qrab.test.disk";
const IMAGE: &str =
    "zot.yapplesauce.com/qrab/manager@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn fixture() -> (tempfile::TempDir, Config) {
    let dir = tempfile::Builder::new()
        .prefix("qc")
        .tempdir_in("/tmp")
        .unwrap();
    let kernel = dir.path().join("kernel");
    std::fs::write(&kernel, []).unwrap();
    let qemu = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sleep.sh");
    let config = serde_json::from_value(json!({
        "name":"events-vm","qemu_binary":qemu,"kernel":kernel,"runtime_dir":dir.path().join("run"),
        "startup_timeout_secs":5,"shutdown_grace_secs":0,"control_socket":true
    }))
    .unwrap();
    (dir, config)
}

/// Minimal QEMU: replies to every command, emits the event QEMU would, and
/// forwards injected events.
async fn mock_qemu(runtime: PathBuf, mut inject: mpsc::Receiver<(String, Value)>) {
    let session = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let found = std::fs::read_dir(&runtime).ok().and_then(|entries| {
                entries
                    .flatten()
                    .find(|e| e.file_name().to_string_lossy().starts_with("session-"))
            });
            if let Some(entry) = found {
                return entry.path();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let listener = UnixListener::bind(session.join("qmp.sock")).unwrap();
    serve_guest_agent(&session.join("qga.sock"));
    let mut session = QmpSession::accept(&listener).await.unwrap();
    session.send_greeting(11, 1, 3).await.unwrap();
    session.negotiate().await.unwrap();
    let (mut reader, mut writer) = session.split();
    let (commands_tx, mut commands) = mpsc::channel(8);
    let read = tokio::spawn(async move {
        while let Ok(command) = reader.recv_command_full().await {
            if commands_tx.send(command).await.is_err() {
                break;
            }
        }
    });
    let mut running = false;
    // Set by an injected FLOOD_NEXT_STOP: the next `stop` is slow and is
    // preceded by a burst far beyond QMP's client event queue.
    let mut flood = 0;
    loop {
        tokio::select! {
            Some((name, data)) = inject.recv() => if name == "FLOOD_NEXT_STOP" {
                flood = data["count"].as_u64().unwrap();
            } else {
                writer.send_event(&name, data).await.unwrap()
            },
            command = commands.recv() => {
                let Some(command) = command else { break };
                if command.name == "stop" && flood > 0 {
                    for offset in 0..std::mem::take(&mut flood) {
                        writer.send_event("RTC_CHANGE", json!({"offset":offset,"qom-path":"/machine/unattached/device[0]"})).await.unwrap();
                    }
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                }
                let reply = match command.name.as_str() {
                    "query-status" => json!({"running":running,"status":if running {"running"} else {"paused"}}),
                    _ => json!({}),
                };
                writer.send_return_with_id(reply, command.id).await.unwrap();
                let event = match command.name.as_str() {
                    "cont" => { running = true; Some(("RESUME", json!({}))) }
                    "stop" => { running = false; Some(("STOP", json!({}))) }
                    "system_reset" => Some(("RESET", json!({"guest":false,"reason":"host-qmp-system-reset"}))),
                    "system_powerdown" => Some(("POWERDOWN", json!({}))),
                    _ => None,
                };
                if let Some((name, data)) = event {
                    writer.send_event(name, data).await.unwrap();
                }
            }
        }
    }
    read.abort();
}

/// One filesystem whose freeze/thaw only flips a flag.
#[derive(Default)]
struct Guest(std::sync::Mutex<bool>);
impl qrab_guest_agent::sys::Platform for Guest {
    fn mounts(&self) -> std::io::Result<Vec<qrab_guest_agent::sys::Mount>> {
        Ok(vec![qrab_guest_agent::sys::Mount {
            dir: "/".into(),
            fstype: "ext4".into(),
            device: (253, 1),
        }])
    }
    fn freeze(&self, _: &qrab_guest_agent::sys::Mount) -> std::io::Result<()> {
        *self.0.lock().unwrap() = true;
        Ok(())
    }
    fn thaw(&self, _: &qrab_guest_agent::sys::Mount) -> std::io::Result<()> {
        *self.0.lock().unwrap() = false;
        Ok(())
    }
    fn trim(&self, _: &qrab_guest_agent::sys::Mount, _: u64) -> std::io::Result<(u64, u64)> {
        Ok((0, 0))
    }
    fn shutdown(&self, _: qrab_guest_agent::sys::ShutdownMode) -> std::io::Result<()> {
        Ok(())
    }
}

/// The production agent library serving QGA where QEMU's chardev would be.
fn serve_guest_agent(path: &Path) {
    let listener = std::os::unix::net::UnixListener::bind(path).unwrap();
    let hooks = std::env::temp_dir().join(format!("qrab-hook-{}", std::process::id()));
    let agent = qrab_guest_agent::Agent::new(
        qrab_guest_agent::Config {
            update: qrab_guest_agent::HookPolicy {
                path: hooks.join("update-hook"),
                require_root: false,
                timeout: Duration::from_secs(5),
            },
            ..Default::default()
        },
        std::sync::Arc::new(Guest::default()),
    );
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let _ = agent.serve(&stream, &stream);
        }
    });
}

async fn wait_for(control: &pod::Control, name: &str) -> pod::VmEvent {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let page = control.events(None, pod::EVENT_PAGE_LIMIT);
            if let Some(event) = page.events.iter().rev().find(|e| e.event == name) {
                return event.clone();
            }
            control.wait_events(page.latest).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{name} event was not recorded"))
}

#[tokio::test]
async fn supervisor_records_qmp_events_and_serves_them_over_control_and_socket() {
    let (_dir, mut config) = fixture();
    config.iscsi_disks = serde_json::from_value(json!([{
        "id":"root","portal":PORTAL,"target":TARGET,"initiator_name":"iqn.2026-11.com.yapplesauce:qrab.vm.x",
        "username":"chap-user","password_file":"/run/qrab-auth/root/password"
    }]))
    .unwrap();
    let deployment = Deployment {
        node: Some("node-a".into()),
        image: Some(IMAGE.into()),
        ..Deployment::default()
    };
    let (control, receiver) = pod::control_channel_for(&config, deployment);
    // The view exists before the supervisor starts; iSCSI is not validated here.
    let view = control.config_view().unwrap();
    assert_eq!(
        view.deployment.as_ref().unwrap().node.as_deref(),
        Some("node-a")
    );
    config.iscsi_disks.clear();
    let (inject, injected) = mpsc::channel(8);
    let qemu = tokio::spawn(mock_qemu(config.runtime_dir.clone(), injected));
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let runner = tokio::spawn(pod::run_controlled(
        config.clone(),
        async {
            let _ = stopped.await;
        },
        receiver,
    ));

    // The supervisor resumes the paused VM; QEMU reports RESUME.
    let resume = wait_for(&control, "RESUME").await;
    assert_eq!(resume.seq, 1);
    assert_eq!(
        resume.qemu_time,
        Some(pod::QemuTime {
            seconds: 0,
            microseconds: 0
        })
    );
    control.action(Action::Pause).await.unwrap();
    wait_for(&control, "STOP").await;
    control.action(Action::Reset).await.unwrap();
    let reset = wait_for(&control, "RESET").await;
    let typed = reset.decode::<qapi::events::ResetEvent>().unwrap().unwrap();
    assert_eq!(typed.guest, Some(false));
    inject
        .send((
            "BLOCK_IO_ERROR".into(),
            json!({"device":"","node-name":"iscsi-root","operation":"write","action":"report","reason":"Input/output error"}),
        ))
        .await
        .unwrap();
    let error = wait_for(&control, "BLOCK_IO_ERROR").await;
    assert_eq!(
        error
            .decode::<qapi::events::BlockIoErrorEvent>()
            .unwrap()
            .unwrap()
            .node_name
            .as_deref(),
        Some("iscsi-root")
    );
    // The supervisor replaced the pre-start view with the configuration it runs.
    assert!(control.config_view().unwrap().disks.is_empty());

    // Socket: private directory, 0600 socket, greeting and typed operations.
    let path = socket::socket_path(&config);
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(path.parent().unwrap()), 0o700);
    assert_eq!(mode(&path), 0o600);
    let mut client = Client::connect(&path).await.unwrap();
    assert_eq!(client.greeting().version, socket::VERSION);
    assert_eq!(client.greeting().vm, "events-vm");
    let page = client.events(Some(0), 2).await.unwrap();
    assert_eq!(
        page.events
            .iter()
            .map(|e| e.event.as_str())
            .collect::<Vec<_>>(),
        ["RESUME", "STOP"]
    );
    assert_eq!(page.next, 2);
    assert_eq!(
        client.events(None, 1).await.unwrap().events[0].event,
        "BLOCK_IO_ERROR"
    );
    let full = client.config(false).await.unwrap();
    assert!(!full.redacted && full.paths.is_some());
    let redacted = client.config(true).await.unwrap();
    assert!(redacted.redacted && redacted.paths.is_none() && redacted.state_id.is_none());
    // One wire form: the flat `agent_*` operations. Connected-agent checks
    // follow below and the not-connected ones in the protocol test.
    let operations = &client.greeting().operations;
    assert!(operations.iter().any(|op| op == "agent_ping"));
    assert!(!operations.iter().any(|op| op == "agent"));
    assert!(client.agent_status().await.unwrap().enabled);
    let error = client
        .agent_update_trigger(vec!["--x".into()])
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("update arguments"), "{error}");
    tokio::time::timeout(Duration::from_secs(3), async {
        while client.status().await.is_err() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    // A follower sees events recorded after its cursor, in order.
    let latest = control.events(None, 1).latest;
    let mut follower = Client::connect(&path)
        .await
        .unwrap()
        .follow(Some(latest - 1))
        .await
        .unwrap();
    assert_eq!(next(&mut follower).await.event, "BLOCK_IO_ERROR");
    client.action(Action::Resume).await.unwrap();
    assert_eq!(next(&mut follower).await.event, "RESUME");
    // A sidecar backend mirrors events with the manager's sequence numbers.
    let remote = pod::Remote::new(path.clone());
    let mirror = tokio::spawn({
        let remote = remote.clone();
        async move { remote.mirror().await }
    });
    let backend = pod::Backend::Socket(remote);
    tokio::time::timeout(Duration::from_secs(5), async {
        while backend.events(None, 1).latest < control.events(None, 1).latest {
            backend.wait_events(backend.events(None, 1).latest).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        backend.events(Some(0), 256).events,
        control.events(Some(0), 256).events
    );
    backend.probe(false).await.unwrap();
    assert_eq!(
        backend.status().await.unwrap().qemu_version,
        Some((11, 1, 3))
    );
    assert_eq!(backend.config_view().await.unwrap().name, "events-vm");
    backend.action(Action::Pause).await.unwrap();
    let paused = wait_for(&control, "STOP").await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while backend.events(None, 1).latest < paused.seq {
            backend.wait_events(paused.seq - 1).await;
        }
    })
    .await
    .unwrap();
    assert!(backend.console().await.is_err());
    mirror.abort();
    // Guest-agent operations over the socket, against the real agent.
    tokio::time::timeout(Duration::from_secs(10), async {
        while !control.agent_status().await.unwrap().connected {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("guest agent never connected");
    let agent = client.agent_status().await.unwrap();
    assert!(agent.enabled && agent.connected, "{agent:?}");
    assert_eq!(agent.version.as_deref(), Some("11.1.3"));
    assert_eq!(client.agent_freeze(None).await.unwrap(), 1);
    assert_eq!(
        client.agent_freeze_status().await.unwrap(),
        pod::FreezeState::Frozen
    );
    assert!(
        client.agent_update_trigger(vec![]).await.is_err(),
        "trigger while frozen"
    );
    assert_eq!(client.agent_thaw().await.unwrap(), 1);
    assert!(!client.agent_update_status().await.unwrap().hook_installed);
    let refused = client
        .agent_update_trigger(vec!["$(reboot)".into()])
        .await
        .unwrap_err()
        .to_string();
    assert!(refused.contains("update arguments"), "{refused}");
    assert!(client
        .greeting()
        .operations
        .iter()
        .any(|op| op == "agent_update_trigger"));
    // The CLI reaches the same operations; its output is unchanged JSON.
    let config_file = config.runtime_dir.with_file_name("vm.json");
    std::fs::write(&config_file, serde_json::to_vec(&config).unwrap()).unwrap();
    let cli = move |args: &[&str]| {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_qrab-manager"))
            .arg("agent")
            .arg(args[0])
            .arg("--config")
            .arg(&config_file)
            .args(&args[1..])
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    let cli_status = tokio::task::spawn_blocking(move || {
        (
            cli(&["status"]),
            cli(&["ping"]),
            cli(&["freeze-status"]),
            cli(&["freeze", "/"]),
            cli(&["thaw"]),
        )
    })
    .await
    .unwrap();
    assert_eq!(cli_status.0["connected"], true);
    assert_eq!(cli_status.1, Value::Null);
    assert_eq!(cli_status.2, "thawed");
    assert_eq!(
        (cli_status.3.clone(), cli_status.4.clone()),
        (json!(1), json!(1))
    );
    // A console request on a console-less VM is refused, not a raw socket.
    assert!(Client::connect(&path)
        .await
        .unwrap()
        .console()
        .await
        .is_err());

    // A burst during a slow dispatch is not lost: events drain on their own
    // task while the supervisor waits for `stop`.
    const FLOOD: u64 = 1500;
    let before = control.events(None, 1).latest;
    inject
        .send(("FLOOD_NEXT_STOP".into(), json!({"count":FLOOD})))
        .await
        .unwrap();
    control.action(Action::Pause).await.unwrap();
    let flooded = wait_for(&control, "STOP").await;
    let recorded = flooded.seq - before - 1;
    // Any loss must become visible; the dropped count is refreshed after
    // each dispatch and on the heartbeat.
    let dropped = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let dropped = control.events(None, 1).qmp_dropped;
            if recorded == FLOOD || dropped > 0 {
                return dropped;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{recorded} of {FLOOD} recorded and no loss reported"));
    assert_eq!((recorded, dropped), (FLOOD, 0));
    // Status comes from memory once the supervisor publishes.
    std::fs::remove_file(config.runtime_dir.join("status.json")).unwrap();
    assert_eq!(control.status().unwrap().qemu_version, Some((11, 1, 3)));

    client.action(Action::Powerdown).await.unwrap();
    // While the supervisor stops QEMU, requests fail at once with a typed
    // error instead of waiting out the 5 s request timeout.
    let started = std::time::Instant::now();
    assert!(matches!(
        control.action(Action::Pause).await,
        Err(manager::Error::ShuttingDown)
    ));
    let mut late = Client::connect(&path).await.unwrap();
    let error = late.action(Action::Reset).await.unwrap_err().to_string();
    assert!(error.contains("shutting down"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    let outcome = tokio::time::timeout(Duration::from_secs(10), runner)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(outcome.reason, "shutdown-requested");
    assert!(control
        .events(None, pod::EVENT_PAGE_LIMIT)
        .events
        .iter()
        .any(|e| e.event == "POWERDOWN"));
    assert!(!path.exists(), "control socket outlived its supervisor");
    let _ = stop.send(());
    qemu.abort();
}

#[test]
fn redacted_view_removes_storage_identity_paths_node_and_image() {
    let (_dir, mut config) = fixture();
    config.iscsi_disks = serde_json::from_value(json!([{
        "id":"root","portal":PORTAL,"target":TARGET,"initiator_name":"iqn.2026-11.com.yapplesauce:qrab.vm.x",
        "username":"chap-user","password_file":"/run/qrab-auth/root/password","boot_index":1
    }]))
    .unwrap();
    config.kernel_args = "console=ttyS0 secret-ish".into();
    let deployment = Deployment {
        node: Some("node-a".into()),
        pod: Some("events-vm".into()),
        namespace: Some("qrab".into()),
        image: Some(IMAGE.into()),
    };
    let (control, _receiver) = pod::control_channel_for(&config, deployment);
    let full = control.config_view().unwrap();
    let full_json = serde_json::to_string(&full).unwrap();
    let redacted = serde_json::to_string(&full.redacted()).unwrap();
    for sensitive in [
        PORTAL,
        TARGET,
        "chap-user",
        "/run/qrab-auth/root/password",
        "node-a",
        IMAGE,
        "sha256",
        "secret-ish",
        "sleep.sh",
        "/run\"",
    ] {
        assert!(
            !redacted.contains(sensitive),
            "{sensitive} leaked: {redacted}"
        );
    }
    for sensitive in [
        PORTAL,
        TARGET,
        "chap-user",
        "/run/qrab-auth/root/password",
        "node-a",
        IMAGE,
    ] {
        assert!(
            full_json.contains(sensitive),
            "{sensitive} missing from admin view"
        );
    }
    let view = full.redacted();
    assert!(matches!(
        &view.disks[..],
        [DiskView::Iscsi { id, boot_index: Some(1), target: None, .. }] if id == "root"
    ));
    assert_eq!(
        (view.name.as_str(), view.cpus, view.guest_network),
        ("events-vm", 1, false)
    );
}

#[tokio::test]
async fn socket_protocol_is_versioned_bounded_and_survives_bad_requests() {
    let (_dir, config) = fixture();
    let (control, receiver) = pod::control_channel_for(&config, Deployment::default());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    // No mock QMP: the supervisor stays in startup, but the socket serves.
    let mut startup = config.clone();
    startup.startup_timeout_secs = 30;
    let runner = tokio::spawn(pod::run_controlled(
        startup,
        async {
            let _ = stopped.await;
        },
        receiver,
    ));
    let path = socket::socket_path(&config);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut stream = BufReader::new(UnixStream::connect(&path).await.unwrap());
    let greeting = read(&mut stream).await;
    assert_eq!(greeting["protocol"], "qrab-control");
    assert_eq!(greeting["version"], 1);
    // Agent status never waits on the supervisor queue: QEMU has not started,
    // and agent commands are refused at once instead of timing out.
    let started = std::time::Instant::now();
    stream
        .get_mut()
        .write_all(b"{\"v\":1,\"id\":3,\"op\":\"agent_status\"}\n")
        .await
        .unwrap();
    let reply = read(&mut stream).await;
    assert_eq!(reply["id"], 3);
    assert_eq!(
        (
            reply["result"]["enabled"].as_bool(),
            reply["result"]["connected"].as_bool()
        ),
        (Some(true), Some(false))
    );
    for request in [
        "{\"v\":1,\"id\":4,\"op\":\"agent_freeze\",\"mountpoints\":[\"/\"]}\n",
        "{\"v\":1,\"id\":5,\"op\":\"agent_thaw\"}\n",
        "{\"v\":1,\"id\":6,\"op\":\"agent_update_trigger\",\"args\":[\"release=2\"]}\n",
        "{\"v\":1,\"id\":7,\"op\":\"agent_update_status\"}\n",
        "{\"v\":1,\"id\":8,\"op\":\"agent_freeze_status\"}\n",
    ] {
        stream
            .get_mut()
            .write_all(request.as_bytes())
            .await
            .unwrap();
        let reply = read(&mut stream).await;
        assert_eq!(reply["error"]["code"], "unavailable", "{request}");
        assert!(reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("not connected"));
    }
    stream
        .get_mut()
        .write_all(b"{\"v\":1,\"id\":9,\"op\":\"agent_ping\"}\n")
        .await
        .unwrap();
    let reply = read(&mut stream).await;
    assert!(reply["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not connected"));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    for (request, code) in [
        (
            "{\"v\":2,\"id\":7,\"op\":\"status\"}\n",
            "unsupported_version",
        ),
        ("not json\n", "bad_request"),
        (
            "{\"v\":1,\"id\":8,\"op\":\"qmp\",\"execute\":\"quit\"}\n",
            "bad_request",
        ),
        (
            "{\"v\":1,\"id\":9,\"op\":\"action\",\"action\":\"quit\"}\n",
            "bad_request",
        ),
        // The former nested guest-agent form is no longer accepted.
        (
            "{\"v\":1,\"id\":10,\"op\":\"agent\",\"agent\":{\"command\":\"ping\"}}\n",
            "bad_request",
        ),
    ] {
        stream
            .get_mut()
            .write_all(request.as_bytes())
            .await
            .unwrap();
        let reply = read(&mut stream).await;
        assert_eq!(reply["error"]["code"], code, "{request}");
    }
    // Wire format of a successful event read, with an echoed request ID.
    assert_eq!(control.events(Some(0), 5).latest, 0);
    stream
        .get_mut()
        .write_all(b"{\"v\":1,\"id\":10,\"op\":\"events\",\"after\":0,\"limit\":5}\n")
        .await
        .unwrap();
    let reply = read(&mut stream).await;
    assert_eq!(reply["id"], 10);
    assert!(reply["result"]["events"].as_array().is_some());
    // An oversized request closes the connection after a typed error.
    let oversized = vec![b'x'; socket::MAX_REQUEST + 2];
    let _ = stream.get_mut().write_all(&oversized).await;
    let reply = read(&mut stream).await;
    assert_eq!(reply["error"]["code"], "frame_too_large");
    let mut rest = String::new();
    assert_eq!(stream.read_line(&mut rest).await.unwrap(), 0);

    // Connections beyond the bound are refused with `busy`.
    let mut held = Vec::new();
    for _ in 0..socket::MAX_CONNECTIONS {
        let mut stream = BufReader::new(UnixStream::connect(&path).await.unwrap());
        assert_eq!(read(&mut stream).await["protocol"], "qrab-control");
        held.push(stream);
    }
    let mut refused = BufReader::new(UnixStream::connect(&path).await.unwrap());
    assert_eq!(read(&mut refused).await["error"]["code"], "busy");
    drop(held);

    let _ = stop.send(());
    let _ = tokio::time::timeout(Duration::from_secs(10), runner)
        .await
        .unwrap();
    drop(control);
}

async fn read(stream: &mut BufReader<UnixStream>) -> Value {
    let mut line = String::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    serde_json::from_str(&line).unwrap()
}

async fn next(follower: &mut socket::Follower) -> pod::VmEvent {
    match tokio::time::timeout(Duration::from_secs(5), follower.next()).await {
        Ok(Ok(FollowFrame::Event(event))) => event,
        other => panic!("unexpected follow frame {other:?}"),
    }
}
