//! Sequential guest-agent transport. A timed-out or cancelled operation closes
//! its socket: the next connection must perform guest-sync-delimited again.
use crate::{
    framing::{self, JsonReader},
    wire, ClientOptions, Error, Result,
};
use qapi::QapiCommand;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::path::Path;
use tokio::{
    io::AsyncWriteExt,
    net::{
        unix::{OwnedReadHalf, OwnedWriteHalf},
        UnixStream,
    },
    time::timeout,
};

struct Session {
    reader: JsonReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

pub struct Qga {
    version: (u32, u32, u32),
    options: ClientOptions,
    session: Option<Session>,
}

impl std::fmt::Debug for Qga {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Qga")
            .field("version", &self.version)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl Qga {
    pub async fn new(path: &Path) -> Result<Self> {
        Self::connect_with_options(path, ClientOptions::default()).await
    }

    pub async fn connect_with_options(path: &Path, options: ClientOptions) -> Result<Self> {
        options.validate()?;
        let stream = timeout(options.connect_timeout, UnixStream::connect(path))
            .await
            .map_err(|_| Error::Timeout("QGA connect timed out".into()))??;
        Self::from_stream_with_options(stream, options).await
    }

    pub async fn from_stream(stream: UnixStream) -> Result<Self> {
        Self::from_stream_with_options(stream, ClientOptions::default()).await
    }

    pub async fn from_stream_with_options(
        stream: UnixStream,
        options: ClientOptions,
    ) -> Result<Self> {
        options.validate()?;
        let (reader, writer) = stream.into_split();
        let mut session = Session {
            reader: JsonReader::new(reader, options.max_message_bytes),
            writer,
        };
        let version = timeout(options.handshake_timeout, async {
            let mut nonce = [0_u8; 8];
            getrandom::fill(&mut nonce)
                .map_err(|error| std::io::Error::other(format!("QGA sync nonce: {error}")))?;
            let nonce = i64::from_ne_bytes(nonce);
            let command = wire::request("guest-sync-delimited", json!({"id": nonce}), false)?;
            let mut bytes = vec![0xff];
            bytes.extend(framing::encode(&command, options.max_message_bytes)?);
            session.writer.write_all(&bytes).await?;
            loop {
                let candidate = session.reader.sync_message().await?;
                if wire::response(candidate).is_ok_and(|value| value.as_i64() == Some(nonce)) {
                    break;
                }
            }
            let info = wire::request("guest-info", json!({}), false)?;
            framing::write_json(&mut session.writer, &info, options.max_message_bytes).await?;
            let info: qapi::qga::GuestAgentInfo =
                wire::decode(wire::response(session.reader.next().await?)?)?;
            parse_version(&info.version)
        })
        .await
        .map_err(|_| Error::Timeout("QGA handshake timed out".into()))??;
        Ok(Self {
            version,
            options,
            session: Some(session),
        })
    }

    pub fn version(&self) -> (u32, u32, u32) {
        self.version
    }
    pub fn is_closed(&self) -> bool {
        self.session.is_none()
    }
    pub fn shutdown(&mut self) {
        self.session.take();
    }

    /// Commands with success-response:false must use send_no_response, which
    /// explicitly acknowledges delivery rather than claiming remote success.
    pub async fn execute<C: QapiCommand>(&mut self, cmd: &C) -> Result<C::Returns> {
        if !C::SUCCESS_RESPONSE {
            return Err(Error::InvalidRequest(format!(
                "{} does not send a success response; use send_no_response",
                C::NAME
            )));
        }
        let args = qapi::with_version(self.version, || serde_json::to_value(cmd))?;
        self.execute_raw(C::NAME, args).await
    }

    pub async fn execute_raw<T: DeserializeOwned + 'static>(
        &mut self,
        name: &str,
        args: Value,
    ) -> Result<T> {
        let request = wire::request(name, args, false)?;
        let bytes = framing::encode(&request, self.options.max_message_bytes)?;
        // Ownership leaves self before any I/O await. Cancellation drops the
        // session, so a late reply cannot be mistaken for the next command.
        let mut session = self.session.take().ok_or(Error::Closed)?;
        let operation = async {
            session.writer.write_all(&bytes).await?;
            wire::response(session.reader.next().await?)
        };
        let result = timeout(self.options.command_timeout, operation)
            .await
            .map_err(|_| {
                Error::Timeout(
                    "QGA command timed out; reconnect and synchronize before reuse".into(),
                )
            })?;
        match result {
            Ok(value) => {
                self.session = Some(session);
                wire::decode(value)
            }
            Err(error @ Error::Qmp(_)) => {
                self.session = Some(session);
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    /// Send an explicitly response-less command and close the connection.
    /// Success only means the bytes were written. The agent can still fail the
    /// command or shut down later, and no remote success acknowledgement exists.
    pub async fn send_no_response<C: QapiCommand<Returns = ()>>(&mut self, cmd: &C) -> Result<()> {
        if C::SUCCESS_RESPONSE {
            return Err(Error::InvalidRequest(
                "command has a response; use execute".into(),
            ));
        }
        let request = qapi::with_version(self.version, || {
            wire::request(C::NAME, serde_json::to_value(cmd)?, false)
        })?;
        let bytes = framing::encode(&request, self.options.max_message_bytes)?;
        let mut session = self.session.take().ok_or(Error::Closed)?;
        timeout(
            self.options.command_timeout,
            session.writer.write_all(&bytes),
        )
        .await
        .map_err(|_| Error::Timeout("QGA write timed out".into()))??;
        Ok(())
    }
}

fn parse_version(raw: &str) -> Result<(u32, u32, u32)> {
    let raw = raw.trim().trim_start_matches(['v', 'V']);
    let parts: Vec<_> = raw.split('.').collect();
    let parse = || -> Option<(u32, u32, u32)> {
        if parts.len() != 3 {
            return None;
        }
        let patch: String = parts[2].chars().take_while(char::is_ascii_digit).collect();
        Some((
            parts[0].parse().ok()?,
            parts[1].parse().ok()?,
            patch.parse().ok()?,
        ))
    };
    parse().ok_or_else(|| Error::Protocol(format!("invalid guest-agent version: {raw:?}")))
}
