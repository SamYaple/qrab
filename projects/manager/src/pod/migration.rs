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
//!
//! The listener lives as long as the manager once QEMU first runs, so a VM
//! powered off in place still answers: it refuses with [`POWERED_OFF`], and
//! the target ends with [`Error::PoweredOff`] instead of a connect failure.
use super::{iscsi::Binding, Config, Phase, Status};
use crate::{Error, Result};
use qapi_protocol::Qmp;
use qrab_netd::wire::Op;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    net::Ipv4Addr,
    path::Path,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
    sync::{mpsc, oneshot, watch},
};

/// The source manager's migration listener port.
pub const PORT: u16 = 7800;
const VERSION: u32 = 1;
const FRAME_LIMIT: usize = 64 << 10;
const FRAME_TIMEOUT: Duration = Duration::from_secs(10);
/// Bound on the target preparing swtpm and QEMU after the offer.
const READY_TIMEOUT: Duration = Duration::from_secs(120);
/// Each switchover exchange; the guest is paused meanwhile.
const SWITCH_TIMEOUT: Duration = Duration::from_secs(10);
/// QEMU's own throttle, in bytes per second.
const MAX_BANDWIDTH: u64 = 1 << 30;
/// The refusal of a source whose QEMU is powered off. A target that receives
/// it fails with [`Error::PoweredOff`].
pub const POWERED_OFF: &str = "the VM is powered off";
/// Termination message of a target whose source refused with
/// [`POWERED_OFF`]; the operator reports the migration as `PoweredOff`.
pub const SOURCE_POWERED_OFF: &str = "source-powered-off";

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
    /// This (the target) node's address, where the source node forwards
    /// tap NICs' traffic during the switch. Required with tap NICs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_ip: Option<Ipv4Addr>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Hello {
    version: u32,
    migration: String,
    state_id: String,
    /// [`SWITCHOVER`] for the tap handoff's control connection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    node_ip: Option<Ipv4Addr>,
}

/// The second connection a tap-mode target opens to its source: while the
/// source's QEMU waits at `pre-switchover`, the source asks the target to
/// activate its anchors before standing its own down.
const SWITCHOVER: &str = "switchover";

/// Frames on the switchover connection.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Switch {
    Activate,
    Activated,
    /// The switch is undone: the target stands its anchors down again.
    Abort,
    Failed(String),
}

type Channel = qrab_spiffe::server::TlsStream<TcpStream>;
type ChannelSlot = oneshot::Sender<(Channel, Option<Ipv4Addr>)>;

/// Switchover connections waiting for their migration's `send`, by migration.
fn switchovers() -> &'static std::sync::Mutex<HashMap<String, ChannelSlot>> {
    static SLOTS: std::sync::OnceLock<std::sync::Mutex<HashMap<String, ChannelSlot>>> =
        std::sync::OnceLock::new();
    SLOTS.get_or_init(Default::default)
}

/// Removes a migration's switchover slot however `send` ends.
struct SlotGuard(String);

impl Drop for SlotGuard {
    fn drop(&mut self) {
        switchovers().lock().unwrap().remove(&self.0);
    }
}

/// One handoff step for every tap NIC through this node's qrab-netd.
async fn netd_all(config: &Config, nics: &[String], op: Op, via: Option<Ipv4Addr>) -> Result<()> {
    for nic in nics {
        let (socket, name) = (config.netd_socket.clone(), nic.clone());
        tokio::task::spawn_blocking(move || qrab_netd::wire::control(&socket, &name, op, via))
            .await
            .map_err(failure)?
            .map_err(|e| failure(format!("tap NIC {nic} {op:?}: {e}")))?;
    }
    Ok(())
}

fn tap_nics(config: &Config) -> Vec<String> {
    config.tap_fds().into_iter().map(|t| t.nic).collect()
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

/// Why a VM in this published state cannot be offered, before any per-VM
/// check: only a running QEMU can stream its guest.
pub(super) fn unavailable(status: Option<&Status>) -> Option<&'static str> {
    match status.map(|status| status.phase) {
        Some(Phase::Running) => None,
        Some(Phase::Off) => Some(POWERED_OFF),
        Some(Phase::Starting) => Some("the VM is starting; retry once it runs"),
        Some(Phase::Stopping) => Some("the VM is stopping"),
        None | Some(Phase::Exited | Phase::Failed) => Some("the VM is not running"),
    }
}

