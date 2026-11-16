use super::{
    control::PowerStop,
    iscsi::Binding,
    migration::{self, Pending, Receiving, Sent},
    persistent::{PersistentState, StateLease},
    process::ManagedChild,
    state::{publish, start_ticks},
    tpm::Swtpm,
    Config, Phase, Status,
};
use crate::{Error, Result};
use qapi::commands::{Cont, QueryStatus, Quit, SystemPowerdown};
use qapi_protocol::{ClientOptions, Qmp};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    future::Future,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    process::{Child, Command},
    sync::{mpsc, oneshot},
    time::timeout,
};

async fn next_migration(outgoing: &mut Option<mpsc::Receiver<Pending>>) -> Option<Pending> {
    match outgoing {
        Some(receiver) => receiver.recv().await,
        None => std::future::pending().await,
    }
}

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub reason: String,
    pub exit_code: Option<i32>,
    pub shutdown_method: String,
}

/// How one QEMU process (a power-on cycle) ended.
enum Cycle {
    /// The manager exits: pod shutdown, or a completed outgoing migration.
    Exit(Outcome),
    /// QEMU stopped cleanly for this reason; the VM rests powered off.
    Off(&'static str),
    /// Power cycle: start a fresh QEMU at once.
    Restart,
}

/// Why the supervision loop of a running QEMU ended.
enum Ended {
    /// QEMU exited by itself.
    Exited(ExitStatus),
    /// The pod is shutting down.
    Shutdown,
    /// A control request stops this QEMU.
    Stop(PowerStop),
}

async fn reap_after_kill(child: &mut Child) -> Result<ExitStatus> {
    child.start_kill()?;
    timeout(Duration::from_secs(2), child.wait())
        .await
        .map_err(|_| Error::Spawn("QEMU did not reap after SIGKILL".into()))?
        .map_err(Error::Io)
}

async fn stop_child(
    child: &mut Child,
    qmp: Option<&Qmp>,
    grace: Duration,
) -> Result<(ExitStatus, &'static str)> {
    if let Some(status) = child.try_wait()? {
        return Ok((status, "already-exited"));
    }
    if let Some(qmp) = qmp {
        let _ = qmp.execute(&SystemPowerdown {}).await;
    } else if let Some(pid) = child.id() {
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid as i32),
            nix::sys::signal::Signal::SIGTERM,
        );
    }
    if let Ok(result) = timeout(grace, child.wait()).await {
        return Ok((result?, "guest-or-term"));
    }
    if let Some(qmp) = qmp {
        let _ = qmp.execute(&Quit {}).await;
        if let Ok(result) = timeout(Duration::from_secs(2), child.wait()).await {
            return Ok((result?, "qmp-quit"));
        }
    }
    Ok((reap_after_kill(child).await?, "sigkill"))
}

/// Reap QEMU after QMP `quit` was sent, killing it if it does not exit soon.
async fn reap_after_quit(child: &mut Child) -> Result<(ExitStatus, &'static str)> {
    if let Ok(result) = timeout(Duration::from_secs(2), child.wait()).await {
        return Ok((result?, "qmp-quit"));
    }
    Ok((reap_after_kill(child).await?, "sigkill"))
}

/// Own one QEMU child until it exits or shutdown resolves. kill_on_drop covers
/// cancellation of this future; explicit paths always wait and reap the child.
pub async fn run(config: Config, shutdown: impl Future<Output = ()>) -> Result<Outcome> {
    let (_control, receiver) = super::control_channel();
    run_controlled(config, shutdown, receiver).await
}

