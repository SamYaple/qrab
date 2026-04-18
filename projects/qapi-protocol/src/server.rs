//! Mock QMP and QGA servers for testing.
//!
//! These allow testing clients by simulating a QEMU or guest agent endpoint.

use crate::{Error, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

/// A mock QMP server session (one connected client).
pub struct QmpSession {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
    /// The `id` from the most recently received command, echoed back in responses
    /// per the QMP spec.
    last_id: Option<serde_json::Value>,
}

impl QmpSession {
    /// Accept a connection on the given listener and set up a session.
    pub async fn accept(listener: &UnixListener) -> Result<Self> {
        let (stream, _) = listener.accept().await?;
        let (read_half, write_half) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(read_half),
            writer: write_half,
            last_id: None,
        })
    }

    /// Create a session from an already-connected stream.
    pub fn from_stream(stream: UnixStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            reader: BufReader::new(read_half),
            writer: write_half,
            last_id: None,
        }
    }

    /// Consume the session and return separate read and write halves.
    ///
    /// Concurrency-test mocks need this: a reader task can pump
    /// commands off the wire while spawned responder tasks dispatch
    /// replies through a shared writer (typically wrapped in
    /// `Arc<Mutex<…>>`). Without splitting, holding the session
    /// `&mut` across an `await` for `read_line` blocks every
    /// would-be writer.
    pub fn split(self) -> (QmpReader, QmpWriter) {
        (
            QmpReader {
                reader: self.reader,
            },
            QmpWriter {
                writer: self.writer,
            },
        )
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
        self.writer
            .write_all(serde_json::to_string(&greeting)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Wait for qmp_capabilities from the client and send success response.
    ///
    /// Accepts the bare `qmp_capabilities` form *and* the
    /// `arguments: {"enable": ["oob", ...]}` form the concurrent client
    /// emits when the greeting advertised OOB support — both are
    /// silently treated as "OK" by this mock.
    pub async fn negotiate(&mut self) -> Result<()> {
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        let cmd: serde_json::Value = serde_json::from_str(line.trim())?;
        if cmd["execute"].as_str() != Some("qmp_capabilities") {
            return Err(Error::Protocol(format!(
                "Expected qmp_capabilities, got: {}", cmd
            )));
        }
        // Accept arguments.enable arrays without inspecting them; real
        // QEMU validates each entry against the supported caps list, but
        // the mock simply echoes success so concurrent-client OOB
        // negotiation tests work out of the box.
        self.last_id = cmd.get("id").cloned();
        self.send_return(serde_json::json!({})).await?;
        Ok(())
    }

    /// Read the next command from the client.
    /// Returns (command_name, arguments). Stores the `id` field (if any)
    /// so subsequent `send_return`/`send_error` echo it back per QMP spec.
    ///
    /// Recognizes both `"execute"` (regular) and `"exec-oob"` (out-of-band)
    /// envelopes; the OOB-ness is preserved on the parsed command via
    /// [`recv_command_full`] when callers need it.
    pub async fn recv_command(&mut self) -> Result<(String, serde_json::Value)> {
        let parsed = self.recv_command_full().await?;
        Ok((parsed.name, parsed.arguments))
    }

    /// Read the next command from the client, returning the verb (OOB
    /// or not), the command name, the arguments, and the raw `id`.
    ///
    /// The structured form is what concurrency tests need: they
    /// inspect `oob` to decide whether to apply the artificial delay,
    /// and they queue the `id` so multiple commands can be answered in
    /// arbitrary order.
    pub async fn recv_command_full(&mut self) -> Result<ParsedCommand> {
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        if line.trim().is_empty() {
            return Err(Error::Protocol("Client disconnected".into()));
        }
        let cmd: serde_json::Value = serde_json::from_str(line.trim())?;
        let (name, oob) = if let Some(s) = cmd["execute"].as_str() {
            (s.to_string(), false)
        } else if let Some(s) = cmd["exec-oob"].as_str() {
            (s.to_string(), true)
        } else {
            return Err(Error::Protocol(
                "Missing 'execute' or 'exec-oob' field".into(),
            ));
        };
        let arguments = cmd
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let id = cmd.get("id").cloned();
        // For backward compat with `recv_command`, the most recent id
        // is also stashed on the session so `send_return`/`send_error`
        // (which take no id) keep echoing the right one.
        self.last_id = id.clone();
        Ok(ParsedCommand {
            name,
            arguments,
            oob,
            id,
        })
    }

    /// Send a success response.
    ///
    /// Echoes the `id` of the most recently received command (per QMP
    /// spec). For concurrent dispatch where multiple commands are
    /// pending, use [`send_return_with_id`] instead.
    pub async fn send_return(&mut self, value: serde_json::Value) -> Result<()> {
        let mut resp = serde_json::json!({"return": value});
        if let Some(id) = self.last_id.clone() {
            resp["id"] = id;
        }
        self.writer
            .write_all(serde_json::to_string(&resp)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Send a success response with an explicit `id`. Use this when
    /// answering pipelined commands out of order.
    pub async fn send_return_with_id(
        &mut self,
        value: serde_json::Value,
        id: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut resp = serde_json::json!({"return": value});
        if let Some(id) = id {
            resp["id"] = id;
        }
        self.writer
            .write_all(serde_json::to_string(&resp)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Send an error response.
    pub async fn send_error(&mut self, class: &str, desc: &str) -> Result<()> {
        let mut resp = serde_json::json!({"error": {"class": class, "desc": desc}});
        if let Some(id) = self.last_id.clone() {
            resp["id"] = id;
        }
        self.writer
            .write_all(serde_json::to_string(&resp)?.as_bytes())
            .await?;
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
        self.writer
            .write_all(serde_json::to_string(&evt)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }
}

/// Read half of a split [`QmpSession`].
pub struct QmpReader {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
}

impl QmpReader {
    /// Read the next command from the wire, returning the structured
    /// form. Mirrors [`QmpSession::recv_command_full`].
    pub async fn recv_command_full(&mut self) -> Result<ParsedCommand> {
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        if line.trim().is_empty() {
            return Err(Error::Protocol("Client disconnected".into()));
        }
        let cmd: serde_json::Value = serde_json::from_str(line.trim())?;
        let (name, oob) = if let Some(s) = cmd["execute"].as_str() {
            (s.to_string(), false)
        } else if let Some(s) = cmd["exec-oob"].as_str() {
            (s.to_string(), true)
        } else {
            return Err(Error::Protocol(
                "Missing 'execute' or 'exec-oob' field".into(),
            ));
        };
        let arguments = cmd
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let id = cmd.get("id").cloned();
        Ok(ParsedCommand {
            name,
            arguments,
            oob,
            id,
        })
    }
}

/// Write half of a split [`QmpSession`].
pub struct QmpWriter {
    writer: tokio::net::unix::OwnedWriteHalf,
}

impl QmpWriter {
    /// Send a success response with an explicit `id`. Mirrors
    /// [`QmpSession::send_return_with_id`].
    pub async fn send_return_with_id(
        &mut self,
        value: serde_json::Value,
        id: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut resp = serde_json::json!({"return": value});
        if let Some(id) = id {
            resp["id"] = id;
        }
        self.writer
            .write_all(serde_json::to_string(&resp)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Send an error response with an explicit `id`.
    pub async fn send_error_with_id(
        &mut self,
        class: &str,
        desc: &str,
        id: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut resp = serde_json::json!({"error": {"class": class, "desc": desc}});
        if let Some(id) = id {
            resp["id"] = id;
        }
        self.writer
            .write_all(serde_json::to_string(&resp)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }
}

/// One command received by [`QmpSession::recv_command_full`].
///
/// Carries the `oob` flag so concurrency tests can decide whether to
/// apply a fake delay before responding, plus the raw `id` for
/// out-of-order pipelined replies.
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub name: String,
    pub arguments: serde_json::Value,
    pub oob: bool,
    pub id: Option<serde_json::Value>,
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
        let sync_id = cmd["arguments"]["id"]
            .as_i64()
            .ok_or_else(|| Error::Protocol("Missing sync id".into()))?;

        // Send 0xFF sentinel + response
        self.writer.write_all(&[0xFF]).await?;
        let resp = serde_json::json!({"return": sync_id});
        self.writer
            .write_all(serde_json::to_string(&resp)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        Ok(sync_id)
    }

    /// Read the next command from the client.
    pub async fn recv_command(&mut self) -> Result<(String, serde_json::Value)> {
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        if line.trim().is_empty() {
            return Err(Error::Protocol("Client disconnected".into()));
        }
        let cmd: serde_json::Value = serde_json::from_str(line.trim())?;
        let name = cmd["execute"]
            .as_str()
            .ok_or_else(|| Error::Protocol("Missing 'execute' field".into()))?
            .to_string();
        let args = cmd
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        Ok((name, args))
    }

    /// Send a success response.
    pub async fn send_return(&mut self, value: serde_json::Value) -> Result<()> {
        let resp = serde_json::json!({"return": value});
        self.writer
            .write_all(serde_json::to_string(&resp)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }

    /// Send an error response.
    pub async fn send_error(&mut self, class: &str, desc: &str) -> Result<()> {
        let resp = serde_json::json!({"error": {"class": class, "desc": desc}});
        self.writer
            .write_all(serde_json::to_string(&resp)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }
}
