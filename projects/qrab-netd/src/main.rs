use anyhow::{Context, Result};
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams, Patch, PatchParams},
    runtime::{reflector, watcher, WatchStreamExt},
    Client, ResourceExt,
};
use qrab_netd::{
    authz, peer, plan::NicPlan, sys, wire, PLUMBED_CONDITION, SOCKET_DIR, SOCKET_NAME,
};
use std::{
    collections::HashMap,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// A plumbed anchor is re-verified this often; an unplumbed one every pass.
const REVERIFY: Duration = Duration::from_secs(60);

fn log(event: &str, detail: serde_json::Value) {
    eprintln!("{}", serde_json::json!({"event": event, "detail": detail}));
}

struct Agent {
    proc_root: PathBuf,
    store: reflector::Store<Pod>,
    /// Anchor UID → when its plan last applied cleanly.
    plumbed: Mutex<HashMap<String, Instant>>,
    /// Anchor UID → the condition last written (status, message).
    reported: Mutex<HashMap<String, (bool, String)>>,
    /// One grant at a time, so the busy check and the attach cannot race.
    granting: Mutex<()>,
    /// The node's Cilium agent API socket (handoff states).
    cilium: PathBuf,
    namespace: String,
    /// Anchor UID → its DHCP responder's stop flag.
    dhcp: Mutex<HashMap<String, Arc<std::sync::atomic::AtomicBool>>>,
    /// Forward routes whose address no local anchor holds any more → since when.
    orphaned: Mutex<HashMap<std::net::Ipv4Addr, Instant>>,
    /// Pods of the VM namespace on every node (conntrack peers).
    api: Api<Pod>,
    runtime: tokio::runtime::Handle,
    /// Node-to-node conntrack requests being served.
    peers_busy: std::sync::atomic::AtomicUsize,
    /// Peer connections refused for not coming from a host link (logged at
    /// powers of two).
    peer_strangers: std::sync::atomic::AtomicU64,
}

/// A tap anchor holding `address`, as a conntrack peer sees it.
fn holder(pod: &Pod, address: std::net::Ipv4Addr) -> Option<peer::Holder> {
    let status = pod.status.as_ref()?;
    (authz::is_tap_anchor(pod) && status.pod_ip.as_deref()?.parse().ok() == Some(address)).then(
        || {
            Some(peer::Holder {
                owner: authz::owner(pod)?.to_owned(),
                nic: authz::nic(pod)?.to_owned(),
                host: status.host_ip.as_deref()?.parse().ok()?,
            })
        },
    )?
}

/// At most this many node-to-node conntrack requests are served at once.
const PEER_CONCURRENCY: usize = 4;

impl Agent {
    fn netns(&self, pod: &Pod) -> Result<(i32, PathBuf), String> {
        let uid = pod.uid().ok_or("Pod has no UID")?;
        let pid =
            sys::pid_of_pod(&self.proc_root, &uid).ok_or("no running process of the anchor yet")?;
        Ok((pid, sys::netns_of(&self.proc_root, pid)))
    }

    fn plumb(&self, pod: &Pod) -> Result<String, String> {
        let mut plan = NicPlan::for_anchor(pod)?;
        let (pid, ns) = self.netns(pod)?;
        if NicPlan::is_vpn(pod) {
            plan = plan.with_tunnel(sys::read_tunnel(
                &self.proc_root,
                &pod.uid().unwrap_or_default(),
            )?);
        }
        let resolver = sys::pod_resolver(&self.proc_root, pid);
        let summary = match &plan.vpn {
            Some(tunnel) => format!(
                "tap {} routes {} only through wg0 to {}",
                plan.tap, plan.guest_ip, tunnel.endpoint
            ),
            None => format!("tap {} routes {}", plan.tap, plan.guest_ip),
        };
        let (tap, guest_mac, guest_ip) = (plan.tap.clone(), plan.guest_mac.clone(), plan.guest_ip);
        let dns = match &plan.vpn {
            Some(tunnel) => tunnel.dns.first().copied(),
            None => Some(qrab_netd::GUEST_GATEWAY),
        };
        let vpn = plan.vpn.is_some();
        let mtu =
            sys::in_netns(&ns, move || sys::apply(&plan, resolver)).map_err(|e| e.to_string())?;
        // One DHCP responder per anchor, for guests that configure by DHCP.
        let uid = pod.uid().unwrap_or_default();
        if let std::collections::hash_map::Entry::Vacant(slot) =
            self.dhcp.lock().unwrap().entry(uid)
        {
            let mac = qrab_netd::dhcp::mac(&guest_mac).ok_or("bad guest MAC")?;
            let lease = qrab_netd::dhcp::Lease {
                mac,
                ip: guest_ip,
                seconds: 86400,
                dns,
                mtu: vpn.then(|| u16::try_from(mtu).unwrap_or(1420)),
            };
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            sys::serve_dhcp(ns, tap, lease, stop.clone()).map_err(|e| e.to_string())?;
            slot.insert(stop);
        }
        Ok(summary)
    }

    fn grant(&self, stream: &UnixStream) -> Result<String, String> {
        let pid = nix::sys::socket::getsockopt(stream, nix::sys::socket::sockopt::PeerCredentials)
            .map_err(|e| e.to_string())?
            .pid();
        let request = wire::read_request(stream).map_err(|e| e.to_string())?;
        let pods = self.store.state();
        let requester = sys::pod_uid_of(&self.proc_root, pid).and_then(|uid| {
            pods.iter()
                .find(|p| p.uid().as_deref() == Some(uid.as_str()))
        });
        let anchor = authz::select_anchor(
            requester.map(|p| p.as_ref()),
            &request.nic,
            pods.iter().map(|p| p.as_ref()),
        )
        .map_err(|r| r.to_string())?;
        let anchor_uid = anchor.uid().unwrap_or_default();
        if request.op == wire::Op::Probe {
            if !self.plumbed.lock().unwrap().contains_key(&anchor_uid) {
                return Err(format!("NIC {:?} is not plumbed yet", request.nic));
            }
            wire::send_probe(stream, &anchor_uid).map_err(|e| e.to_string())?;
            return Ok(format!(
                "pid {pid}: NIC {:?} is served by anchor {anchor_uid}",
                request.nic
            ));
        }
        if request.op == wire::Op::CtPull {
            return self
                .conntrack(stream, anchor, &request)
                .map(|s| format!("pid {pid}: {s}"));
        }
        if request.op != wire::Op::Fds {
            let summary = self.control(anchor, &request)?;
            wire::send_ok(stream).map_err(|e| e.to_string())?;
            return Ok(format!("pid {pid}: {summary}"));
        }
        if !self.plumbed.lock().unwrap().contains_key(&anchor_uid) {
            return Err(format!("NIC {:?} is not plumbed yet", request.nic));
        }
        let plan = NicPlan::for_anchor(anchor)?;
        let _one = self.granting.lock().unwrap();
        let holders = sys::tap_holders(&self.proc_root, &plan.tap);
        if !holders.is_empty() {
            return Err(format!(
                "tap {} is already held by pid {holders:?}",
                plan.tap
            ));
        }
        let (_, ns) = self.netns(anchor)?;
        let (tap, queues) = (plan.tap.clone(), request.queues);
        let taps = sys::in_netns(&ns, move || sys::open_tap_queues(&tap, queues))
            .map_err(|e| e.to_string())?;
        let vhosts = sys::open_vhost(queues).map_err(|e| e.to_string())?;
        wire::send_grant(stream, &plan.tap, &anchor_uid, &taps, &vhosts)
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "pid {pid} received {queues} queue(s) of {} for NIC {:?}",
            plan.tap, request.nic
        ))
    }

    /// Every node's tap anchors holding `address` (one API list).
    fn holders(&self, address: std::net::Ipv4Addr) -> Result<Vec<peer::Holder>, String> {
        let params = ListParams::default()
            .fields(&format!("status.podIP={address}"))
            .labels("app.kubernetes.io/name=qrab");
        let pods = self
            .runtime
            .block_on(self.api.list(&params))
            .map_err(|e| format!("listing the anchors of {address}: {e}"))?;
        Ok(pods
            .items
            .iter()
            .filter_map(|p| holder(p, address))
            .collect())
    }

    /// `ct-pull` for `anchor`, the caller's NIC here: fetch the address's
    /// entries from the other node holding the NIC ([`peer`]) and import them
    /// (only entries of this NIC whose other side is not local here; see
    /// `qrab_netd::ct::importable`).
    fn conntrack(
        &self,
        stream: &UnixStream,
        anchor: &Pod,
        _request: &wire::Request,
    ) -> Result<String, String> {
        let ip: std::net::Ipv4Addr = anchor
            .status
            .as_ref()
            .and_then(|s| s.pod_ip.as_deref())
            .and_then(|ip| ip.parse().ok())
            .ok_or("the anchor has no IPv4 address")?;
        let mine = holder(anchor, ip).ok_or("the anchor has no node address")?;
        let sources = peer::sources(&mine, &self.holders(ip)?);
        if sources.is_empty() {
            return Err(format!("no other node holds an anchor of {ip}"));
        }
        let host = sys::host_netns(&self.proc_root);
        let (mut imported, mut failures) = (0, Vec::new());
        for source in sources {
            let (root, host) = (self.proc_root.clone(), host.clone());
            let pulled = sys::in_netns(&host.clone(), move || {
                let address = std::net::SocketAddr::from((source, peer::PEER_PORT));
                let connection = std::net::TcpStream::connect_timeout(&address, peer::DEADLINE)?;
                peer::fetch(&connection, ip)
            })
            .and_then(|export| {
                sys::in_netns(&host, move || sys::import_conntrack(&root, ip, &export))
            });
            match pulled {
                Ok(count) => imported += count,
                Err(error) => failures.push(format!("{source}: {error}")),
            }
        }
        if imported == 0 && !failures.is_empty() {
            return Err(failures.join("; "));
        }
        wire::send_imported(stream, imported).map_err(|e| e.to_string())?;
        Ok(format!(
            "imported {imported} conntrack entries of {ip}{}",
            if failures.is_empty() {
                String::new()
            } else {
                format!(" ({})", failures.join("; "))
            }
        ))
    }

    /// Serve other nodes' conntrack pulls on [`peer::PEER_PORT`] in the
    /// host network namespace.
    fn serve_peers(self: Arc<Self>, listener: std::net::TcpListener) {
        use std::sync::atomic::Ordering;
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let Ok(std::net::SocketAddr::V4(from)) = stream.peer_addr() else {
                continue;
            };
            if self.peers_busy.fetch_add(1, Ordering::SeqCst) >= PEER_CONCURRENCY {
                self.peers_busy.fetch_sub(1, Ordering::SeqCst);
                log("peer_busy", serde_json::json!({"from": from.ip()}));
                continue;
            }
            let agent = self.clone();
            std::thread::spawn(move || {
                let peer = *from.ip();
                // Only another node (on a host link) may ask; anything else
                // is dropped before it costs an API call.
                let host = sys::host_netns(&agent.proc_root);
                if !sys::in_netns(&host, move || Ok(sys::is_link_peer(peer))).unwrap_or(false) {
                    let strangers = agent.peer_strangers.fetch_add(1, Ordering::SeqCst) + 1;
                    if strangers.is_power_of_two() {
                        log(
                            "conntrack_refused",
                            serde_json::json!({"to": peer, "error": "not a node on a host link", "refused_so_far": strangers}),
                        );
                    }
                    agent.peers_busy.fetch_sub(1, Ordering::SeqCst);
                    return;
                }
                let mut outcome = None;
                let served = peer::serve_one(&stream, |address| {
                    let result = agent.answer_peer(peer, address);
                    outcome = Some((
                        address,
                        result
                            .as_ref()
                            .map(|e| e.entries.len())
                            .map_err(Clone::clone),
                    ));
                    result
                });
                match (served, outcome) {
                    (Ok(()), Some((address, Ok(count)))) => log(
                        "conntrack_served",
                        serde_json::json!({"to": peer, "address": address, "entries": count}),
                    ),
                    (Ok(()), Some((address, Err(error)))) => log(
                        "conntrack_refused",
                        serde_json::json!({"to": peer, "address": address, "error": error}),
                    ),
                    (served, _) => log(
                        "conntrack_refused",
                        serde_json::json!({"to": peer, "error": served.err().map(|e| e.to_string())}),
                    ),
                }
                agent.peers_busy.fetch_sub(1, Ordering::SeqCst);
            });
        }
    }

    /// The entries of `address` for the node at `peer`, if it may have them.
    fn answer_peer(
        &self,
        peer: std::net::Ipv4Addr,
        address: std::net::Ipv4Addr,
    ) -> Result<qrab_netd::ct::Export, String> {
        let local: Vec<peer::Holder> = self
            .store
            .state()
            .iter()
            .filter_map(|p| holder(p, address))
            .collect();
        peer::may_fetch(&local, &self.holders(address)?, peer)?;
        let root = self.proc_root.clone();
        sys::in_netns(&sys::host_netns(&self.proc_root), move || {
            sys::export_conntrack(&root, address)
        })
        .map_err(|e| e.to_string())
    }

    /// A live-migration handoff step for `anchor`, the requester's NIC here.
    fn control(&self, anchor: &Pod, request: &wire::Request) -> Result<String, String> {
        let endpoint = format!("cep-name:{}/{}", self.namespace, anchor.name_any());
        let state = match request.op {
            wire::Op::Prepare => Some("prepare"),
            wire::Op::Activate => Some("active"),
            wire::Op::Standby => Some("standby"),
            _ => None,
        };
        if let Some(state) = state {
            sys::cilium_handoff(&self.cilium, &endpoint, state).map_err(|e| e.to_string())?;
            return Ok(format!("{endpoint} is {state}"));
        }
        let ip: std::net::Ipv4Addr = anchor
            .status
            .as_ref()
            .and_then(|s| s.pod_ip.as_deref())
            .and_then(|ip| ip.parse().ok())
            .ok_or("the anchor has no IPv4 address")?;
        let host = sys::host_netns(&self.proc_root);
        match (request.op, request.via) {
            (wire::Op::Forward, Some(via)) => {
                // Only to another node: a manager cannot route its address
                // to anything off the node network (checked locally, since
                // this runs inside the guest's pause).
                sys::in_netns(&host, move || {
                    if !sys::is_link_peer(via) {
                        return Err(std::io::Error::other(format!(
                            "{via} is not a node on a host link"
                        )));
                    }
                    sys::forward(ip, via)
                })
                .map_err(|e| e.to_string())?;
                Ok(format!("{ip}/32 forwarded via {via}"))
            }
            (wire::Op::Unforward, _) => {
                sys::in_netns(&host, move || sys::unforward(ip)).map_err(|e| e.to_string())?;
                Ok(format!("{ip}/32 no longer forwarded"))
            }
            _ => Err("unsupported request".into()),
        }
    }

    /// Drop forward routes whose address no anchor on this node has held for
    /// [`FORWARD_GRACE`]: by then the new node announces the address.
    fn sweep_forwards(&self) {
        let host = sys::host_netns(&self.proc_root);
        let Ok(routes) = sys::in_netns(&host, sys::forwards) else {
            return;
        };
        let held: Vec<String> = self
            .store
            .state()
            .iter()
            .filter(|p| authz::is_tap_anchor(p))
            .filter_map(|p| p.status.as_ref()?.pod_ip.clone())
            .collect();
        let mut orphaned = self.orphaned.lock().unwrap();
        orphaned.retain(|ip, _| routes.contains(ip));
        for ip in routes {
            if held.contains(&ip.to_string()) {
                orphaned.remove(&ip);
                continue;
            }
            let since = *orphaned.entry(ip).or_insert_with(Instant::now);
            if since.elapsed() >= FORWARD_GRACE {
                match sys::in_netns(&host, move || sys::unforward(ip)) {
                    Ok(()) => log("unforwarded", serde_json::json!({"address": ip})),
                    Err(error) => log(
                        "unforward_failed",
                        serde_json::json!({"address": ip, "error": error.to_string()}),
                    ),
                }
                orphaned.remove(&ip);
            }
        }
    }
}

