use super::{
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
    fs::OpenOptions,
    future::Future,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    process::{Child, Command},
    time::timeout,
};

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub reason: String,
    pub exit_code: Option<i32>,
    pub shutdown_method: String,
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

/// Own one QEMU child until it exits or shutdown resolves. kill_on_drop covers
/// cancellation of this future; explicit paths always wait and reap the child.
pub async fn run(config: Config, shutdown: impl Future<Output = ()>) -> Result<Outcome> {
    let (_control, receiver) = super::control_channel();
    run_controlled(config, shutdown, receiver).await
}

/// Supervise a VM with a bounded in-process control channel for its BMC.
pub async fn run_controlled(
    config: Config,
    shutdown: impl Future<Output = ()>,
    mut control: super::ControlReceiver,
) -> Result<Outcome> {
    config.validate()?;
    std::fs::create_dir_all(&config.runtime_dir)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(config.runtime_dir.join("owner.lock"))?;
    let _lock = nix::fcntl::Flock::lock(lock, nix::fcntl::FlockArg::LockExclusiveNonblock)
        .map_err(|_| Error::Config("another manager owns this runtime directory".into()))?;
    let session = tempfile::Builder::new()
        .prefix("session-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(&config.runtime_dir)?;
    let socket = session.path().join("qmp.sock");
    let console = config.console.then(|| session.path().join("vnc.sock"));
    if socket.as_os_str().as_encoded_bytes().len() > 107
        || (config.tpm.is_some()
            && session
                .path()
                .join("swtpm.sock")
                .as_os_str()
                .as_encoded_bytes()
                .len()
                > 107)
    {
        return Err(Error::Config(
            "runtime directory is too long for a QMP Unix socket".into(),
        ));
    }
    let mut persistent = PersistentState::prepare(&config)?;
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
    };
    publish(&config, &mut state)?;
    let mut shutdown = Box::pin(shutdown);
    let mut tpm = None;
    let result = async {
        if let Some(tpm_config) = &config.tpm {
            let identity=persistent.as_ref().expect("persistent TPM state");
            tpm = Some(Swtpm::spawn(tpm_config, session.path(), &identity.tpm_directory(), identity.lease())?);
            let process = tpm.as_mut().expect("TPM process");
            state.swtpm_pid = process.id();
            state.swtpm_start_ticks = process.id().and_then(|pid| start_ticks(pid).ok());
            publish(&config, &mut state)?;
            let ready = tokio::select! {
                _=&mut shutdown => None,
                result=timeout(Duration::from_secs(config.startup_timeout_secs),process.ready()) => Some(result),
            };
            match ready {
                None => {
                    state.phase=Phase::Exited;
                    return Ok(Outcome {reason:"shutdown-during-tpm-startup".into(),exit_code:None,shutdown_method:"swtpm-term".into()});
                },
                Some(Ok(result)) => result?,
                Some(Err(_)) => return Err(Error::Spawn("swtpm startup timed out".into())),
            }
            persistent.as_mut().expect("persistent TPM state").mark_tpm_initialized()?;
        }
        run_qemu(&config, &mut shutdown, &mut control, (&socket, console.as_deref()), &mut state, &mut tpm, persistent.as_ref().map(PersistentState::lease)).await
    }.await;
    let result = match (&mut tpm, result) {
        (Some(process), result) => match process.stop(!config.iscsi_disks.is_empty()).await {
            Ok(()) => result,
            Err(error) => Err(error),
        },
        (None, result) => result,
    };
    if let Err(error) = &result {
        state.phase = Phase::Failed;
        state.detail = Some(error.to_string());
    }
    state.ready = false;
    publish(&config, &mut state)?;
    result
}

