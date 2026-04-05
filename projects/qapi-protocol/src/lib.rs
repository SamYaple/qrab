pub mod qga;
pub mod server;

use anyhow::Result;
use qapi::QapiCommand;

use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::{spawn, JoinHandle};

/// A raw QMP event as received from QEMU.
#[derive(Debug, serde::Deserialize)]
pub struct QmpEvent {
    pub event: String,
    pub timestamp: Timestamp,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// QMP event timestamp.
#[derive(Debug, serde::Deserialize)]
pub struct Timestamp {
    pub seconds: i64,
    pub microseconds: u32,
}

/// Async QMP client connected to a QEMU instance.
#[derive(Debug)]
pub struct Qmp {
    command_tx: mpsc::Sender<(String, oneshot::Sender<serde_json::Value>)>,
    shutdown_signal: watch::Sender<()>,
    read_handle: Option<JoinHandle<()>>,
    write_handle: Option<JoinHandle<()>>,
    event_handle: Option<JoinHandle<()>>,
}

impl Qmp {
    /// Connect to a QEMU QMP socket and perform the capabilities negotiation.
    ///
    /// Returns the `Qmp` handle and a receiver for QMP events.
    /// Also sets the global QEMU version via `qapi::set_qemu_version()`.
    pub async fn new(socket_path: &Path) -> Result<(Self, mpsc::Receiver<QmpEvent>)> {
        let stream = UnixStream::connect(socket_path).await?;
        let (read_half, write_half) = stream.into_split();

        let mut reader = BufReader::new(read_half);

        // Read greeting synchronously before spawning workers
        let mut greeting_line = String::new();
        reader.read_line(&mut greeting_line).await?;
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&greeting_line) {
            if let Some(qemu) = parsed
                .get("QMP")
                .and_then(|q| q.get("version"))
                .and_then(|v| v.get("qemu"))
            {
                let major = qemu["major"].as_u64().unwrap_or(0) as u32;
                let minor = qemu["minor"].as_u64().unwrap_or(0) as u32;
                let micro = qemu["micro"].as_u64().unwrap_or(0) as u32;
                qapi::set_qemu_version(major, minor, micro);
            }
        }

        // Send qmp_capabilities and read the response
        let mut write_half = write_half;
        write_half
            .write_all(b"{\"execute\":\"qmp_capabilities\"}\n")
            .await?;

        let mut caps_line = String::new();
        reader.read_line(&mut caps_line).await?;
        let caps_resp: serde_json::Value = serde_json::from_str(&caps_line)?;
        if let Some(error) = caps_resp.get("error") {
            anyhow::bail!(
                "qmp_capabilities failed: {}",
                error["desc"].as_str().unwrap_or("unknown")
            );
        }

        // Now spawn the background workers
        let lines = reader.lines();
        let (command_tx, command_rx) = mpsc::channel(1);
        let (response_tx, response_rx) = mpsc::channel(1);
        let (raw_event_tx, raw_event_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);
        let (shutdown_tx, shutdown_rx) = watch::channel(());

        let read_handle = spawn(Self::read_loop(
            lines,
            response_tx,
            raw_event_tx,
            shutdown_rx.clone(),
        ));

        let write_handle = spawn(Self::write_loop(
            write_half,
            command_rx,
            response_rx,
            shutdown_rx.clone(),
        ));

        let event_handle = spawn(Self::event_worker(
            raw_event_rx,
            shutdown_rx.clone(),
            event_tx,
        ));

        let qmp = Self {
            command_tx,
            shutdown_signal: shutdown_tx,
            read_handle: Some(read_handle),
            write_handle: Some(write_handle),
            event_handle: Some(event_handle),
        };

        Ok((qmp, event_rx))
    }

    /// Execute a typed QMP command and deserialize the response.
    pub async fn execute<C: QapiCommand>(&self, cmd: &C) -> Result<C::Returns> {
        let args = serde_json::to_value(cmd)?;
        let request = if args.is_object() && args.as_object().unwrap().is_empty() {
            serde_json::json!({ "execute": C::NAME })
        } else {
            serde_json::json!({ "execute": C::NAME, "arguments": args })
        };

        let request_str = serde_json::to_string(&request)?;
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send((request_str, reply_tx))
            .await
            .map_err(|_| anyhow::anyhow!("command channel closed"))?;

        let response = reply_rx.await?;

        if let Some(error) = response.get("error") {
            anyhow::bail!(
                "QMP error {}: {}",
                error["class"].as_str().unwrap_or("unknown"),
                error["desc"].as_str().unwrap_or("unknown error")
            );
        }

        let ret = response
            .get("return")
            .cloned()
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        // Handle commands that return () — QMP returns {"return":{}} which
        // can't deserialize as (). Treat empty objects as null for unit types.
        let ret = if ret.is_object() && ret.as_object().unwrap().is_empty() {
            // Try deserializing as-is first; fall back to null for unit return types.
            match serde_json::from_value::<C::Returns>(ret.clone()) {
                Ok(v) => return Ok(v),
                Err(_) => serde_json::Value::Null,
            }
        } else {
            ret
        };

        Ok(serde_json::from_value(ret)?)
    }

    /// Gracefully shut down all background tasks.
    pub async fn shutdown(&mut self) {
        let _ = self.shutdown_signal.send(());
        if let Some(handle) = self.write_handle.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.event_handle.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.read_handle.take() {
            let _ = handle.await;
        }
    }

    async fn read_loop(
        mut lines: Lines<BufReader<OwnedReadHalf>>,
        response_tx: mpsc::Sender<serde_json::Value>,
        event_tx: mpsc::Sender<String>,
        mut shutdown_rx: watch::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                result = lines.next_line() => {
                    match result {
                        Ok(Some(line)) => {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&line) {
                                if parsed.get("event").is_some() {
                                    let _ = event_tx.send(line).await;
                                } else {
                                    let _ = response_tx.send(parsed).await;
                                }
                            }
                        }
                        Ok(None) | Err(_) => break,
                    }
                }
                _ = shutdown_rx.changed() => break,
            }
        }
    }

    async fn write_loop(
        mut write_half: OwnedWriteHalf,
        mut command_rx: mpsc::Receiver<(String, oneshot::Sender<serde_json::Value>)>,
        mut response_rx: mpsc::Receiver<serde_json::Value>,
        mut shutdown_rx: watch::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                Some((command, reply_tx)) = command_rx.recv() => {
                    if write_half.write_all(command.as_bytes()).await.is_err() {
                        break;
                    }
                    let _ = write_half.write_all(b"\n").await;

                    match response_rx.recv().await {
                        Some(response) => { let _ = reply_tx.send(response); }
                        None => break,
                    }
                }
                _ = shutdown_rx.changed() => break,
            }
        }
    }

    async fn event_worker(
        mut raw_event_rx: mpsc::Receiver<String>,
        mut shutdown_rx: watch::Receiver<()>,
        event_tx: mpsc::Sender<QmpEvent>,
    ) {
        loop {
            tokio::select! {
                Some(raw) = raw_event_rx.recv() => {
                    if let Ok(event) = serde_json::from_str::<QmpEvent>(&raw) {
                        let _ = event_tx.send(event).await;
                    }
                }
                _ = shutdown_rx.changed() => break,
            }
        }
    }
}