/// Supervise a VM with a bounded in-process control channel for its BMC.
///
/// The manager is the VM's init: the runtime lock, control socket and
/// persistent-state lease live as long as this call, across every QEMU it
/// starts. A requested or guest-initiated power off leaves the VM `Off` until
/// a power-on request; a power cycle starts a fresh QEMU at once. Pod
/// shutdown, a completed outgoing migration or a QEMU failure ends the call.
pub async fn run_controlled(
    mut config: Config,
    shutdown: impl Future<Output = ()>,
    mut control: super::ControlReceiver,
) -> Result<Outcome> {
    config.validate()?;
    config.status_sink = control.status_sink();
    std::fs::create_dir_all(&config.runtime_dir)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(config.runtime_dir.join("owner.lock"))?;
    let _lock = nix::fcntl::Flock::lock(lock, nix::fcntl::FlockArg::LockExclusiveNonblock)
        .map_err(|_| Error::Config("another manager owns this runtime directory".into()))?;
    control.set_config(&config);
    // Declared after the lock, so the socket server stops before it is released.
    let _control_socket = match config.control_socket {
        true => Some(super::socket::spawn(&config, control.control())?),
        false => None,
    };
    // A target learns the source's machine type and retained state before it
    // prepares anything of its own.
    let mut receiving = match &config.incoming {
        Some(incoming) => {
            let receiving = migration::connect(&config, incoming).await?;
            config.machine = Some(receiving.offer.machine.clone());
            // The source's live NICs, hot-plugged ones and slots included.
            if let Some(nics) = &receiving.offer.nics {
                config.nics = nics.clone();
            }
            // The relay port is part of the device model the stream carries.
            config.spire_relay.enabled = receiving.offer.spire_relay;
            // So is the qrab-identity disk; its files come with the offer.
            config.guest_identity.enabled = receiving.offer.guest_identity.is_some();
            Some(receiving)
        }
        None => None,
    };
    let mut persistent = PersistentState::prepare(&config)?;
    let _firmware_lease = control.firmware_lease(persistent.as_ref().map(PersistentState::lease));
    if let (Some(state), Some(receiving)) = (persistent.as_mut(), &receiving) {
        state.retain(&receiving.offer.iscsi_disks)?;
    }
    let retained = persistent
        .as_ref()
        .map(PersistentState::bindings)
        .unwrap_or_default();

    let mut state = Status {
        phase: Phase::Starting,
        ready: false,
        manager_pid: std::process::id(),
        manager_start_ticks: start_ticks(std::process::id())?,
        qemu_pid: None,
        qemu_start_ticks: None,
        qemu_version: None,
        swtpm_pid: None,
        swtpm_start_ticks: None,
        run_state: None,
        observed_unix_ms: 0,
        exit_code: None,
        detail: None,
        resize: super::Resize::new(&config),
        relaunch: false,
    };
    let mut shutdown = Box::pin(shutdown);
    let base = config;
    // Why the next QEMU starts; `None` for the pod's first one.
    let mut relaunch: Option<&'static str> = None;
    // Opened when QEMU first runs, then kept across power cycles so a VM
    // powered off in place still refuses migrations instead of going silent.
    let mut outgoing = None;
    loop {
        let cycle = power_on(
            &base,
            &mut shutdown,
            &mut control,
            &mut state,
            &mut persistent,
            (receiving.take(), &retained, &mut outgoing),
            relaunch,
        )
        .await;
        match cycle {
            Ok(Cycle::Exit(outcome)) => {
                state.ready = false;
                publish(&base, &mut state)?;
                return Ok(outcome);
            }
            Ok(Cycle::Restart) => relaunch = Some("power-cycle"),
            Ok(Cycle::Off(reason)) => {
                if !powered_off(
                    &base,
                    &mut shutdown,
                    &mut control,
                    &mut state,
                    &mut outgoing,
                    reason,
                )
                .await?
                {
                    return Ok(Outcome {
                        reason: "shutdown-requested".into(),
                        exit_code: state.exit_code,
                        shutdown_method: "powered-off".into(),
                    });
                }
                relaunch = Some("power-on");
            }
            Err(error) => {
                control.set_stopping();
                state.phase = Phase::Failed;
                state.detail = Some(error.to_string());
                state.ready = false;
                publish(&base, &mut state)?;
                return Err(error);
            }
        }
    }
}