async fn run_qemu(
    config: &Config,
    shutdown: impl Future<Output = ()>,
    control: &mut super::ControlReceiver,
    sockets: (&Path, Option<&Path>),
    state: &mut Status,
    tpm: &mut Option<Swtpm>,
    lease: Option<StateLease>,
) -> Result<Outcome> {
    let (socket, console) = sockets;
    let child = match Command::new(&config.qemu_binary)
        .args(config.arguments(socket))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            state.phase = Phase::Failed;
            state.detail = Some(error.to_string());
            let _ = publish(config, state);
            return Err(Error::Io(error));
        }
    };
    let mut child = ManagedChild::new(child, lease);
    state.qemu_pid = Some(child.spawned_pid());
    state.qemu_start_ticks = start_ticks(child.spawned_pid()).ok();
    publish(config, state)?;
    let mut shutdown = Box::pin(shutdown);
    let options = ClientOptions {
        connect_timeout: Duration::from_millis(200),
        handshake_timeout: Duration::from_secs(1),
        command_timeout: Duration::from_secs(2),
        ..ClientOptions::default()
    };
    let startup = async {
        loop {
            if let Some(exit) = child.try_wait()? {
                return Err(Error::Spawn(format!("QEMU exited during startup: {exit}")));
            }
            match Qmp::connect_with_options(socket, options.clone()).await {
                Ok((qmp, _events)) => return Ok(qmp),
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
            Ok(Ok(qmp)) => Some(qmp),
            failure => {
                let detail = match failure { Ok(Err(error))=>error.to_string(), Err(_)=>"QEMU startup timed out".into(), _=>unreachable!() };
                if let Ok((exit,_))=stop_child(&mut child,None,Duration::from_secs(1)).await {state.exit_code=exit.code();}
                state.phase=Phase::Failed;state.detail=Some(detail.clone());let _=publish(config,state);
                return Err(Error::Spawn(detail));
            }
        },
        _=&mut shutdown => None,
    };
    let Some(qmp) = qmp else {
        let (exit, method) = stop_child(&mut child, None, Duration::from_secs(1)).await?;
        state.exit_code = exit.code();
        native_clean_shutdown(config, &exit, method)?;
        state.phase = Phase::Exited;
        publish(config, state)?;
        return Ok(Outcome {
            reason: "shutdown-during-startup".into(),
            exit_code: exit.code(),
            shutdown_method: method.into(),
        });
    };
    state.qemu_version = Some(qmp.version());
    eprintln!(
        "{}",
        serde_json::json!({"event":"qmp_connected","version":qmp.version(),"qemu_pid":state.qemu_pid})
    );
    let execution = async {
        if qmp.version() < (11,1,0) { return Err(Error::Config("pod manager requires QEMU 11.1 or newer".into())); }
        tokio::select! {
            biased;
            _=&mut shutdown => return Ok(None),
            started=qmp.execute(&Cont {}) => {started?;}
        }
        let mut heartbeat=tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _=&mut shutdown => return Ok(None),
                exit=child.wait() => return Ok(Some(exit?)),
                exit=super::tpm::wait(tpm) => return Err(Error::Spawn(format!("swtpm exited while QEMU was active: {}", exit?))),
                Some(request)=control.0.recv() => if request.dispatch(&qmp, console).await { return Ok(None); },
                reason=qmp.closed() => {
                    state.ready=false;state.phase=Phase::Stopping;publish(config,state)?;
                    if let Ok(exit)=timeout(Duration::from_secs(2),child.wait()).await { return Ok(Some(exit?)); }
                    return Err(Error::Spawn(format!("QMP disconnected: {reason}")));
                },
                _=heartbeat.tick() => {
                    let status=qmp.execute(&QueryStatus {}).await?;
                    state.phase=Phase::Running;state.ready=status.running;state.run_state=Some(status.status.to_string());
                    publish(config,state)?;
                }
            }
        }
    }.await;
    state.ready = false;
    let result = match execution {
        Ok(Some(exit)) => {
            state.exit_code = exit.code();
            if exit.success() {
                state.phase = Phase::Exited;
                Ok(Outcome {
                    reason: "guest-exited".into(),
                    exit_code: exit.code(),
                    shutdown_method: "guest".into(),
                })
            } else {
                state.phase = Phase::Failed;
                Err(Error::Spawn(format!("QEMU exited unexpectedly: {exit}")))
            }
        }
        other => {
            state.phase = Phase::Stopping;
            let _ = publish(config, state);
            let stopped = stop_child(
                &mut child,
                Some(&qmp),
                Duration::from_secs(config.shutdown_grace_secs),
            )
            .await;
            match (other, stopped) {
                (Ok(None), Ok((exit, method))) => {
                    state.exit_code = exit.code();
                    native_clean_shutdown(config, &exit, method)?;
                    state.phase = Phase::Exited;
                    Ok(Outcome {
                        reason: "shutdown-requested".into(),
                        exit_code: exit.code(),
                        shutdown_method: method.into(),
                    })
                }
                (Err(error), _) => {
                    state.phase = Phase::Failed;
                    state.detail = Some(error.to_string());
                    Err(error)
                }
                (_, Err(error)) => {
                    state.phase = Phase::Failed;
                    state.detail = Some(error.to_string());
                    Err(error)
                }
                _ => unreachable!(),
            }
        }
    };
    qmp.shutdown().await;
    publish(config, state)?;
    result
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
