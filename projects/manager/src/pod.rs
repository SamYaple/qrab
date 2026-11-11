//! QEMU supervision inside an existing container sandbox. No host namespaces,
//! qrab-init helper, confidential-guest hardware or Kubernetes API is required.
use crate::{Error, Result};
use qapi::commands::{Cont, QueryStatus, Quit, SystemPowerdown};
use qapi_protocol::{ClientOptions, Qmp};
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    future::Future,
    io::Write,
    os::unix::fs::{FileTypeExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    process::{Child, Command},
    time::timeout,
};

fn binary() -> PathBuf {
    "/opt/qemu/bin/qemu-system-x86_64".into()
}
fn name() -> String {
    "vm".into()
}
fn memory() -> u64 {
    256
}
fn cpus() -> u32 {
    1
}
fn runtime() -> PathBuf {
    "/run/qrab".into()
}
fn startup() -> u64 {
    30
}
fn grace() -> u64 {
    5
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Accelerator {
    #[default]
    Tcg,
    Kvm,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiskFormat {
    #[default]
    Raw,
    Qcow2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disk {
    pub path: PathBuf,
    #[serde(default)]
    pub format: DiskFormat,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "name")]
    pub name: String,
    #[serde(default = "binary")]
    pub qemu_binary: PathBuf,
    #[serde(default = "memory")]
    pub memory_mib: u64,
    #[serde(default = "cpus")]
    pub cpus: u32,
    #[serde(default)]
    pub accelerator: Accelerator,
    pub kernel: PathBuf,
    #[serde(default)]
    pub initrd: Option<PathBuf>,
    #[serde(default)]
    pub kernel_args: String,
    #[serde(default)]
    pub disk: Option<Disk>,
    #[serde(default)]
    pub firmware_dir: Option<PathBuf>,
    #[serde(default = "runtime")]
    pub runtime_dir: PathBuf,
    #[serde(default = "startup")]
    pub startup_timeout_secs: u64,
    #[serde(default = "grace")]
    pub shutdown_grace_secs: u64,
}

impl Config {
    pub fn read(path: &Path) -> Result<Self> {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty()
            || self.name.len() > 63
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        {
            return Err(Error::Config(
                "name must be 1..63 ASCII letters, digits, dots, hyphens or underscores".into(),
            ));
        }
        if !(64..=1_048_576).contains(&self.memory_mib)
            || !(1..=256).contains(&self.cpus)
            || !(1..=300).contains(&self.startup_timeout_secs)
            || self.shutdown_grace_secs > 120
        {
            return Err(Error::Config(
                "invalid memory, CPU count or lifecycle timeout".into(),
            ));
        }
        for path in [&self.qemu_binary, &self.kernel]
            .into_iter()
            .chain(self.initrd.iter())
        {
            if !path.is_absolute() || !path.is_file() {
                return Err(Error::Config(format!(
                    "required absolute file is missing: {}",
                    path.display()
                )));
            }
        }
        if !self.runtime_dir.is_absolute() {
            return Err(Error::Config("runtime_dir must be absolute".into()));
        }
        if let Some(path) = &self.firmware_dir {
            if !path.is_absolute() || !path.is_dir() {
                return Err(Error::Config(
                    "firmware_dir must be an existing absolute directory".into(),
                ));
            }
        }
        if let Some(disk) = &self.disk {
            let kind = std::fs::metadata(&disk.path)?.file_type();
            if !disk.path.is_absolute() || !(kind.is_file() || kind.is_block_device()) {
                return Err(Error::Config(
                    "disk must name an absolute regular file or block device".into(),
                ));
            }
        }
        if self.accelerator == Accelerator::Kvm {
            OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/kvm")
                .map_err(|e| Error::Config(format!("KVM requires access to /dev/kvm: {e}")))?;
        }
        Ok(())
    }

    /// JSON blockdev options keep filenames with commas or spaces unambiguous.
    pub fn arguments(&self, socket: &Path) -> Vec<String> {
        let mut args: Vec<String> = [
            "-name",
            &self.name,
            "-machine",
            "q35",
            "-m",
            &self.memory_mib.to_string(),
            "-smp",
            &self.cpus.to_string(),
            "-accel",
            match self.accelerator {
                Accelerator::Tcg => "tcg,thread=multi",
                Accelerator::Kvm => "kvm",
            },
            "-cpu",
            match self.accelerator {
                Accelerator::Tcg => "max",
                Accelerator::Kvm => "host",
            },
            "-nodefaults",
            "-display",
            "none",
            "-nic",
            "none",
            "-S",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        args.extend([
            "-chardev".into(),
            "stdio,id=serial,signal=off".into(),
            "-serial".into(),
            "chardev:serial".into(),
            "-chardev".into(),
            format!(
                "socket,id=qmp,path={},server=on,wait=off",
                socket.to_string_lossy().replace(',', ",,")
            ),
            "-object".into(),
            serde_json::json!({"qom-type":"monitor-qmp", "id":"monitor", "chardev":"qmp"})
                .to_string(),
            "-kernel".into(),
            self.kernel.to_string_lossy().into_owned(),
        ]);
        if let Some(initrd) = &self.initrd {
            args.extend(["-initrd".into(), initrd.to_string_lossy().into_owned()]);
        }
        if !self.kernel_args.is_empty() {
            args.extend(["-append".into(), self.kernel_args.clone()]);
        }
        if let Some(path) = &self.firmware_dir {
            args.extend(["-L".into(), path.to_string_lossy().into_owned()]);
        }
        if let Some(disk) = &self.disk {
            args.extend(["-blockdev".into(), serde_json::json!({"driver":"file", "filename":disk.path, "node-name":"disk-file", "read-only":disk.read_only}).to_string(),
                "-blockdev".into(), serde_json::json!({"driver":disk.format, "file":"disk-file", "node-name":"disk", "read-only":disk.read_only}).to_string(),
                "-device".into(), "virtio-blk-pci,drive=disk".into()]);
        }
        args
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Starting,
    Running,
    Stopping,
    Exited,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub phase: Phase,
    pub ready: bool,
    pub manager_pid: u32,
    pub manager_start_ticks: u64,
    pub qemu_pid: Option<u32>,
    pub qemu_start_ticks: Option<u64>,
    pub qemu_version: Option<(u32, u32, u32)>,
    pub run_state: Option<String>,
    pub observed_unix_ms: u64,
    pub exit_code: Option<i32>,
    pub detail: Option<String>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

/// PID reuse must not make an old readiness record look current.
fn start_ticks(pid: u32) -> Result<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
    stat.rsplit_once(") ")
        .and_then(|(_, fields)| fields.split_whitespace().nth(19))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| Error::Spawn("invalid process identity".into()))
}

fn publish(config: &Config, status: &mut Status) -> Result<()> {
    status.observed_unix_ms = now_ms();
    let mut file = tempfile::NamedTempFile::new_in(&config.runtime_dir)?;
    serde_json::to_writer(&mut file, status)?;
    file.write_all(b"\n")?;
    file.as_file().sync_data()?;
    file.persist(config.runtime_dir.join("status.json"))
        .map_err(|e| Error::Io(e.error))?;
    Ok(())
}

pub fn status(config: &Config) -> Result<Status> {
    Ok(serde_json::from_slice(&std::fs::read(
        config.runtime_dir.join("status.json"),
    )?)?)
}

/// Readiness means QMP reports running, not that guest applications are ready.
/// Liveness permits paused VMs, but both probes require a fresh QMP heartbeat
/// and exact manager/child process identities. No second QMP client is opened.
pub fn probe(config: &Config, readiness: bool) -> Result<()> {
    let status = status(config)?;
    let age = now_ms()
        .checked_sub(status.observed_unix_ms)
        .ok_or_else(|| Error::Spawn("status timestamp is in the future".into()))?;
    let child_alive = status
        .qemu_pid
        .zip(status.qemu_start_ticks)
        .is_some_and(|(pid, ticks)| start_ticks(pid).ok() == Some(ticks));
    if age > 5_000
        || start_ticks(status.manager_pid).ok() != Some(status.manager_start_ticks)
        || !child_alive
        || status.phase != Phase::Running
        || (readiness && !status.ready)
    {
        return Err(Error::Spawn(
            "VM is not healthy or its status is stale".into(),
        ));
    }
    Ok(())
}

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
        .tempdir_in(&config.runtime_dir)?;
    let socket = session.path().join("qmp.sock");
    if socket.as_os_str().as_encoded_bytes().len() > 107 {
        return Err(Error::Config(
            "runtime directory is too long for a QMP Unix socket".into(),
        ));
    }
    let mut state = Status {
        phase: Phase::Starting,
        ready: false,
        manager_pid: std::process::id(),
        manager_start_ticks: start_ticks(std::process::id())?,
        qemu_pid: None,
        qemu_start_ticks: None,
        qemu_version: None,
        run_state: None,
        observed_unix_ms: 0,
        exit_code: None,
        detail: None,
    };
    publish(&config, &mut state)?;
    let mut child = match Command::new(&config.qemu_binary)
        .args(config.arguments(&socket))
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
            let _ = publish(&config, &mut state);
            return Err(Error::Io(error));
        }
    };
    state.qemu_pid = child.id();
    state.qemu_start_ticks = child.id().and_then(|pid| start_ticks(pid).ok());
    publish(&config, &mut state)?;
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
            match Qmp::connect_with_options(&socket, options.clone()).await {
                Ok((qmp, _events)) => return Ok(qmp),
                Err(qapi_protocol::Error::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                    ) => {}
                Err(qapi_protocol::Error::Timeout(_)) => {}
                Err(error) => return Err(Error::Protocol(error)),
            }
            publish(&config, &mut state)?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    let qmp = tokio::select! {
        result=timeout(Duration::from_secs(config.startup_timeout_secs),startup) => match result {
            Ok(Ok(qmp)) => Some(qmp),
            failure => {
                let detail = match failure { Ok(Err(error))=>error.to_string(), Err(_)=>"QEMU startup timed out".into(), _=>unreachable!() };
                if let Ok((exit,_))=stop_child(&mut child,None,Duration::from_secs(1)).await {state.exit_code=exit.code();}
                state.phase=Phase::Failed;state.detail=Some(detail.clone());let _=publish(&config,&mut state);
                return Err(Error::Spawn(detail));
            }
        },
        _=&mut shutdown => None,
    };
    let Some(qmp) = qmp else {
        let (exit, method) = stop_child(&mut child, None, Duration::from_secs(1)).await?;
        state.phase = Phase::Exited;
        state.exit_code = exit.code();
        publish(&config, &mut state)?;
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
                reason=qmp.closed() => {
                    state.ready=false;state.phase=Phase::Stopping;publish(&config,&mut state)?;
                    if let Ok(exit)=timeout(Duration::from_secs(2),child.wait()).await { return Ok(Some(exit?)); }
                    return Err(Error::Spawn(format!("QMP disconnected: {reason}")));
                },
                _=heartbeat.tick() => {
                    let status=qmp.execute(&QueryStatus {}).await?;
                    state.phase=Phase::Running;state.ready=status.running;state.run_state=Some(status.status.to_string());
                    publish(&config,&mut state)?;
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
            let _ = publish(&config, &mut state);
            let stopped = stop_child(
                &mut child,
                Some(&qmp),
                Duration::from_secs(config.shutdown_grace_secs),
            )
            .await;
            match (other, stopped) {
                (Ok(None), Ok((exit, method))) => {
                    state.phase = Phase::Exited;
                    state.exit_code = exit.code();
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
    publish(&config, &mut state)?;
    result
}
