//! Live migration between two managers of the same VM.
//!
//! The source manager serves a SPIFFE mTLS listener (TCP [`PORT`]) that admits
//! only a peer holding the source's own SPIFFE ID: every pod of one VM holds
//! the same per-VM ID, so no other workload can attach. The target manager
//! dials it, names the migration and the VM in a [`Hello`], and receives an
//! [`Offer`] with what it must reproduce (machine type, retained disk
//! bindings). Once its QEMU waits in `-incoming defer` it sends "ready" and
//! each manager splices its QEMU's local Unix socket with the TLS stream, so
//! QEMU never sees a TCP socket and the stream is encrypted end to end.
//!
//! Frames are single JSON lines before the QEMU stream starts; after "ready"
//! the connection carries only QEMU's migration stream (with return path).
use super::{iscsi::Binding, Config};
use crate::{Error, Result};
use qapi_protocol::Qmp;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
    sync::mpsc,
};

/// The source manager's migration listener port.
pub const PORT: u16 = 7800;
const VERSION: u32 = 1;
const FRAME_LIMIT: usize = 64 << 10;
const FRAME_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound on the target preparing swtpm and QEMU after the offer.
const READY_TIMEOUT: Duration = Duration::from_secs(120);
/// QEMU's own throttle, in bytes per second.
const MAX_BANDWIDTH: u64 = 1 << 30;

/// Source side: accept migrations of this VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outgoing {
    #[serde(default = "port")]
    pub port: u16,
    /// Workload API endpoint; defaults to `QRAB_SPIFFE_ENDPOINT_SOCKET`, then
    /// the csi.spiffe.io socket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_api: Option<String>,
}

fn port() -> u16 {
    PORT
}

/// Target side: start by receiving the VM from `source` instead of booting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Incoming {
    /// The migration's UID, echoed to the source and logged on both sides.
    pub migration: String,
    /// `host:port` of the source manager's listener.
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_api: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Hello {
    version: u32,
    migration: String,
    state_id: String,
}

