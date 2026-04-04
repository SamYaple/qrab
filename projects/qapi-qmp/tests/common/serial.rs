use anyhow::Result;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;

/// Async serial console reader connected via unix socket.
pub struct SerialConsole {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
}

impl SerialConsole {
    /// Connect to a QEMU serial unix socket.
    pub async fn connect(path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(path).await?;
        let (read_half, _write_half) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(read_half),
        })
    }

    /// Wait until a line containing `pattern` appears, or timeout.
    /// Returns the matching line.
    pub async fn wait_for(&mut self, pattern: &str, timeout: Duration) -> Result<String> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let mut line = String::new();
            match tokio::time::timeout_at(deadline, self.reader.read_line(&mut line)).await {
                Ok(Ok(0)) => anyhow::bail!("Serial console EOF while waiting for '{}'", pattern),
                Ok(Ok(_)) => {
                    if line.contains(pattern) {
                        return Ok(line);
                    }
                }
                Ok(Err(e)) => anyhow::bail!("Serial read error: {}", e),
                Err(_) => anyhow::bail!("Timeout waiting for '{}' on serial console", pattern),
            }
        }
    }

    /// Wait for the cirros login prompt.
    pub async fn wait_for_boot(&mut self, timeout: Duration) -> Result<()> {
        self.wait_for("login:", timeout).await?;
        Ok(())
    }
}
