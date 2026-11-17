//! Fixed-purpose bridge in the Prometheus Pod. Unencrypted legs use literal
//! loopback only. All VM traffic is TLS 1.3 with API-bound, exact SPIFFE IDs.
use anyhow::{ensure, Context, Result};
use axum::{
    body::Bytes,
    extract::{ConnectInfo, RawQuery, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use k8s_openapi::api::core::v1::Pod;
use kube::{api::ListParams, Api, Client, ResourceExt};
use qrab_operator::api::VirtualMachine;
use qrab_redfish::history::Metric;
use qrab_spiffe::{http::Peer, Identity, SpiffeId};
use std::{
    collections::{BTreeMap, HashMap},
    net::{IpAddr, SocketAddr},
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Semaphore;

pub const ID: &str = "spiffe://yapplesauce.com/kubernetes/monitoring/qrab-telemetry";
pub const SCRAPE_ADDRESS: &str = "127.0.0.1:19993";
pub const HISTORY_ADDRESS: &str = "0.0.0.0:9091";
pub const MAX_AGE: Duration = Duration::from_secs(15);
const DEADLINE: Duration = Duration::from_secs(8);
const MAX_BODY: usize = 1024 * 1024;
const MAX_INVENTORY: u32 = 512;

#[derive(Clone, Debug)]
struct Vm {
    namespace: String,
    pods: BTreeMap<String, PodTarget>,
}

#[derive(Clone, Debug)]
struct PodTarget {
    ip: IpAddr,
    uid: String,
}

#[derive(Clone)]
struct Snapshot {
    refreshed: Instant,
    vms: BTreeMap<SpiffeId, Vm>,
}

/// Only API-derived identities and owned Pod addresses enter this snapshot.
/// Losing discovery fails closed after MAX_AGE, including existing sessions.
#[derive(Clone, Default)]
pub struct Inventory(Arc<RwLock<Option<Snapshot>>>);
impl Inventory {
    fn snapshot(&self) -> Option<Snapshot> {
        self.0
            .read()
            .ok()?
            .as_ref()
            .filter(|s| s.refreshed.elapsed() < MAX_AGE)
            .cloned()
    }
    pub fn authorizes(&self, peer: &SpiffeId) -> bool {
        self.0.read().ok().is_some_and(|snapshot| {
            snapshot
                .as_ref()
                .is_some_and(|s| s.refreshed.elapsed() < MAX_AGE && s.vms.contains_key(peer))
        })
    }
    fn replace(&self, namespace: &str, vms: &[VirtualMachine], pods: &[Pod]) -> Result<()> {
        let mut selected = BTreeMap::new();
        for vm in vms
            .iter()
            .filter(|vm| vm.metadata.deletion_timestamp.is_none())
        {
            ensure!(
                vm.namespace().as_deref() == Some(namespace),
                "VM namespace mismatch"
            );
            let uid = vm.uid().context("VM UID missing")?;
            let peer = qrab_spiffe::vm_service_id("yapplesauce.com", namespace, &uid, "redfish")?;
            let mut addresses = BTreeMap::new();
            for pod in pods
                .iter()
                .filter(|pod| pod.metadata.deletion_timestamp.is_none())
            {
                let owner = pod.metadata.owner_references.as_deref().unwrap_or_default();
                if pod.namespace().as_deref() != Some(namespace)
                    || !owner.iter().any(|o| {
                        o.controller == Some(true)
                            && o.uid == uid
                            && o.kind == "VirtualMachine"
                            && o.api_version == "qrab.yapplesauce.com/v1alpha1"
                    })
                    || !pod.spec.as_ref().is_some_and(|s| {
                        s.init_containers
                            .as_deref()
                            .unwrap_or_default()
                            .iter()
                            .any(|c| {
                                c.name == "redfish" && c.restart_policy.as_deref() == Some("Always")
                            })
                    })
                {
                    continue;
                }
                let Some(pod_uid) = pod.uid() else {
                    continue;
                };
                if qrab_redfish::history::PodIdentity::new(namespace, &pod.name_any(), &pod_uid)
                    .is_err()
                {
                    continue;
                }
                if let Some(ip) = pod
                    .status
                    .as_ref()
                    .and_then(|s| s.pod_ip.as_ref())
                    .and_then(|ip| ip.parse().ok())
                {
                    addresses.insert(pod.name_any(), PodTarget { ip, uid: pod_uid });
                }
            }
            // No live Redfish Pod means no authorized client and no scrape.
            if !addresses.is_empty() {
                selected.insert(
                    peer,
                    Vm {
                        namespace: namespace.into(),
                        pods: addresses,
                    },
                );
            }
        }
        let counts = (
            selected.len(),
            selected.values().map(|vm| vm.pods.len()).sum::<usize>(),
        );
        let mut current = self
            .0
            .write()
            .map_err(|_| anyhow::anyhow!("inventory poisoned"))?;
        let previous = current.as_ref().map(|s| {
            (
                s.vms.len(),
                s.vms.values().map(|vm| vm.pods.len()).sum::<usize>(),
            )
        });
        if previous != Some(counts) {
            tracing::info!(
                eligible_vms = counts.0,
                owned_pods = counts.1,
                "VM observability inventory updated"
            );
        }
        *current = Some(Snapshot {
            refreshed: Instant::now(),
            vms: selected,
        });
        Ok(())
    }
    pub async fn refresh(&self, client: Client, namespace: &str) -> Result<()> {
        let api: Api<VirtualMachine> = Api::namespaced(client.clone(), namespace);
        let pods: Api<Pod> = Api::namespaced(client, namespace);
        let params = ListParams::default().limit(MAX_INVENTORY);
        let pod_params = params
            .clone()
            .labels("app.kubernetes.io/name=qrab,app.kubernetes.io/component=vm");
        let (vms, pods) = tokio::time::timeout(DEADLINE, async {
            tokio::try_join!(api.list(&params), pods.list(&pod_params))
        })
        .await
        .context("inventory deadline")??;
        ensure!(
            vms.metadata.continue_.as_deref().unwrap_or("").is_empty()
                && pods.metadata.continue_.as_deref().unwrap_or("").is_empty(),
            "inventory limit exceeded"
        );
        self.replace(namespace, &vms.items, &pods.items)
    }
}

#[derive(Clone)]
pub struct App {
    identity: Identity,
    inventory: Inventory,
    local: reqwest::Client,
    admission: Arc<Semaphore>,
    #[cfg(test)]
    history_port: u16,
}
impl App {
    pub fn new(identity: Identity, inventory: Inventory) -> Result<Self> {
        ensure!(
            identity.id()? == qrab_spiffe::id(ID)?,
            "unexpected telemetry identity"
        );
        Ok(Self {
            identity,
            inventory,
            local: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(2))
                .pool_max_idle_per_host(2)
                .build()?,
            admission: Arc::new(Semaphore::new(16)),
            #[cfg(test)]
            history_port: 9090,
        })
    }
    pub fn scrape_router(self) -> Router {
        Router::new()
            .route("/readyz", get(ready))
            .route("/metrics", get(scrape))
            .with_state(self)
    }
    pub fn history_router(self) -> Router {
        Router::new()
            .route("/api/v1/query_range", get(history))
            .layer(axum::middleware::from_fn(close_history_connection))
            .with_state(self)
    }
    async fn fetch(&self, client: &reqwest::Client, url: url::Url) -> Result<(String, Bytes)> {
        let _permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .context("query capacity")?;
        tokio::time::timeout(DEADLINE, async {
            let mut response = client.get(url).send().await?;
            ensure!(
                response.status() == StatusCode::OK,
                "upstream response refused"
            );
            let content_type = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_owned();
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                ensure!(
                    chunk.len() <= MAX_BODY.saturating_sub(body.len()),
                    "upstream response too large"
                );
                body.extend_from_slice(&chunk);
            }
            Ok((content_type, Bytes::from(body)))
        })
        .await
        .context("query deadline")?
    }
}

async fn close_history_connection(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CONNECTION,
        axum::http::HeaderValue::from_static("close"),
    );
    response
}