/// Accept target connections in the background until the receiver is
/// dropped, which releases the port. Connecting to the Workload API is
/// retried, so a VM starts even while SPIRE is unavailable; only migration
/// waits for it. A target arriving while the published `status` is not
/// running is refused at once; otherwise it is queued, one at a time.
pub(super) fn listen(
    outgoing: &Outgoing,
    status: watch::Receiver<Option<Status>>,
) -> mpsc::Receiver<Pending> {
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
                    tokio::select! {
                        _ = sender.closed() => return,
                        _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                    }
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
            let accepted = tokio::select! {
                _ = sender.closed() => return,
                accepted = listener.accept() => accepted,
            };
            let Ok((tcp, peer)) = accepted else {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            };
            let acceptor = acceptor.clone();
            let sender = sender.clone();
            let status = status.clone();
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
                if hello.channel.as_deref() == Some(SWITCHOVER) {
                    let slot = switchovers().lock().unwrap().remove(&hello.migration);
                    match slot {
                        Some(slot) => {
                            let _ = slot.send((stream, hello.node_ip));
                        }
                        None => log(
                            json!({"event":"migration_switchover_unexpected","peer":peer,"migration":hello.migration}),
                        ),
                    }
                    return;
                }
                log(json!({"event":"migration_requested","peer":peer,"migration":hello.migration}));
                let pending = Pending { stream, hello };
                let unavailable = unavailable(status.borrow().as_ref());
                let (mut pending, reason) = match unavailable {
                    Some(reason) => (pending, reason),
                    None => match sender.try_send(pending) {
                        Ok(()) => return,
                        Err(mpsc::error::TrySendError::Full(pending)) => {
                            (pending, "another migration is in progress")
                        }
                        Err(mpsc::error::TrySendError::Closed(pending)) => {
                            (pending, "the VM is stopping")
                        }
                    },
                };
                log(
                    json!({"event":"migration_refused","peer":peer,"migration":pending.hello.migration,"reason":reason}),
                );
                let _ = write_frame(&mut pending.stream, &Reply::Refused(reason.into())).await;
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

async fn capabilities(qmp: &Qmp, pause_before_switchover: bool) -> Result<()> {
    let mut capabilities = vec![
        json!({"capability":"return-path","state":true}),
        json!({"capability":"events","state":true}),
    ];
    // Tap NICs switch nodes while the source waits at pre-switchover.
    if pause_before_switchover {
        capabilities.push(json!({"capability":"pause-before-switchover","state":true}));
    }
    qmp.execute_raw(
        json!({"execute":"migrate-set-capabilities","arguments":{"capabilities":capabilities}}),
    )
    .await?;
    Ok(())
}

/// The source's half of the tap handoff, run while its QEMU waits at
/// `pre-switchover`: forward each NIC's address from this node to the
/// target node, have the target activate its anchors, then stand this
/// node's anchors down.
async fn switch_over(
    config: &Config,
    nics: &[String],
    channel: &mut Channel,
    via: Ipv4Addr,
) -> Result<()> {
    netd_all(config, nics, Op::Forward, Some(via)).await?;
    write_frame(channel, &Switch::Activate).await?;
    match read_frame(channel, SWITCH_TIMEOUT).await? {
        Switch::Activated => {}
        Switch::Failed(reason) => {
            return Err(failure(format!("the target could not activate: {reason}")))
        }
        other => return Err(failure(format!("unexpected switchover frame {other:?}"))),
    }
    netd_all(config, nics, Op::Standby, None).await
}

/// Undo a partial or complete switch so the source keeps serving the guest:
/// this node's anchors active again, the target's standing by, no forward.
async fn switch_back(config: &Config, nics: &[String], channel: &mut Channel) {
    let report = |op: &str, error: Error| {
        log(json!({"event":"migration_switch_back_failed","op":op,"error":error.to_string()}));
    };
    if let Err(error) = netd_all(config, nics, Op::Activate, None).await {
        report("activate", error);
    }
    let _ = write_frame(channel, &Switch::Abort).await;
    let _ = read_frame::<_, Switch>(channel, SWITCH_TIMEOUT).await;
    if let Err(error) = netd_all(config, nics, Op::Unforward, None).await {
        report("unforward", error);
    }
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The target's half: serve the source's switchover requests.
async fn serve_switchover(
    config: Config,
    nics: Vec<String>,
    mut channel: qrab_spiffe::client::TlsStream<TcpStream>,
) {
    loop {
        let frame = match read_frame::<_, Switch>(&mut channel, Duration::from_secs(3600)).await {
            Ok(frame) => frame,
            Err(_) => return,
        };
        let (op, reply_ok) = match frame {
            Switch::Activate => (Op::Activate, Switch::Activated),
            Switch::Abort => (Op::Standby, Switch::Abort),
            _ => return,
        };
        let reply = match netd_all(&config, &nics, op, None).await {
            Ok(()) => reply_ok,
            Err(error) => Switch::Failed(error.to_string()),
        };
        log(
            json!({"event":"migration_switchover","op":format!("{op:?}"),"reply":format!("{reply:?}")}),
        );
        if write_frame(&mut channel, &reply).await.is_err() {
            return;
        }
    }
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
    let nics = tap_nics(config);
    let tap = !nics.is_empty();
    // The target's switchover connection may arrive while this side still
    // prepares, so its slot exists from the offer on.
    let (slot, channel) = oneshot::channel();
    let _slot = tap.then(|| {
        switchovers()
            .lock()
            .unwrap()
            .insert(migration.clone(), slot);
        SlotGuard(migration.clone())
    });
    let mut switch: Option<(Channel, Ipv4Addr)> = None;
    let mut switched = false;
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
        if tap {
            // Fence this node's anchors now (it regenerates their programs),
            // so standing them down later is a map update inside the pause.
            netd_all(config, &nics, Op::Prepare, None).await?;
            let (connection, node_ip) = tokio::time::timeout(SWITCH_TIMEOUT * 3, channel)
                .await
                .map_err(|_| failure("the target opened no switchover connection"))?
                .map_err(|_| failure("the switchover slot closed"))?;
            let via = node_ip.ok_or_else(|| failure("the target sent no node address"))?;
            switch = Some((connection, via));
        }
        let path = session.join("migrate-out.sock");
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        capabilities(qmp, tap).await?;
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
        // A tap migration polls often: the pre-switchover wait is guest downtime.
        let period = if tap { 20 } else { 250 };
        let mut interval = tokio::time::interval(Duration::from_millis(period));
        loop {
            interval.tick().await;
            let (state, error) = status(qmp).await?;
            match state.as_str() {
                "completed" => break,
                "failed" | "cancelled" => {
                    splice.abort();
                    return Err(failure(error.unwrap_or(state)));
                }
                "pre-switchover" if !switched => {
                    let (connection, via) = switch
                        .as_mut()
                        .expect("a tap migration has a switchover connection");
                    let started = tokio::time::Instant::now();
                    switched = true;
                    switch_over(config, &nics, connection, *via).await?;
                    qmp.execute_raw(json!({"execute":"migrate-continue","arguments":{"state":"pre-switchover"}}))
                        .await?;
                    log(
                        json!({"event":"migration_switched","migration":migration,"ms":started.elapsed().as_millis()}),
                    );
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
    let outcome = attempt.await;
    if outcome.is_err() && switched {
        // The source keeps the guest: take the network back before it resumes.
        let _ = qmp.execute_raw(json!({"execute":"migrate_cancel"})).await;
        if let Some((connection, _)) = switch.as_mut() {
            switch_back(config, &nics, connection).await;
        }
    }
    match outcome {
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
    /// With tap NICs: how to open and serve the switchover connection.
    switchover: Option<Switchover>,
}

struct Switchover {
    connector: qrab_spiffe::Connector,
    source: String,
    hello: Hello,
    config: Config,
    nics: Vec<String>,
}

/// Dial the source and obtain its offer. Connection attempts are retried for
/// up to a minute while the source's listener and policy settle.
pub(super) async fn connect(config: &Config, incoming: &Incoming) -> Result<Receiving> {
    let nics = tap_nics(config);
    if !nics.is_empty() && incoming.node_ip.is_none() {
        return Err(failure("a tap-mode target needs incoming.node_ip"));
    }
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
            state_id: state_id.clone(),
            channel: None,
            node_ip: None,
        },
    )
    .await?;
    let switchover = (!nics.is_empty()).then(|| Switchover {
        connector: connector.clone(),
        source: incoming.source.clone(),
        hello: Hello {
            version: VERSION,
            migration: incoming.migration.clone(),
            state_id,
            channel: Some(SWITCHOVER.into()),
            node_ip: incoming.node_ip,
        },
        config: config.clone(),
        nics,
    });
    match read_frame(&mut stream, FRAME_TIMEOUT).await? {
        Reply::Offer(offer) => {
            log(
                json!({"event":"migration_offered","migration":incoming.migration,"machine":offer.machine}),
            );
            Ok(Receiving {
                stream,
                offer,
                migration: incoming.migration.clone(),
                switchover,
            })
        }
        Reply::Refused(reason) => {
            log(
                json!({"event":"migration_refused_by_source","migration":incoming.migration,"reason":reason}),
            );
            if reason == POWERED_OFF {
                return Err(Error::PoweredOff);
            }
            Err(failure(format!("the source refused: {reason}")))
        }
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
            switchover,
            ..
        } = self;
        let path = session.join("migrate-in.sock");
        capabilities(qmp, false).await?;
        qmp.execute_raw(json!({"execute":"migrate-incoming","arguments":{"uri":format!("unix:{}", path.display())}}))
            .await?;
        let mut local = UnixStream::connect(&path).await?;
        write_frame(&mut stream, &Ready { ready: true }).await?;
        // Dropped (and so aborted) when the guest has arrived or this fails.
        let _switchover = match switchover {
            Some(plan) => {
                let tcp = TcpStream::connect(&plan.source).await?;
                tcp.set_nodelay(true)?;
                let mut channel = plan.connector.connect(tcp).await.map_err(failure)?;
                write_frame(&mut channel, &plan.hello).await?;
                Some(AbortOnDrop(tokio::spawn(serve_switchover(
                    plan.config,
                    plan.nics,
                    channel,
                ))))
            }
            None => None,
        };
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
            channel: None,
            node_ip: None,
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

    fn published(phase: Phase) -> Status {
        serde_json::from_value(json!({"phase":phase,"ready":phase == Phase::Running,
            "manager_pid":1,"manager_start_ticks":1,"observed_unix_ms":0}))
        .unwrap()
    }

    #[test]
    fn only_a_running_qemu_is_offered_and_off_is_refused_as_powered_off() {
        assert_eq!(unavailable(Some(&published(Phase::Running))), None);
        assert_eq!(unavailable(Some(&published(Phase::Off))), Some(POWERED_OFF));
        for phase in [
            Phase::Starting,
            Phase::Stopping,
            Phase::Exited,
            Phase::Failed,
        ] {
            let reason = unavailable(Some(&published(phase))).unwrap();
            assert_ne!(reason, POWERED_OFF, "{phase:?}");
        }
        assert!(unavailable(None).is_some());
    }

    async fn listening(workload: &Workload) -> (u16, mpsc::Receiver<Pending>) {
        let (_, status) = watch::channel(Some(published(Phase::Running)));
        listening_with(workload, status).await
    }

    async fn listening_with(
        workload: &Workload,
        status: watch::Receiver<Option<Status>>,
    ) -> (u16, mpsc::Receiver<Pending>) {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let receiver = listen(
            &Outgoing {
                port,
                workload_api: Some(workload.endpoint.clone()),
            },
            status,
        );
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
            node_ip: None,
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

    #[tokio::test]
    async fn a_powered_off_source_answers_and_its_target_fails_as_powered_off() {
        let ca = Authority::new();
        let source = Workload::serve(vec![ca.svid(VM)]);
        let target = Workload::serve(vec![ca.svid(VM)]);
        let (phase, status) = watch::channel(Some(published(Phase::Off)));
        let (port, mut pending) = listening_with(&source, status).await;
        let started = std::time::Instant::now();
        let refused = connect(&config(), &incoming(&target, port)).await;
        assert!(matches!(refused, Err(Error::PoweredOff)));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "refused at once"
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(300), pending.recv())
                .await
                .is_err(),
            "nothing is queued for the supervisor"
        );
        // Powered on again, the same listener queues the next target.
        phase.send_replace(Some(published(Phase::Running)));
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

    #[tokio::test]
    async fn dropping_the_receiver_releases_the_port() {
        let ca = Authority::new();
        let source = Workload::serve(vec![ca.svid(VM)]);
        let (port, pending) = listening(&source).await;
        drop(pending);
        for _ in 0..200 {
            if std::net::TcpListener::bind(("0.0.0.0", port)).is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("the listener kept port {port}");
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

    /// What a fake qrab-netd was asked, in order.
    type Seen = std::sync::Arc<std::sync::Mutex<Vec<(Op, Option<Ipv4Addr>)>>>;

    /// A stand-in qrab-netd: answers each control request with `ok` unless
    /// `refuse` names its op, and records what it was asked.
    fn fake_netd(dir: &Path, name: &str, refuse: Option<Op>) -> (Config, Seen) {
        let socket = dir.join(format!("{name}.sock"));
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let record = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let request = qrab_netd::wire::read_request(&stream).unwrap();
                record.lock().unwrap().push((request.op, request.via));
                if Some(request.op) == refuse {
                    qrab_netd::wire::send_refusal(&stream, "no").unwrap();
                } else {
                    qrab_netd::wire::send_ok(&stream).unwrap();
                }
            }
        });
        let mut config = config();
        config.nics =
            serde_json::from_value(json!([{"id":"lan","mac":"02:00:00:00:00:01"}])).unwrap();
        config.netd_socket = socket;
        (config, seen)
    }

    /// The switchover connection of `migration`: the target dials the
    /// source's listener, which hands the connection to the waiting slot.
    async fn switchover_pair(
        source: &Workload,
        target: &Workload,
        migration: &str,
    ) -> (
        Channel,
        Option<Ipv4Addr>,
        qrab_spiffe::client::TlsStream<TcpStream>,
    ) {
        let (port, _pending) = listening(source).await;
        let (slot, waiting) = oneshot::channel();
        // Slots are process-wide: each test uses its own migration.
        switchovers().lock().unwrap().insert(migration.into(), slot);
        let (identity, id) = identity(Some(&target.endpoint)).await.unwrap();
        let connector = identity.connector(std::slice::from_ref(&id)).unwrap();
        let tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let mut client = connector.connect(tcp).await.unwrap();
        let mut hello = hello("uid-1");
        hello.migration = migration.into();
        hello.channel = Some(SWITCHOVER.into());
        hello.node_ip = Some("10.203.0.12".parse().unwrap());
        write_frame(&mut client, &hello).await.unwrap();
        let (server, node_ip) = tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .unwrap()
            .unwrap();
        (server, node_ip, client)
    }

    #[tokio::test]
    async fn the_switch_forwards_activates_the_target_and_stands_the_source_down_in_that_order() {
        let ca = Authority::new();
        let (source, target) = (
            Workload::serve(vec![ca.svid(VM)]),
            Workload::serve(vec![ca.svid(VM)]),
        );
        let dir = tempfile::tempdir().unwrap();
        let (source_config, source_ops) = fake_netd(dir.path(), "source", None);
        let (target_config, target_ops) = fake_netd(dir.path(), "target", None);
        let (mut channel, node_ip, client) = switchover_pair(&source, &target, "m-forward").await;
        assert_eq!(
            node_ip,
            Some("10.203.0.12".parse().unwrap()),
            "the target names its node"
        );
        let serving = tokio::spawn(serve_switchover(target_config, vec!["lan".into()], client));
        let via = node_ip.unwrap();
        switch_over(&source_config, &["lan".into()], &mut channel, via)
            .await
            .unwrap();
        assert_eq!(
            *source_ops.lock().unwrap(),
            [(Op::Forward, Some(via)), (Op::Standby, None)]
        );
        assert_eq!(*target_ops.lock().unwrap(), [(Op::Activate, None)]);
        serving.abort();
    }

    #[tokio::test]
    async fn a_target_that_cannot_activate_leaves_the_network_with_the_source() {
        let ca = Authority::new();
        let (source, target) = (
            Workload::serve(vec![ca.svid(VM)]),
            Workload::serve(vec![ca.svid(VM)]),
        );
        let dir = tempfile::tempdir().unwrap();
        let (source_config, source_ops) = fake_netd(dir.path(), "source", None);
        let (target_config, target_ops) = fake_netd(dir.path(), "target", Some(Op::Activate));
        let (mut channel, node_ip, client) = switchover_pair(&source, &target, "m-refused").await;
        let serving = tokio::spawn(serve_switchover(target_config, vec!["lan".into()], client));
        let error = switch_over(
            &source_config,
            &["lan".into()],
            &mut channel,
            node_ip.unwrap(),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(error.contains("the target could not activate"), "{error}");
        // The source never stood down; the rollback restores it and drops the forward.
        assert_eq!(*source_ops.lock().unwrap(), [(Op::Forward, node_ip)]);
        switch_back(&source_config, &["lan".into()], &mut channel).await;
        assert_eq!(
            *source_ops.lock().unwrap(),
            [
                (Op::Forward, node_ip),
                (Op::Activate, None),
                (Op::Unforward, None)
            ]
        );
        assert_eq!(
            *target_ops.lock().unwrap(),
            [(Op::Activate, None), (Op::Standby, None)]
        );
        serving.abort();
    }

    #[test]
    fn only_tap_migrations_pause_before_switchover() {
        // Without NICs nothing switches, so the stream path is unchanged.
        assert!(tap_nics(&config()).is_empty());
        let dir = tempfile::tempdir().unwrap();
        let (with_nic, _) = fake_netd(dir.path(), "netd", None);
        assert_eq!(tap_nics(&with_nic), ["lan"]);
    }
}
