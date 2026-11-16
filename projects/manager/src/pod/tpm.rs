use super::{persistent::StateLease, process::ManagedChild, Tpm};
use crate::{Error, Result};
use std::{
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{
    process::{Child, Command},
    time::{timeout, Instant},
};

pub(super) struct Swtpm {
    child: ManagedChild,
    socket: PathBuf,
    permanent: PathBuf,
}

impl Swtpm {
    pub(super) fn spawn(
        config: &Tpm,
        session: &Path,
        state: &Path,
        lease: StateLease,
    ) -> Result<Self> {
        let socket = session.join("swtpm.sock");
        let child = Command::new(&config.swtpm_binary)
            .args(["socket", "--tpm2", "--tpmstate"])
            .arg(format!("dir={},mode=0600", state.display()))
            .arg("--ctrl")
            .arg(format!("type=unixio,path={},mode=0600", socket.display()))
            .args(["--flags", "not-need-init"])
            // A parent environment must not cause an unexpected TCP listener or
            // redirect identity state to a different directory.
            .env_remove("TPM_PORT")
            .env_remove("TPM_PATH")
            .env_remove("NOTIFY_SOCKET")
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        Ok(Self {
            child: ManagedChild::new(child, Some(lease)),
            socket,
            permanent: state.join("tpm2-00.permall"),
        })
    }

    pub(super) fn id(&self) -> Option<u32> {
        Some(self.child.spawned_pid())
    }

    pub(super) async fn ready(&mut self) -> Result<()> {
        loop {
            if let Some(exit) = self.child.try_wait()? {
                return Err(Error::Spawn(format!("swtpm exited during startup: {exit}")));
            }
            if std::fs::symlink_metadata(&self.socket).is_ok_and(|m| m.file_type().is_socket())
                && std::fs::symlink_metadata(&self.permanent)
                    .is_ok_and(|m| m.file_type().is_file() && m.len() > 0)
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    pub(super) async fn wait(&mut self) -> Result<ExitStatus> {
        Ok(self.child.wait().await?)
    }

    pub(super) async fn stop(&mut self, require_clean: bool) -> Result<()> {
        if let Some(exit) = self.child.try_wait()? {
            if require_clean && !exit.success() {
                return Err(Error::Spawn(
                    "swtpm did not exit cleanly; native disk release is not proven".into(),
                ));
            }
            return Ok(());
        }
        if let Some(result) = terminate(&mut self.child, Duration::from_secs(2)).await {
            let exit = result?;
            if require_clean && !exit.success() {
                return Err(Error::Spawn(
                    "swtpm did not exit cleanly; native disk release is not proven".into(),
                ));
            }
            return Ok(());
        }
        self.child.start_kill()?;
        timeout(Duration::from_secs(2), self.child.wait())
            .await
            .map_err(|_| Error::Spawn("swtpm did not reap after SIGKILL".into()))??;
        if require_clean {
            Err(Error::Spawn(
                "swtpm required SIGKILL; native disk release is not proven".into(),
            ))
        } else {
            Ok(())
        }
    }
}

/// How often [`terminate`] repeats SIGTERM while swtpm is still running.
const RESIGNAL: Duration = Duration::from_millis(100);

/// SIGTERM `child` until it exits, for at most `within`; `None` when it is
/// still running at the deadline.
///
/// swtpm 0.10 can consume a SIGTERM that arrives while it handles a client
/// disconnect and then keep running (busy, nothing pending); a later SIGTERM
/// still ends it cleanly. The handler is idempotent, so repeating is safe.
pub(super) async fn terminate(
    child: &mut Child,
    within: Duration,
) -> Option<std::io::Result<ExitStatus>> {
    let deadline = Instant::now() + within;
    loop {
        if let Some(pid) = child.id() {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGTERM,
            );
        }
        let step = deadline
            .saturating_duration_since(Instant::now())
            .min(RESIGNAL);
        if let Ok(result) = timeout(step, child.wait()).await {
            return Some(result);
        }
        if Instant::now() >= deadline {
            return None;
        }
    }
}

pub(super) async fn wait(tpm: &mut Option<Swtpm>) -> Result<ExitStatus> {
    match tpm {
        Some(tpm) => tpm.wait().await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::UnixStream};

    const SWTPM: &str = "/usr/bin/swtpm";

    /// swtpm 0.10.2 ignored the single SIGTERM in about 1 of 5 runs of this
    /// sequence, so 50 runs fail reliably without the repeated signal.
    #[tokio::test]
    async fn swtpm_stopped_right_after_a_client_disconnect_exits_cleanly() {
        if !Path::new(SWTPM).exists() {
            eprintln!("skipping: {SWTPM} is not installed");
            return;
        }
        for run in 0..50 {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("swtpm.sock");
            let mut child = Command::new(SWTPM)
                .args(["socket", "--tpm2", "--tpmstate"])
                .arg(format!("dir={},mode=0600", dir.path().display()))
                .arg("--server")
                .arg(format!("type=unixio,path={},mode=0600", socket.display()))
                .args(["--flags", "not-need-init,startup-clear"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            let mut client = loop {
                match UnixStream::connect(&socket).await {
                    Ok(client) => break client,
                    Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
                }
            };
            // TPM2_GetRandom(8), so the disconnect follows real traffic.
            client
                .write_all(&[0x80, 0x01, 0, 0, 0, 0x0c, 0, 0, 0x01, 0x7b, 0, 0x08])
                .await
                .unwrap();
            let mut response = [0u8; 10];
            client.read_exact(&mut response).await.unwrap();
            drop(client);
            let exit = terminate(&mut child, Duration::from_secs(2))
                .await
                .unwrap_or_else(|| panic!("run {run}: swtpm ignored SIGTERM for 2 s"))
                .unwrap();
            assert!(exit.success(), "run {run}: swtpm exited with {exit}");
        }
    }
}
