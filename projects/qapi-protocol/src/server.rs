//! Bounded QMP/QGA test sessions using the same JSON framing as the clients.
use crate::{
    framing::{self, JsonReader},
    wire, ClientOptions, Error, Result,
};
use serde_json::{json, Value};
use tokio::{
    io::AsyncWriteExt,
    net::{
        unix::{OwnedReadHalf, OwnedWriteHalf},
        UnixListener, UnixStream,
    },
};

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub name: String,
    pub arguments: Value,
    pub oob: bool,
    pub id: Option<Value>,
}

pub struct QmpReader {
    reader: JsonReader<OwnedReadHalf>,
}
pub struct QmpWriter {
    writer: OwnedWriteHalf,
}

impl QmpReader {
    pub async fn recv_command_full(&mut self) -> Result<ParsedCommand> {
        let mut value = self.reader.next().await?;
        let id = value.as_object_mut().and_then(|o| o.remove("id"));
        let oob = value.get("exec-oob").is_some();
        wire::validate_request(&value, oob)?;
        let verb = if oob { "exec-oob" } else { "execute" };
        Ok(ParsedCommand {
            name: value[verb].as_str().unwrap().to_owned(),
            arguments: value.get("arguments").cloned().unwrap_or_else(|| json!({})),
            oob,
            id,
        })
    }
}

impl QmpWriter {
    async fn send(&mut self, value: Value) -> Result<()> {
        framing::write_json(
            &mut self.writer,
            &value,
            ClientOptions::default().max_message_bytes,
        )
        .await
    }
    pub async fn send_return_with_id(&mut self, value: Value, id: Option<Value>) -> Result<()> {
        let mut reply = json!({"return": value});
        if let Some(id) = id {
            reply["id"] = id;
        }
        self.send(reply).await
    }
    pub async fn send_error_with_id(
        &mut self,
        class: &str,
        desc: &str,
        id: Option<Value>,
    ) -> Result<()> {
        let mut reply = json!({"error": {"class": class, "desc": desc}});
        if let Some(id) = id {
            reply["id"] = id;
        }
        self.send(reply).await
    }
    pub async fn send_event(&mut self, name: &str, data: Value) -> Result<()> {
        self.send(
            json!({"event": name, "data": data, "timestamp": {"seconds": 0, "microseconds": 0}}),
        )
        .await
    }
}

pub struct QmpSession {
    reader: QmpReader,
    writer: QmpWriter,
    last_id: Option<Value>,
}

impl QmpSession {
    pub async fn accept(listener: &UnixListener) -> Result<Self> {
        Ok(Self::from_stream(listener.accept().await?.0))
    }
    pub fn from_stream(stream: UnixStream) -> Self {
        let (read, write) = stream.into_split();
        Self {
            reader: QmpReader {
                reader: JsonReader::new(read, ClientOptions::default().max_message_bytes),
            },
            writer: QmpWriter { writer: write },
            last_id: None,
        }
    }
    pub fn split(self) -> (QmpReader, QmpWriter) {
        (self.reader, self.writer)
    }
    pub async fn send_greeting(&mut self, major: u32, minor: u32, micro: u32) -> Result<()> {
        self.writer.send(json!({"QMP": {"version": {"qemu": {"major": major, "minor": minor, "micro": micro}, "package": ""}, "capabilities": ["oob"]}})).await
    }
    pub async fn negotiate(&mut self) -> Result<()> {
        let command = self.recv_command_full().await?;
        if command.name != "qmp_capabilities" || command.oob {
            return Err(Error::Protocol("expected in-band qmp_capabilities".into()));
        }
        if let Some(enable) = command.arguments.get("enable") {
            if enable != &json!(["oob"]) {
                return Err(Error::Protocol("unsupported capabilities".into()));
            }
        }
        self.send_return(json!({})).await
    }
    pub async fn recv_command(&mut self) -> Result<(String, Value)> {
        let command = self.recv_command_full().await?;
        Ok((command.name, command.arguments))
    }
    pub async fn recv_command_full(&mut self) -> Result<ParsedCommand> {
        let command = self.reader.recv_command_full().await?;
        self.last_id = command.id.clone();
        Ok(command)
    }
    /// Replies to the most recently received command. For pipelined mocks,
    /// preserve each ParsedCommand.id and use send_return_with_id instead.
    pub async fn send_return(&mut self, value: Value) -> Result<()> {
        self.writer
            .send_return_with_id(value, self.last_id.clone())
            .await
    }
    pub async fn send_return_with_id(&mut self, value: Value, id: Option<Value>) -> Result<()> {
        self.writer.send_return_with_id(value, id).await
    }
    pub async fn send_error(&mut self, class: &str, desc: &str) -> Result<()> {
        self.writer
            .send_error_with_id(class, desc, self.last_id.clone())
            .await
    }
    pub async fn send_event(&mut self, name: &str, data: Value) -> Result<()> {
        self.writer.send_event(name, data).await
    }
}

pub struct QgaSession {
    reader: QmpReader,
    writer: QmpWriter,
}
impl QgaSession {
    pub async fn accept(listener: &UnixListener) -> Result<Self> {
        Ok(Self::from_stream(listener.accept().await?.0))
    }
    pub fn from_stream(stream: UnixStream) -> Self {
        let session = QmpSession::from_stream(stream);
        Self {
            reader: session.reader,
            writer: session.writer,
        }
    }
    pub async fn handle_sync(&mut self) -> Result<i64> {
        self.reader.reader.reset_at_sentinel().await?;
        let command = self.reader.recv_command_full().await?;
        if command.name != "guest-sync-delimited" || command.oob {
            return Err(Error::Protocol("expected guest-sync-delimited".into()));
        }
        let nonce = command.arguments["id"]
            .as_i64()
            .ok_or_else(|| Error::Protocol("missing sync nonce".into()))?;
        let mut bytes = vec![0xff];
        bytes.extend(framing::encode(
            &json!({"return": nonce}),
            ClientOptions::default().max_message_bytes,
        )?);
        self.writer.writer.write_all(&bytes).await?;
        Ok(nonce)
    }
    pub async fn recv_command(&mut self) -> Result<(String, Value)> {
        let command = self.reader.recv_command_full().await?;
        if command.oob {
            return Err(Error::Protocol("QGA does not support OOB".into()));
        }
        Ok((command.name, command.arguments))
    }
    pub async fn send_return(&mut self, value: Value) -> Result<()> {
        self.writer.send_return_with_id(value, None).await
    }
    pub async fn send_error(&mut self, class: &str, desc: &str) -> Result<()> {
        self.writer.send_error_with_id(class, desc, None).await
    }
}