/// One power-on cycle: a fresh session directory, staged firmware and Secure
/// Boot settings applied, swtpm, then QEMU until it ends. A migration target
/// receives its guest only in the first cycle.
async fn power_on(
    base: &Config,
    shutdown: &mut (impl Future<Output = ()> + Unpin),
    control: &mut super::ControlReceiver,
    state: &mut Status,
    persistent: &mut Option<PersistentState>,
    migration: (Option<Receiving>, &BTreeMap<String, Binding>, &mut Outgoing),
    relaunch: Option<&'static str>,
) -> Result<Cycle> {
    let (receiving, retained, outgoing) = migration;
    let mut config = base.clone();
    if let Some(reason) = relaunch {
        config.incoming = None;
        // NICs and the seed follow the VM ConfigMap across in-place power cycles.
        if let Some(fresh) = config.source.as_deref().and_then(|p| Config::read(p).ok()) {
            config.nics = fresh.nics;
            config.seed = fresh.seed;
        }
        control.set_powering_on();
        control.record_power("PoweringOn", reason);
        eprintln!(
            "{}",
            serde_json::json!({"event":"qemu_relaunch","reason":reason})
        );
    }
    *state = Status {
        phase: Phase::Starting,
        ready: false,
        qemu_pid: None,
        qemu_start_ticks: None,
        qemu_version: None,
        swtpm_pid: None,
        swtpm_start_ticks: None,
        run_state: None,
        exit_code: None,
        detail: None,
        relaunch: relaunch.is_some(),
        ..state.clone()
    };
    let session = tempfile::Builder::new()
        .prefix("session-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(&config.runtime_dir)?;
    let socket = session.path().join("qmp.sock");
    let console = config.console.then(|| session.path().join("vnc.sock"));
    let socket_too_long = |enabled: bool, name: &str| {
        enabled
            && session
                .path()
                .join(name)
                .as_os_str()
                .as_encoded_bytes()
                .len()
                > 107
    };
    if socket_too_long(true, "qmp.sock")
        || socket_too_long(config.tpm.is_some(), "swtpm.sock")
        || socket_too_long(config.guest_agent, "qga.sock")
        || socket_too_long(config.spire_relay.enabled, "spire.sock")
    {
        return Err(Error::Config(
            "runtime directory is too long for a QMP Unix socket".into(),
        ));
    }
    if let Some(seed) = &config.seed {
        seed.write(&super::seed::image_path(&socket))?;
    }
    if receiving.is_none() {
        // A power cycle is where staged firmware and settings take effect.
        let _staging = control.firmware_guard().await;
        super::firmware::apply_staged_firmware(&mut config)?;
        super::secure_boot_policy::apply(&config)?;
    }
    config.validate()?;
    control.set_config(&config);
    state.resize = super::Resize::new(&config);
    publish(&config, state)?;
    if config.guest_identity.enabled {
        // Before swtpm: provisioning needs the TPM state to itself.
        let offered = receiving
            .as_ref()
            .and_then(|r| r.offer.guest_identity.as_ref());
        super::identity::prepare(&config, offered).await?;
    }
    let mut tpm = None;
    let result = async {
        if let Some(tpm_config) = &config.tpm {
            let identity = persistent.as_ref().expect("persistent TPM state");
            tpm = Some(Swtpm::spawn(
                tpm_config,
                session.path(),
                &identity.tpm_directory(),
                identity.lease(),
            )?);
            let process = tpm.as_mut().expect("TPM process");
            state.swtpm_pid = process.id();
            state.swtpm_start_ticks = process.id().and_then(|pid| start_ticks(pid).ok());
            publish(&config, state)?;
            let ready = process.ready();
            tokio::pin!(ready);
            let deadline = tokio::time::sleep(Duration::from_secs(config.startup_timeout_secs));
            tokio::pin!(deadline);
            // Keep the heartbeat fresh: a relaunch is live while it starts.
            let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
            let ready = loop {
                tokio::select! {
                    _=&mut *shutdown => break None,
                    _=&mut deadline => break Some(Err(())),
                    result=&mut ready => break Some(Ok(result)),
                    _=heartbeat.tick() => publish(&config, state)?,
                }
            };
            match ready {
                None => {
                    control.set_stopping();
                    state.phase = Phase::Exited;
                    return Ok(Cycle::Exit(Outcome {
                        reason: "shutdown-during-tpm-startup".into(),
                        exit_code: None,
                        shutdown_method: "swtpm-term".into(),
                    }));
                }
                Some(Ok(result)) => result?,
                Some(Err(())) => return Err(Error::Spawn("swtpm startup timed out".into())),
            }
            persistent
                .as_mut()
                .expect("persistent TPM state")
                .mark_tpm_initialized()?;
        }
        let migration = Migration {
            receiving,
            retained: retained.clone(),
            outgoing,
        };
        run_qemu(
            &config,
            &mut *shutdown,
            control,
            (&socket, console.as_deref()),
            state,
            &mut tpm,
            persistent.as_ref().map(PersistentState::lease),
            migration,
            relaunch,
        )
        .await
    }
    .await;
    match (&mut tpm, result) {
        (Some(process), result) => match process.stop(!config.iscsi_disks.is_empty()).await {
            Ok(()) => result,
            Err(error) => Err(error),
        },
        (None, result) => result,
    }
}

/// Serve the control socket with QEMU powered off until a power-on request
/// (true) or pod shutdown (false). A migration queued before QEMU stopped is
/// refused as powered off.
async fn powered_off(
    config: &Config,
    shutdown: &mut (impl Future<Output = ()> + Unpin),
    control: &mut super::ControlReceiver,
    state: &mut Status,
    outgoing: &mut Outgoing,
    reason: &'static str,
) -> Result<bool> {
    control.set_off();
    *state = Status {
        phase: Phase::Off,
        ready: false,
        qemu_pid: None,
        qemu_start_ticks: None,
        qemu_version: None,
        swtpm_pid: None,
        swtpm_start_ticks: None,
        run_state: None,
        detail: None,
        resize: super::Resize::new(config),
        relaunch: false,
        ..state.clone()
    };
    publish(config, state)?;
    control.record_power("Off", reason);
    eprintln!(
        "{}",
        serde_json::json!({"event":"powered_off","reason":reason,"exit_code":state.exit_code})
    );
    let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            biased;
            _ = &mut *shutdown => {
                control.set_stopping();
                state.phase = Phase::Exited;
                publish(config, state)?;
                return Ok(false);
            }
            Some(request) = control.requests.recv() => {
                if request.powered_off() {
                    return Ok(true);
                }
            }
            Some(pending) = next_migration(outgoing) => {
                migration::reject(pending, migration::POWERED_OFF).await;
            }
            _ = heartbeat.tick() => publish(config, state)?,
        }
    }
}

