use super::{persistent::StateLease, process::ManagedChild, Tpm};
use crate::{Error, Result};
use std::{
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::Duration,
};
use tokio::{process::Command, time::timeout};

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
        if let Some(pid) = self.child.id() {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGTERM,
            );
        }
        if let Ok(result) = timeout(Duration::from_secs(2), self.child.wait()).await {
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

pub(super) async fn wait(tpm: &mut Option<Swtpm>) -> Result<ExitStatus> {
    match tpm {
        Some(tpm) => tpm.wait().await,
        None => std::future::pending().await,
    }
}