/// How long a forward route outlives the last local anchor of its address.
const FORWARD_GRACE: Duration = Duration::from_secs(20);

async fn report(pods: &Api<Pod>, agent: &Agent, pod: &Pod, outcome: &Result<String, String>) {
    let (Some(uid), name) = (pod.uid(), pod.name_any()) else {
        return;
    };
    let (ok, message) = match outcome {
        Ok(summary) => (true, summary.clone()),
        Err(error) => (false, error.clone()),
    };
    if agent.reported.lock().unwrap().get(&uid) == Some(&(ok, message.clone())) {
        return;
    }
    let condition = serde_json::json!({
        "type": PLUMBED_CONDITION,
        "status": if ok { "True" } else { "False" },
        "reason": if ok { "Plumbed" } else { "NotPlumbed" },
        "message": message,
        "lastTransitionTime": k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(k8s_openapi::jiff::Timestamp::now()),
    });
    let patch = Patch::Strategic(serde_json::json!({"status": {"conditions": [condition]}}));
    match pods
        .patch_status(&name, &PatchParams::default(), &patch)
        .await
    {
        Ok(_) => {
            log(
                "reported",
                serde_json::json!({"pod": name, "plumbed": ok, "message": message}),
            );
            agent.reported.lock().unwrap().insert(uid, (ok, message));
        }
        Err(error) => log(
            "report_failed",
            serde_json::json!({"pod": name, "error": error.to_string()}),
        ),
    }
}

