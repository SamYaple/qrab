use anyhow::{Context, Result};
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams, Patch, PatchParams},
    runtime::{reflector, watcher, WatchStreamExt},
    Client, ResourceExt,
};
use qrab_netd::{
    anchor::{Anchor, DhcpWorker},
    authz, natpmp, peer, peer_tls,
    plan::NicPlan,
    process::SocketPeer,
    sys, wire, PLUMBED_CONDITION, PORT_FORWARD_CONDITION, SOCKET_DIR, SOCKET_NAME,
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

#[cfg(test)]
#[path = "status_tests.rs"]
mod status_tests;

fn log(event: &str, detail: serde_json::Value) {
    eprintln!("{}", serde_json::json!({"event": event, "detail": detail}));
}

/// One NAT-PMP round: when, and the forwarded port or why not.
type PortRound = (Instant, Result<u16, String>);

#[derive(Clone)]
struct Plumbed {
    anchor: Arc<Anchor>,
    verified_at: Instant,
    summary: String,
}

struct Agent {
    proc_root: PathBuf,
    store: reflector::Store<Pod>,
    /// Anchor UID → the namespace last plumbed and when it was verified.
    plumbed: Mutex<HashMap<String, Plumbed>>,
    /// Anchor UID → the condition last written (status, message).
    reported: Mutex<HashMap<String, (bool, String)>>,
    /// One grant at a time, so the busy check and the attach cannot race.
    granting: Mutex<()>,
    /// The node's Cilium agent API socket (handoff states).
    cilium: PathBuf,
    namespace: String,
    /// Anchor UID → its namespace-bound DHCP responder.
    dhcp: Mutex<HashMap<String, DhcpWorker>>,
    /// Forward routes whose address no local anchor holds any more → since when.
    orphaned: Mutex<HashMap<std::net::Ipv4Addr, Instant>>,
    /// Pods of the VM namespace on every node (conntrack peers).
    api: Api<Pod>,
    runtime: tokio::runtime::Handle,
    /// Node-to-node conntrack requests being served.
    peers_busy: std::sync::atomic::AtomicUsize,
    /// Rotating node-bound identity; missing identity disables peers, never TLS.
    peer_tls: Mutex<Option<peer_tls::Transport>>,
    /// Peer connections refused for not coming from a host link (logged at
    /// powers of two).
    peer_strangers: std::sync::atomic::AtomicU64,
    /// VPN anchor UID → digest of the config last plumbed.
    tunnels: Mutex<HashMap<String, [u8; 32]>>,
    /// Port-forwarding anchor UID → the last NAT-PMP round and its result.
    forwarded: Mutex<HashMap<String, PortRound>>,
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
    /// Keep each plumbed port-forwarding VPN anchor's mapping alive (every
    /// [`natpmp::RENEW`], or 10 s after a failure) and report its port. A
    /// migration's standby anchor gets no reply (its traffic is dropped) and
    /// reports so; it maps once it is active.
    fn renew_port_forwards(&self) {
        let anchors: Vec<_> = self
            .store
            .state()
            .into_iter()
            .filter(|p| authz::is_tap_anchor(p) && NicPlan::is_vpn(p))
            .collect();
        let live: Vec<String> = anchors.iter().filter_map(|p| p.uid()).collect();
        self.forwarded
            .lock()
            .unwrap()
            .retain(|uid, _| live.contains(uid));
        for pod in anchors {
            let uid = pod.uid().unwrap_or_default();
            let Ok(anchor) = self.ready_anchor(&pod) else {
                continue;
            };
            let previous = self.forwarded.lock().unwrap().get(&uid).cloned();
            let due = previous.as_ref().is_none_or(|(at, result)| {
                at.elapsed()
                    >= if result.is_ok() {
                        natpmp::RENEW
                    } else {
                        Duration::from_secs(10)
                    }
            });
            if !due {
                continue;
            }
            let Ok(tunnel) = sys::read_tunnel(anchor.process()) else {
                continue;
            };
            let gateway = match (tunnel.port_forwarding, tunnel.gateway()) {
                (true, Some(gateway)) => gateway,
                _ => {
                    if previous.is_some() {
                        self.forwarded.lock().unwrap().remove(&uid);
                        self.report_port(&pod, &Err("port forwarding is not requested".into()));
                    }
                    continue;
                }
            };
            let result = match anchor.namespace() {
                Ok(ns) => sys::in_namespace(ns, move || Ok(sys::map_forwarded_port(gateway)))
                    .unwrap_or_else(|e| Err(e.to_string())),
                Err(error) => Err(error.to_string()),
            };
            let changed = previous.as_ref().map(|(_, r)| r) != Some(&result);
            self.forwarded
                .lock()
                .unwrap()
                .insert(uid, (Instant::now(), result.clone()));
            if changed {
                log(
                    "port_forward",
                    serde_json::json!({"pod": pod.name_any(), "result": format!("{result:?}")}),
                );
                self.report_port(&pod, &result);
            }
        }
    }

    fn report_port(&self, pod: &Pod, result: &Result<u16, String>) {
        let (ok, message) = match result {
            Ok(port) => (true, port.to_string()),
            Err(error) => (false, error.clone()),
        };
        let condition = serde_json::json!({
            "type": PORT_FORWARD_CONDITION,
            "status": if ok { "True" } else { "False" },
            "reason": if ok { "Mapped" } else { "NotMapped" },
            "message": message,
            "lastTransitionTime": k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(k8s_openapi::jiff::Timestamp::now()),
        });
        let name = pod.name_any();
        if let Err(error) = self
            .runtime
            .block_on(write_condition(&self.api, pod, condition))
        {
            log(
                "report_failed",
                serde_json::json!({"pod": name, "error": error.to_string()}),
            );
            // Report again next round.
            self.forwarded
                .lock()
                .unwrap()
                .remove(&pod.uid().unwrap_or_default());
        }
    }

    fn ready_anchor(&self, pod: &Pod) -> Result<Arc<Anchor>, String> {
        let uid = pod.uid().ok_or("Pod has no UID")?;
        let anchor = self
            .plumbed
            .lock()
            .unwrap()
            .get(&uid)
            .map(|ready| ready.anchor.clone())
            .ok_or("NIC is not plumbed yet")?;
        anchor.ensure_for(pod).map_err(|e| e.to_string())?;
        if !self
            .dhcp
            .lock()
            .unwrap()
            .get(&uid)
            .is_some_and(|worker| worker.serves(&anchor))
        {
            return Err("NIC DHCP responder is not running".into());
        }
        Ok(anchor)
    }

    fn invalidate_ready(&self, uid: &str) {
        self.plumbed.lock().unwrap().remove(uid);
        self.forwarded.lock().unwrap().remove(uid);
        // A process can restart while its namespace stays the same. Keep
        // its responder until the replacement context identifies the
        // namespace; matching healthy sockets can then be reused. A changed
        // namespace or a gone Pod retires the old worker below.
    }

    fn anchor_for(&self, pod: &Pod) -> Result<Arc<Anchor>, String> {
        Anchor::find(&self.proc_root, pod)
            .map(Arc::new)
            .map_err(|e| e.to_string())
    }

    fn plumb(&self, pod: &Pod) -> Result<(String, Arc<Anchor>), String> {
        let uid = pod.uid().ok_or("Pod has no UID")?;
        let anchor = self.anchor_for(pod)?;
        let mut plan = NicPlan::for_anchor(pod)?;
        if NicPlan::is_vpn(pod) {
            plan = plan.with_tunnel(sys::read_tunnel(anchor.process())?);
        }
        let resolver = sys::pod_resolver(anchor.process());
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
        let mtu = sys::in_namespace(anchor.namespace().map_err(|e| e.to_string())?, move || {
            sys::apply(&plan, resolver)
        })
        .map_err(|e| e.to_string())?;
        let lease = qrab_netd::dhcp::Lease {
            mac: qrab_netd::dhcp::mac(&guest_mac).ok_or("bad guest MAC")?,
            ip: guest_ip,
            seconds: 86400,
            dns,
            mtu: vpn.then(|| u16::try_from(mtu).unwrap_or(1420)),
        };
        // The socket belongs to the namespace; the offered lease belongs
        // to the applied plan. Refresh the latter without rebinding port 67.
        let mut workers = self.dhcp.lock().unwrap();
        if workers
            .get(&uid)
            .is_some_and(|worker| !worker.serves(&anchor))
        {
            workers.remove(&uid);
        }
        match workers.entry(uid) {
            std::collections::hash_map::Entry::Occupied(slot) => slot.get().update(lease),
            std::collections::hash_map::Entry::Vacant(slot) => {
                let worker = DhcpWorker::new(&anchor, lease);
                sys::serve_dhcp(
                    anchor.namespace().map_err(|e| e.to_string())?,
                    tap,
                    worker.lease(),
                    worker.stop_flag(),
                )
                .map_err(|e| e.to_string())?;
                slot.insert(worker);
            }
        }
        anchor.ensure_for(pod).map_err(|e| e.to_string())?;
        Ok((summary, anchor))
    }

    fn grant(&self, stream: &UnixStream) -> Result<String, String> {
        let peer = SocketPeer::capture(stream).map_err(|e| e.to_string())?;
        let (process, uid) = peer.process(&self.proc_root).map_err(|e| e.to_string())?;
        let pid = process.pid();
        let request = wire::read_request(stream).map_err(|e| e.to_string())?;
        if process.pod_uid().map_err(|e| e.to_string())? != uid {
            return Err("socket peer changed Pod while the request was read".into());
        }
        peer.ensure_alive().map_err(|e| e.to_string())?;
        let pods = self.store.state();
        let requester = pods
            .iter()
            .find(|p| p.uid().as_deref() == Some(uid.as_str()));
        let anchor = authz::select_anchor(
            requester.map(|p| p.as_ref()),
            &request.nic,
            pods.iter().map(|p| p.as_ref()),
        )
        .map_err(|r| r.to_string())?;
        let anchor_uid = anchor.uid().unwrap_or_default();
        if request.op == wire::Op::Probe {
            let ready = self.ready_anchor(anchor)?;
            wire::send_probe(stream, &ready.token()).map_err(|e| e.to_string())?;
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
        let plan = NicPlan::for_anchor(anchor)?;
        let _one = self.granting.lock().unwrap();
        let ready = self.ready_anchor(anchor)?;
        let holders = sys::tap_holders(&self.proc_root, &plan.tap);
        if !holders.is_empty() {
            return Err(format!(
                "tap {} is already held by pid {holders:?}",
                plan.tap
            ));
        }
        let (tap, queues) = (plan.tap.clone(), request.queues);
        let taps = sys::in_namespace(ready.namespace().map_err(|e| e.to_string())?, move || {
            sys::open_tap_queues(&tap, queues)
        })
        .map_err(|e| e.to_string())?;
        let vhosts = sys::open_vhost(queues).map_err(|e| e.to_string())?;
        ready.ensure_for(anchor).map_err(|e| e.to_string())?;
        wire::send_grant(stream, &plan.tap, &ready.token(), &taps, &vhosts)
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

    /// Current API-backed binding from peer host IP to node SPIFFE identity.
    fn peer_node(&self, host: std::net::Ipv4Addr) -> std::io::Result<String> {
        let params = ListParams::default()
            .labels("app.kubernetes.io/name=qrab,app.kubernetes.io/component=vm-nic");
        let pods = self.runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(1), self.api.list(&params))
                .await
                .map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "anchor lookup timed out")
                })?
                .map_err(std::io::Error::other)
        })?;
        peer_tls::node_for_host(&pods.items, host)
    }

    fn peer_transport(&self) -> std::io::Result<peer_tls::Transport> {
        self.peer_tls.lock().unwrap().clone().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "netd SPIFFE identity unavailable",
            )
        })
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
            let pulled = (|| {
                let transport = self.peer_transport()?;
                let node = self.peer_node(source)?;
                // Create/connect in the host netns, then wrap this exact socket
                // in TLS on the runtime. No async task changes a worker's netns.
                let connection = sys::in_netns(&host, move || {
                    let address = std::net::SocketAddr::from((source, peer::PEER_PORT));
                    std::net::TcpStream::connect_timeout(&address, Duration::from_secs(1))
                })?;
                connection.set_nonblocking(true)?;
                let export = self.runtime.block_on(async {
                    let tcp = tokio::net::TcpStream::from_std(connection)?;
                    transport.fetch(tcp, &node, ip).await
                })?;
                sys::in_netns(&host, move || sys::import_conntrack(&root, ip, &export))
            })();
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
                let served = (|| {
                    let transport = agent.peer_transport()?;
                    let node = agent.peer_node(peer)?;
                    stream.set_nonblocking(true)?;
                    agent.runtime.block_on(async {
                        let stream = tokio::net::TcpStream::from_std(stream)?;
                        transport
                            .serve(stream, &node, |address| {
                                let outcome = &mut outcome;
                                let worker = agent.clone();
                                async move {
                                    let result = tokio::task::spawn_blocking(move || {
                                        worker.answer_peer(peer, address)
                                    })
                                    .await
                                    .map_err(|e| e.to_string())?;
                                    *outcome = Some((
                                        address,
                                        result
                                            .as_ref()
                                            .map(|e| e.entries.len())
                                            .map_err(Clone::clone),
                                    ));
                                    result
                                }
                            })
                            .await
                    })
                })();
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

