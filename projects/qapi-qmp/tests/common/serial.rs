use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Serial console monitor that reads from a log file.
/// QEMU writes serial output to a file via `-serial file:<path>`.
/// We poll the file for the expected pattern.
pub struct SerialConsole {
    log_path: PathBuf,
}

impl SerialConsole {
    /// Create a new serial console monitor for a given log file.
    pub fn new(log_path: PathBuf) -> Self {
        Self { log_path }
    }

    /// Wait until the log file contains a line matching `pattern`, or timeout.
    pub async fn wait_for(&self, pattern: &str, timeout: Duration) -> Result<String> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut last_size = 0u64;

        loop {
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!(
                    "Timeout ({:?}) waiting for '{}' in serial log {}",
                    timeout, pattern, self.log_path.display()
                );
            }

            if let Ok(content) = tokio::fs::read_to_string(&self.log_path).await {
                if content.contains(pattern) {
                    return Ok(content);
                }
                let size = content.len() as u64;
                if size != last_size {
                    last_size = size;
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Wait for the cirros login prompt.
    pub async fn wait_for_boot(&self, timeout: Duration) -> Result<()> {
        self.wait_for("login:", timeout).await?;
        Ok(())
    }

    /// Path to the serial log file.
    pub fn path(&self) -> &Path {
        &self.log_path
    }
}
