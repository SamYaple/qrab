//! Supervisor event capture, Control and the control socket against the
//! qapi-protocol mock QMP server. The fixture "QEMU" only sleeps; the test
//! binds the supervisor's private QMP socket itself.
use manager::pod::{
    self,
    socket::{self, Client, FollowFrame},
    Action, Config, Deployment, DiskView,
};
use qapi_protocol::server::{QgaSession, QmpSession};
use serde_json::{json, Value};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
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
async fn mock_qemu(runtime: PathBuf, inject: mpsc::Receiver<(String, Value)>) {
    mock_qemu_with(runtime, inject, serve_guest_agent).await
}

/// [`mock_qemu`] with `agent` serving the guest-agent chardev socket.
async fn mock_qemu_with(
    runtime: PathBuf,
    mut inject: mpsc::Receiver<(String, Value)>,
    agent: impl FnOnce(&Path),
) {
    let session = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            // A relaunched QEMU gets a fresh session: skip one already served.
            let found = std::fs::read_dir(&runtime).ok().and_then(|entries| {
                entries.flatten().find(|e| {
                    e.file_name().to_string_lossy().starts_with("session-")
                        && !e.path().join("qmp.sock").exists()
                })
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
    agent(&session.join("qga.sock"));
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
                    "query-cpus-fast" => json!([{"cpu-index":0,"qom-path":"/machine/private-cpu","qom-type":"max-x86_64-cpu","target":"x86_64","thread-id":123}]),
                    "query-memory-size-summary" => json!({"base-memory":1073741824,"plugged-memory":268435456}),
                    "query-blockstats" => json!([{
                        "device":"disk0","qdev":"/machine/private-storage",
                        "stats":{"rd_bytes":4096,"wr_bytes":8192,"rd_operations":2,"wr_operations":3,
                            "flush_operations":1,"flush_total_time_ns":10,"rd_total_time_ns":20,"wr_total_time_ns":30,"wr_highest_offset":8192},
                        "parent":{"stats":{"rd_bytes":99999,"wr_bytes":99999,"rd_operations":99,"wr_operations":99,
                            "flush_operations":99,"flush_total_time_ns":10,"rd_total_time_ns":20,"wr_total_time_ns":30,"wr_highest_offset":99999}}
                    }]),
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
    let _first_qemu = tokio::spawn(mock_qemu(config.runtime_dir.clone(), injected));
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
    control.action(Action::Nmi).await.unwrap();
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
    assert!(client
        .greeting()
        .operations
        .iter()
        .any(|op| op == "metrics"));
    let metrics = client.metrics().await.unwrap();
    assert_eq!(metrics.logical_processors, 1);
    assert_eq!(metrics.memory_bytes, 1_342_177_280);
    assert_eq!(metrics.block_read_bytes, 4096);
    assert_eq!(metrics.block_write_bytes, 8192);
    assert_eq!(metrics.block_read_operations, 2);
    assert_eq!(metrics.block_flush_operations, 1);
    assert_eq!(metrics.block_write_time_ns, 30);
    assert!(metrics.observed_unix_ms > 0);
    let serialized = serde_json::to_string(&metrics).unwrap();
    assert!(!serialized.contains("private") && !serialized.contains("99999"));
    // Remote readers and in-process callers share the same bounded cache.
    assert_eq!(control.metrics().await.unwrap(), metrics);
    let remote = pod::Backend::Socket(pod::Remote::new(path.clone()));
    assert_eq!(remote.metrics().await.unwrap(), metrics);
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
    // A graceful shutdown leaves the manager serving a powered-off VM.
    let first = wait_for_phase(&control, pod::Phase::Off).await;
    assert!(control
        .events(None, pod::EVENT_PAGE_LIMIT)
        .events
        .iter()
        .any(|e| e.event == "POWERDOWN"));
    let off = wait_for(&control, pod::POWER_EVENT).await;
    assert_eq!(
        off.data,
        json!({"power_state":"Off","reason":"graceful-shutdown"})
    );
    assert_eq!(off.qemu_time, None);
    assert!(first.qemu_pid.is_none() && first.swtpm_pid.is_none() && !first.ready);
    assert!(!runner.is_finished());
    pod::probe(&config, false).unwrap();
    assert!(pod::probe(&config, true).is_err());
    // QEMU requests fail at once with a typed error, in process and over
    // the socket; a repeated power off is already in effect.
    assert!(matches!(
        control.metrics().await,
        Err(manager::Error::PoweredOff)
    ));
    assert!(matches!(
        control.action(Action::Reset).await,
        Err(manager::Error::PoweredOff)
    ));
    let mut off_client = Client::connect(&path).await.unwrap();
    assert!(matches!(
        off_client.action(Action::Pause).await,
        Err(manager::Error::PoweredOff)
    ));
    assert!(matches!(
        Client::connect(&path).await.unwrap().console().await,
        Err(manager::Error::PoweredOff)
    ));
    assert_eq!(off_client.status().await.unwrap().phase, pod::Phase::Off);
    off_client.action(Action::Powerdown).await.unwrap();
    off_client.action(Action::ForceOff).await.unwrap();
    // Power on starts a new QEMU in the same manager and event numbering.
    let epoch = control.events(None, 1).epoch;
    let qemu = tokio::spawn(mock_qemu(config.runtime_dir.clone(), mpsc::channel(1).1));
    let before = control.events(None, 1).latest;
    off_client.action(Action::On).await.unwrap();
    let on = wait_for(&control, pod::POWER_EVENT).await;
    assert!(on.seq > before);
    let running = wait_for_phase(&control, pod::Phase::Running).await;
    assert!(running.qemu_pid.is_some());
    let resumed = wait_for(&control, "RESUME").await;
    assert!(resumed.seq > before);
    assert_eq!(control.events(None, 1).epoch, epoch);
    let power: Vec<_> = control
        .events(Some(before), pod::EVENT_PAGE_LIMIT)
        .events
        .into_iter()
        .filter(|e| e.event == pod::POWER_EVENT)
        .map(|e| e.data["power_state"].clone())
        .collect();
    assert_eq!(power, [json!("PoweringOn"), json!("On")]);
    control.action(Action::Pause).await.unwrap();
    assert!(matches!(
        control.action(Action::On).await,
        Err(manager::Error::Config(_))
    ));
    // Pod shutdown still ends the manager.
    let _ = stop.send(());
    let outcome = tokio::time::timeout(Duration::from_secs(10), runner)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(outcome.reason, "shutdown-requested");
    assert_eq!(control.status().unwrap().phase, pod::Phase::Exited);
    assert!(!path.exists(), "control socket outlived its supervisor");
    qemu.abort();
}

/// The first status record in `phase`, as the supervisor publishes it.
async fn wait_for_phase(control: &pod::Control, phase: pod::Phase) -> pod::Status {
    let mut watch = control.watch_status();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(status) = watch.borrow_and_update().clone() {
                if status.phase == phase {
                    return status;
                }
            }
            watch.changed().await.unwrap();
        }
    })
    .await
    .unwrap_or_else(|_| panic!("phase {phase:?} was not published"))
}

