use crate::{Error, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

/// Partial input lives in the reader, not in a disposable read future. This
/// makes selecting reads against outbound requests and shutdown cancellation safe.
pub(crate) struct JsonReader<R> {
    reader: BufReader<R>,
    partial: Vec<u8>,
    limit: usize,
    depth: usize,
    pub last_message_bytes: usize,
    in_string: bool,
    escaped: bool,
}

impl<R: AsyncRead + Unpin> JsonReader<R> {
    pub fn new(reader: R, limit: usize) -> Self {
        Self {
            reader: BufReader::new(reader),
            partial: Vec::new(),
            limit,
            depth: 0,
            last_message_bytes: 0,
            in_string: false,
            escaped: false,
        }
    }

    pub async fn next(&mut self) -> Result<Value> {
        loop {
            let available = self.reader.fill_buf().await?;
            if available.is_empty() {
                return if self.partial.is_empty() {
                    Err(Error::Closed)
                } else {
                    Err(Error::Protocol("truncated JSON message at EOF".into()))
                };
            }
            // QMP pretty output contains newlines inside the JSON object.
            // Locate the terminator without reparsing an ever-growing prefix.
            let mut end = None;
            for (index, byte) in available.iter().copied().enumerate() {
                if byte == b'\n' && self.depth == 0 && !self.in_string {
                    end = Some(index);
                    break;
                }
                if self.in_string {
                    if self.escaped {
                        self.escaped = false;
                    } else if byte == b'\\' {
                        self.escaped = true;
                    } else if byte == b'"' {
                        self.in_string = false;
                    }
                } else {
                    match byte {
                        b'"' => self.in_string = true,
                        b'{' | b'[' => self.depth += 1,
                        b'}' | b']' => self.depth = self.depth.saturating_sub(1),
                        _ => {}
                    }
                }
            }
            let take = end.unwrap_or(available.len());
            // A CR belongs to the terminator; leave one byte of allowance until LF.
            if self.partial.len().saturating_add(take) > self.limit.saturating_add(1) {
                return Err(Error::Protocol(format!(
                    "JSON message exceeds {} bytes",
                    self.limit
                )));
            }
            self.partial.extend_from_slice(&available[..take]);
            self.reader.consume(take + usize::from(end.is_some()));
            if end.is_some() {
                if self.partial.last() == Some(&b'\r') {
                    self.partial.pop();
                }
                if self.partial.len() > self.limit {
                    return Err(Error::Protocol(format!(
                        "JSON message exceeds {} bytes",
                        self.limit
                    )));
                }
                self.escaped = false;
                self.last_message_bytes = self.partial.len();
                let bytes = std::mem::take(&mut self.partial);
                if bytes.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                return Ok(serde_json::from_slice(&bytes)?);
            }
        }
    }

    pub async fn reset_at_sentinel(&mut self) -> Result<()> {
        self.partial.clear();
        self.depth = 0;
        self.in_string = false;
        self.escaped = false;
        while self.reader.read_u8().await? != 0xff {}
        Ok(())
    }

    /// Return a JSON candidate following a QGA sentinel, resetting at *every*
    /// sentinel and ignoring stale/malformed data. The QGA handshake validates
    /// the candidate's envelope and nonce under its overall deadline.
    pub async fn sync_message(&mut self) -> Result<Value> {
        self.partial.clear();
        self.depth = 0;
        self.in_string = false;
        self.escaped = false;
        let mut started = false;
        loop {
            let byte = self.reader.read_u8().await?;
            match byte {
                0xff => {
                    self.partial.clear();
                    started = true;
                }
                b'\n' if started => {
                    let bytes = std::mem::take(&mut self.partial);
                    if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                        return Ok(value);
                    }
                }
                _ if started => {
                    if self.partial.len() >= self.limit {
                        // Stale data is not a protocol reply. Keep scanning for
                        // another sentinel without retaining an unbounded buffer.
                        self.partial.clear();
                        started = false;
                    } else {
                        self.partial.push(byte);
                    }
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn encode(value: &Value, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value)?;
    if bytes.len() > limit {
        return Err(Error::InvalidRequest(format!(
            "JSON request exceeds {limit} bytes"
        )));
    }
    bytes.extend_from_slice(b"\r\n");
    Ok(bytes)
}

/// write_all can partially write or be cancelled. The owner must discard the
/// session on any failure; putting bytes in one buffer does not make I/O atomic.
pub(crate) async fn write_json<W: AsyncWrite + Unpin>(
    writer: &mut W,
    value: &Value,
    limit: usize,
) -> Result<()> {
    let bytes = encode(value, limit)?;
    writer.write_all(&bytes).await?;
    Ok(())
}
