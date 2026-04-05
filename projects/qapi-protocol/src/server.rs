//! Mock QMP and QGA servers for testing.
//!
//! These allow testing clients by simulating a QEMU or guest agent endpoint.

use anyhow::Result;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

/// A mock QMP server session (one connected client).
pub struct QmpSession {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
}

impl QmpSession {
    /// Accept a connection on the given listener and set up a session.
    pub async fn accept(listener: &UnixListener) -> Result<Self> {
        let (stream, _) = listener.accept().await?;
        let (read_half, write_half) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(read_half),
            writer: write_half,
        })
    }

    /// Create a session from an already-connected stream.
    pub fn from_stream(stream: UnixStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            reader: BufReader::new(read_half),
            writer: write_half,
        }
    }

    /// Send the QMP greeting (server → client).
    pub async fn send_greeting(&mut self, major: u32, minor: u32, micro: u32) -> Result<()> {
        let greeting = serde_json::json!({
            "QMP": {
                "version": {
                    "qemu": {"major": major, "minor": minor, "micro": micro},
                    "package": ""
                },
                "capabilities": ["oob"]
            }
        });
        self.writer.write_all(serde_json::to_string(&greeting)?.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Wait for qmp_capabilities from the client and send success response.
    pub async fn negotiate(&mut self) -> Result<()> {
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        let cmd: serde_json::Value = serde_json::from_str(line.trim())?;
        anyhow::ensure!(
            cmd["execute"].as_str() == Some("qmp_capabilities"),
            "Expected qmp_capabilities, got: {}",
            cmd
        );
        self.send_return(serde_json::json!({})).await?;
        Ok(())
    }

    /// Read the next command from the client.
    /// Returns (command_name, arguments).
    pub async fn recv_command(&mut self) -> Result<(String, serde_json::Value)> {
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        if line.is_empty() {
            anyhow::bail!("Client disconnected");
        }
        let cmd: serde_json::Value = serde_json::from_str(line.trim())?;
        let name = cmd["execute"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'execute' field"))?
            .to_string();
        let args = cmd.get("arguments").cloned().unwrap_or(serde_json::json!({}));
        Ok((name, args))
    }

    /// Send a success response.
    pub async fn send_return(&mut self, value: serde_json::Value) -> Result<()> {
        let resp = serde_json::json!({"return": value});
        self.writer.write_all(serde_json::to_string(&resp)?.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Send an error response.
    pub async fn send_error(&mut self, class: &str, desc: &str) -> Result<()> {
        let resp = serde_json::json!({"error": {"class": class, "desc": desc}});
        self.writer.write_all(serde_json::to_string(&resp)?.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Send an event to the client.
    pub async fn send_event(&mut self, name: &str, data: serde_json::Value) -> Result<()> {
        let evt = serde_json::json!({
            "event": name,
            "data": data,
            "timestamp": {"seconds": 0, "microseconds": 0}
        });
        self.writer.write_all(serde_json::to_string(&evt)?.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }
}

/// A mock QGA server session (one connected client).
pub struct QgaSession {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
}

impl QgaSession {
    /// Accept a connection on the given listener.
    pub async fn accept(listener: &UnixListener) -> Result<Self> {
        let (stream, _) = listener.accept().await?;
        let (read_half, write_half) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(read_half),
            writer: write_half,
        })
    }

    /// Handle the guest-sync-delimited handshake from a client.
    /// Reads the 0xFF sentinel + sync command, sends 0xFF + sync response.
    pub async fn handle_sync(&mut self) -> Result<i64> {
        use tokio::io::AsyncReadExt;

        // Read until 0xFF sentinel
        loop {
            let byte = self.reader.read_u8().await?;
            if byte == 0xFF {
                break;
            }
        }

        // Read the sync command
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        let cmd: serde_json::Value = serde_json::from_str(line.trim())?;
        let sync_id = cmd["arguments"]["id"].as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing sync id"))?;

        // Send 0xFF sentinel + response
        self.writer.write_all(&[0xFF]).await?;
        let resp = serde_json::json!({"return": sync_id});
        self.writer.write_all(serde_json::to_string(&resp)?.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        Ok(sync_id)
    }

    /// Read the next command from the client.
    pub async fn recv_command(&mut self) -> Result<(String, serde_json::Value)> {
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        if line.is_empty() {
            anyhow::bail!("Client disconnected");
        }
        let cmd: serde_json::Value = serde_json::from_str(line.trim())?;
        let name = cmd["execute"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'execute' field"))?
            .to_string();
        let args = cmd.get("arguments").cloned().unwrap_or(serde_json::json!({}));
        Ok((name, args))
    }

    /// Send a success response.
    pub async fn send_return(&mut self, value: serde_json::Value) -> Result<()> {
        let resp = serde_json::json!({"return": value});
        self.writer.write_all(serde_json::to_string(&resp)?.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Send an error response.
    pub async fn send_error(&mut self, class: &str, desc: &str) -> Result<()> {
        let resp = serde_json::json!({"error": {"class": class, "desc": desc}});
        self.writer.write_all(serde_json::to_string(&resp)?.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }
}
