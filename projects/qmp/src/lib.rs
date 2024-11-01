use anyhow::Result;

use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{
    unix::{OwnedReadHalf, OwnedWriteHalf},
    UnixStream,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::{spawn, JoinHandle};

pub use qapi_old_hack as qapi;
use qapi::{Event, QMPCommand};

#[derive(Debug)]
pub struct QMP {
    command: mpsc::Sender<(QMPCommand, oneshot::Sender<String>)>,
    shutdown_signal: watch::Sender<()>,
    read_handle: Option<JoinHandle<()>>,
    write_handle: Option<JoinHandle<()>>,
    event_handle: Option<JoinHandle<()>>,
}

impl QMP {
    pub async fn new(socket_path: &Path) -> Result<(Self, mpsc::Receiver<Event>)> {
        let stream = UnixStream::connect(socket_path).await?;
        let (stream_read_half, stream_write_half) = stream.into_split();

        let (command_tx, command_rx) = mpsc::channel(1);
        let (response_tx, response_rx) = mpsc::channel(1);
        let (event_tx, event_rx) = mpsc::channel(100);
        let (event_forwarder, event_receiver) = mpsc::channel(100);
        let (shutdown_tx, shutdown_rx) = watch::channel(());

        // Read loop
        let shutdown_rx_clone = shutdown_rx.clone();
        let read_handle = spawn(Self::read_loop(
            stream_read_half,
            response_tx,
            event_tx,
            shutdown_rx_clone,
        ));

        // Write loop
        let write_handle = spawn(Self::write_loop(
            stream_write_half,
            command_rx,
            response_rx,
            shutdown_rx.clone(),
        ));

        // Event loop
        let event_handle = spawn(Self::event_worker(
            event_rx,
            shutdown_rx.clone(),
            event_forwarder,
        ));

        let qmp = Self {
            command: command_tx,
            shutdown_signal: shutdown_tx,
            write_handle: Some(write_handle),
            read_handle: Some(read_handle),
            event_handle: Some(event_handle),
        };
        Ok((qmp, event_receiver))
    }

    pub async fn shutdown(&mut self) {
        self.shutdown_signal
            .send(())
            .expect("failed to send shutdown signal");
        if let Some(ref mut handle) = self.write_handle {
            handle
                .await
                .expect("async qmp write worker did not exit cleanly");
            self.write_handle = None;
        }
        if let Some(ref mut handle) = self.event_handle {
            handle
                .await
                .expect("async qmp event worker did not exit cleanly");
            self.event_handle = None;
        }
        if let Some(ref mut handle) = self.read_handle {
            handle
                .await
                .expect("async qmp read worker did not exit cleanly");
            self.read_handle = None;
        }
    }

    /// Execute a QMP command
    ///
    /// With QMP, some commands have an additional `allow-oob` flag that uses an
    /// async version of the normally synchronous command/response interaction
    /// over the socket. That async interface is used internally but not exposed
    /// here.
    ///
    /// TODO: this function is already async, the oob cabability should always
    /// be used when available, I dont want to expose the async command queue
    /// depth to the user of this crate.
    pub async fn execute(&mut self, cmd: QMPCommand) -> Result<String> {
        // Set up a message response channel for our command
        let (reply_tx, reply_rx) = oneshot::channel();

        // Send the command along with a channel for the response.
        self.command.send((cmd, reply_tx)).await.unwrap();
        let reply = reply_rx.await?;
        Ok(reply)
    }

    /// Spawn an async worker
    async fn event_worker(
        mut event_rx: mpsc::Receiver<String>,
        mut shutdown_rx: watch::Receiver<()>,
        event_forwarder: mpsc::Sender<Event>,
    ) {
        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    let event: Event = serde_json::from_str(&event).unwrap();
                    event_forwarder.send(event).await.unwrap();
                },
                _ = shutdown_rx.changed() => { break; },
            }
        }
    }

    async fn read_loop(
        read_half: OwnedReadHalf,
        response_tx: mpsc::Sender<String>,
        event_tx: mpsc::Sender<String>,
        mut shutdown_rx: watch::Receiver<()>,
    ) {
        let reader = BufReader::new(read_half);
        let mut lines = reader.lines();
        if let Some(_line) = lines.next_line().await.unwrap() {
            // init message -- version string and capabilties
        }
        if let Some(_line) = lines.next_line().await.unwrap() {
            // return message after cababilities negotiation query
        };
        loop {
            tokio::select! {
                Ok(Some(line)) = lines.next_line() => {
                    if line.contains("\"return\":") {
                        response_tx.send(line).await.unwrap();
                    } else if line.contains("\"event\":") {
                        event_tx.send(line).await.unwrap();
                    } else {
                        eprintln!("DEBUG: UNKNOWN response -- ```{}```", line);
                    }

                },
                _ = shutdown_rx.changed() => { break; },
            }
        }
    }

    async fn write_loop(
        mut write_half: OwnedWriteHalf,
        mut command_rx: mpsc::Receiver<(QMPCommand, oneshot::Sender<String>)>,
        mut response_rx: mpsc::Receiver<String>,
        mut shutdown_rx: watch::Receiver<()>,
    ) {
        let cmd = serde_json::to_string(&QMPCommand::QmpCapabilities).unwrap();
        write_half
            .write_all(cmd.as_bytes())
            .await
            .expect("Failed to send init message");
        loop {
            tokio::select! {
                Some((command, reply_tx)) = command_rx.recv() => {
                    let cmd = serde_json::to_string(&command).unwrap();
                    if let Err(e) = write_half.write_all(cmd.as_bytes()).await {
                        eprintln!("Failed to send command: {}", e);
                        continue;
                    }
                    match response_rx.recv().await {
                        Some(msg) => {
                            if let Err(e) = reply_tx.send(msg) {
                                eprintln!("Failed to send response: {}", e);
                            }
                        }
                        None => {
                            eprintln!("Response channel closed unexpectedly");
                            break;
                        }
                    }
                },
                _ = shutdown_rx.changed() => { break; },
            }
        }
    }
}