/// Bind a delayed local outcome to the exact Pod snapshot observed. A
/// same-name replacement or a newer status update must reject the whole patch.
async fn write_condition(
    pods: &Api<Pod>,
    pod: &Pod,
    condition: serde_json::Value,
) -> Result<(), String> {
    let uid = pod
        .metadata
        .uid
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or("Pod has no UID")?;
    let version = pod
        .metadata
        .resource_version
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or("Pod has no resourceVersion")?;
    let patch = Patch::Strategic(serde_json::json!({
        "metadata": {"uid":uid, "resourceVersion":version},
        "status": {"conditions": [condition]},
    }));
    pods.patch_status(&pod.name_any(), &PatchParams::default(), &patch)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

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
    match write_condition(pods, pod, condition).await {
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
        let cached = agent.plumbed.lock().unwrap().get(&uid).cloned();
        let current = agent.ready_anchor(pod).is_ok();
        let fresh = current
            && cached
                .as_ref()
                .is_some_and(|ready| ready.verified_at.elapsed() < REVERIFY);
        if !current {
            agent.invalidate_ready(&uid);
            if cached.is_some() {
                report(
                    pods,
                    agent,
                    pod,
                    &Err("anchor namespace changed; plumbing is pending".into()),
                )
                .await;
            }
        }
        // A rewritten VPN Secret (a server switch) is applied on the next
        // tick, not at the next re-verify.
        let digest = NicPlan::is_vpn(pod)
            .then(|| {
                agent
                    .anchor_for(pod)
                    .ok()
                    .and_then(|anchor| sys::tunnel_digest(anchor.process()))
            })
            .flatten();
        let rewritten = digest.is_some_and(|d| agent.tunnels.lock().unwrap().get(&uid) != Some(&d));
        if fresh && !rewritten {
            // A previous status PATCH may have conflicted or failed after
            // successful kernel work. Retry from this current watch snapshot
            // without repeating plumbing or waiting for the reverify interval.
            if let Some(ready) = cached {
                report(pods, agent, pod, &Ok(ready.summary)).await;
            }
            continue;
        }
        let worker = agent.clone();
        let target = pod.clone();
        let outcome = tokio::task::spawn_blocking(move || worker.plumb(&target))
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
        let outcome = outcome.map(|(summary, anchor)| {
            agent.plumbed.lock().unwrap().insert(
                uid.clone(),
                Plumbed {
                    anchor,
                    verified_at: Instant::now(),
                    summary: summary.clone(),
                },
            );
            summary
        });
        match &outcome {
            Ok(_) => {
                if let Some(digest) = digest {
                    agent.tunnels.lock().unwrap().insert(uid.clone(), digest);
                    // A new server needs a new mapping right away.
                    if rewritten {
                        agent.forwarded.lock().unwrap().remove(&uid);
                    }
                }
            }
            Err(error) => {
                agent.invalidate_ready(&uid);
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
    agent
        .dhcp
        .lock()
        .unwrap()
        .retain(|uid, _| live.contains(uid));
    agent
        .plumbed
        .lock()
        .unwrap()
        .retain(|uid, _| live.contains(uid));
    agent
        .tunnels
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
    qrab_netd::process::check_peer_support().context("netd socket peer identity support")?;
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
        peer_tls: Mutex::new(None),
        peer_strangers: Default::default(),
        forwarded: Mutex::default(),
        tunnels: Mutex::default(),
    });
    // Keep local TAP/DHCP plumbing available while SPIRE is unavailable.
    // Conntrack peers fail closed until this node has its exact SVID. The
    // Workload API source then follows rotations without restarting listeners.
    let identity_agent = agent.clone();
    tokio::spawn(async move {
        loop {
            let result = async {
                let identity = qrab_spiffe::Identity::from_env()
                    .await
                    .map_err(std::io::Error::other)?;
                peer_tls::Transport::new(identity, &node)
            }
            .await;
            match result {
                Ok(transport) => {
                    *identity_agent.peer_tls.lock().unwrap() = Some(transport);
                    log("peer_identity_ready", serde_json::json!({"node": node}));
                    break;
                }
                Err(error) => log(
                    "peer_identity_unavailable",
                    serde_json::json!(error.to_string()),
                ),
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
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
    // NAT-PMP renewals for port-forwarding VPN NICs, apart from plumbing so a
    // slow gateway never delays it.
    let forwarder = agent.clone();
    std::thread::spawn(move || loop {
        forwarder.renew_port_forwards();
        std::thread::sleep(Duration::from_secs(5));
    });
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