#[tokio::test]
async fn power_cycle_and_force_off_relaunch_qemu_without_ending_the_manager() {
    let (_dir, config) = fixture();
    let (control, _inject, runner) = start(&config);
    wait_for(&control, "RESUME").await;
    let first = wait_for_phase(&control, pod::Phase::Running).await;
    let first_pid = first.qemu_pid.unwrap();
    // Power cycle: QMP quit, then a fresh QEMU with no Off in between.
    tokio::spawn(mock_qemu(config.runtime_dir.clone(), mpsc::channel(1).1));
    control.action(Action::ColdRestart).await.unwrap();
    let cycled = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let status = wait_for_phase(&control, pod::Phase::Running).await;
            if status.qemu_pid.is_some_and(|pid| pid != first_pid) {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("power cycle did not start a new QEMU");
    assert_eq!(cycled.manager_pid, first.manager_pid);
    assert!(
        !std::path::PathBuf::from(format!("/proc/{first_pid}")).exists(),
        "the cycled QEMU was not reaped"
    );
    let reasons: Vec<_> = control
        .events(Some(0), pod::EVENT_PAGE_LIMIT)
        .events
        .into_iter()
        .filter(|e| e.event == pod::POWER_EVENT)
        .map(|e| e.data)
        .collect();
    assert_eq!(
        reasons,
        [
            json!({"power_state":"PoweringOn","reason":"power-cycle"}),
            json!({"power_state":"On","reason":"power-cycle"})
        ]
    );
    // Force off: the VM rests Off, then a power cycle while off powers it on.
    control.action(Action::ForceOff).await.unwrap();
    let off = wait_for_phase(&control, pod::Phase::Off).await;
    assert!(off.qemu_pid.is_none());
    assert_eq!(
        wait_for(&control, pod::POWER_EVENT).await.data["reason"],
        "force-off"
    );
    tokio::spawn(mock_qemu(config.runtime_dir.clone(), mpsc::channel(1).1));
    control.action(Action::ColdRestart).await.unwrap();
    assert!(wait_for_phase(&control, pod::Phase::Running)
        .await
        .qemu_pid
        .is_some());
    assert!(!runner.task.is_finished());
    assert_eq!(runner.shutdown().await.reason, "shutdown-requested");
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

/// A supervisor task and its pod-shutdown trigger.
struct Runner {
    stop: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<manager::Result<pod::Outcome>>,
}

impl Runner {
    fn spawn(config: &Config, receiver: pod::ControlReceiver) -> Self {
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(pod::run_controlled(
            config.clone(),
            async {
                let _ = stopped.await;
            },
            receiver,
        ));
        Self { stop, task }
    }

    /// Pod shutdown: the manager stops QEMU and exits.
    async fn shutdown(self) -> pod::Outcome {
        let _ = self.stop.send(());
        tokio::time::timeout(Duration::from_secs(10), self.task)
            .await
            .expect("manager did not exit on shutdown")
            .unwrap()
            .unwrap()
    }
}

/// Start a supervisor with its mock QEMU; returns the control, an event
/// injector and the supervisor.
fn start(config: &Config) -> (pod::Control, mpsc::Sender<(String, Value)>, Runner) {
    let (control, receiver) = pod::control_channel_for(config, Deployment::default());
    let (inject, injected) = mpsc::channel(8);
    tokio::spawn(mock_qemu(config.runtime_dir.clone(), injected));
    (control, inject, Runner::spawn(config, receiver))
}

#[tokio::test]
async fn sidecar_mirror_resyncs_after_a_manager_restart_and_reuses_connections() {
    let (_dir, config) = fixture();
    let path = socket::socket_path(&config);
    let (control, inject, runner) = start(&config);
    wait_for(&control, "RESUME").await;
    for _ in 0..4 {
        inject
            .send(("RTC_CHANGE".into(), json!({"offset":1,"qom-path":"/rtc"})))
            .await
            .unwrap();
    }
    let remote = pod::Remote::new(path.clone());
    let mirror = tokio::spawn({
        let remote = remote.clone();
        async move { remote.mirror().await }
    });
    let backend = pod::Backend::Socket(remote.clone());
    tokio::time::timeout(Duration::from_secs(5), async {
        while control
            .events(Some(0), 256)
            .events
            .iter()
            .filter(|e| e.event == "RTC_CHANGE")
            .count()
            < 4
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let latest = control.events(None, 1).latest;
    tokio::time::timeout(Duration::from_secs(5), async {
        while backend.events(None, 1).latest < latest {
            backend.wait_events(backend.events(None, 1).latest).await;
        }
    })
    .await
    .unwrap();

    // Sequential requests reuse one pooled connection.
    tokio::time::timeout(Duration::from_secs(3), async {
        while backend.status().await.is_err() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let opened = remote.connections_opened();
    for _ in 0..20 {
        backend.status().await.unwrap();
        backend.agent_status().await.unwrap();
    }
    assert_eq!(remote.connections_opened(), opened);
    assert_eq!(remote.idle_connections(), 1);
    // Concurrent requests open what they need; at most the pool size stays idle.
    let calls: Vec<_> = (0..10)
        .map(|_| {
            let backend = backend.clone();
            tokio::spawn(async move { backend.config_view().await.map(|_| ()) })
        })
        .collect();
    for call in calls {
        call.await.unwrap().unwrap();
    }
    assert!(remote.idle_connections() <= pod::REMOTE_POOL_SIZE);
    assert!(remote.connections_opened() <= opened + 10);
    // A refused request keeps its healthy connection.
    let before = remote.connections_opened();
    assert!(backend
        .agent_update_trigger(vec!["--x".into()])
        .await
        .is_err());
    backend.status().await.unwrap();
    assert_eq!(remote.connections_opened(), before);

    runner.shutdown().await;
    // The mirror holds the old manager's full numbering, through its shutdown.
    let old = control.events(None, 1).latest;
    tokio::time::timeout(Duration::from_secs(5), async {
        while backend.events(None, 1).latest < old {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    // A waiter holding the old numbering wakes when the mirror resets.
    let waiter = tokio::spawn({
        let backend = backend.clone();
        async move { backend.wait_events(old).await }
    });

    // A new manager on the same socket numbers its events from 1 again.
    let (control, _inject, runner) = start(&config);
    let resumed = wait_for(&control, "RESUME").await;
    assert_eq!(resumed.seq, 1);
    tokio::time::timeout(Duration::from_secs(10), waiter)
        .await
        .expect("waiter kept the old numbering")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let page = backend.events(Some(0), 256);
            if page.latest < old && page.events.iter().any(|e| e.event == "RESUME") {
                assert!(
                    !page.events.iter().any(|e| e.event == "RTC_CHANGE"),
                    "{page:?}"
                );
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("mirror did not resync after the restart");
    // Pooled connections to the old manager are replaced transparently.
    backend.status().await.unwrap();
    mirror.abort();
    runner.shutdown().await;
}

/// A wedged guest agent: it completes every handshake and answers pings but
/// never answers any other command. `received` counts those commands.
fn serve_hung_agent(path: &Path, received: Arc<AtomicUsize>) {
    let listener = UnixListener::bind(path).unwrap();
    tokio::spawn(async move {
        while let Ok(mut session) = QgaSession::accept(&listener).await {
            let received = received.clone();
            tokio::spawn(async move {
                if session.handle_sync().await.is_err() {
                    return;
                }
                match session.recv_command().await {
                    Ok((name, _)) if name == "guest-info" => {}
                    _ => return,
                }
                let info = json!({"version":"11.1.3","supported_commands":[]});
                if session.send_return(info).await.is_err() {
                    return;
                }
                while let Ok((name, _)) = session.recv_command().await {
                    if name == "guest-ping" {
                        let _ = session.send_return(json!({})).await;
                    } else {
                        received.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
        }
    });
}

/// A running supervisor whose connected guest agent never answers a read.
async fn start_with_hung_agent(
    config: &Config,
) -> (
    pod::Control,
    mpsc::Sender<(String, Value)>,
    Arc<AtomicUsize>,
    Runner,
) {
    let (control, receiver) = pod::control_channel_for(config, Deployment::default());
    let (inject, injected) = mpsc::channel(8);
    let received = Arc::new(AtomicUsize::new(0));
    tokio::spawn(mock_qemu_with(config.runtime_dir.clone(), injected, {
        let received = received.clone();
        move |path: &Path| serve_hung_agent(path, received)
    }));
    let runner = Runner::spawn(config, receiver);
    wait_for(&control, "RESUME").await;
    tokio::time::timeout(Duration::from_secs(10), async {
        while !control.agent_status().await.unwrap().connected {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("guest agent never connected");
    (control, inject, received, runner)
}

/// A raw control connection past its greeting; panics on a `busy` refusal.
async fn open(path: &Path) -> BufReader<UnixStream> {
    let mut stream = BufReader::new(UnixStream::connect(path).await.unwrap());
    let greeting = read(&mut stream).await;
    assert_eq!(greeting["protocol"], "qrab-control", "{greeting}");
    stream
}

/// Every control connection slot, once all of them are free within `within`.
async fn all_connections(path: &Path, within: Duration) -> Vec<Client> {
    tokio::time::timeout(within, async {
        loop {
            let mut held = Vec::new();
            while held.len() < socket::MAX_CONNECTIONS {
                match Client::connect(path).await {
                    Ok(client) => held.push(client),
                    Err(_) => break,
                }
            }
            if held.len() == socket::MAX_CONNECTIONS {
                return held;
            }
            drop(held);
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("abandoned guest reads still hold control connections")
}

#[tokio::test]
async fn a_disconnected_client_abandons_its_pending_guest_read_at_once() {
    let (_dir, config) = fixture();
    let path = socket::socket_path(&config);
    let (_control, _inject, _received, runner) = start_with_hung_agent(&config).await;
    // Every connection slot waits on the same unanswered guest read: none is
    // turned away as busy by the agent queue.
    let mut pending = Vec::new();
    for id in 0..socket::MAX_CONNECTIONS {
        let mut stream = open(&path).await;
        let request = format!("{{\"v\":1,\"id\":{id},\"op\":\"agent_freeze_status\"}}\n");
        stream
            .get_mut()
            .write_all(request.as_bytes())
            .await
            .unwrap();
        pending.push(stream);
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    for stream in &mut pending {
        let mut line = String::new();
        let answered =
            tokio::time::timeout(Duration::from_millis(10), stream.read_line(&mut line)).await;
        assert!(answered.is_err(), "answered: {line}");
    }
    let mut refused = BufReader::new(UnixStream::connect(&path).await.unwrap());
    assert_eq!(read(&mut refused).await["error"]["code"], "busy");
    // The clients give up; their reads are abandoned within a second, long
    // before the agent worker's own read deadline.
    drop(pending);
    let mut held = all_connections(&path, Duration::from_secs(1)).await;
    for client in &mut held {
        client.status().await.unwrap();
    }
    drop(held);
    runner.shutdown().await;
}

#[tokio::test]
async fn polling_a_hung_agent_never_exhausts_control_connections() {
    let (_dir, config) = fixture();
    let path = socket::socket_path(&config);
    let (_control, _inject, received, runner) = start_with_hung_agent(&config).await;
    // A Redfish-like poller: every 100 ms a new client asks for the boot state
    // and gives up after 1 s. None is ever refused a connection.
    let clients: Vec<_> = (0..40u64)
        .map(|i| {
            let path = path.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100 * i)).await;
                let mut stream = open(&path).await;
                stream
                    .get_mut()
                    .write_all(b"{\"v\":1,\"id\":1,\"op\":\"agent_boot\"}\n")
                    .await
                    .unwrap();
                let mut line = String::new();
                // Answered only when the shared read's deadline expires, never
                // turned away because the agent queue is full.
                let _ =
                    tokio::time::timeout(Duration::from_secs(1), stream.read_line(&mut line)).await;
                assert!(!line.contains("busy"), "{line}");
            })
        })
        .collect();
    for client in clients {
        client.await.unwrap();
    }
    // The polls shared their guest reads instead of queueing one each.
    let reads = received.load(Ordering::SeqCst);
    assert!((1..=2).contains(&reads), "{reads} guest reads");
    // Every slot is free again, and each answers, including a console request.
    let mut held = all_connections(&path, Duration::from_secs(1)).await;
    for client in &mut held {
        client.status().await.unwrap();
    }
    let console = held.pop().unwrap().console().await.unwrap_err().to_string();
    assert!(
        console.contains("graphical console is disabled"),
        "{console}"
    );
    drop(held);
    runner.shutdown().await;
}
