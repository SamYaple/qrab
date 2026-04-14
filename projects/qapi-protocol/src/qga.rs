//! QEMU Guest Agent (QGA) async client.
//!
//! The QGA protocol uses the same JSON-RPC wire format as QMP, but with
//! a different handshake (guest-sync-delimited instead of qmp_capabilities).

use crate::{read_line_bounded, LineRead, MAX_LINE_BYTES};
use anyhow::Result;
use qapi::QapiCommand;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// Maximum time to wait for a QGA response before giving up. A wedged guest
/// agent must not block the caller forever.
const QGA_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum time to wait for the `guest-sync-delimited` handshake to complete.
/// Separate from — and shorter than — `QGA_RESPONSE_TIMEOUT`: sync is a
/// zero-work ping, so any QGA that can't echo it in 10s is effectively dead
/// and the caller should not wait the full command budget.
const QGA_SYNC_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum time to wait for `connect(2)` on the QGA socket. Unlike a hot
/// QMP monitor, a guest agent may be quiescent but the connect itself
/// should still return promptly on a live system; 5s matches `Qmp::new`.
const QGA_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum serialized size of a QGA request. Real QGA commands cap out
/// at a few KiB (file writes go through `guest-file-write` which itself
/// base64-encodes payloads in chunks, so even a large push arrives as
/// many small requests). 256 KiB is comfortably above any expected
/// command and catches accidental unbounded growth — e.g. a caller
/// shipping a multi-megabyte argument blob — before it hits the wire.
const MAX_REQUEST_BYTES: usize = 256 * 1024;

/// Async QGA client connected to a QEMU guest agent.
pub struct Qga {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
}

impl Qga {
    /// Connect to a QGA socket and perform the sync-delimited handshake.
    pub async fn new(socket_path: &Path) -> Result<Self> {
        // Bound `connect(2)` itself so a bound-but-unserved socket or
        // a hung filesystem can't wedge the caller indefinitely. See
        // `Qmp::new` for the same pattern.
        let stream = tokio::time::timeout(
            QGA_CONNECT_TIMEOUT,
            UnixStream::connect(socket_path),
        )
        .await
        .map_err(|_| anyhow::anyhow!("QGA connect timed out after 5s"))??;
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
        qapi::set_qga_version(normalize_qga_version(&info.version));

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

        // Wrap the entire handshake in a timeout. A wedged QGA (or one
        // flushing a long backlog of stale lines ahead of the 0xFF sentinel)
        // would otherwise block `Qga::new()` forever. We use a dedicated
        // `QGA_SYNC_TIMEOUT` rather than reusing the 30s response budget:
        // sync is a trivial echo, so 10s is more than enough and gives the
        // caller faster feedback on a dead agent.
        tokio::time::timeout(QGA_SYNC_TIMEOUT, async {
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
            let line = match read_line_bounded(&mut self.reader).await? {
                LineRead::Line(line) => line,
                LineRead::Eof => anyhow::bail!("QGA peer closed during sync"),
                LineRead::TooLong => {
                    anyhow::bail!("QGA sync response exceeded {} bytes", MAX_LINE_BYTES)
                }
            };
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

            Ok::<(), anyhow::Error>(())
        })
        .await
        .map_err(|_| anyhow::anyhow!("QGA sync handshake timed out"))??;

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
        let request = if args.as_object().is_some_and(|o| o.is_empty()) {
            serde_json::json!({ "execute": name })
        } else {
            serde_json::json!({ "execute": name, "arguments": args })
        };

        // Guard the serialized request size. A caller passing an
        // oversized argument blob (or a protocol bug producing one)
        // would otherwise write it straight to the guest socket, where
        // it hits the peer's own line limits and could hang a
        // ring-buffered virtio-serial channel.
        let request_str = serde_json::to_string(&request)?;
        anyhow::ensure!(
            request_str.len() <= MAX_REQUEST_BYTES,
            "QGA request too large: {} bytes (limit {})",
            request_str.len(),
            MAX_REQUEST_BYTES
        );

        self.writer.write_all(request_str.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        let line = tokio::time::timeout(
            QGA_RESPONSE_TIMEOUT,
            read_line_bounded(&mut self.reader),
        )
        .await
        .map_err(|_| anyhow::anyhow!("QGA command timed out"))??;
        let line = match line {
            LineRead::Line(line) => line,
            // `read_line_bounded` returns `Eof` at stream close, mirroring
            // the previous `read_line` Ok(0) handling — either way the
            // peer hung up and we bail with a meaningful message instead
            // of feeding an empty string into serde_json.
            LineRead::Eof => anyhow::bail!("QGA connection closed"),
            LineRead::TooLong => {
                anyhow::bail!("QGA response exceeded {} bytes", MAX_LINE_BYTES)
            }
        };
        if line.trim().is_empty() {
            anyhow::bail!("QGA connection closed");
        }
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
        let ret = if ret.as_object().is_some_and(|o| o.is_empty()) {
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

/// Normalize a `guest-info` version string before handing it to
/// `qapi::set_qga_version`, which does a strict `split('.')` parse.
///
/// Every released `qemu-ga` emits `"major.minor.patch"`, but real-world
/// inputs have included leading/trailing whitespace, a vendor `"v"`
/// prefix, or fewer than three components. `set_qga_version` falls back
/// to `(0, 0, 0)` on any parse failure, so these "almost-valid" strings
/// silently lost all version info. Trim whitespace and strip a leading
/// `v`/`V` here so `"v1"`, `" 1.0.0 "`, `"1.0"` and friends parse as
/// expected; truly malformed input still falls through to `(0, 0, 0)`
/// in `set_qga_version`, matching `qemu_version`'s missing-greeting
/// behavior.
fn normalize_qga_version(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    stripped.to_string()
}

#[cfg(test)]
mod tests {
    use super::normalize_qga_version;

    #[test]
    fn normalize_empty() {
        assert_eq!(normalize_qga_version(""), "");
    }

    #[test]
    fn normalize_single_number() {
        assert_eq!(normalize_qga_version("1"), "1");
    }

    #[test]
    fn normalize_two_components() {
        assert_eq!(normalize_qga_version("1.0"), "1.0");
    }

    #[test]
    fn normalize_triple() {
        assert_eq!(normalize_qga_version("1.0.0"), "1.0.0");
    }

    #[test]
    fn normalize_prerelease_suffix() {
        assert_eq!(normalize_qga_version("1.0.0rc1"), "1.0.0rc1");
    }

    #[test]
    fn normalize_leading_v() {
        assert_eq!(normalize_qga_version("v1"), "1");
        assert_eq!(normalize_qga_version("v1.2.3"), "1.2.3");
    }

    #[test]
    fn normalize_uppercase_v() {
        assert_eq!(normalize_qga_version("V2.0"), "2.0");
    }

    #[test]
    fn normalize_whitespace() {
        assert_eq!(normalize_qga_version(" 1.0.0 "), "1.0.0");
        assert_eq!(normalize_qga_version("\t1.2.3\n"), "1.2.3");
    }

    #[test]
    fn normalize_whitespace_and_v() {
        assert_eq!(normalize_qga_version("  v3.1.4  "), "3.1.4");
    }
}
