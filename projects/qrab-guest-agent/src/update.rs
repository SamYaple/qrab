//! Asynchronous runs of the single fixed update hook.
use crate::proto::{validate_update_args, UpdateState, UpdateStatus, MAX_UPDATE_OUTPUT_BYTES};
use std::{
    io::Read,
    os::unix::process::{CommandExt, ExitStatusExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct HookPolicy {
    pub path: PathBuf,
    /// Production agents require root ownership; tests run unprivileged.
    pub require_root: bool,
    pub timeout: Duration,
}

pub struct Updater {
    policy: HookPolicy,
    status: Arc<Mutex<UpdateStatus>>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Updater {
    pub fn new(policy: HookPolicy) -> Self {
        Self {
            policy,
            status: Arc::new(Mutex::new(UpdateStatus {
                agent_version: env!("CARGO_PKG_VERSION").into(),
                ..UpdateStatus::default()
            })),
        }
    }

    pub fn hook_ready(&self) -> Result<(), String> {
        crate::sys::check_hook(&self.policy.path, self.policy.require_root)
    }

    pub fn running(&self) -> bool {
        self.status.lock().unwrap().state == UpdateState::Running
    }

    pub fn status(&self) -> UpdateStatus {
        let mut status = self.status.lock().unwrap().clone();
        status.hook_installed = self.hook_ready().is_ok();
        status
    }

    pub fn trigger(&self, args: Vec<String>) -> Result<UpdateStatus, String> {
        validate_update_args(&args)?;
        self.hook_ready()?;
        let mut status = self.status.lock().unwrap();
        if status.state == UpdateState::Running {
            return Err(format!("update run {} is still running", status.run_id));
        }
        let (mut reader, writer) = std::io::pipe().map_err(|e| format!("pipe: {e}"))?;
        let run_id = status.run_id + 1;
        let mut command = Command::new(&self.policy.path);
        command
            .args(&args)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("QRAB_UPDATE_RUN_ID", run_id.to_string())
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(writer.try_clone().map_err(|e| format!("pipe: {e}"))?)
            .stderr(writer)
            .process_group(0);
        let mut child = command
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", self.policy.path.display()))?;
        // Drop the parent's copies of the write end so EOF follows hook exit.
        drop(command);
        *status = UpdateStatus {
            state: UpdateState::Running,
            run_id,
            args,
            started_unix_ms: Some(now_ms()),
            hook_installed: true,
            agent_version: status.agent_version.clone(),
            ..UpdateStatus::default()
        };
        let snapshot = status.clone();
        drop(status);
        let shared = self.status.clone();
        let output = Arc::new(Mutex::new((Vec::<u8>::new(), false)));
        let collector = {
            let output = output.clone();
            std::thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                while let Ok(n @ 1..) = reader.read(&mut chunk) {
                    let mut tail = output.lock().unwrap();
                    tail.0.extend_from_slice(&chunk[..n]);
                    if tail.0.len() > MAX_UPDATE_OUTPUT_BYTES {
                        let excess = tail.0.len() - MAX_UPDATE_OUTPUT_BYTES;
                        tail.0.drain(..excess);
                        tail.1 = true;
                    }
                }
            })
        };
        let timeout = self.policy.timeout;
        std::thread::spawn(move || {
            let deadline = Instant::now() + timeout;
            let (exit, error) = loop {
                match child.try_wait() {
                    Ok(Some(exit)) => break (Some(exit), None),
                    Ok(None) if Instant::now() >= deadline => {
                        // SAFETY: kill has no memory preconditions; the hook
                        // leads its own process group, so descendants stop too.
                        unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                        break (child.wait().ok(), Some("update hook timed out".to_owned()));
                    }
                    Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                    Err(e) => break (None, Some(format!("wait failed: {e}"))),
                }
            };
            // A daemonized descendant may hold the pipe open; never wait on it.
            let collected = Instant::now() + Duration::from_secs(1);
            while !collector.is_finished() && Instant::now() < collected {
                std::thread::sleep(Duration::from_millis(20));
            }
            let (bytes, truncated) = output.lock().unwrap().clone();
            let mut status = shared.lock().unwrap();
            status.finished_unix_ms = Some(now_ms());
            status.exit_code = exit.and_then(|e| e.code());
            status.signal = exit.and_then(|e| e.signal());
            status.output = String::from_utf8_lossy(&bytes).into_owned();
            status.output_truncated = truncated;
            status.state = if error.is_none() && exit.is_some_and(|e| e.success()) {
                UpdateState::Succeeded
            } else {
                UpdateState::Failed
            };
            status.error = error.or_else(|| {
                exit.filter(|e| !e.success())
                    .map(|e| format!("update hook exited with {e}"))
            });
            eprintln!(
                "qrab-guest-agent: update run {} finished: {:?}",
                status.run_id, status.state
            );
        });
        Ok(snapshot)
    }
}
