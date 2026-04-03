use qapi::QapiCommand;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Qmp(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {}", e),
            Error::Json(e) => write!(f, "JSON error: {}", e),
            Error::Qmp(e) => write!(f, "QMP error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct QmpConnection {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl QmpConnection {
    /// Connect to a QEMU QMP socket.
    pub fn connect(path: &str) -> Result<Self> {
        let stream = UnixStream::connect(path)?;
        let writer = stream.try_clone()?;
        let reader = BufReader::new(stream);
        Ok(QmpConnection { reader, writer })
    }

    /// Read the QMP greeting and send qmp_capabilities to complete the handshake.
    /// Returns the QEMU version as (major, minor, patch).
    pub fn negotiate(&mut self) -> Result<(u32, u32, u32)> {
        // Read greeting
        let greeting = self.read_response()?;
        let qmp = &greeting["QMP"]["version"]["qemu"];
        let major = qmp["major"].as_u64().unwrap_or(0) as u32;
        let minor = qmp["minor"].as_u64().unwrap_or(0) as u32;
        let micro = qmp["micro"].as_u64().unwrap_or(0) as u32;

        // Send qmp_capabilities
        self.send_raw(r#"{"execute": "qmp_capabilities"}"#)?;
        let resp = self.read_response()?;
        if resp.get("error").is_some() {
            return Err(Error::Qmp(format!("qmp_capabilities failed: {}", resp)));
        }

        Ok((major, minor, micro))
    }

    /// Execute a QapiCommand and return the deserialized result.
    pub fn execute<C: QapiCommand>(&mut self, cmd: &C) -> Result<C::Returns> {
        let args = serde_json::to_value(cmd)?;
        let request = if args.is_object() && args.as_object().unwrap().is_empty() {
            serde_json::json!({ "execute": C::NAME })
        } else {
            serde_json::json!({ "execute": C::NAME, "arguments": args })
        };

        self.send_raw(&serde_json::to_string(&request)?)?;

        // Read responses, skipping events until we get a return or error
        loop {
            let resp = self.read_response()?;
            if resp.get("event").is_some() {
                continue; // skip events
            }
            if let Some(error) = resp.get("error") {
                return Err(Error::Qmp(format!(
                    "{}: {}",
                    error["class"].as_str().unwrap_or("unknown"),
                    error["desc"].as_str().unwrap_or("unknown error")
                )));
            }
            if let Some(ret) = resp.get("return") {
                return Ok(serde_json::from_value(ret.clone())?);
            }
            return Err(Error::Qmp(format!("unexpected response: {}", resp)));
        }
    }

    fn send_raw(&mut self, msg: &str) -> Result<()> {
        self.writer.write_all(msg.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }

    fn read_response(&mut self) -> Result<Value> {
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        Ok(serde_json::from_str(&line)?)
    }
}