async fn reconcile(agent: &Arc<Agent>, pods: &Api<Pod>) {
    let anchors: Vec<_> = agent
        .store
        .state()
        .into_iter()
        .filter(|p| authz::is_tap_anchor(p))
        .collect();
    for pod in &anchors {
        let uid = pod.uid().unwrap_or_default();
        let fresh = agent
            .plumbed
            .lock()
            .unwrap()
            .get(&uid)
            .is_some_and(|at| at.elapsed() < REVERIFY);
        if fresh {
            continue;
        }
        let worker = agent.clone();
        let target = pod.clone();
        let outcome = tokio::task::spawn_blocking(move || worker.plumb(&target))
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        match &outcome {
            Ok(_) => {
                agent.plumbed.lock().unwrap().insert(uid, Instant::now());
            }
            Err(error) => {
                agent.plumbed.lock().unwrap().remove(&uid);
                log(
                    "plumb_failed",
                    serde_json::json!({"pod": pod.name_any(), "error": error}),
                );
            }
        }
        report(pods, agent, pod, &outcome).await;
    }
    let live: Vec<String> = anchors.iter().filter_map(|p| p.uid()).collect();
    // A responder's socket pins its anchor's namespace: stop those of gone anchors.
    agent.dhcp.lock().unwrap().retain(|uid, stop| {
        let keep = live.contains(uid);
        if !keep {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        keep
    });
    agent
        .plumbed
        .lock()
        .unwrap()
        .retain(|uid, _| live.contains(uid));
    agent
        .reported
        .lock()
        .unwrap()
        .retain(|uid, _| live.contains(uid));
}

/// Control connections served at once; see [`serve`].
const CONTROL_CONCURRENCY: usize = 16;

async fn serve(agent: Arc<Agent>, socket: PathBuf) -> Result<()> {
    std::fs::create_dir_all(socket.parent().context("socket path has no directory")?)?;
    let _ = std::fs::remove_file(&socket);
    let listener = tokio::net::UnixListener::bind(&socket)?;
    // VM managers run unprivileged; access control is the peer check.
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o666))?;
    log("listening", serde_json::json!(socket));
    let slots = Arc::new(tokio::sync::Semaphore::new(CONTROL_CONCURRENCY));
    let mut turned_away: u64 = 0;
    loop {
        let (stream, _) = listener.accept().await?;
        // Each connection holds a blocking thread for up to its request
        // deadline; past the bound, refuse at once instead of queueing behind
        // slow peers (managers retry). Plumbing shares the blocking pool.
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            turned_away += 1;
            if turned_away.is_power_of_two() {
                log("busy", serde_json::json!({"turned_away": turned_away}));
            }
            drop(stream);
            continue;
        };
        let agent = agent.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let stream = match stream
                .into_std()
                .and_then(|s| s.set_nonblocking(false).map(|()| s))
            {
                Ok(stream) => stream,
                Err(error) => return log("accept_failed", serde_json::json!(error.to_string())),
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            match agent.grant(&stream) {
                Ok(summary) => log("granted", serde_json::json!(summary)),
                Err(error) => {
                    log("refused", serde_json::json!(error));
                    let _ = wire::send_refusal(&stream, &error);
                }
            }
        });
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => {}
        ["request", nic] | ["request", nic, _] => {
            // Diagnostic client: what a VM's manager would receive.
            let queues = args.get(2).map_or(Ok(1), |q| q.parse()).context("queues must be 1..=8")?;
            let grant = wire::request(&Path::new(SOCKET_DIR).join(SOCKET_NAME), nic, queues)?;
            println!(
                "{}",
                serde_json::json!({"tap": grant.tap, "taps": grant.taps.len(), "vhosts": grant.vhosts.len()})
            );
            return Ok(());
        }
        _ => anyhow::bail!(
            "usage: qrab-netd (configure with NODE_NAME, QRAB_NAMESPACE, QRAB_NETD_PROC)\n       qrab-netd request NIC [QUEUES]"
        ),
    }
    let node =
        std::env::var("NODE_NAME").context("NODE_NAME (the downward-API node name) is required")?;
    let namespace = std::env::var("QRAB_NAMESPACE").unwrap_or_else(|_| "qrab".into());
    let proc_root =
        PathBuf::from(std::env::var("QRAB_NETD_PROC").unwrap_or_else(|_| "/proc".into()));
    if let Err(error) = sys::check_devices() {
        log("devices_unavailable", serde_json::json!(error));
        anyhow::bail!(error);
    }
    let pods: Api<Pod> = Api::namespaced(Client::try_default().await?, &namespace);
    let (store, writer) = reflector::store();
    let config = watcher::Config::default()
        .fields(&format!("spec.nodeName={node}"))
        .labels("app.kubernetes.io/name=qrab");
    let changed = Arc::new(tokio::sync::Notify::new());
    let events = reflector(writer, watcher(pods.clone(), config))
        .default_backoff()
        .touched_objects();
    let notify = changed.clone();
    tokio::spawn(events.for_each(move |event| {
        if let Err(error) = event {
            log("watch_error", serde_json::json!(error.to_string()));
        }
        notify.notify_one();
        std::future::ready(())
    }));
    store
        .wait_until_ready()
        .await
        .context("pod watch never synced")?;
    log(
        "started",
        serde_json::json!({"node": node, "namespace": namespace}),
    );
    let agent = Arc::new(Agent {
        proc_root,
        store,
        plumbed: Mutex::default(),
        reported: Mutex::default(),
        granting: Mutex::default(),
        cilium: PathBuf::from(
            std::env::var("QRAB_CILIUM_SOCKET")
                .unwrap_or_else(|_| "/var/run/cilium/cilium.sock".into()),
        ),
        namespace: namespace.clone(),
        orphaned: Mutex::default(),
        dhcp: Mutex::default(),
        api: pods.clone(),
        runtime: tokio::runtime::Handle::current(),
        peers_busy: Default::default(),
        peer_strangers: Default::default(),
    });
    // Conntrack pulls from other nodes, in the host network namespace. Best
    // effort like the copy itself: a port that cannot be bound is retried,
    // never a reason to stop plumbing NICs.
    let peers = agent.clone();
    std::thread::spawn(move || loop {
        let host = sys::host_netns(&peers.proc_root);
        match sys::in_netns(&host, || {
            std::net::TcpListener::bind(("0.0.0.0", peer::PEER_PORT))
        }) {
            Ok(listener) => {
                log(
                    "peer_listening",
                    serde_json::json!({"port": peer::PEER_PORT}),
                );
                return peers.serve_peers(listener);
            }
            Err(error) => {
                log(
                    "peer_listen_failed",
                    serde_json::json!({"port": peer::PEER_PORT, "error": error.to_string()}),
                );
                std::thread::sleep(Duration::from_secs(10));
            }
        }
    });
    let mut server = tokio::spawn(serve(
        agent.clone(),
        Path::new(SOCKET_DIR).join(SOCKET_NAME),
    ));
    let mut tick = tokio::time::interval(Duration::from_secs(10));
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    loop {
        tokio::select! {
            _ = changed.notified() => {}
            _ = tick.tick() => {}
            _ = term.recv() => return Ok(()),
            result = &mut server => {
                return result?.context("socket server stopped");
            }
        }
        reconcile(&agent, &pods).await;
        let sweeper = agent.clone();
        let _ = tokio::task::spawn_blocking(move || sweeper.sweep_forwards()).await;
    }
}
