use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Child;

/// Manages a swtpm process providing an emulated TPM 2.0 device.
///
/// swtpm runs alongside the QEMU process, communicating via a Unix socket.
/// The socket path lives in the VM's private tmpdir, so it's automatically
/// cleaned up when the VM shuts down.
pub struct SwtpmProcess {
    child: Child,
    sock: PathBuf,
    #[allow(dead_code)]
    state_dir: PathBuf,
}

impl SwtpmProcess {
    /// Spawn a new swtpm instance.
    ///
    /// Creates a Unix socket at `{tmpdir}/swtpm.sock` and stores TPM state
    /// in `state_dir`. If `state_dir` is None, uses `{tmpdir}/tpm-state/`.
    ///
    /// swtpm command:
    /// ```sh
    /// swtpm socket \
    ///   --tpmstate dir={state_dir} \
    ///   --ctrl type=unixio,path={tmpdir}/swtpm.sock \
    ///   --tpm2 \
    ///   --log level=1
    /// ```
    pub async fn spawn(
        tmpdir: &Path,
        state_dir: Option<PathBuf>,
        swtpm_binary: Option<&Path>,
    ) -> Result<Self> {
        let binary = match swtpm_binary {
            Some(p) => p.to_path_buf(),
            None => which::which("swtpm").map_err(|_| {
                anyhow::anyhow!(
                    "swtpm binary not found in PATH — install swtpm \
                     (e.g. `apt install swtpm` or `dnf install swtpm`)"
                )
            })?,
        };

        let state_dir = state_dir.unwrap_or_else(|| tmpdir.join("tpm-state"));
        tokio::fs::create_dir_all(&state_dir).await.map_err(|e| {
            anyhow::anyhow!(
                "failed to create swtpm state directory {}: {e}",
                state_dir.display()
            )
        })?;

        let sock = tmpdir.join("swtpm.sock");

        let child = tokio::process::Command::new(&binary)
            .arg("socket")
            .arg("--tpmstate")
            .arg(format!("dir={}", state_dir.display()))
            .arg("--ctrl")
            .arg(format!("type=unixio,path={}", sock.display()))
            .arg("--tpm2")
            .arg("--log")
            .arg("level=1")
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                anyhow::anyhow!("failed to spawn swtpm at {}: {e}", binary.display())
            })?;

        // Wait for socket to appear (poll up to 5s).
        let mut appeared = false;
        for _ in 0..50 {
            if tokio::fs::try_exists(&sock).await.unwrap_or(false) {
                appeared = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if !appeared {
            anyhow::bail!(
                "swtpm socket did not appear at {} within 5s",
                sock.display()
            );
        }

        tracing::debug!(
            sock = %sock.display(),
            state_dir = %state_dir.display(),
            "swtpm started"
        );

        Ok(Self {
            child,
            sock,
            state_dir,
        })
    }

    /// Path to the swtpm control socket (for QEMU chardev).
    pub fn sock(&self) -> &Path {
        &self.sock
    }

    /// Kill the swtpm process and wait for exit.
    pub async fn cleanup(&mut self) {
        let _ = self.child.start_kill();
        let _ = self.child.wait().await;
    }
}

impl Drop for SwtpmProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
