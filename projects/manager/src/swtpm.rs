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

        // Initialize TPM state if the state directory is empty (first use).
        // Without this, OVMF hangs waiting for a valid TPM2_Startup response.
        let needs_setup = Self::state_dir_empty(&state_dir).await;
        if needs_setup {
            Self::run_setup(&binary, &state_dir).await?;
        }

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

    /// Check if state directory has no TPM state files.
    async fn state_dir_empty(state_dir: &Path) -> bool {
        let mut entries = match tokio::fs::read_dir(state_dir).await {
            Ok(e) => e,
            Err(_) => return true,
        };
        // Any file means state already exists (e.g. tpm2-00.permall).
        entries.next_entry().await.ok().flatten().is_none()
    }

    /// Run `swtpm_setup` to create initial TPM 2.0 state (EK, platform
    /// certificates). Uses the same binary directory as the swtpm binary
    /// to find `swtpm_setup`.
    async fn run_setup(swtpm_binary: &Path, state_dir: &Path) -> Result<()> {
        // swtpm_setup is typically installed alongside swtpm.
        let setup_binary = swtpm_binary
            .parent()
            .map(|p| p.join("swtpm_setup"))
            .unwrap_or_else(|| PathBuf::from("swtpm_setup"));

        tracing::debug!(
            state_dir = %state_dir.display(),
            "initializing swtpm state (first use)"
        );

        let output = tokio::process::Command::new(&setup_binary)
            .arg("--tpmstate")
            .arg(format!("dir={}", state_dir.display()))
            .arg("--tpm2")
            .arg("--createek")
            .output()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "failed to run swtpm_setup at {}: {e}",
                    setup_binary.display()
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "swtpm_setup failed (exit {}): {}",
                output.status,
                stderr.trim()
            );
        }

        tracing::debug!("swtpm_setup completed successfully");
        Ok(())
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