/// The manager-lifetime queue of authenticated migration targets.
type Outgoing = Option<mpsc::Receiver<Pending>>;

/// What a QEMU run needs for live migration in either direction.
struct Migration<'a> {
    /// The source's offer when this run is a migration target.
    receiving: Option<Receiving>,
    /// This VM's retained iSCSI bindings, offered to a target.
    retained: BTreeMap<String, Binding>,
    /// Targets asking to receive this VM; opened by the first run.
    outgoing: &'a mut Outgoing,
}

#[allow(clippy::too_many_arguments)]
async fn run_qemu(
    config: &Config,
    shutdown: impl Future<Output = ()>,
    control: &mut super::ControlReceiver,
    sockets: (&Path, Option<&Path>),
    state: &mut Status,
    tpm: &mut Option<Swtpm>,
    lease: Option<StateLease>,
    migration: Migration<'_>,
    relaunch: Option<&'static str>,
) -> Result<Cycle> {
    let (socket, console) = sockets;
    let session = socket
        .parent()
        .expect("the QMP socket lives in the session directory");
    let Migration {
        receiving,
        retained,
        outgoing,
    } = migration;
    let mut migrated = false;
    // Tap NICs get fresh queues from qrab-netd for every QEMU start; the
    // previous QEMU's queues closed when it exited.
    // Refusals are retried (the anchors may still be plumbing), publishing
    // each wait so the heartbeat stays fresh and the reason is visible.
    let waiting_since = tokio::time::Instant::now();
    let inherited = loop {
        match super::tap::fetch(config).await {
            Ok(inherited) => break inherited,
            Err(error) if waiting_since.elapsed() < super::tap::PATIENCE => {
                state.detail = Some(format!("waiting for qrab-netd: {error}"));
                publish(config, state)?;
                tokio::time::sleep(super::tap::RETRY).await;
            }
            Err(error) => {
                state.phase = Phase::Failed;
                state.detail = Some(error.to_string());
                let _ = publish(config, state);
                return Err(error);
            }
        }
    };
    if !config.tap_fds().is_empty() {
        state.detail = None;
    }
    let mut nics = super::nics::Nics::booted(config, &inherited.1);
    let mut command = Command::new(&config.qemu_binary);
    command
        .args(config.arguments(socket))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    let spawned = inherited
        .install(&mut command)
        .and_then(|()| command.spawn().map_err(Error::Io));
    drop(command);
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            state.phase = Phase::Failed;
            state.detail = Some(error.to_string());
            let _ = publish(config, state);
            return Err(error);
        }
    };
    let serial = child
        .stdout
        .take()
        .map(|stdout| control.pump_serial(stdout));
    let _serial = SerialPump(serial);
    let mut child = ManagedChild::new(child, lease);
    state.qemu_pid = Some(child.spawned_pid());
    state.qemu_start_ticks = start_ticks(child.spawned_pid()).ok();
    publish(config, state)?;
    let mut shutdown = Box::pin(shutdown);
    let options = ClientOptions {
        connect_timeout: Duration::from_millis(200),
        handshake_timeout: Duration::from_secs(1),
        command_timeout: Duration::from_secs(2),
        // Headroom for bursts; events are also drained by their own task.
        event_capacity: 1024,
        ..ClientOptions::default()
    };
    let startup = async {
        loop {
            if let Some(exit) = child.try_wait()? {
                return Err(Error::Spawn(format!("QEMU exited during startup: {exit}")));
            }
            match Qmp::connect_with_options(socket, options.clone()).await {
                Ok(connected) => return Ok(connected),
                Err(qapi_protocol::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                    ) => {}
                Err(qapi_protocol::Error::Timeout(_)) => {}
                Err(error) => return Err(Error::Protocol(error)),
            }
            publish(config, state)?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    let qmp = tokio::select! {
        result=timeout(Duration::from_secs(config.startup_timeout_secs),startup) => match result {
            Ok(Ok(connected)) => Some(connected),
            failure => {
                let detail = match failure { Ok(Err(error))=>error.to_string(), Err(_)=>"QEMU startup timed out".into(), _=>unreachable!() };
                if let Ok((exit,_))=stop_child(&mut child,None,Duration::from_secs(1)).await {state.exit_code=exit.code();}
                state.phase=Phase::Failed;state.detail=Some(detail.clone());let _=publish(config,state);
                return Err(Error::Spawn(detail));
            }
        },
        _=&mut shutdown => None,
    };
    let Some((qmp, events)) = qmp else {
        control.set_stopping();
        let (exit, method) = stop_child(&mut child, None, Duration::from_secs(1)).await?;
        state.exit_code = exit.code();
        native_clean_shutdown(config, &exit, method)?;
        state.phase = Phase::Exited;
        publish(config, state)?;
        return Ok(Cycle::Exit(Outcome {
            reason: "shutdown-during-startup".into(),
            exit_code: exit.code(),
            shutdown_method: method.into(),
        }));
    };
    state.qemu_version = Some(qmp.version());
    eprintln!(
        "{}",
        serde_json::json!({"event":"qmp_connected","version":qmp.version(),"qemu_pid":state.qemu_pid})
    );
    let pump = control.pump_events(events);
    // QEMU created the agent chardev before QMP became available.
    let agent = config.guest_agent.then(|| {
        super::agent::GuestAgent::spawn_publishing(
            socket.with_file_name("qga.sock"),
            Default::default(),
            control.agent_status_publisher(),
        )
    });
    let spire_relay = config.spire_relay.enabled.then(|| {
        super::spire_relay::Relay::spawn(
            socket.with_file_name("spire.sock"),
            (&config.spire_relay).into(),
        )
    });
    let execution = async {
        if qmp.version() < (11,1,0) { return Err(Error::Config("pod manager requires QEMU 11.1 or newer".into())); }
        let received_tap = receiving.is_some() && !config.tap_fds().is_empty();
        if let Some(receiving) = receiving {
            let _management=control.migration_guard()?;
            // Healthy but not ready while the guest arrives.
            state.phase=Phase::Running;state.run_state=Some("inmigrate".into());publish(config,state)?;
            let received = tokio::select! {
                biased;
                _=&mut shutdown => return Ok(Ended::Shutdown),
                received=receiving.receive(&qmp, session, |progress| {
                    state.run_state=Some(format!("inmigrate:{progress}"));
                    std::future::ready(publish(config,state).map(|_| true))
                }) => received,
            };
            received?;
        }
        tokio::select! {
            biased;
            _=&mut shutdown => return Ok(Ended::Shutdown),
            started=qmp.execute(&Cont {}) => {started?;}
        }
        // Tap NICs moved nodes: let the guest's MACs be relearned at once.
        if received_tap {
            if let Err(error) = qmp.execute_raw(serde_json::json!({"execute":"announce-self","arguments":{"initial":50,"max":550,"rounds":5,"step":100}})).await {
                eprintln!("{}", serde_json::json!({"event":"announce_self_failed","error":error.to_string()}));
            }
        }
        if let Some(reason) = relaunch { control.record_power("On", reason); }
        if outgoing.is_none() {
            *outgoing = config.migration.as_ref().map(|o| migration::listen(o, control.control().watch_status()));
        }
        let mut heartbeat=tokio::time::interval(Duration::from_secs(1));
        let mut disk_growth=super::disks::DiskGrowth::default();
        loop {
            tokio::select! {
                _=&mut shutdown => return Ok(Ended::Shutdown),
                exit=child.wait() => return Ok(Ended::Exited(exit?)),
                exit=super::tpm::wait(tpm) => return Err(Error::Spawn(format!("swtpm exited while QEMU was active: {}", exit?))),
                Some(pending)=next_migration(outgoing) => {
                    let Ok(_management)=control.migration_guard() else {
                        migration::reject(pending,"firmware management is busy").await;continue;
                    };
                    if nics.changing() {
                        migration::reject(pending,"a guest NIC is being plugged or unplugged").await;continue;
                    }
                    // The target recreates the NICs QEMU has now, hot-plugged ones included.
                    let live = nics.live(config);
                    let config = &live;
                    let grown=state.resize.as_ref().is_some_and(|resize| resize.current!=resize.boot);
                    let sent = {
                        // Progress callbacks do not run while the target prepares
                        // its storage, swtpm and NICs. Keep observing QMP during
                        // every await, with the same probe freshness requirement.
                        let shared = std::sync::Mutex::new(&mut *state);
                        let sending=migration::send(config, &qmp, session, pending, retained.clone(), grown, |progress| {
                            let published=shared.lock()
                                .map_err(|_| Error::Spawn("migration status lock poisoned".into()))
                                .and_then(|mut record| {
                                    record.run_state=Some(format!("migrating:{progress}"));
                                    publish(config,&mut record).map(|_| true)
                                });
                            std::future::ready(published)
                        });
                        tokio::pin!(sending);
                        loop {
                            tokio::select! {
                                biased;
                                _=&mut shutdown => {
                                    let _=qmp.execute_raw(serde_json::json!({"execute":"migrate_cancel"})).await;
                                    return Ok(Ended::Shutdown);
                                }
                                sent=&mut sending => break sent,
                                _=heartbeat.tick() => {
                                    let observed=qmp.execute(&QueryStatus {}).await?;
                                    let mut record=shared.lock()
                                        .map_err(|_| Error::Spawn("migration status lock poisoned".into()))?;
                                    record.ready=observed.running;
                                    record.run_state=Some(format!("migrating:{}",observed.status));
                                    publish(config,&mut record)?;
                                }
                            }
                        }
                    };
                    match sent {
                        Sent::Completed => {
                            // The guest runs on the target now; this QEMU holds
                            // inactive disks and must not resume.
                            migrated=true;
                            state.ready=false;state.run_state=Some("postmigrate".into());publish(config,state)?;
                            let _=qmp.execute(&Quit {}).await;
                            return match timeout(Duration::from_secs(10), child.wait()).await {
                                Ok(exit) => Ok(Ended::Exited(exit?)),
                                Err(_) => Err(Error::Spawn("QEMU did not quit after migration".into())),
                            };
                        }
                        Sent::Failed(reason) => {
                            state.detail=Some(format!("migration failed: {reason}"));publish(config,state)?;
                        }
                    }
                },
                Some(request)=control.requests.recv() => {
                    let stop = request.dispatch(&qmp, console, agent.as_ref()).await;
                    // A slow command is when the QMP event queue can overflow.
                    control.set_qmp_dropped(qmp.dropped_events());
                    if let Some(stop) = stop { return Ok(Ended::Stop(stop)); }
                },
                reason=qmp.closed() => {
                    state.ready=false;state.phase=Phase::Stopping;publish(config,state)?;
                    if let Ok(exit)=timeout(Duration::from_secs(2),child.wait()).await { return Ok(Ended::Exited(exit?)); }
                    return Err(Error::Spawn(format!("QMP disconnected: {reason}")));
                },
                _=heartbeat.tick() => {
                    let status=qmp.execute(&QueryStatus {}).await?;
                    state.phase=Phase::Running;state.ready=status.running;state.run_state=Some(status.status.to_string());
                    if let Some(resize)=state.resize.as_mut() { super::hotplug::reconcile(config,&qmp,resize).await; }
                    super::nics::reconcile(config,&qmp,&mut nics).await;
                    super::disks::reconcile(config,&qmp,&mut disk_growth).await;
                    control.set_qmp_dropped(qmp.dropped_events());
                    publish(config,state)?;
                }
            }
        }
    }.await;
    drop(agent);
    drop(spire_relay);
    control.set_stopping();
    state.ready = false;
    let result = match execution {
        Ok(Ended::Exited(exit)) => {
            state.exit_code = exit.code();
            if !exit.success() {
                state.phase = Phase::Failed;
                Err(Error::Spawn(format!("QEMU exited unexpectedly: {exit}")))
            } else if migrated {
                state.phase = Phase::Exited;
                Ok(Cycle::Exit(Outcome {
                    reason: "migrated".into(),
                    exit_code: exit.code(),
                    shutdown_method: "migration".into(),
                }))
            } else {
                // The guest powered itself off.
                Ok(Cycle::Off("guest-poweroff"))
            }
        }
        other => {
            let stop = match &other {
                Ok(Ended::Stop(stop)) => Some(*stop),
                _ => None,
            };
            state.phase = Phase::Stopping;
            state.relaunch = stop.is_some();
            let _ = publish(config, state);
            let grace = Duration::from_secs(config.shutdown_grace_secs);
            let stopping = async {
                match stop {
                    Some(PowerStop::ForceOff | PowerStop::ColdRestart) => {
                        reap_after_quit(&mut child).await
                    }
                    Some(PowerStop::PowerOff) | None => {
                        stop_child(&mut child, Some(&qmp), grace).await
                    }
                }
            };
            let graceful = stop == Some(PowerStop::PowerOff);
            // A force off during a graceful stop cuts the ACPI grace short.
            if graceful {
                control.set_graceful_stop();
            }
            let mut forced = false;
            let stopped = match drive_stop(stopping, config, control, state, graceful).await {
                Ok(stopped) => stopped,
                Err(reply) => {
                    forced = true;
                    control.set_stopping();
                    eprintln!("{}", serde_json::json!({"event":"power_stop_escalated"}));
                    let quitting = async {
                        // SIGKILL follows a failed quit, so the force off is in effect.
                        let _ = qmp.execute(&Quit {}).await;
                        let _ = reply.send(Ok(()));
                        reap_after_quit(&mut child).await
                    };
                    match drive_stop(quitting, config, control, state, false).await {
                        Ok(stopped) => stopped,
                        Err(_) => unreachable!("a forced stop does not escalate"),
                    }
                }
            };
            match (other, stopped) {
                (Ok(ended), Ok((exit, method))) => {
                    state.exit_code = exit.code();
                    native_clean_shutdown(config, &exit, method)?;
                    match ended {
                        Ended::Shutdown => {
                            state.phase = Phase::Exited;
                            Ok(Cycle::Exit(Outcome {
                                reason: "shutdown-requested".into(),
                                exit_code: exit.code(),
                                shutdown_method: method.into(),
                            }))
                        }
                        Ended::Stop(PowerStop::PowerOff) if forced => Ok(Cycle::Off("force-off")),
                        Ended::Stop(PowerStop::PowerOff) => Ok(Cycle::Off("graceful-shutdown")),
                        Ended::Stop(PowerStop::ForceOff) => Ok(Cycle::Off("force-off")),
                        Ended::Stop(PowerStop::ColdRestart) => Ok(Cycle::Restart),
                        Ended::Exited(_) => unreachable!(),
                    }
                }
                (Err(error), _) | (_, Err(error)) => {
                    state.phase = Phase::Failed;
                    state.detail = Some(error.to_string());
                    Err(error)
                }
            }
        }
    };
    control.set_qmp_dropped(qmp.dropped_events());
    control.end_qmp_connection();
    qmp.shutdown().await;
    // Shutdown and exit events arrive while stopping; the pump ends when the
    // closed connection drops its event queue.
    let _ = timeout(Duration::from_secs(1), pump).await;
    publish(config, state)?;
    result
}

/// Drive a stop to completion, answering the requests that raced the
/// stopping flag instead of queueing them and keeping the heartbeat fresh.
/// When `escalable`, a live force off abandons `stopping` and its reply is
/// returned for the caller's forced stop to answer.
async fn drive_stop<T>(
    stopping: impl Future<Output = T>,
    config: &Config,
    control: &mut super::ControlReceiver,
    state: &mut Status,
    escalable: bool,
) -> std::result::Result<T, oneshot::Sender<Result<()>>> {
    tokio::pin!(stopping);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            biased;
            stopped = &mut stopping => return Ok(stopped),
            Some(request) = control.requests.recv() => {
                if !escalable {
                    request.shutting_down();
                } else if let Some(reply) = request.escalates_graceful_stop() {
                    return Err(reply);
                }
            }
            _ = heartbeat.tick() => { let _ = publish(config, state); }
        }
    }
}

fn native_clean_shutdown(config: &Config, exit: &ExitStatus, method: &str) -> Result<()> {
    if !config.iscsi_disks.is_empty() && (!exit.success() || method == "sigkill") {
        return Err(Error::Spawn(
            "native iSCSI QEMU exit was forced or unsuccessful; clean disk release is not proven"
                .into(),
        ));
    }
    Ok(())
}

struct SerialPump(Option<tokio::task::JoinHandle<()>>);
impl Drop for SerialPump {
    fn drop(&mut self) {
        if let Some(task) = self.0.take() {
            task.abort();
        }
    }
}