/// What the target must reproduce to accept the source's stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Offer {
    /// Resolved machine type of the source, e.g. `pc-q35-11.1`.
    pub machine: String,
    /// Every iSCSI binding the source ever recorded, including removed IDs.
    pub iscsi_disks: BTreeMap<String, Binding>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Reply {
    Offer(Offer),
    Refused(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    ready: bool,
}

fn log(event: Value) {
    eprintln!("{event}");
}

fn failure(message: impl std::fmt::Display) -> Error {
    Error::Migration(message.to_string())
}

/// The VM's own identity: both sides accept only it.
async fn identity(
    endpoint: Option<&str>,
) -> Result<(qrab_spiffe::Identity, qrab_spiffe::SpiffeId)> {
    let identity = match endpoint {
        Some(endpoint) => qrab_spiffe::Identity::connect(endpoint).await,
        None => qrab_spiffe::Identity::from_env().await,
    }
    .map_err(failure)?;
    let id = identity.id().map_err(failure)?;
    Ok((identity, id))
}

async fn write_frame<W: AsyncWrite + Unpin, T: Serialize>(stream: &mut W, frame: &T) -> Result<()> {
    let mut line = serde_json::to_vec(frame)?;
    line.push(b'\n');
    stream.write_all(&line).await?;
    stream.flush().await?;
    Ok(())
}

/// Read one JSON line byte by byte, so nothing of the stream that follows is
/// consumed into a buffer.
async fn read_frame<R: AsyncRead + Unpin, T: for<'de> Deserialize<'de>>(
    stream: &mut R,
    limit: Duration,
) -> Result<T> {
    let read = async {
        let mut line = Vec::new();
        loop {
            let byte = stream.read_u8().await?;
            if byte == b'\n' {
                break;
            }
            if line.len() == FRAME_LIMIT {
                return Err(failure("migration frame is too long"));
            }
            line.push(byte);
        }
        Ok(serde_json::from_slice(&line)?)
    };
    tokio::time::timeout(limit, read)
        .await
        .map_err(|_| failure("migration peer did not answer in time"))?
}

/// An authenticated connection from a target, with its hello.
pub(super) struct Pending {
    stream: qrab_spiffe::server::TlsStream<TcpStream>,
    hello: Hello,
}

/// Accept target connections in the background. Connecting to the Workload
/// API is retried, so a VM starts even while SPIRE is unavailable; only
/// migration waits for it. One migration is handled at a time.
pub(super) fn listen(outgoing: &Outgoing) -> mpsc::Receiver<Pending> {
    let (sender, receiver) = mpsc::channel(1);
    let outgoing = outgoing.clone();
    tokio::spawn(async move {
        let (identity, id) = loop {
            match identity(outgoing.workload_api.as_deref()).await {
                Ok(found) => break found,
                Err(error) => {
                    log(
                        json!({"event":"migration_identity_unavailable","error":error.to_string()}),
                    );
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        };
        let acceptor = match identity.acceptor(std::slice::from_ref(&id)) {
            Ok(acceptor) => acceptor,
            Err(error) => {
                log(json!({"event":"migration_listener_failed","error":error.to_string()}));
                return;
            }
        };
        let listener = match TcpListener::bind(("0.0.0.0", outgoing.port)).await {
            Ok(listener) => listener,
            Err(error) => {
                log(json!({"event":"migration_listener_failed","error":error.to_string()}));
                return;
            }
        };
        log(json!({"event":"migration_listening","port":outgoing.port,"spiffe_id":id.to_string()}));
        loop {
            let Ok((tcp, peer)) = listener.accept().await else {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            };
            let acceptor = acceptor.clone();
            let sender = sender.clone();
            tokio::spawn(async move {
                let _ = tcp.set_nodelay(true);
                let mut stream = match acceptor.accept(tcp).await {
                    Ok(stream) => stream,
                    Err(error) => {
                        log(
                            json!({"event":"migration_peer_refused","peer":peer,"error":error.to_string()}),
                        );
                        return;
                    }
                };
                let hello: Hello = match read_frame(&mut stream, FRAME_TIMEOUT).await {
                    Ok(hello) => hello,
                    Err(error) => {
                        log(
                            json!({"event":"migration_peer_refused","peer":peer,"error":error.to_string()}),
                        );
                        return;
                    }
                };
                log(json!({"event":"migration_requested","peer":peer,"migration":hello.migration}));
                if let Err(mpsc::error::TrySendError::Full(mut pending)) =
                    sender.try_send(Pending { stream, hello })
                {
                    let _ = write_frame(
                        &mut pending.stream,
                        &Reply::Refused("another migration is in progress".into()),
                    )
                    .await;
                }
            });
        }
    });
    receiver
}

async fn machine_type(qmp: &Qmp) -> Result<String> {
    let value = qmp
        .execute_raw(json!({"execute":"qom-get","arguments":{"path":"/machine","property":"type"}}))
        .await?;
    let name = value
        .as_str()
        .ok_or_else(|| failure("QEMU reported no machine type"))?;
    Ok(name.strip_suffix("-machine").unwrap_or(name).to_owned())
}

async fn capabilities(qmp: &Qmp) -> Result<()> {
    qmp.execute_raw(
        json!({"execute":"migrate-set-capabilities","arguments":{"capabilities":[
            {"capability":"return-path","state":true},
            {"capability":"events","state":true},
        ]}}),
    )
    .await?;
    Ok(())
}

async fn status(qmp: &Qmp) -> Result<(String, Option<String>)> {
    let info = qmp.execute_raw(json!({"execute":"query-migrate"})).await?;
    Ok((
        info["status"].as_str().unwrap_or("none").to_owned(),
        info["error-desc"].as_str().map(str::to_owned),
    ))
}

/// Why this VM cannot be offered for migration, if anything.
pub(super) fn refusal(config: &Config, hello: &Hello, grown: bool) -> Option<String> {
    let state_id = config.state_id.as_deref().unwrap_or(&config.name);
    if hello.version != VERSION {
        return Some(format!(
            "unsupported migration protocol version {}",
            hello.version
        ));
    }
    if hello.state_id != state_id {
        return Some("the target belongs to a different VM".into());
    }
    // These management records are not yet part of the migration offer. Never
    // silently drop an accepted cold policy or select different CODE on target.
    for name in ["secure-boot-settings.json", "firmware-stage.json"] {
        if config.state_dir.join("identity").join(name).exists() {
            return Some("retained firmware/Secure Boot policy transfer is not supported by live migration yet".into());
        }
    }
    if std::env::var_os("QRAB_MEDIA_ROOT").is_some() {
        return Some("virtual-media graph transfer is not supported by live migration yet".into());
    }
    if config.disk.is_some() {
        return Some("a local disk cannot migrate; only shared iSCSI disks can".into());
    }
    if grown {
        return Some("hot-plugged vCPUs or memory are not migrated; resize at boot first".into());
    }
    None
}

/// Outcome of one outgoing attempt; on failure QEMU keeps running the guest.
pub(super) enum Sent {
    /// The guest now runs on the target; this QEMU must quit.
    Completed,
    /// The attempt ended and this VM continues here.
    Failed(String),
}

pub(super) async fn reject(mut pending: Pending, reason: &str) {
    let _ = write_frame(&mut pending.stream, &Reply::Refused(reason.to_owned())).await;
}

/// Offer this VM to `pending` and stream it. `tick` runs between status
/// polls so the caller keeps publishing health while the guest runs.
pub(super) async fn send<F, Fut>(
    config: &Config,
    qmp: &Qmp,
    session: &Path,
    pending: Pending,
    retained: BTreeMap<String, Binding>,
    grown: bool,
    mut tick: F,
) -> Sent
where
    F: FnMut(&str) -> Fut,
    Fut: std::future::Future<Output = Result<bool>>,
{
    let Pending { mut stream, hello } = pending;
    let migration = hello.migration.clone();
    if let Some(reason) = refusal(config, &hello, grown) {
        let _ = write_frame(&mut stream, &Reply::Refused(reason.clone())).await;
        return Sent::Failed(reason);
    }
    let attempt = async {
        let machine = machine_type(qmp).await?;
        write_frame(
            &mut stream,
            &Reply::Offer(Offer {
                machine,
                iscsi_disks: retained,
            }),
        )
        .await?;
        let ready: Ready = read_frame(&mut stream, READY_TIMEOUT).await?;
        if !ready.ready {
            return Err(failure("the target is not ready"));
        }
        let path = session.join("migrate-out.sock");
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        capabilities(qmp).await?;
        qmp.execute_raw(
            json!({"execute":"migrate-set-parameters","arguments":{"max-bandwidth":MAX_BANDWIDTH}}),
        )
        .await?;
        qmp.execute_raw(
            json!({"execute":"migrate","arguments":{"uri":format!("unix:{}", path.display())}}),
        )
        .await?;
        let (mut local, _) = tokio::time::timeout(FRAME_TIMEOUT, listener.accept())
            .await
            .map_err(|_| failure("QEMU did not open the migration socket"))??;
        drop(listener);
        let _ = std::fs::remove_file(&path);
        let splice = tokio::spawn(async move {
            let result = tokio::io::copy_bidirectional(&mut local, &mut stream).await;
            let _ = stream.shutdown().await;
            result
        });
        log(json!({"event":"migration_started","migration":migration}));
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            interval.tick().await;
            let (state, error) = status(qmp).await?;
            match state.as_str() {
                "completed" => break,
                "failed" | "cancelled" => {
                    splice.abort();
                    return Err(failure(error.unwrap_or(state)));
                }
                _ => {}
            }
            if !tick(&state).await? {
                let _ = qmp.execute_raw(json!({"execute":"migrate_cancel"})).await;
                splice.abort();
                return Err(failure("cancelled by shutdown"));
            }
        }
        // QEMU closes its end after the last byte; wait for it to drain.
        match tokio::time::timeout(Duration::from_secs(30), splice).await {
            Ok(Ok(Ok(_))) => Ok(()),
            Ok(Ok(Err(error))) => Err(failure(format!("stream did not drain: {error}"))),
            _ => Err(failure("stream did not drain")),
        }
    };
    match attempt.await {
        Ok(()) => {
            log(json!({"event":"migration_completed","migration":hello.migration}));
            Sent::Completed
        }
        Err(error) => {
            log(
                json!({"event":"migration_failed","migration":hello.migration,"error":error.to_string()}),
            );
            Sent::Failed(error.to_string())
        }
    }
}

/// The target's connection to the source, after the offer.
pub(super) struct Receiving {
    stream: qrab_spiffe::client::TlsStream<TcpStream>,
    pub offer: Offer,
    migration: String,
}

/// Dial the source and obtain its offer. Connection attempts are retried for
/// up to a minute while the source's listener and policy settle.
pub(super) async fn connect(config: &Config, incoming: &Incoming) -> Result<Receiving> {
    let (identity, id) = identity(incoming.workload_api.as_deref()).await?;
    let connector = identity
        .connector(std::slice::from_ref(&id))
        .map_err(failure)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let tcp = loop {
        match TcpStream::connect(&incoming.source).await {
            Ok(tcp) => break tcp,
            Err(error) if tokio::time::Instant::now() < deadline => {
                log(
                    json!({"event":"migration_source_unreachable","source":incoming.source,"error":error.to_string()}),
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(error) => return Err(error.into()),
        }
    };
    tcp.set_nodelay(true)?;
    let mut stream = connector.connect(tcp).await.map_err(failure)?;
    let state_id = config
        .state_id
        .clone()
        .unwrap_or_else(|| config.name.clone());
    write_frame(
        &mut stream,
        &Hello {
            version: VERSION,
            migration: incoming.migration.clone(),
            state_id,
        },
    )
    .await?;
    match read_frame(&mut stream, FRAME_TIMEOUT).await? {
        Reply::Offer(offer) => {
            log(
                json!({"event":"migration_offered","migration":incoming.migration,"machine":offer.machine}),
            );
            Ok(Receiving {
                stream,
                offer,
                migration: incoming.migration.clone(),
            })
        }
        Reply::Refused(reason) => Err(failure(format!("the source refused: {reason}"))),
    }
}

impl Receiving {
    /// Receive the stream into a QEMU started with `-incoming defer`. Returns
    /// once QEMU has loaded the guest; the caller resumes it.
    pub(super) async fn receive<F, Fut>(self, qmp: &Qmp, session: &Path, mut tick: F) -> Result<()>
    where
        F: FnMut(&str) -> Fut,
        Fut: std::future::Future<Output = Result<bool>>,
    {
        let Receiving {
            mut stream,
            migration,
            ..
        } = self;
        let path = session.join("migrate-in.sock");
        capabilities(qmp).await?;
        qmp.execute_raw(json!({"execute":"migrate-incoming","arguments":{"uri":format!("unix:{}", path.display())}}))
            .await?;
        let mut local = UnixStream::connect(&path).await?;
        write_frame(&mut stream, &Ready { ready: true }).await?;
        log(json!({"event":"migration_receiving","migration":migration}));
        let mut splice = tokio::spawn(async move {
            let result = tokio::io::copy_bidirectional(&mut local, &mut stream).await;
            let _ = stream.shutdown().await;
            result
        });
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                _ = interval.tick() => {}
                // The stream ending is expected at completion; the status says how it ended.
                _ = &mut splice, if !splice.is_finished() => {}
            }
            let (state, error) = status(qmp).await?;
            match state.as_str() {
                "completed" => break,
                "failed" | "cancelled" => return Err(failure(error.unwrap_or(state))),
                _ => {}
            }
            if !tick(&state).await? {
                return Err(failure("cancelled by shutdown"));
            }
        }
        log(json!({"event":"migration_received","migration":migration}));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qrab_spiffe::fixture::{Authority, Workload};

    const VM: &str = "spiffe://yapplesauce.com/kubernetes/qrab/vm/uid-1";
    const OTHER: &str = "spiffe://yapplesauce.com/kubernetes/qrab/vm/uid-2";

    fn config() -> Config {
        serde_json::from_value(json!({"name":"vm","qemu_binary":"/bin/true","state_id":"uid-1"}))
            .unwrap()
    }

    fn hello(state_id: &str) -> Hello {
        Hello {
            version: VERSION,
            migration: "m-1".into(),
            state_id: state_id.into(),
        }
    }

    #[test]
    fn only_the_same_vm_with_shared_disks_and_boot_size_is_offered() {
        let config = config();
        assert_eq!(refusal(&config, &hello("uid-1"), false), None);
        assert!(refusal(&config, &hello("uid-2"), false)
            .unwrap()
            .contains("different VM"));
        assert!(refusal(&config, &hello("uid-1"), true)
            .unwrap()
            .contains("hot-plugged"));
        let mut stale = hello("uid-1");
        stale.version = 9;
        assert!(refusal(&config, &stale, false).unwrap().contains("version"));
        let mut local: Config = serde_json::from_value(
            json!({"name":"vm","qemu_binary":"/bin/true","state_id":"uid-1",
            "disk":{"path":"/tmp/disk.qcow2","format":"qcow2"}}),
        )
        .unwrap();
        assert!(refusal(&local, &hello("uid-1"), false)
            .unwrap()
            .contains("local disk"));
        local.disk = None;
        assert_eq!(refusal(&local, &hello("uid-1"), false), None);
    }

    async fn listening(workload: &Workload) -> (u16, mpsc::Receiver<Pending>) {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let receiver = listen(&Outgoing {
            port,
            workload_api: Some(workload.endpoint.clone()),
        });
        for _ in 0..200 {
            if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
                return (port, receiver);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("listener did not start");
    }

    fn incoming(workload: &Workload, port: u16) -> Incoming {
        Incoming {
            migration: "m-1".into(),
            source: format!("127.0.0.1:{port}"),
            workload_api: Some(workload.endpoint.clone()),
        }
    }

    #[tokio::test]
    async fn the_same_vm_identity_is_admitted_and_frames_precede_the_stream() {
        let ca = Authority::new();
        let source = Workload::serve(vec![ca.svid(VM)]);
        let target = Workload::serve(vec![ca.svid(VM)]);
        let (port, mut pending) = listening(&source).await;
        let dial = tokio::spawn({
            let incoming = incoming(&target, port);
            async move { connect(&config(), &incoming).await }
        });
        let Pending { mut stream, hello } = pending.recv().await.unwrap();
        assert_eq!(
            (hello.migration.as_str(), hello.state_id.as_str()),
            ("m-1", "uid-1")
        );
        let offer = Offer {
            machine: "pc-q35-11.1".into(),
            iscsi_disks: BTreeMap::new(),
        };
        write_frame(&mut stream, &Reply::Offer(offer))
            .await
            .unwrap();
        let mut receiving = dial.await.unwrap().unwrap();
        assert_eq!(receiving.offer.machine, "pc-q35-11.1");
        // After "ready" the connection is a raw byte stream in both directions.
        write_frame(&mut receiving.stream, &Ready { ready: true })
            .await
            .unwrap();
        let ready: Ready = read_frame(&mut stream, FRAME_TIMEOUT).await.unwrap();
        assert!(ready.ready);
        stream.write_all(b"QEVM\0\0\0\x03stream").await.unwrap();
        stream.flush().await.unwrap();
        let mut bytes = [0; 14];
        receiving.stream.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"QEVM\0\0\0\x03stream");
    }

    #[tokio::test]
    async fn another_vm_identity_never_reaches_the_supervisor() {
        let ca = Authority::new();
        let source = Workload::serve(vec![ca.svid(VM)]);
        let intruder = Workload::serve(vec![ca.svid(OTHER)]);
        let (port, mut pending) = listening(&source).await;
        let result = connect(&config(), &incoming(&intruder, port)).await;
        assert!(result.is_err());
        assert!(
            tokio::time::timeout(Duration::from_millis(300), pending.recv())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_refusal_reaches_the_target() {
        let ca = Authority::new();
        let source = Workload::serve(vec![ca.svid(VM)]);
        let target = Workload::serve(vec![ca.svid(VM)]);
        let (port, mut pending) = listening(&source).await;
        let dial = tokio::spawn({
            let incoming = incoming(&target, port);
            async move {
                connect(&config(), &incoming)
                    .await
                    .err()
                    .map(|e| e.to_string())
            }
        });
        let Pending { mut stream, .. } = pending.recv().await.unwrap();
        write_frame(&mut stream, &Reply::Refused("busy".into()))
            .await
            .unwrap();
        assert!(dial.await.unwrap().unwrap().contains("refused: busy"));
    }
    #[test]
    fn migration_never_drops_retained_management_policy() {
        let dir = tempfile::tempdir().unwrap();
        let identity = dir.path().join("identity");
        std::fs::create_dir(&identity).unwrap();
        let mut config = config();
        config.state_dir = dir.path().into();
        assert!(refusal(&config, &hello("uid-1"), false).is_none());
        for name in ["secure-boot-settings.json", "firmware-stage.json"] {
            std::fs::write(identity.join(name), b"{}").unwrap();
            assert!(refusal(&config, &hello("uid-1"), false)
                .unwrap()
                .contains("policy transfer"));
            std::fs::remove_file(identity.join(name)).unwrap();
        }
        assert!(refusal(&config, &hello("uid-1"), false).is_none());
    }
}