fn reply(result: Result<(String, Bytes)>) -> Response {
    match result {
        Ok((content_type, bytes)) => (
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, "no-store".into()),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::BAD_GATEWAY, "telemetry upstream unavailable").into_response(),
    }
}
async fn ready(State(app): State<App>) -> StatusCode {
    if app.inventory.snapshot().is_some()
        && app
            .identity
            .id()
            .ok()
            .is_some_and(|id| id.to_string() == ID)
    {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
fn fields(query: Option<&str>) -> Option<HashMap<String, String>> {
    let raw = query?;
    if raw.len() > 8192 {
        return None;
    }
    let mut result = HashMap::new();
    for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
        if result
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return None;
        }
    }
    Some(result)
}
fn scrape_target(snapshot: &Snapshot, target: &str) -> Option<(SpiffeId, SocketAddr)> {
    let parts: Vec<_> = target.split('/').collect();
    let [namespace, uid, pod_uid, ip] = parts.as_slice() else {
        return None;
    };
    let ip: IpAddr = ip.parse().ok()?;
    let peer = qrab_spiffe::vm_service_id("yapplesauce.com", namespace, uid, "redfish").ok()?;
    let vm = snapshot.vms.get(&peer)?;
    if !vm
        .pods
        .values()
        .any(|target| target.ip == ip && target.uid == *pod_uid)
    {
        return None;
    }
    Some((peer, SocketAddr::new(ip, 9181)))
}
async fn scrape(State(app): State<App>, RawQuery(raw): RawQuery) -> Response {
    let Some(snapshot) = app.inventory.snapshot() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(query) = fields(raw.as_deref()).filter(|q| q.len() == 1) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some((peer, address)) = query
        .get("target")
        .and_then(|t| scrape_target(&snapshot, t))
    else {
        return StatusCode::FORBIDDEN.into_response();
    };
    reply(
        async {
            let client = qrab_spiffe::http::client(&app.identity, peer)?;
            app.fetch(&client, format!("https://{address}/metrics").parse()?)
                .await
        }
        .await,
    )
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |t| t.as_secs())
}
/// Accept exactly the Redfish templates for a Pod currently owned by this
/// certificate's VM. No arbitrary PromQL, URLs, headers or admin API routes.
fn history_query(vm: &Vm, query: &HashMap<String, String>, now: u64) -> Option<url::Url> {
    if query.len() != 5 || query.get("timeout")? != "2s" {
        return None;
    }
    let start: u64 = query.get("start")?.parse().ok()?;
    let end: u64 = query.get("end")?.parse().ok()?;
    let step: u64 = query.get("step")?.parse().ok()?;
    if start > end
        || end > now.saturating_add(30)
        || end - start > 7 * 86400
        || !(30..=86400).contains(&step)
        || (end - start) / step > 300
    {
        return None;
    }
    let expression = query.get("query")?;
    let window = expression
        .rsplit_once('[')
        .and_then(|(_, s)| s.strip_suffix("s]))"))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(120);
    if !(120..=7 * 86400).contains(&window) {
        return None;
    }
    if !vm.pods.iter().any(|(pod, target)| {
        Metric::ALL
            .iter()
            .any(|m| m.promql(&vm.namespace, pod, &target.uid, window) == *expression)
    }) {
        return None;
    }
    let mut url: url::Url = "http://127.0.0.1:9090/api/v1/query_range".parse().ok()?;
    for key in ["query", "start", "end", "step", "timeout"] {
        url.query_pairs_mut().append_pair(key, query.get(key)?);
    }
    Some(url)
}
async fn history(
    State(app): State<App>,
    ConnectInfo(peer): ConnectInfo<Peer>,
    RawQuery(raw): RawQuery,
) -> Response {
    if !peer.valid() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(snapshot) = app.inventory.snapshot() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(vm) = snapshot.vms.get(&peer.id) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let Some(url) = fields(raw.as_deref()).and_then(|q| history_query(vm, &q, now())) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    #[cfg(test)]
    let url = {
        let mut url = url;
        url.set_port(Some(app.history_port)).unwrap();
        url
    };
    reply(app.fetch(&app.local, url).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::serve::Listener as _;
    use qrab_spiffe::{
        fixture::{Authority, Workload},
        id,
    };
    use serde_json::json;
    use std::future::IntoFuture;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };

    const VM_ID: &str = "spiffe://yapplesauce.com/kubernetes/qrab/vm/one/redfish";
    const OTHER_ID: &str = "spiffe://yapplesauce.com/kubernetes/qrab/vm/two/redfish";
    fn vm(uid: &str) -> VirtualMachine {
        serde_json::from_value(json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
            "metadata":{"name":uid,"namespace":"qrab","uid":uid},"spec":{"stateVolumeClaim":"state"}})).unwrap()
    }
    fn pod(name: &str, uid: &str, ip: &str) -> Pod {
        serde_json::from_value(json!({"metadata":{"name":name,"namespace":"qrab","uid":format!("pod-{name}"),
            "ownerReferences":[{"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine","name":uid,"uid":uid,"controller":true}]},
            "spec":{"containers":[{"name":"manager","image":"unused"}],
                "initContainers":[{"name":"redfish","image":"unused","restartPolicy":"Always"}]},
            "status":{"podIP":ip}})).unwrap()
    }
    fn inventory() -> Inventory {
        let inventory = Inventory::default();
        inventory
            .replace(
                "qrab",
                &[vm("one"), vm("two")],
                &[
                    pod("one-source", "one", "10.244.1.5"),
                    pod("one-target", "one", "10.244.2.8"),
                    pod("two-pod", "two", "10.244.3.1"),
                ],
            )
            .unwrap();
        inventory
    }
    fn query(expression: String) -> HashMap<String, String> {
        [
            ("query".into(), expression),
            ("start".into(), "1000".into()),
            ("end".into(), "1900".into()),
            ("step".into(), "30".into()),
            ("timeout".into(), "2s".into()),
        ]
        .into()
    }
    #[test]
    fn discovery_binds_vm_uid_owner_and_current_pod_ip_and_expires_closed() {
        let inventory = inventory();
        assert!(inventory.authorizes(&id(VM_ID).unwrap()));
        let snapshot = inventory.snapshot().unwrap();
        assert!(scrape_target(&snapshot, "qrab/one/pod-one-source/10.244.1.5").is_some());
        assert!(scrape_target(&snapshot, "qrab/one/pod-one-target/10.244.2.8").is_some());
        for target in [
            "qrab/one/pod-one-source/10.244.3.1",
            "qrab/two/pod-one-source/10.244.1.5",
            "other/one/pod-one-source/10.244.1.5",
            "qrab/one/pod-one-source/127.0.0.1",
            "qrab/one/pod-one-source/10.244.1.5:9181",
            "qrab/one/previous-pod/10.244.1.5",
            "qrab/one/10.244.1.5",
            "http://evil/metrics",
        ] {
            assert!(scrape_target(&snapshot, target).is_none(), "{target}");
        }
        inventory.0.write().unwrap().as_mut().unwrap().refreshed = Instant::now() - MAX_AGE;
        assert!(!inventory.authorizes(&id(VM_ID).unwrap()) && inventory.snapshot().is_none());
        let mut terminating = pod("source", "one", "10.244.1.5");
        terminating.metadata.deletion_timestamp =
            Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(
                k8s_openapi::jiff::Timestamp::now(),
            ));
        let mut wrong = pod("forged", "one", "10.244.3.1");
        wrong.metadata.owner_references.as_mut().unwrap()[0].controller = Some(false);
        let mut nonnative = pod("ordinary", "one", "10.244.2.8");
        let mut container = nonnative
            .spec
            .as_mut()
            .unwrap()
            .init_containers
            .take()
            .unwrap()
            .pop()
            .unwrap();
        container.restart_policy = None;
        nonnative.spec.as_mut().unwrap().containers.push(container);
        inventory
            .replace("qrab", &[vm("one")], &[terminating, wrong, nonnative])
            .unwrap();
        assert!(!inventory.authorizes(&id(VM_ID).unwrap()));
    }
    #[test]
    fn history_accepts_only_fixed_templates_of_the_certificate_owners_live_pods() {
        let snapshot = inventory().snapshot().unwrap();
        let vm = snapshot.vms.get(&id(VM_ID).unwrap()).unwrap();
        for metric in Metric::ALL {
            for pod in ["one-source", "one-target"] {
                assert!(history_query(
                    vm,
                    &query(metric.promql("qrab", pod, &format!("pod-{pod}"), 240)),
                    2000
                )
                .is_some());
            }
        }
        for expression in [
            Metric::BlockRead.promql("qrab", "two-pod", "pod-two-pod", 120),
            Metric::BlockRead.promql("qrab", "one-source", "previous-incarnation", 120),
            r#"sum by (drive) (rate(qrab_vm_block_read_bytes_total{namespace="qrab",pod="one-source"}[120s]))"#.into(),
            Metric::ContainerCpu.promql("other", "one-source", "pod-one-source", 120),
            "up".into(),
            format!(
                "{} or up",
                Metric::BlockRead.promql("qrab", "one-source", "pod-one-source", 120)
            ),
            Metric::BlockRead.promql("qrab", "one-source", "pod-one-source", 604801),
        ] {
            assert!(history_query(vm, &query(expression), 2000).is_none());
        }
        for (key, value) in [
            ("step", "0"),
            ("start", "1901"),
            ("end", "3000"),
            ("timeout", "30s"),
            ("extra", "admin"),
        ] {
            let mut q =
                query(Metric::BlockRead.promql("qrab", "one-source", "pod-one-source", 120));
            q.insert(key.into(), value.into());
            assert!(history_query(vm, &q, 2000).is_none());
        }
        assert!(fields(Some("target=a&target=b")).is_none());
    }

    #[test]
    fn same_name_and_ip_replacement_invalidates_the_previous_history_scope() {
        let inventory = inventory();
        let old = Metric::ContainerMemory.promql("qrab", "one-source", "pod-one-source", 120);
        let mut current = pod("one-source", "one", "10.244.1.5");
        current.metadata.uid = Some("replacement-pod".into());
        inventory
            .replace("qrab", &[vm("one")], &[current.clone()])
            .unwrap();
        let snapshot = inventory.snapshot().unwrap();
        let vm = snapshot.vms.get(&id(VM_ID).unwrap()).unwrap();
        assert!(
            scrape_target(&snapshot, "qrab/one/10.244.1.5").is_none(),
            "a scrape descriptor without Pod UID could be relabelled across incarnations"
        );
        assert!(scrape_target(&snapshot, "qrab/one/pod-one-source/10.244.1.5").is_none());
        assert!(scrape_target(&snapshot, "qrab/one/replacement-pod/10.244.1.5").is_some());
        assert!(history_query(vm, &query(old), 2000).is_none());
        assert!(history_query(
            vm,
            &query(Metric::ContainerMemory.promql("qrab", "one-source", "replacement-pod", 120)),
            2000
        )
        .is_some());
        current.metadata.uid = None;
        inventory
            .replace("qrab", &[tests::vm("one")], &[current])
            .unwrap();
        assert!(!inventory.authorizes(&id(VM_ID).unwrap()));
    }
    async fn identity(ca: &Authority, name: &str) -> (Workload, Identity) {
        let api = Workload::serve(vec![ca.svid(name)]);
        let identity = Identity::connect(&api.endpoint).await.unwrap();
        (api, identity)
    }
    #[tokio::test]
    async fn metrics_pin_the_vm_identity_refuse_http_redirects_and_bound_bodies() {
        qrab_spiffe::http::install_tls13_provider().unwrap();
        let ca = Authority::new();
        let (_gateway_api, gateway) = identity(&ca, ID).await;
        let (_rf_api, rf) = identity(&ca, VM_ID).await;
        let (_other_api, other) = identity(&ca, OTHER_ID).await;
        let mut tasks = Vec::new();
        let mut addresses = Vec::new();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for server in [rf, other] {
            let listener = qrab_spiffe::http::Listener::bind(
                "127.0.0.1:0".parse().unwrap(),
                &server,
                &[id(ID).unwrap()],
            )
            .await
            .unwrap();
            addresses.push(listener.local_addr().unwrap());
            let calls = calls.clone();
            let app = Router::new()
                .route(
                    "/metrics",
                    axum::routing::get(move || {
                        calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        async { "qrab_vm_power_state{state=\"On\"} 1\n" }
                    }),
                )
                .route(
                    "/large",
                    axum::routing::get(|| async { "x".repeat(MAX_BODY + 1) }),
                )
                .route(
                    "/redirect",
                    axum::routing::get(|| async {
                        (
                            StatusCode::FOUND,
                            [(header::LOCATION, "http://127.0.0.1:1/metrics")],
                        )
                    }),
                );
            tasks.push(tokio::spawn(axum::serve(listener, app).into_future()));
        }
        let app = App::new(gateway.clone(), inventory()).unwrap();
        let client = qrab_spiffe::http::client(&gateway, id(VM_ID).unwrap()).unwrap();
        let fetch = |scheme: &str, address: SocketAddr, path: &str| {
            format!("{scheme}://{address}/{path}").parse().unwrap()
        };
        let (_, body) = app
            .fetch(&client, fetch("https", addresses[0], "metrics"))
            .await
            .unwrap();
        assert!(std::str::from_utf8(&body)
            .unwrap()
            .contains("qrab_vm_power_state"));
        assert!(app
            .fetch(&client, fetch("https", addresses[1], "metrics"))
            .await
            .is_err());
        assert!(app
            .fetch(&client, fetch("http", addresses[0], "metrics"))
            .await
            .is_err());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(app
            .fetch(&client, fetch("https", addresses[0], "large"))
            .await
            .is_err());
        assert!(app
            .fetch(&client, fetch("https", addresses[0], "redirect"))
            .await
            .is_err());
        for task in tasks {
            task.abort();
        }
    }
    fn path(pod: &str) -> String {
        let end = now();
        let mut q = query(Metric::ContainerMemory.promql("qrab", pod, &format!("pod-{pod}"), 120));
        q.insert("start".into(), (end - 900).to_string());
        q.insert("end".into(), end.to_string());
        let encoded: String = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(&q)
            .finish();
        format!("/api/v1/query_range?{encoded}")
    }
    async fn get(
        identity: &Identity,
        address: SocketAddr,
        peer: &str,
        path: &str,
    ) -> Result<String> {
        let connector = identity.connector(&[id(peer)?])?;
        let mut stream = connector
            .connect(TcpStream::connect(address).await?)
            .await?;
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: telemetry\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await?;
        let mut response = String::new();
        stream.read_to_string(&mut response).await?;
        Ok(response)
    }
    #[tokio::test]
    async fn real_tls_preserves_scoped_history_rejects_wrong_ids_and_tracks_discovery_and_rotation()
    {
        qrab_spiffe::http::install_tls13_provider().unwrap();
        let ca = Authority::new();
        let (server_api, server) = identity(&ca, ID).await;
        let (vm_api, client) = identity(&ca, VM_ID).await;
        let (_other_api, other) = identity(
            &ca,
            "spiffe://yapplesauce.com/kubernetes/qrab/qrab-dashboard",
        )
        .await;
        let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_port = upstream.local_addr().unwrap().port();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let called = calls.clone();
        let backend = tokio::spawn(axum::serve(upstream, Router::new().route("/api/v1/query_range", axum::routing::get(move || {
            called.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async { "{\"status\":\"success\",\"data\":{\"resultType\":\"matrix\",\"result\":[]}}" }
        }))).into_future());
        // Test-only local endpoint injection never exists in production.
        let inventory = inventory();
        let mut app = App::new(server.clone(), inventory.clone()).unwrap();
        app.history_port = upstream_port;
        let policy = inventory.clone();
        let listener = qrab_spiffe::http::Listener::bind_authorized(
            "127.0.0.1:0".parse().unwrap(),
            &server,
            move |peer: &SpiffeId| policy.authorizes(peer),
        )
        .await
        .unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(
            axum::serve(
                listener,
                app.history_router()
                    .into_make_service_with_connect_info::<Peer>(),
            )
            .into_future(),
        );
        assert!(get(&client, address, ID, &path("one-source"))
            .await
            .unwrap()
            .starts_with("HTTP/1.1 200"));
        assert!(get(&client, address, ID, &path("two-pod"))
            .await
            .unwrap()
            .starts_with("HTTP/1.1 403"));
        assert!(
            get(&client, address, ID, "/api/v1/admin/tsdb/delete_series")
                .await
                .unwrap()
                .starts_with("HTTP/1.1 404")
        );
        assert!(get(&other, address, ID, &path("one-source")).await.is_err());
        assert!(get(&client, address, OTHER_ID, &path("one-source"))
            .await
            .is_err());
        // An HTTP spoof cannot bypass the TLS listener.
        let mut plain = TcpStream::connect(address).await.unwrap();
        plain
            .write_all(b"GET /api/v1/query_range HTTP/1.1\r\nHost: telemetry\r\n\r\n")
            .await
            .unwrap();
        let mut output = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(1), plain.read_to_end(&mut output)).await;
        assert!(!output.starts_with(b"HTTP/1.1 200"));
        // A certificate valid at handshake cannot keep querying past expiry.
        let short_api = Workload::serve(vec![ca.svid_for(VM_ID, 2)]);
        let short = Identity::connect(&short_api.endpoint).await.unwrap();
        let mut tls = short
            .connector(&[id(ID).unwrap()])
            .unwrap()
            .connect(TcpStream::connect(address).await.unwrap())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(3)).await;
        tls.write_all(
            format!(
                "GET {} HTTP/1.1\r\nHost: telemetry\r\n\r\n",
                path("one-source")
            )
            .as_bytes(),
        )
        .await
        .unwrap();
        let mut expired = String::new();
        tokio::time::timeout(Duration::from_secs(2), tls.read_to_string(&mut expired))
            .await
            .unwrap()
            .unwrap();
        assert!(expired.to_ascii_lowercase().contains("connection: close"));
        assert!(expired.starts_with("HTTP/1.1 401"));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // The same X509Source receives a replacement SVID and new connections work.
        let mut updates = client.source().updated();
        vm_api.replace(vec![ca.svid(VM_ID)]);
        tokio::time::timeout(Duration::from_secs(3), updates.changed())
            .await
            .unwrap()
            .unwrap();
        let mut server_updates = server.source().updated();
        server_api.replace(vec![ca.svid(ID)]);
        tokio::time::timeout(Duration::from_secs(3), server_updates.changed())
            .await
            .unwrap()
            .unwrap();
        assert!(get(&client, address, ID, &path("one-target"))
            .await
            .unwrap()
            .starts_with("HTTP/1.1 200"));
        inventory
            .replace("qrab", &[vm("two")], &[pod("two-pod", "two", "10.244.3.1")])
            .unwrap();
        assert!(get(&client, address, ID, &path("one-source"))
            .await
            .is_err());
        backend.abort();
        task.abort();
    }
}
