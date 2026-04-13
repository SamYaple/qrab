//! QEMU Guest Agent (QGA) async client.
//!
//! The QGA protocol uses the same JSON-RPC wire format as QMP, but with
//! a different handshake (guest-sync-delimited instead of qmp_capabilities).

use anyhow::Result;
use qapi::QapiCommand;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// Async QGA client connected to a QEMU guest agent.
pub struct Qga {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
}

impl Qga {
    /// Connect to a QGA socket and perform the sync-delimited handshake.
    pub async fn new(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path).await?;
        let (read_half, write_half) = stream.into_split();
        let reader = BufReader::new(read_half);

        let mut qga = Self {
            reader,
            writer: write_half,
        };

        // Perform sync-delimited handshake
        qga.sync().await?;

        // Query guest-info to get the QGA version
        let info: qapi::qga::GuestAgentInfo =
            qga.execute_raw("guest-info", serde_json::json!({})).await?;
        qapi::set_qga_version(info.version);

        Ok(qga)
    }

    /// Perform the guest-sync-delimited handshake.
    async fn sync(&mut self) -> Result<()> {
        // If the system clock is before UNIX_EPOCH (embedded boards with no
        // RTC, severe clock skew, etc.) duration_since returns Err. For a
        // sync-id any value will do, so fall back to a zero-valued Duration
        // instead of panicking.
        //
        // `as_nanos()` returns u128; narrow to u64 first (which wraps around
        // every ~584 years) and then reinterpret as i64. A negative sync id
        // is harmless — the guest echoes it verbatim — and this preserves all
        // 64 bits of entropy instead of silently truncating to i64::MAX.
        let sync_id: i64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64 as i64;

        // Send 0xFF sentinel byte followed by the sync command
        self.writer.write_all(&[0xFF]).await?;
        let cmd = serde_json::json!({
            "execute": "guest-sync-delimited",
            "arguments": {"id": sync_id}
        });
        self.writer
            .write_all(serde_json::to_string(&cmd)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        // Read until we find 0xFF sentinel, then read the response
        loop {
            let byte = self.reader.read_u8().await?;
            if byte == 0xFF {
                break;
            }
        }

        // Read the sync response
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        let resp: serde_json::Value = serde_json::from_str(line.trim())?;

        if let Some(ret) = resp.get("return") {
            let returned_id = ret.as_i64().unwrap_or(0);
            if returned_id != sync_id {
                anyhow::bail!(
                    "guest-sync-delimited: id mismatch (sent {}, got {})",
                    sync_id,
                    returned_id
                );
            }
        } else if let Some(error) = resp.get("error") {
            anyhow::bail!("guest-sync-delimited failed: {}", error);
        }

        Ok(())
    }

    /// Execute a typed QGA command.
    pub async fn execute<C: QapiCommand>(&mut self, cmd: &C) -> Result<C::Returns> {
        let args = serde_json::to_value(cmd)?;
        self.execute_raw(C::NAME, args).await
    }

    /// Execute a command by name with raw JSON arguments.
    async fn execute_raw<T: serde::de::DeserializeOwned>(
        &mut self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<T> {
        let request = if args.is_object() && args.as_object().unwrap().is_empty() {
            serde_json::json!({ "execute": name })
        } else {
            serde_json::json!({ "execute": name, "arguments": args })
        };

        self.writer
            .write_all(serde_json::to_string(&request)?.as_bytes())
            .await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        let resp: serde_json::Value = serde_json::from_str(line.trim())?;

        if let Some(error) = resp.get("error") {
            anyhow::bail!(
                "QGA error {}: {}",
                error["class"].as_str().unwrap_or("unknown"),
                error["desc"].as_str().unwrap_or("unknown error")
            );
        }

        let ret = resp
            .get("return")
            .cloned()
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        // Handle () returns
        let ret = if ret.is_object() && ret.as_object().unwrap().is_empty() {
            match serde_json::from_value::<T>(ret.clone()) {
                Ok(v) => return Ok(v),
                Err(_) => serde_json::Value::Null,
            }
        } else {
            ret
        };

        Ok(serde_json::from_value(ret)?)
    }
}
