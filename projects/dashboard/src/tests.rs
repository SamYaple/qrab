//! HTTP and WebSocket tests against an in-memory cluster, a fake Kanidm and
//! fake VM Pods (BMC console and Redfish) on loopback addresses.
mod browser;
use super::*;
use crate::access::tests::{ADMINS, ALICE, BOB, VIEWERS, vm};
use axum::{
    Json,
    body::Body,
    extract::{Request, ws::WebSocketUpgrade},
    http::{HeaderValue, Method},
};
use cluster::Usage;
use k8s_openapi::api::core::v1::{PersistentVolumeClaim, Pod};
use std::{collections::HashMap, sync::Mutex};
use tower::ServiceExt;

const ORIGIN: &str = "https://vms.example";
const LAUNCHERS: &str = "55555555-5555-4555-8555-555555555555";
const VM_USERS: &str = "66666666-6666-4666-8666-666666666666";
const IMAGE_ADMINS: &str = "77777777-7777-4777-8777-777777777777";
const KANIDM_TOKEN: &str = "service-account-token";

#[derive(Default)]
struct FakeCluster {
    vms: Mutex<Vec<VirtualMachine>>,
    pods: Mutex<HashMap<String, Pod>>,
    pvcs: Mutex<HashMap<String, PersistentVolumeClaim>>,
    created_vms: Mutex<Vec<serde_json::Value>>,
    deleted: Mutex<Vec<(String, String)>>,
    /// `set_running` calls: (name, uid, running).
    running: Mutex<Vec<(String, String, bool)>>,
    /// `set_vm_finalizers` calls: (name, finalizers).
    finalizer_writes: Mutex<Vec<(String, Vec<String>)>>,
    fail_vm_create: std::sync::atomic::AtomicBool,
    /// Appears after the first read misses it (a race with the preflight).
    racing_pvc: Mutex<Option<PersistentVolumeClaim>>,
    /// ConfigMap qrab-base-images, key images.json.
    catalog: Mutex<Option<String>>,
    fail_catalog: std::sync::atomic::AtomicBool,
    vpn_profiles: Mutex<Vec<qrab_proton::crd::VpnProfile>>,
    /// ConfigMap proton-catalog, key catalog.json.gz.
    vpn_catalog: Mutex<Option<Vec<u8>>>,
    /// VPN profile writes: ("create" | "patch" | "delete", name, body).
    vpn_writes: Mutex<Vec<(String, String, serde_json::Value)>>,
    base_images: Mutex<Vec<qrab_operator::image::BaseImage>>,
    fail_base_images: std::sync::atomic::AtomicBool,
    /// BaseImage writes: ("create" | "patch", name, body).
    image_writes: Mutex<Vec<(String, String, serde_json::Value)>>,
}

impl FakeCluster {
    fn mutations(&self) -> usize {
        self.created_vms.lock().unwrap().len()
            + self.deleted.lock().unwrap().len()
            + self.running.lock().unwrap().len()
            + self.pvcs.lock().unwrap().len()
            + self.vpn_writes.lock().unwrap().len()
            + self.image_writes.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl cluster::Cluster for FakeCluster {
    async fn list_vms(&self) -> anyhow::Result<Vec<VirtualMachine>> {
        Ok(self.vms.lock().unwrap().clone())
    }
    async fn get_vm(&self, name: &str) -> anyhow::Result<Option<VirtualMachine>> {
        Ok(self
            .vms
            .lock()
            .unwrap()
            .iter()
            .find(|v| v.metadata.name.as_deref() == Some(name))
            .cloned())
    }
    async fn get_pod(&self, name: &str) -> anyhow::Result<Option<Pod>> {
        Ok(self.pods.lock().unwrap().get(name).cloned())
    }
    async fn pod_usage(&self) -> anyhow::Result<HashMap<String, Usage>> {
        Ok(HashMap::from([(
            "a".to_string(),
            Usage {
                cpu_cores: 0.25,
                memory_bytes: 300.0 * 1048576.0,
            },
        )]))
    }
    async fn base_image_catalog(&self) -> anyhow::Result<Option<String>> {
        if self.fail_catalog.load(std::sync::atomic::Ordering::SeqCst) {
            anyhow::bail!("configmaps \"qrab-base-images\" is forbidden");
        }
        Ok(self.catalog.lock().unwrap().clone())
    }
    async fn get_pvc(&self, name: &str) -> anyhow::Result<Option<PersistentVolumeClaim>> {
        let mut pvcs = self.pvcs.lock().unwrap();
        let found = pvcs.get(name).cloned();
        if found.is_none()
            && let Some(pvc) = self.racing_pvc.lock().unwrap().take()
        {
            pvcs.insert(name.into(), pvc);
        }
        Ok(found)
    }
    async fn create_pvc(&self, pvc: &PersistentVolumeClaim) -> anyhow::Result<()> {
        self.pvcs
            .lock()
            .unwrap()
            .insert(pvc.metadata.name.clone().unwrap(), pvc.clone());
        Ok(())
    }
    async fn create_vm(&self, vm: &serde_json::Value) -> anyhow::Result<()> {
        if self
            .fail_vm_create
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            anyhow::bail!("admission webhook denied the request");
        }
        self.created_vms.lock().unwrap().push(vm.clone());
        Ok(())
    }
    async fn delete_vm(&self, name: &str, uid: &str) -> anyhow::Result<()> {
        self.deleted.lock().unwrap().push((name.into(), uid.into()));
        Ok(())
    }
    async fn set_running(&self, name: &str, uid: &str, running: bool) -> anyhow::Result<()> {
        self.running
            .lock()
            .unwrap()
            .push((name.into(), uid.into(), running));
        Ok(())
    }
    async fn set_vm_finalizers(
        &self,
        name: &str,
        uid: &str,
        resource_version: &str,
        finalizers: &[String],
    ) -> anyhow::Result<()> {
        let mut vms = self.vms.lock().unwrap();
        let Some(index) = vms.iter().position(|v| {
            v.metadata.name.as_deref() == Some(name) && v.metadata.uid.as_deref() == Some(uid)
        }) else {
            anyhow::bail!("virtualmachines \"{name}\" not found");
        };
        let vm = &mut vms[index];
        anyhow::ensure!(
            vm.metadata.resource_version.as_deref() == Some(resource_version),
            "the object has been modified"
        );
        self.finalizer_writes
            .lock()
            .unwrap()
            .push((name.into(), finalizers.to_vec()));
        vm.metadata.finalizers = Some(finalizers.to_vec());
        let next = resource_version.parse::<u64>().unwrap_or(0) + 1;
        vm.metadata.resource_version = Some(next.to_string());
        // The API server removes a terminating object once its finalizers are gone.
        if vm.metadata.deletion_timestamp.is_some() && finalizers.is_empty() {
            vms.remove(index);
        }
        Ok(())
    }
    async fn vpn_profiles(&self) -> anyhow::Result<Vec<qrab_proton::crd::VpnProfile>> {
        Ok(self.vpn_profiles.lock().unwrap().clone())
    }
    async fn vpn_catalog(&self) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.vpn_catalog.lock().unwrap().clone())
    }
    async fn create_vpn_profile(
        &self,
        profile: &qrab_proton::crd::VpnProfile,
    ) -> anyhow::Result<()> {
        self.vpn_writes.lock().unwrap().push((
            "create".into(),
            profile.metadata.name.clone().unwrap_or_default(),
            serde_json::to_value(profile).unwrap(),
        ));
        Ok(())
    }
    async fn patch_vpn_profile(
        &self,
        name: &str,
        uid: &str,
        proton: &qrab_proton::crd::ProtonSpec,
    ) -> anyhow::Result<()> {
        self.vpn_writes.lock().unwrap().push((
            "patch".into(),
            name.into(),
            cluster::vpn_patch(uid, proton),
        ));
        Ok(())
    }
    async fn delete_vpn_profile(&self, name: &str, uid: &str) -> anyhow::Result<()> {
        self.vpn_writes.lock().unwrap().push((
            "delete".into(),
            name.into(),
            serde_json::json!({"uid": uid}),
        ));
        Ok(())
    }
    async fn list_base_images(&self) -> anyhow::Result<Vec<qrab_operator::image::BaseImage>> {
        if self
            .fail_base_images
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            anyhow::bail!("baseimages.qrab.yapplesauce.com is forbidden");
        }
        Ok(self.base_images.lock().unwrap().clone())
    }
    async fn create_base_image(
        &self,
        image: &qrab_operator::image::BaseImage,
    ) -> anyhow::Result<()> {
        self.image_writes.lock().unwrap().push((
            "create".into(),
            image.metadata.name.clone().unwrap_or_default(),
            serde_json::to_value(image).unwrap(),
        ));
        self.base_images.lock().unwrap().push(image.clone());
        Ok(())
    }
    async fn patch_base_image_retired(
        &self,
        name: &str,
        uid: &str,
        retired: bool,
    ) -> anyhow::Result<()> {
        self.image_writes.lock().unwrap().push((
            "patch".into(),
            name.into(),
            cluster::retire_patch(uid, retired),
        ));
        Ok(())
    }
}

/// Requests a fake server saw: method, path and Authorization header.
type Log = Arc<Mutex<Vec<(String, String, String, String)>>>;

fn record(log: &Log, req: &Request, body: &str) {
    log.lock().unwrap().push((
        req.method().to_string(),
        req.uri()
            .path_and_query()
            .map(|p| p.to_string())
            .unwrap_or_default(),
        req.headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .into(),
        body.into(),
    ));
}

async fn serve(listener: tokio::net::TcpListener, app: axum::Router) {
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
}

/// The ResetTypes the fake Redfish allows: no PowerCycle (not operator-restartable).
const FAKE_RESET_TYPES: [&str; 7] = [
    "On",
    "GracefulShutdown",
    "ForceRestart",
    "Pause",
    "Resume",
    "Nmi",
    "GracefulRestart",
];

/// A fake Redfish `MetricHistory` answer for the dashboard's fixed 1h reads:
/// container series with markup in a label, an empty NIC rx series, and a
/// 503 for NIC tx.
fn fake_history(query: &str) -> Response {
    let metric = query
        .strip_prefix("metric=")
        .and_then(|rest| rest.strip_suffix("&range=1h"))
        .unwrap_or("");
    let (name, unit, series) = match metric {
        "container-cpu" => (
            "Container CPU",
            "cores",
            serde_json::json!([
                {"labels":{"container":"bmc"},"points":[[1000,0.001],[1060,0.002]]},
                {"labels":{"container":"<b>manager</b>"},"points":[[1000,0.25],[1030,0.5],[1060,0.125]]}
            ]),
        ),
        "container-memory" => (
            "Container memory working set",
            "bytes",
            serde_json::json!([
                {"labels":{"container":"manager"},"points":[[1000,524288000.0],[1060,536870912.0]]}
            ]),
        ),
        "block-read" | "block-write" => (
            "Disk",
            "bytes/s",
            serde_json::json!([{"labels":{"drive":"boot"},"points":[[1000,0.0],[1060,2048.0]]}]),
        ),
        "nic-rx" => ("Network received", "bytes/s", serde_json::json!([])),
        _ => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":{
                    "message":"Prometheus is unreachable, so metric history is unavailable."}})),
            )
                .into_response();
        }
    };
    Json(
        serde_json::json!({"Name": name, "metric": metric, "range": "1h", "unit": unit,
        "step": 15, "start": 1000, "end": 1060, "series": series}),
    )
    .into_response()
}

/// A fake VM Pod: the BMC console and a Redfish service whose system is named
/// `label`. Its guest strings carry markup. Alice's token is a Redfish admin;
/// a stale token is only a viewer there, so Redfish refuses its resets.
fn fake_bmc(label: &'static str, log: Log) -> axum::Router {
    let handler = move |req: Request| {
        let log = log.clone();
        async move {
            let (parts, body) = req.into_parts();
            let bytes = axum::body::to_bytes(body, 4096).await.unwrap();
            let req = Request::from_parts(parts.clone(), Body::empty());
            record(&log, &req, &String::from_utf8_lossy(&bytes));
            let admin = match parts
                .headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
            {
                Some("Bearer id-token-of-alice") => true,
                Some("Bearer id-token-of-stale") => false,
                _ => return StatusCode::UNAUTHORIZED.into_response(),
            };
            match (parts.method.clone(), parts.uri.path()) {
                (Method::GET, "/redfish/v1/EventService/SSE") => {
                    let stream = futures::stream::unfold((), |_| async {
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        Some((Ok::<_, std::io::Error>("data: {}\n\n"), ()))
                    });
                    ([("content-type", "text/event-stream")], Body::from_stream(stream)).into_response()
                }
                (Method::GET, "/redfish/v1/Systems/1") => Json(serde_json::json!({
                    "Id": "1", "Name": label, "PowerState": "On",
                    "Status": {"State": "Enabled", "Health": "OK"},
                    "Actions": {"#ComputerSystem.Reset": {
                        "target": "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset",
                        "ResetType@Redfish.AllowableValues": FAKE_RESET_TYPES}},
                    "Oem": {"Qrab": {
                        "Configuration": {"name": label, "memory_mib": 512, "cpus": 1,
                            "accelerator": "tcg", "boot": "uefi", "console": true, "tpm": true,
                            "guest_network": false, "disks": [], "startup_timeout_secs": 60,
                            "shutdown_grace_secs": 30, "redacted": false},
                        "GuestAgent": {"enabled": true, "connected": true,
                            "version": "<script>alert(1)</script>", "FreezeState": "Frozen",
                            "LastUpdate": {"State": "Failed", "RunId": 7, "ExitCode": 2,
                                "HookInstalled": true, "EndTime": "2026-11-14T00:00:00.000Z",
                                "Error": "<iframe src=x>", "Output": "line 1\n<svg onload=alert(3)>",
                                "OutputTruncated": true}}}}
                }))
                .into_response(),
                (Method::GET, "/redfish/v1/Systems/1/ResetActionInfo") => Json(serde_json::json!({
                    "Id": "ResetActionInfo",
                    "Parameters": [{"Name": "ResetType", "Required": true, "DataType": "String",
                        "AllowableValues": FAKE_RESET_TYPES}]
                }))
                .into_response(),
                (Method::GET, "/redfish/v1/Managers/1/LogServices/Events") => {
                    Json(serde_json::json!({"Id": "Events", "Oem": {"Qrab": {"QmpDropped": 2}}}))
                        .into_response()
                }
                (Method::GET, "/redfish/v1/Managers/1/LogServices/Events/Entries") => {
                    Json(serde_json::json!({"Members@odata.count": 2, "Members": [
                        {"Id": "1", "Created": "1970-01-01T00:00:00.001Z", "Severity": "OK",
                         "Message": "STOP", "Oem": {"Qrab": {"Sequence": 1, "Event": "STOP",
                         "Data": {}, "Truncated": false}}},
                        {"Id": "2", "Created": "1970-01-01T00:00:00.002Z", "Severity": "OK",
                         "Message": "RESET", "Oem": {"Qrab": {"Sequence": 2, "Event": "RESET",
                         "Data": {"reason": "<img src=x onerror=alert(2)>"}, "Truncated": false}}}
                    ]}))
                    .into_response()
                }
                (Method::GET, "/redfish/v1/TelemetryService/Oem/Qrab/MetricHistory") => {
                    fake_history(parts.uri.query().unwrap_or(""))
                }
                (Method::POST, "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset") if admin => {
                    StatusCode::NO_CONTENT.into_response()
                }
                (Method::POST, "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset") => {
                    StatusCode::FORBIDDEN.into_response()
                }
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }
    };
    // A query (admin-bucket features) is announced before the RFB greeting.
    let console = |headers: HeaderMap,
                   axum::extract::RawQuery(query): axum::extract::RawQuery,
                   ws: WebSocketUpgrade| async move {
        if headers.get("authorization").and_then(|v| v.to_str().ok())
            != Some("Bearer id-token-of-alice")
            || headers.contains_key("origin")
            || headers.contains_key("cookie")
        {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        ws.on_upgrade(|mut socket| async move {
            use axum::extract::ws::Message;
            if let Some(query) = query {
                let _ = socket
                    .send(Message::Binary(format!("query:{query}").into()))
                    .await;
            }
            let _ = socket.send(Message::Binary("RFB 003.008\n".into())).await;
            while let Some(Ok(Message::Binary(bytes))) = socket.recv().await {
                let mut echo = b"echo:".to_vec();
                echo.extend_from_slice(&bytes);
                if socket.send(Message::Binary(echo.into())).await.is_err() {
                    break;
                }
            }
        })
    };
    axum::Router::new()
        .route("/api/console", axum::routing::get(console))
        .fallback(handler)
}

#[derive(Default)]
struct Directory {
    groups: HashMap<String, String>,
    /// Group name to description.
    descriptions: HashMap<String, String>,
    members: HashMap<String, Vec<String>>,
    /// Lookups miss even existing groups.
    blind: bool,
    /// Every lookup finds a group, as if the name were already taken.
    taken: bool,
}

impl Directory {
    /// The `qrab_vm_<launch id>_<role>` group a launch created, and its UUID.
    fn launched(&self, role: &str) -> (String, String) {
        let (name, id) = self
            .groups
            .iter()
            .find(|(name, _)| launched_group(name, role))
            .unwrap_or_else(|| panic!("no launched {role} group in {:?}", self.groups));
        (name.clone(), id.clone())
    }
}

/// `qrab_vm_<32 lowercase hex>_<role>`.
fn launched_group(name: &str, role: &str) -> bool {
    name.strip_prefix("qrab_vm_")
        .and_then(|rest| rest.strip_suffix(&format!("_{role}")))
        .is_some_and(|id| {
            id.len() == 32
                && id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}

/// The launched group of `role` named in the first Kanidm call that names one.
fn logged_group(log: &Log, role: &str) -> String {
    log.lock()
        .unwrap()
        .iter()
        .find_map(|(_, path, _, _)| {
            let name = path.strip_prefix("/v1/group/")?;
            launched_group(name, role).then(|| name.to_owned())
        })
        .unwrap_or_else(|| panic!("no {role} group in the Kanidm log"))
}

fn fake_kanidm(log: Log, dir: Arc<Mutex<Directory>>) -> axum::Router {
    axum::Router::new().fallback(move |req: Request| {
        let (log, dir) = (log.clone(), dir.clone());
        async move {
            let (parts, body) = req.into_parts();
            let bytes = axum::body::to_bytes(body, 4096).await.unwrap();
            let req = Request::from_parts(parts.clone(), Body::empty());
            record(&log, &req, &String::from_utf8_lossy(&bytes));
            if parts
                .headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                != Some(&format!("Bearer {KANIDM_TOKEN}"))
            {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            let path: Vec<&str> = parts
                .uri
                .path()
                .trim_start_matches('/')
                .split('/')
                .collect();
            let mut dir = dir.lock().unwrap();
            match (parts.method.clone(), path.as_slice()) {
                (Method::GET, ["v1", "group", id]) if dir.taken => {
                    Json(serde_json::json!({"attrs": {"name": [id], "uuid": [BOB]}}))
                        .into_response()
                }
                (Method::GET, ["v1", "group", id]) => match dir
                    .groups
                    .iter()
                    .find(|(name, uuid)| name.as_str() == *id || uuid.as_str() == *id)
                {
                    Some(_) if dir.blind => StatusCode::NOT_FOUND.into_response(),
                    Some((name, uuid)) => {
                        let description = dir.descriptions.get(name).cloned().unwrap_or_default();
                        Json(serde_json::json!({"attrs": {
                            "name": [name], "uuid": [uuid], "description": [description]}}))
                        .into_response()
                    }
                    None => StatusCode::NOT_FOUND.into_response(),
                },
                (Method::POST, ["v1", "group"]) => {
                    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    let name = v["attrs"]["name"][0].as_str().unwrap().to_owned();
                    let description = v["attrs"]["description"][0].as_str().unwrap().to_owned();
                    let id = uuid::Uuid::new_v4().to_string();
                    dir.descriptions.insert(name.clone(), description);
                    dir.groups.insert(name, id);
                    Json(serde_json::Value::Null).into_response()
                }
                (Method::POST, ["v1", "group", id, "_attr", "member"]) => {
                    let members: Vec<String> = serde_json::from_slice(&bytes).unwrap();
                    dir.members
                        .entry(id.to_string())
                        .or_default()
                        .extend(members);
                    Json(serde_json::Value::Null).into_response()
                }
                (Method::DELETE, ["v1", "group", id]) => {
                    dir.groups.retain(|k, v| v != id && k != id);
                    Json(serde_json::Value::Null).into_response()
                }
                _ => StatusCode::BAD_REQUEST.into_response(),
            }
        }
    })
}

struct Harness {
    state: AppState,
    cluster: Arc<FakeCluster>,
    bmc_a: Log,
    bmc_b: Log,
    kanidm: Log,
    dir: Arc<Mutex<Directory>>,
}

fn pod(name: &str, ip: &str) -> Pod {
    serde_json::from_value(serde_json::json!({
        "metadata": {"name": name, "labels": {"app.kubernetes.io/name": "qrab",
            "app.kubernetes.io/component": "vm", "app.kubernetes.io/instance": name,
            "qrab.yapplesauce.com/owner-uid": format!("uid-{name}")},
            "ownerReferences": [{"apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "VirtualMachine",
                "name": name, "uid": format!("uid-{name}"), "controller": true}]},
        "spec": {"containers": [{"name": "manager", "image": "x",
            "resources": {"requests": {"cpu": "1", "memory": "768Mi"}, "limits": {"cpu": "2", "memory": "1Gi"}}}]},
        "status": {"podIP": ip}
    }))
    .unwrap()
}

fn running(mut machine: VirtualMachine) -> VirtualMachine {
    let name = machine.metadata.name.clone().unwrap();
    machine.status = Some(
        serde_json::from_value(
            serde_json::json!({"observedGeneration": 1, "phase": "Running",
            "ready": true, "reason": "", "message": "", "podName": name, "nodeName": "kube01"}),
        )
        .unwrap(),
    );
    machine
}

async fn harness() -> Harness {
    let console = |fields: serde_json::Value| {
        let mut c = serde_json::json!({"publicUrl": "https://vm.example"});
        c.as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        serde_json::json!({ "console": c })
    };
    let cluster = Arc::new(FakeCluster::default());
    *cluster.vms.lock().unwrap() = vec![
        running(vm(
            "a",
            console(serde_json::json!({"allowedSubjects": [ALICE]})),
            None,
        )),
        running(vm(
            "b",
            console(serde_json::json!({"viewerGroups": [VIEWERS]})),
            None,
        )),
        running(vm(
            "c",
            console(serde_json::json!({"allowedSubjects": [BOB]})),
            None,
        )),
    ];
    // Two VM Pods on the same port at different loopback addresses.
    let first = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = first.local_addr().unwrap().port();
    let second = tokio::net::TcpListener::bind(("127.0.0.2", port))
        .await
        .unwrap();
    let (bmc_a, bmc_b): (Log, Log) = Default::default();
    serve(first, fake_bmc("bmc-a", bmc_a.clone())).await;
    serve(second, fake_bmc("bmc-b", bmc_b.clone())).await;
    cluster.pods.lock().unwrap().extend([
        ("a".to_string(), pod("a", "127.0.0.1")),
        ("b".to_string(), pod("b", "127.0.0.2")),
        ("c".to_string(), pod("c", "127.0.0.2")),
    ]);
    let kanidm_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let kanidm_url = format!("http://{}", kanidm_listener.local_addr().unwrap());
    let kanidm: Log = Default::default();
    let dir: Arc<Mutex<Directory>> = Default::default();
    serve(kanidm_listener, fake_kanidm(kanidm.clone(), dir.clone())).await;
    let metadata: openidconnect::core::CoreProviderMetadata =
        serde_json::from_value(serde_json::json!({
            "issuer": "https://idp.example", "authorization_endpoint": "https://idp.example/auth",
            "token_endpoint": "https://idp.example/token", "jwks_uri": "https://idp.example/jwks",
            "response_types_supported": ["code"], "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"]
        }))
        .unwrap();
    let state = AppState {
        oidc: auth::Oidc::from_metadata(
            metadata,
            "qrab-dashboard".into(),
            "client-secret-never-sent".into(),
            format!("{ORIGIN}/auth/callback"),
            openidconnect::reqwest::Client::new(),
        )
        .unwrap(),
        origin: ORIGIN.into(),
        namespace: "qrab".into(),
        launcher_group: LAUNCHERS.into(),
        vm_users_group: VM_USERS.into(),
        image_admin_group: IMAGE_ADMINS.into(),
        storage_node: Some("kube03".into()),
        cluster: cluster.clone(),
        kanidm: kanidm::Kanidm::new(&kanidm_url, KANIDM_TOKEN.into(), None, true).unwrap(),
        bmc: bmc::Bmc::for_test(port),
        redfish: redfish::Redfish::for_test(port).unwrap(),
        sessions: Default::default(),
        novnc_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/novnc"),
        consoles: Arc::new(Semaphore::new(16)),
        console_subjects: Default::default(),
    };
    Harness {
        state,
        cluster,
        bmc_a,
        bmc_b,
        kanidm,
        dir,
    }
}

async fn session(store: &session_store::BoundedStore, subject: &str, groups: &[&str]) -> String {
    session_with_token(store, subject, groups, "id-token-of-alice").await
}

/// A session whose ID token Redfish may judge differently from the dashboard.
async fn session_with_token(
    store: &session_store::BoundedStore,
    subject: &str,
    groups: &[&str],
    token: &str,
) -> String {
    let session = Session::new(None, Arc::new(store.clone()), None);
    session
        .insert(
            "user",
            auth::User {
                subject: subject.into(),
                name: "alice".into(),
                groups: groups.iter().map(|g| g.to_string()).collect(),
                csrf: "correct".into(),
                expires: auth::now() + 600,
                id_token: token.into(),
            },
        )
        .await
        .unwrap();
    session.save().await.unwrap();
    format!("__Host-qrab-dashboard={}", session.id().unwrap())
}

async fn send(h: &Harness, req: Request<Body>) -> (StatusCode, HeaderMap, String) {
    let response = router(h.state.clone()).oneshot(req).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    (status, headers, String::from_utf8_lossy(&body).into_owned())
}

fn get(path: &str, cookie: Option<&str>) -> Request<Body> {
    let mut req = Request::builder().uri(path);
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    req.body(Body::empty()).unwrap()
}

fn post(path: &str, cookie: Option<&str>, origin: Option<&str>, body: &str) -> Request<Body> {
    let mut req = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/x-www-form-urlencoded");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    if let Some(o) = origin {
        req = req.header("origin", o);
    }
    req.body(Body::from(body.to_owned())).unwrap()
}

#[tokio::test]
async fn the_list_shows_only_visible_vms_and_launch_only_to_launchers() {
    let h = harness().await;
    let (status, _, body) = send(&h, get("/", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Sign in with Kanidm") && !body.contains("/vms/qrab/a"));
    let viewer = session(&h.state.sessions, ALICE, &[VIEWERS]).await;
    let (status, headers, body) = send(&h, get("/", Some(&viewer))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["cache-control"], "no-store");
    assert!(
        headers["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("default-src 'none'")
    );
    assert!(body.contains("href=\"/vms/qrab/a\"") && body.contains("href=\"/vms/qrab/b\""));
    assert!(!body.contains("/vms/qrab/c"));
    assert!(body.contains("0.25 cores") && body.contains("300 MiB"));
    assert!(!body.contains("href=\"/launch\""));
    assert!(!body.contains("id-token-of-alice"));
    let launcher = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (_, _, body) = send(&h, get("/", Some(&launcher))).await;
    assert!(body.contains("href=\"/launch\""));
    // No VM Pod is contacted for the list.
    assert!(h.bmc_a.lock().unwrap().is_empty() && h.bmc_b.lock().unwrap().is_empty());
}

#[tokio::test]
async fn invisible_absent_and_foreign_namespace_vms_are_404() {
    let h = harness().await;
    let cookie = session(&h.state.sessions, ALICE, &[]).await;
    for path in [
        "/vms/qrab/c",
        "/vms/qrab/missing",
        "/vms/other/a",
        "/vms/qrab/c/console",
        "/vms/qrab/B",
    ] {
        let (status, _, body) = send(&h, get(path, Some(&cookie))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(!body.contains("bmc-b"));
    }
    assert_eq!(
        send(&h, get("/vms/qrab/a", None)).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(h.bmc_b.lock().unwrap().is_empty());
}

#[tokio::test]
async fn detail_reads_only_the_selected_vms_redfish_with_the_bearer_and_escapes_guest_text() {
    let h = harness().await;
    let cookie = session(&h.state.sessions, ALICE, &[VIEWERS]).await;
    let (status, _, body) = send(&h, get("/vms/qrab/a", Some(&cookie))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("bmc-a") && !body.contains("bmc-b"));
    assert!(body.contains("Open console"));
    assert!(body.contains("No guest NIC."));
    assert!(body.contains("cpu 1, memory 768Mi") && body.contains("cpu 2, memory 1Gi"));
    assert!(!body.contains("<script>alert") && !body.contains("<img src=x"));
    assert!(
        body.contains("&#60;script&#62;alert(1)&#60;/script&#62;"),
        "{body}"
    );
    assert!(!body.contains("id-token-of-alice"));
    // The guest agent's freeze state and last update run, guest text escaped.
    assert!(body.contains("frozen"), "{body}");
    assert!(
        body.contains("failed · run 7 · exit 2 · 2026-11-14T00:00:00.000Z"),
        "{body}"
    );
    assert!(body.contains("Update output (tail)") && body.contains("Update error"));
    // The page draws its own history SVGs; guest markup never becomes one.
    assert!(!body.contains("<iframe") && !body.contains("<svg onload"));
    assert!(
        body.contains("&#60;iframe src=x&#62;") && body.contains("&#60;svg onload=alert(3)&#62;")
    );
    // Systems/1 PowerState and Status, the log service's drop count, and the
    // log entries newest first.
    assert!(body.contains("Power state") && body.contains("Enabled"));
    assert!(body.contains("2 QMP event(s) were dropped"));
    let (reset, stop) = (body.find("<b>RESET</b>"), body.find("<b>STOP</b>"));
    assert!(reset.unwrap() < stop.unwrap(), "{body}");
    // The power form follows the system's allowable reset types.
    for reset in ["GracefulRestart", "Nmi", "ForceRestart", "Pause"] {
        assert!(body.contains(&format!("value=\"{reset}\"")), "{reset}");
    }
    assert!(!body.contains("value=\"PowerCycle\"") && !body.contains("value=\"On\""));
    let seen = h.bmc_a.lock().unwrap().clone();
    let mut paths: Vec<_> = seen.iter().map(|(m, p, _, _)| format!("{m} {p}")).collect();
    paths.sort();
    let mut expected = vec![
        "GET /redfish/v1/Managers/1/LogServices/Events".to_string(),
        "GET /redfish/v1/Managers/1/LogServices/Events/Entries".to_string(),
        "GET /redfish/v1/Systems/1".to_string(),
    ];
    // The fixed 1h history of every metric, from the same Redfish.
    for metric in [
        "block-read",
        "block-write",
        "container-cpu",
        "container-memory",
        "nic-rx",
        "nic-tx",
    ] {
        expected.push(format!(
            "GET /redfish/v1/TelemetryService/Oem/Qrab/MetricHistory?metric={metric}&range=1h"
        ));
    }
    expected.sort();
    assert_eq!(paths, expected);
    assert!(
        seen.iter()
            .all(|(_, _, auth, _)| auth == "Bearer id-token-of-alice")
    );
    assert!(h.bmc_b.lock().unwrap().is_empty());
    // The viewer VM b lives at the other address.
    let (_, _, body) = send(&h, get("/vms/qrab/b", Some(&cookie))).await;
    assert!(
        body.contains("bmc-b") && !body.contains("Open console") && !body.contains("Delete VM")
    );
    // Viewers see the panel but no power form.
    assert!(body.contains("Recent QMP events") && !body.contains("power-title"));
    // A viewer learns why, and can refresh a sign-in that predates a new grant.
    assert!(
        body.contains("Console and power controls require an admin")
            && body.contains("href=\"/auth/login?next=/vms/qrab/b\"")
    );
    assert_eq!(h.bmc_b.lock().unwrap().len(), 9);
    assert_eq!(h.bmc_a.lock().unwrap().len(), 9);
}

#[tokio::test]
async fn a_pod_that_is_not_the_vms_own_is_never_called() {
    let h = harness().await;
    // VM a's status points at VM b's Pod: its labels name b, so no BMC call.
    h.cluster.vms.lock().unwrap()[0]
        .status
        .as_mut()
        .unwrap()
        .pod_name = Some("b".into());
    let cookie = session(&h.state.sessions, ALICE, &[]).await;
    let (_, _, body) = send(&h, get("/vms/qrab/a", Some(&cookie))).await;
    assert!(body.contains("unavailable until the VM"));
    assert!(h.bmc_a.lock().unwrap().is_empty() && h.bmc_b.lock().unwrap().is_empty());
}

#[tokio::test]
async fn every_post_requires_session_csrf_and_exact_origin() {
    let h = harness().await;
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let routes = [
        ("/auth/logout", "".to_string()),
        ("/launch", "name=web&cpus=1&memory_mib=512".to_string()),
        ("/vms/qrab/a/action", "action=Pause".to_string()),
        ("/vms/qrab/a/delete", "confirm_name=a".to_string()),
        ("/images", "image=x&generation=1".to_string()),
        ("/images/cirros-g1/retire", "confirm=yes".to_string()),
    ];
    for (path, fields) in &routes {
        for (cookie, csrf, origin, expected) in [
            (None, "correct", Some(ORIGIN), StatusCode::UNAUTHORIZED),
            (
                Some(cookie.as_str()),
                "wrong",
                Some(ORIGIN),
                StatusCode::FORBIDDEN,
            ),
            (
                Some(cookie.as_str()),
                "",
                Some(ORIGIN),
                StatusCode::FORBIDDEN,
            ),
            (
                Some(cookie.as_str()),
                "correct",
                Some("https://evil.example"),
                StatusCode::FORBIDDEN,
            ),
            (
                Some(cookie.as_str()),
                "correct",
                Some("https://vms.example:8443"),
                StatusCode::FORBIDDEN,
            ),
            (
                Some(cookie.as_str()),
                "correct",
                None,
                StatusCode::FORBIDDEN,
            ),
        ] {
            let body = format!("{fields}&csrf={csrf}");
            let (status, _, _) = send(&h, post(path, cookie, origin, &body)).await;
            assert_eq!(status, expected, "{path} {csrf} {origin:?}");
        }
    }
    assert_eq!(h.cluster.mutations(), 0);
    assert!(h.kanidm.lock().unwrap().is_empty());
    assert!(h.bmc_a.lock().unwrap().is_empty());
    // The session survived every refused logout.
    assert!(
        send(&h, get("/", Some(&cookie)))
            .await
            .2
            .contains("Sign out")
    );
    let (status, _, _) = send(
        &h,
        post("/auth/logout", Some(&cookie), Some(ORIGIN), "csrf=correct"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert!(
        send(&h, get("/", Some(&cookie)))
            .await
            .2
            .contains("Sign in with Kanidm")
    );
}

#[tokio::test]
async fn power_actions_are_admin_only_confirmed_and_sent_to_redfish_with_the_bearer() {
    let h = harness().await;
    let cookie = session(&h.state.sessions, ALICE, &[VIEWERS]).await;
    let (status, _, _) = send(
        &h,
        post(
            "/vms/qrab/b/action",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&action=Pause",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = send(
        &h,
        post(
            "/vms/qrab/c/action",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&action=Pause",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    for bad in [
        "action=qmp",
        "action=reset",
        "action=On",
        "action=ForceRestart",
        "action=GracefulShutdown&confirm=no",
    ] {
        let (status, _, _) = send(
            &h,
            post(
                "/vms/qrab/a/action",
                Some(&cookie),
                Some(ORIGIN),
                &format!("csrf=correct&{bad}"),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
    assert!(h.bmc_a.lock().unwrap().is_empty() && h.bmc_b.lock().unwrap().is_empty());
    let (status, headers, _) = send(
        &h,
        post(
            "/vms/qrab/a/action",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&action=ForceRestart&confirm=yes",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers["location"], "/vms/qrab/a");
    let seen = h.bmc_a.lock().unwrap().clone();
    assert_eq!(
        seen,
        [(
            "POST".into(),
            "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset".into(),
            "Bearer id-token-of-alice".into(),
            r#"{"ResetType":"ForceRestart"}"#.into()
        )]
    );
    let (_, _, body) = send(&h, get("/vms/qrab/a", Some(&cookie))).await;
    assert!(body.contains("Reset sent to a."));
}

#[tokio::test]
async fn a_redfish_refusal_is_a_refused_action_not_a_failure() {
    let h = harness().await;
    // The dashboard holds Alice as admin, but her token is only a viewer in
    // Redfish (for example, a grant that changed since she signed in).
    let cookie = session_with_token(&h.state.sessions, ALICE, &[], "id-token-of-stale").await;
    let (status, headers, _) = send(
        &h,
        post(
            "/vms/qrab/a/action",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&action=Pause",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers["location"], "/vms/qrab/a");
    let (_, _, body) = send(&h, get("/vms/qrab/a", Some(&cookie))).await;
    assert!(
        body.contains("Pause was refused: Redfish refused this for your role (HTTP 403)."),
        "{body}"
    );
    let (status, _, text) = send(
        &h,
        post_json(
            "/vms/qrab/a/power",
            Some(&cookie),
            Some(ORIGIN),
            Some("correct"),
            serde_json::json!({"action": "Nmi", "confirm": true}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        text,
        "NMI was refused: Redfish refused this for your role (HTTP 403)."
    );
    let resets: Vec<_> = h
        .bmc_a
        .lock()
        .unwrap()
        .iter()
        .filter(|(m, _, _, _)| m == "POST")
        .map(|(_, _, auth, body)| (auth.clone(), body.clone()))
        .collect();
    assert_eq!(
        resets,
        [
            (
                "Bearer id-token-of-stale".to_string(),
                r#"{"ResetType":"Pause"}"#.to_string()
            ),
            (
                "Bearer id-token-of-stale".to_string(),
                r#"{"ResetType":"Nmi"}"#.to_string()
            ),
        ]
    );
}

#[tokio::test]
async fn delete_is_admin_only_confirmed_and_names_what_goes() {
    let h = harness().await;
    let cookie = session(&h.state.sessions, ALICE, &[VIEWERS]).await;
    let (status, _, _) = send(
        &h,
        post(
            "/vms/qrab/b/delete",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&confirm_name=b",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = send(
        &h,
        post(
            "/vms/qrab/a/delete",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&confirm_name=b",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Alice is a console admin of a, but a was not launched by the dashboard.
    let (status, _, body) = send(&h, get("/vms/qrab/a", Some(&cookie))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("Delete VM"));
    let (status, _, _) = send(
        &h,
        post(
            "/vms/qrab/a/delete",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&confirm_name=a",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(h.cluster.deleted.lock().unwrap().is_empty());
    h.cluster.vms.lock().unwrap()[0].metadata.labels = Some(
        [(
            "app.kubernetes.io/managed-by".to_string(),
            "qrab-dashboard".to_string(),
        )]
        .into(),
    );
    assert!(
        send(&h, get("/vms/qrab/a", Some(&cookie)))
            .await
            .2
            .contains("Delete VM")
    );
    let (status, _, _) = send(
        &h,
        post(
            "/vms/qrab/a/delete",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&confirm_name=a",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        *h.cluster.deleted.lock().unwrap(),
        [("a".to_string(), "uid-a".to_string())]
    );
    let (_, _, body) = send(&h, get("/", Some(&cookie))).await;
    assert!(body.contains("its state PVC a-state are deleted as it goes"));
}

fn catalog_with_cirros(h: &Harness) {
    let broken = serde_json::json!({"name": "broken", "generation": 1});
    *h.cluster.catalog.lock().unwrap() =
        Some(serde_json::json!([crate::images::tests::cirros_json(), broken]).to_string());
}

fn base_images(h: &Harness, images: Vec<qrab_operator::image::BaseImage>) {
    *h.cluster.base_images.lock().unwrap() = images;
}

use crate::images::tests::resource as base_image;

#[tokio::test]
async fn the_images_page_lists_every_generation_for_any_signed_in_user() {
    let h = harness().await;
    assert_eq!(
        send(&h, get("/images", None)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let viewer = session(&h.state.sessions, BOB, &[LAUNCHERS]).await;
    let (status, _, body) = send(&h, get("/images", Some(&viewer))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("No base images are staged yet."));
    assert!(body.contains("href=\"/images\""));
    catalog_with_cirros(&h);
    let mut writing = base_image("cirros", 2, "Writing", false);
    writing.status.as_mut().unwrap().message = "converting <qcow2>".into();
    writing.status.as_mut().unwrap().sha256 = None;
    let mut adopted = base_image("alpine", 1, "Sealed", false);
    adopted.status.as_mut().unwrap().adopted = true;
    base_images(
        &h,
        vec![writing, base_image("cirros", 3, "Sealed", true), adopted],
    );
    let (status, _, body) = send(&h, get("/images", Some(&viewer))).await;
    assert_eq!(status, StatusCode::OK);
    for text in [
        "<code>cirros</code>",
        "<code>alpine</code>",
        "CirrOS 0.6.2",
        "Writing",
        "converting &#60;qcow2&#62;",
        "112 MiB",
        "title=\"fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210\"",
        "fedcba987654…",
        ">download.cirros-cloud.net</a>",
        "Retired",
        "Launchable",
        "Listed by ConfigMap qrab-base-images only",
        "A separate disk for each VM",
        "Image verification and staging",
        "4 generations",
    ] {
        assert!(body.contains(text), "{text}");
    }
    assert!(!body.contains("broken"));
    // Grouped by name, newest generation first.
    let at = |text: &str| body.find(text).unwrap();
    assert!(at("<code>alpine</code>") < at("<code>cirros</code>"));
    assert!(at("<code>cirros-g3</code>") < at("<code>cirros-g2</code>"));
    assert!(at("<code>cirros-g2</code>") < at("Listed by ConfigMap"));
    // Not an image admin: no forms, no refresh or adopt links.
    for absent in [
        "action=\"/images\"",
        "/retire",
        "?refresh=",
        "?adopt=",
        "Stage an image",
    ] {
        assert!(!body.contains(absent), "{absent}");
    }
    // A catalog the dashboard cannot read is an upstream failure, not an empty list.
    h.cluster
        .fail_base_images
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        send(&h, get("/images", Some(&viewer))).await.0,
        StatusCode::BAD_GATEWAY
    );
    h.cluster
        .fail_base_images
        .store(false, std::sync::atomic::Ordering::SeqCst);
    h.cluster
        .fail_catalog
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        send(&h, get("/images", Some(&viewer))).await.0,
        StatusCode::BAD_GATEWAY
    );
}

const STAGE: &str = "csrf=correct&image=debian&generation=1&title=Debian+13&os=debian&version=13&url=https%3A%2F%2Fcloud.debian.org%2Fimages%2Fcloud%2Ftrixie%2Flatest%2Fdebian-13-generic-amd64.qcow2&sha256=ABCDEF0123456789abcdef0123456789abcdef0123456789abcdef0123456789&format=qcow2";

#[tokio::test]
async fn image_forms_are_for_image_admins_and_404_for_everyone_else() {
    let h = harness().await;
    base_images(&h, vec![base_image("cirros", 1, "Sealed", false)]);
    for groups in [&[][..], &[LAUNCHERS, ADMINS][..]] {
        let cookie = session(&h.state.sessions, BOB, groups).await;
        for (path, body) in [
            ("/images", STAGE.to_string()),
            (
                "/images/cirros-g1/retire",
                "csrf=correct&confirm=yes".into(),
            ),
        ] {
            let (status, _, _) = send(&h, post(path, Some(&cookie), Some(ORIGIN), &body)).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path} {groups:?}");
        }
        let (_, _, page) = send(&h, get("/images?refresh=cirros", Some(&cookie))).await;
        assert!(!page.contains("name=\"generation\""));
    }
    assert_eq!(h.cluster.mutations(), 0);
    // An admin still needs CSRF and the exact Origin.
    let admin = session(&h.state.sessions, ALICE, &[IMAGE_ADMINS]).await;
    let (status, _, _) = send(
        &h,
        post("/images", Some(&admin), Some("https://evil.example"), STAGE),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = send(
        &h,
        post(
            "/images",
            Some(&admin),
            Some(ORIGIN),
            &STAGE.replace("csrf=correct", "csrf=wrong"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(h.cluster.mutations(), 0);
    let (_, _, page) = send(&h, get("/images", Some(&admin))).await;
    for text in [
        "action=\"/images\"",
        "action=\"/images/cirros-g1/retire\"",
        "href=\"/images?refresh=cirros#stage\"",
        "Refresh (generation 2)",
        "Stage an image",
        "name=\"csrf\" value=\"correct\"",
    ] {
        assert!(page.contains(text), "{text}");
    }
}

#[tokio::test]
async fn an_image_admin_stages_the_exact_base_image() {
    let h = harness().await;
    let admin = session(&h.state.sessions, ALICE, &[IMAGE_ADMINS]).await;
    let (status, headers, _) = send(&h, post("/images", Some(&admin), Some(ORIGIN), STAGE)).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers["location"], "/images");
    let writes = h.cluster.image_writes.lock().unwrap().clone();
    assert_eq!(writes.len(), 1);
    assert_eq!(
        (writes[0].0.as_str(), writes[0].1.as_str()),
        ("create", "debian-g1")
    );
    assert_eq!(
        writes[0].2,
        serde_json::json!({
            "apiVersion": "qrab.yapplesauce.com/v1alpha1",
            "kind": "BaseImage",
            "metadata": {
                "name": "debian-g1",
                "namespace": "qrab",
                "annotations": {"qrab.yapplesauce.com/creator-subject": ALICE},
                "labels": {"app.kubernetes.io/managed-by": "qrab-dashboard"},
            },
            "spec": {
                "image": "debian", "generation": 1, "title": "Debian 13", "os": "debian",
                "version": "13", "retired": false,
                "source": {
                    "url": "https://cloud.debian.org/images/cloud/trixie/latest/debian-13-generic-amd64.qcow2",
                    "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
                    "format": "qcow2",
                },
            },
        })
    );
    let (_, _, page) = send(&h, get("/images", Some(&admin))).await;
    assert!(page.contains("Staging Debian 13 as debian-g1."));
    assert!(page.contains("Pending"));
    // A generation is never staged twice.
    let (status, _, page) = send(&h, post("/images", Some(&admin), Some(ORIGIN), STAGE)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(page.contains("debian g1 already exists"), "{page}");
    assert!(page.contains("such as 2"));
    assert_eq!(h.cluster.image_writes.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn invalid_stage_fields_rerender_the_form_and_create_nothing() {
    let h = harness().await;
    let admin = session(&h.state.sessions, ALICE, &[IMAGE_ADMINS]).await;
    let body = STAGE
        .replace("image=debian", "image=Debian_13")
        .replace("generation=1", "generation=0")
        .replace("url=https%3A", "url=http%3A")
        .replace("sha256=ABCDEF", "sha256=XYZ")
        .replace("format=qcow2", "format=vmdk");
    let (status, _, page) = send(&h, post("/images", Some(&admin), Some(ORIGIN), &body)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    for text in [
        "Check the form",
        "Nothing was staged.",
        "href=\"#stage-image\"",
        "href=\"#stage-generation\"",
        "href=\"#stage-url\"",
        "href=\"#stage-sha256\"",
        "href=\"#stage-format\"",
        "id=\"error-url\"",
        "aria-invalid=\"true\"",
        // The rejected values come back, the CSRF token only as the form's own.
        "value=\"Debian_13\"",
        "value=\"Debian 13\"",
    ] {
        assert!(page.contains(text), "{text}");
    }
    assert!(!page.contains("href=\"#stage-title\""));
    assert!(page.contains("<title>Check image form"));
    assert_eq!(h.cluster.mutations(), 0);
}

#[tokio::test]
async fn refresh_and_adopt_prefill_the_stage_form() {
    let h = harness().await;
    catalog_with_cirros(&h);
    base_images(&h, vec![base_image("cirros", 2, "Sealed", false)]);
    let admin = session(&h.state.sessions, ALICE, &[IMAGE_ADMINS]).await;
    let (_, _, page) = send(&h, get("/images?refresh=cirros", Some(&admin))).await;
    assert!(page.contains("Refresh cirros"));
    let form = &page[page.find("id=\"stage\"").unwrap()..];
    for text in [
        "name=\"image\" value=\"cirros\"",
        "name=\"generation\" value=\"3\"",
        "name=\"title\" value=\"CirrOS 0.6.2\"",
        "name=\"url\" value=\"\"",
        "name=\"sha256\" value=\"\"",
    ] {
        assert!(form.contains(text), "{text}: {form}");
    }
    // The ConfigMap-only g1 can be adopted as it is.
    assert!(page.contains("href=\"/images?adopt=cirros:1#stage\""));
    let (_, _, page) = send(&h, get("/images?adopt=cirros:1", Some(&admin))).await;
    let form = &page[page.find("id=\"stage\"").unwrap()..];
    assert!(form.contains("Adopt cirros g1"));
    assert!(form.contains("name=\"generation\" value=\"1\""));
    assert!(form.contains(&format!(
        "name=\"sha256\" value=\"{}\"",
        crate::images::tests::cirros().upstream_sha256
    )));
    assert_eq!(h.cluster.mutations(), 0);
}

#[tokio::test]
async fn retiring_is_confirmed_and_patches_by_uid() {
    let h = harness().await;
    base_images(
        &h,
        vec![
            base_image("cirros", 1, "Sealed", false),
            base_image("cirros", 2, "Sealed", true),
        ],
    );
    let admin = session(&h.state.sessions, ALICE, &[IMAGE_ADMINS]).await;
    let (_, _, page) = send(&h, get("/images", Some(&admin))).await;
    assert!(page.contains("action=\"/images/cirros-g1/retire\""));
    assert!(
        !page.contains("action=\"/images/cirros-g2/retire\""),
        "already retired"
    );
    assert!(page.contains("permanently"));
    let retire = |path: &str, body: &str| post(path, Some(&admin), Some(ORIGIN), body);
    for (path, body, needle) in [
        (
            "/images/cirros-g1/retire",
            "csrf=correct",
            "cannot be undone",
        ),
        (
            "/images/cirros-g2/retire",
            "csrf=correct&confirm=yes",
            "already retired",
        ),
        (
            "/images/missing-g1/retire",
            "csrf=correct&confirm=yes",
            "No such base image",
        ),
    ] {
        let (status, _, page) = send(&h, retire(path, body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert!(page.contains(needle), "{needle}");
    }
    assert_eq!(h.cluster.mutations(), 0);
    let (status, headers, _) = send(
        &h,
        retire("/images/cirros-g1/retire", "csrf=correct&confirm=yes"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers["location"], "/images");
    assert_eq!(
        h.cluster.image_writes.lock().unwrap().clone(),
        [(
            "patch".to_string(),
            "cirros-g1".to_string(),
            serde_json::json!({"metadata": {"uid": "uid-cirros-g1"}, "spec": {"retired": true}})
        )]
    );
    let (_, _, page) = send(&h, get("/images", Some(&admin))).await;
    assert!(page.contains("Retired cirros-g1."));
}

#[tokio::test]
async fn launch_offers_and_accepts_only_launchable_generations() {
    let h = harness().await;
    // The ConfigMap lists cirros g1; a BaseImage for g1 still writing hides it.
    catalog_with_cirros(&h);
    base_images(
        &h,
        vec![
            base_image("cirros", 1, "Writing", false),
            base_image("cirros", 2, "Sealed", true),
            base_image("cirros", 3, "Sealed", false),
        ],
    );
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (_, _, form) = send(&h, get("/launch", Some(&cookie))).await;
    assert!(form.contains("value=\"cirros:3\""));
    assert!(!form.contains("value=\"cirros:1\"") && !form.contains("value=\"cirros:2\""));
    let launch = |os: &str| {
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            &format!("csrf=correct&name=web&cpus=1&memory_mib=512&os={os}&root_size_gib=1"),
        )
    };
    for os in ["cirros:1", "cirros:2"] {
        let (status, _, body) = send(&h, launch(os)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{os}");
        assert!(body.contains("not in the base-image catalog"), "{os}");
    }
    assert_eq!(h.cluster.mutations(), 0);
    assert_eq!(send(&h, launch("cirros:3")).await.0, StatusCode::SEE_OTHER);
    let created = h.cluster.created_vms.lock().unwrap()[0].clone();
    assert_eq!(
        created["spec"]["disks"][0]["baseImage"],
        serde_json::json!({"name": "cirros", "generation": 3})
    );
    // The VM page names its boot source from the BaseImage, retired or not.
    let mut vm: VirtualMachine = serde_json::from_value(created).unwrap();
    vm.metadata.uid = Some("uid-web".into());
    h.cluster.vms.lock().unwrap().push(vm);
    base_images(&h, vec![base_image("cirros", 3, "Sealed", true)]);
    let (_, _, body) = send(&h, get("/vms/qrab/web", Some(&cookie))).await;
    assert!(
        body.contains("CirrOS 0.6.3 (cirros g3) clone on root"),
        "{body}"
    );
}

#[tokio::test]
async fn the_image_admin_group_resolves_by_name_or_uuid() {
    let h = harness().await;
    let kanidm = &h.state.kanidm;
    h.dir
        .lock()
        .unwrap()
        .groups
        .insert("qrab_image_admins".into(), IMAGE_ADMINS.into());
    assert_eq!(
        kanidm
            .resolve_group("qrab_image_admins")
            .await
            .unwrap()
            .as_deref(),
        Some(IMAGE_ADMINS)
    );
    assert_eq!(kanidm.resolve_group("qrab_missing").await.unwrap(), None);
    let calls = h.kanidm.lock().unwrap().len();
    assert_eq!(
        kanidm
            .resolve_group(&IMAGE_ADMINS.to_uppercase())
            .await
            .unwrap()
            .as_deref(),
        Some(IMAGE_ADMINS)
    );
    assert_eq!(
        h.kanidm.lock().unwrap().len(),
        calls,
        "a UUID needs no lookup"
    );
}

#[tokio::test]
async fn launching_with_an_os_clones_the_catalog_image_as_root() {
    let h = harness().await;
    catalog_with_cirros(&h);
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (status, _, body) = send(&h, get("/launch", Some(&cookie))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("value=\"cirros:1\"") && body.contains("None: demo ESP only"));
    assert!(body.contains("name=\"root_size_gib\""));
    // A stale or unknown choice and a too-small root create nothing.
    for (fields, needle) in [
        ("os=cirros:2", "not in the base-image catalog"),
        ("os=cirros:1&root_size_gib=0", "Root disk size"),
        (
            "os=cirros:1&disk0_id=root&disk0_size_gib=1",
            "cannot be named root",
        ),
    ] {
        let (status, _, body) = send(
            &h,
            post(
                "/launch",
                Some(&cookie),
                Some(ORIGIN),
                &format!("csrf=correct&name=web&cpus=1&memory_mib=512&{fields}"),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{fields}");
        assert!(body.contains(needle), "{needle}");
    }
    assert!(h.kanidm.lock().unwrap().is_empty());
    assert_eq!(h.cluster.mutations(), 0);
    let (status, headers, _) = send(
        &h,
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&name=web&cpus=1&memory_mib=512&os=cirros:1&root_size_gib=2",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers["location"], "/auth/login?next=/vms/qrab/web");
    let created = h.cluster.created_vms.lock().unwrap()[0].clone();
    assert_eq!(
        created["spec"]["disks"],
        serde_json::json!([{"id": "root", "sizeBytes": 2147483648u64,
            "baseImage": {"name": "cirros", "generation": 1}, "bootIndex": 0}])
    );
    assert_eq!(created["spec"]["storageNode"], "kube03");
    assert_eq!(
        created["metadata"]["labels"]["app.kubernetes.io/managed-by"],
        "qrab-dashboard"
    );
    assert_eq!(
        created["metadata"]["annotations"]["qrab.yapplesauce.com/creator-subject"],
        ALICE
    );
    let (_, admins) = h.dir.lock().unwrap().launched("admins");
    assert_eq!(
        created["spec"]["console"]["adminGroups"],
        serde_json::json!([admins])
    );
    // The detail page names the boot source.
    let mut vm: VirtualMachine = serde_json::from_value(created).unwrap();
    vm.metadata.uid = Some("uid-web".into());
    h.cluster.vms.lock().unwrap().push(vm);
    let (_, _, body) = send(&h, get("/vms/qrab/web", Some(&cookie))).await;
    assert!(
        body.contains("CirrOS 0.6.3 (cirros g1) clone on root"),
        "{body}"
    );
    let (_, _, body) = send(&h, get("/vms/qrab/a", Some(&cookie))).await;
    assert!(body.contains("demo ESP"));
}

#[tokio::test]
async fn without_an_os_an_unreadable_catalog_does_not_block_launches() {
    let h = harness().await;
    h.cluster
        .fail_catalog
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (status, _, body) = send(&h, get("/launch", Some(&cookie))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("catalog is unavailable"));
    let launch = |os: &str| {
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            &format!("csrf=correct&name=web&cpus=1&memory_mib=512&os={os}"),
        )
    };
    assert_eq!(
        send(&h, launch("cirros:1")).await.0,
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(h.cluster.mutations(), 0);
    assert_eq!(send(&h, launch("none")).await.0, StatusCode::SEE_OTHER);
    let created = h.cluster.created_vms.lock().unwrap()[0].clone();
    assert!(created["spec"].get("disks").is_none());
    assert!(created["spec"].get("storageNode").is_none());
}

#[tokio::test]
async fn launch_creates_groups_pvc_and_vm_in_order() {
    let h = harness().await;
    let outsider = session(&h.state.sessions, ALICE, &[]).await;
    assert_eq!(
        send(&h, get("/launch", Some(&outsider))).await.0,
        StatusCode::FORBIDDEN
    );
    let form = "csrf=correct&name=web&cpus=2&max_cpus=4&memory_mib=1024&max_memory_mib=&accelerator=kvm&redfish=on&disk0_id=data&disk0_size_gib=4&disk1_id=&disk1_size_gib=";
    assert_eq!(
        send(&h, post("/launch", Some(&outsider), Some(ORIGIN), form))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert!(h.kanidm.lock().unwrap().is_empty());
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (status, _, body) = send(&h, get("/launch", Some(&cookie))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("name=\"disk3_id\"") && body.contains("name=\"csrf\""));
    let (status, _, body) = send(
        &h,
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&name=Web&cpus=0&memory_mib=512",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("DNS label") && body.contains("vCPUs must"));
    assert!(h.kanidm.lock().unwrap().is_empty());
    let (status, headers, _) = send(&h, post("/launch", Some(&cookie), Some(ORIGIN), form)).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    // A launch refreshes the sign-in and returns to the new VM.
    assert_eq!(headers["location"], "/auth/login?next=/vms/qrab/web");
    let (admins, viewers) = {
        let dir = h.dir.lock().unwrap();
        let (admins_name, admins) = dir.launched("admins");
        let (viewers_name, viewers) = dir.launched("viewers");
        // Both groups come from the same launch ID.
        assert_eq!(
            admins_name.trim_end_matches("_admins"),
            viewers_name.trim_end_matches("_viewers")
        );
        let mut nested = dir.members[VM_USERS].clone();
        nested.sort();
        let mut expected = vec![admins.clone(), viewers.clone()];
        expected.sort();
        assert_eq!(nested, expected);
        assert_eq!(dir.members[&admins], [ALICE]);
        assert!(!dir.members.contains_key(&viewers));
        (admins, viewers)
    };
    let calls: Vec<_> = h
        .kanidm
        .lock()
        .unwrap()
        .iter()
        .map(|(m, p, a, _)| {
            assert_eq!(a, &format!("Bearer {KANIDM_TOKEN}"));
            format!("{m} {p}")
        })
        .collect();
    let admins_name = logged_group(&h.kanidm, "admins");
    let viewers_name = logged_group(&h.kanidm, "viewers");
    assert_eq!(
        calls[..3],
        [
            format!("GET /v1/group/{admins_name}"),
            format!("GET /v1/group/{viewers_name}"),
            "POST /v1/group".to_owned()
        ]
    );
    assert!(calls.contains(&format!("POST /v1/group/{admins}/_attr/member")));
    let pvc = serde_json::to_value(&h.cluster.pvcs.lock().unwrap()["web-state"]).unwrap();
    assert_eq!(pvc["spec"]["storageClassName"], "zfs-block");
    let created = h.cluster.created_vms.lock().unwrap()[0].clone();
    assert_eq!(
        created["metadata"]["annotations"]["qrab.yapplesauce.com/creator-subject"],
        ALICE
    );
    assert_eq!(
        created["spec"]["console"],
        serde_json::json!({"adminGroups": [admins], "viewerGroups": [viewers]})
    );
    // The form's Redfish field is ignored: the BMC's Redfish sidecar serves the dashboard.
    assert!(created["spec"].get("components").is_none());
    assert_eq!(
        created["metadata"]["labels"]["app.kubernetes.io/managed-by"],
        "qrab-dashboard"
    );
    assert_eq!(created["spec"]["storageNode"], "kube03");
    assert_eq!(created["spec"]["disks"][0]["sizeBytes"], 4u64 << 30);
    assert_eq!(created["spec"]["accelerator"], "kvm");
    // The created VM is a valid VirtualMachine, and its launcher's groups
    // make them its admin. The same name again is refused before anything is created.
    let created: VirtualMachine = serde_json::from_value(created).unwrap();
    assert_eq!(
        access::role(&created, BOB, std::slice::from_ref(&admins)),
        Some(Role::Admin)
    );
    assert_eq!(access::role(&created, ALICE, &[]), Some(Role::Viewer));
    h.cluster.vms.lock().unwrap().push(created);
    let before = h.kanidm.lock().unwrap().len();
    let (status, _, body) = send(&h, post("/launch", Some(&cookie), Some(ORIGIN), form)).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body.contains("already exists") && body.contains("Nothing was created"));
    assert_eq!(h.kanidm.lock().unwrap().len(), before);
}

#[tokio::test]
async fn a_failed_launch_reports_and_rolls_back_its_groups() {
    let h = harness().await;
    h.cluster
        .fail_vm_create
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (status, _, body) = send(
        &h,
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&name=web&cpus=1&memory_mib=512",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(
        body.contains("Create the VirtualMachine") && body.contains("admission webhook denied")
    );
    let admins_name = logged_group(&h.kanidm, "admins");
    assert!(body.contains(&format!("Kanidm group {admins_name} deleted")));
    assert!(body.contains("PersistentVolumeClaim qrab/web-state: kept"));
    assert!(h.dir.lock().unwrap().groups.is_empty());
    // An existing group is never adopted.
    h.cluster
        .fail_vm_create
        .store(false, std::sync::atomic::Ordering::SeqCst);
    h.dir.lock().unwrap().taken = true;
    let (status, _, body) = send(
        &h,
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&name=db&cpus=1&memory_mib=512",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body.contains("never adopted"));
    assert!(!h.cluster.pvcs.lock().unwrap().contains_key("db-state"));
    assert!(h.cluster.created_vms.lock().unwrap().is_empty());
}

#[tokio::test]
async fn launches_roll_back_by_name_and_recheck_the_pvc() {
    let h = harness().await;
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let launch = |name: &str| {
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            &format!("csrf=correct&name={name}&cpus=1&memory_mib=512"),
        )
    };
    // The group is created but its UUID cannot be read back (two lookups):
    // it is still rolled back, by name.
    h.dir.lock().unwrap().blind = true;
    let (status, _, body) = send(&h, launch("web")).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let admins_name = logged_group(&h.kanidm, "admins");
    let admins_path = format!("/v1/group/{admins_name}");
    assert!(body.contains(&format!("Read Kanidm group {admins_name}")));
    assert!(body.contains(&format!("Kanidm group {admins_name} deleted")));
    assert!(h.dir.lock().unwrap().groups.is_empty());
    let log = h.kanidm.lock().unwrap().clone();
    let lookups = log
        .iter()
        .filter(|(m, p, _, _)| m == "GET" && *p == admins_path)
        .count();
    assert_eq!(lookups, 3, "preflight plus one retry after creation");
    assert!(
        log.iter()
            .any(|(m, p, _, _)| m == "DELETE" && *p == admins_path)
    );
    h.dir.lock().unwrap().blind = false;
    // A bound claim that appears after the preflight is not reused.
    let bound: PersistentVolumeClaim = serde_json::from_value(serde_json::json!({
        "metadata": {"name": "db-state", "labels": {"app.kubernetes.io/managed-by": "qrab-dashboard"}},
        "status": {"phase": "Bound"}
    }))
    .unwrap();
    *h.cluster.racing_pvc.lock().unwrap() = Some(bound);
    let (status, _, body) = send(&h, launch("db")).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(body.contains("Reuse the state PVC"));
    assert!(h.dir.lock().unwrap().groups.is_empty());
    assert!(h.cluster.created_vms.lock().unwrap().is_empty());
}

/// Captures every log line written while it is the thread's default subscriber.
#[derive(Clone, Default)]
struct Logs(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Logs {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn launching_with_a_nic_and_cloud_init_never_shows_or_logs_user_data() {
    let logs = Logs::default();
    let writer = logs.clone();
    let _guard = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish(),
    );
    let h = harness().await;
    catalog_with_cirros(&h);
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    // A fresh form attaches eth0 and warns where user-data is stored.
    let (_, _, body) = send(&h, get("/launch", Some(&cookie))).await;
    assert!(
        body.contains("name=\"nic0_enabled\" value=\"on\" checked"),
        "{body}"
    );
    assert!(body.contains("Attach a network interface (eth0)"));
    assert!(body.contains("name=\"nic0_security_groups\""));
    assert!(body.contains("name=\"hostname\"") && body.contains("name=\"user_data\""));
    assert!(body.contains("default-allow") && body.contains("docs/networking.md"));
    assert!(body.contains("stored in the VM&#39;s ConfigMap"));
    // More than the 16 KiB default body limit: user-data may be 32 KiB.
    let secret = "SECRET-USER-DATA-hunter2";
    let user_data = format!("#cloud-config\r\n# {secret}\r\n{}\r\n", "#".repeat(20_000));
    let form = |extra: &[(&str, &str)]| {
        let mut s = url::form_urlencoded::Serializer::new(String::new());
        s.extend_pairs([
            ("csrf", "correct"),
            ("name", "web"),
            ("cpus", "1"),
            ("memory_mib", "512"),
            ("os", "cirros:1"),
            ("user_data", user_data.as_str()),
        ]);
        s.extend_pairs(extra);
        s.finish()
    };
    // A rejected form names the problem and never sends user-data back.
    let (status, _, body) = send(
        &h,
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            &form(&[
                ("nic0_enabled", "on"),
                ("nic0_security_groups", "web,Bad"),
                ("hostname", "web-1"),
            ]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("Security group Bad on eth0"), "{body}");
    assert!(body.contains("Paste it again") && body.contains("value=\"web,Bad\""));
    assert!(!body.contains(secret));
    assert_eq!(h.cluster.mutations(), 0);
    // Unchecking the NIC sticks on a re-rendered form.
    let (_, _, body) = send(
        &h,
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            &form(&[("hostname", "Web")]),
        ),
    )
    .await;
    assert!(body.contains("Hostname must be a DNS label"));
    assert!(body.contains("name=\"nic0_enabled\" value=\"on\""));
    assert!(!body.contains("name=\"nic0_enabled\" value=\"on\" checked"));
    // Parallel tests without a subscriber may have cached "never" for the
    // launch log line's callsite.
    tracing::callsite::rebuild_interest_cache();
    let (status, _, _) = send(
        &h,
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            &form(&[
                ("nic0_enabled", "on"),
                ("nic0_security_groups", "web, ssh"),
                ("hostname", "web-1"),
            ]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let created = h.cluster.created_vms.lock().unwrap()[0].clone();
    assert_eq!(
        created["spec"]["nics"],
        serde_json::json!([{"name": "eth0", "securityGroups": ["web", "ssh"]}])
    );
    assert_eq!(created["spec"]["cloudInit"]["hostname"], "web-1");
    let stored = created["spec"]["cloudInit"]["userData"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(stored.contains(secret) && !stored.contains('\r'));
    // The new VM's pages show the NIC and the user-data size only.
    let mut vm: VirtualMachine = serde_json::from_value(created).unwrap();
    vm.metadata.uid = Some("uid-web".into());
    vm.status = Some(
        serde_json::from_value(
            serde_json::json!({"observedGeneration": 1, "phase": "Running",
            "ready": true, "reason": "", "message": "", "nodeName": "kube01",
            "nics": [{"name": "eth0", "mac": "02:aa:bb:cc:dd:ee", "podName": "web-nic-eth0",
                "ip": "10.244.1.9", "ready": true}]}),
        )
        .unwrap(),
    );
    let bytes = stored.len();
    h.cluster.vms.lock().unwrap().push(vm);
    let (_, _, detail) = send(&h, get("/vms/qrab/web", Some(&cookie))).await;
    assert!(detail.contains(&format!("hostname web-1, user-data {bytes} bytes")));
    for cell in [
        "02:aa:bb:cc:dd:ee",
        "10.244.1.9",
        "web, ssh",
        "web-nic-eth0",
        "data-label=\"Security groups\"",
        "Guest traffic leaves from the NIC pod&#39;s IP; the VM pod (BMC) never carries it.",
    ] {
        assert!(detail.contains(cell), "{cell}");
    }
    let (_, _, list) = send(&h, get("/", Some(&cookie))).await;
    assert!(list.contains("data-label=\"Location\"") && list.contains("10.244.1.9"));
    let logs = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("VM launch"), "{logs}");
    for page in [&detail, &list, &logs] {
        assert!(!page.contains(secret) && !page.contains("hunter2"));
    }
    // Other routes keep the 16 KiB limit.
    let (status, _, _) = send(
        &h,
        post(
            "/vms/qrab/a/action",
            Some(&cookie),
            Some(ORIGIN),
            &format!("csrf=correct&action=reset&pad={}", "x".repeat(20_000)),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn oversized_redfish_responses_are_refused() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let padded = |size: usize| {
        let mut big = String::from("{\"Name\":\"");
        big.push_str(&"x".repeat(size));
        big.push_str("\",\"Members\":[]}");
        ([("content-type", "application/json")], big)
    };
    serve(
        listener,
        axum::Router::new()
            .route(
                redfish::SYSTEM,
                axum::routing::get(move || async move { padded(redfish::MAX_BODY) }),
            )
            .route(
                redfish::RESET_INFO,
                axum::routing::get(|| async { Json(serde_json::json!({"Parameters": []})) }),
            )
            .route(
                redfish::EVENT_ENTRIES,
                axum::routing::get(move || async move { padded(2 * redfish::MAX_BODY) }),
            )
            .route(
                redfish::EVENT_LOG,
                axum::routing::get(move || async move { padded(redfish::MAX_LOG_BODY) }),
            ),
    )
    .await;
    let client = redfish::Redfish::for_test(port).unwrap();
    let ip = "127.0.0.1".parse().unwrap();
    let metadata_vm: VirtualMachine = serde_json::from_value(serde_json::json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine","metadata":{"name":"test","namespace":"qrab","uid":"uid-test"},"spec":{"stateVolumeClaim":"test-state"}})).unwrap();
    let client = client.for_vm(&metadata_vm, ip).unwrap();
    let system: Result<redfish::System, _> =
        client.get(redfish::SYSTEM, "t", redfish::MAX_BODY).await;
    assert_eq!(system.unwrap_err(), redfish::RedfishError::Invalid);
    assert_eq!(client.reset_types("t").await.unwrap(), []);
    // The whole event log fits its larger bound; nothing else does.
    let panel = client.panel("t").await;
    assert!(panel.system.is_err());
    let events = panel.events.unwrap();
    assert!(events.entries.is_empty() && events.qmp_dropped.is_none());
}

#[tokio::test]
async fn one_subject_holds_at_most_four_consoles() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let h = harness().await;
    let admin = session(&h.state.sessions, ALICE, &[]).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    serve(listener, router(h.state.clone())).await;
    let connect = || {
        let mut req = format!("ws://{addr}/vms/qrab/a/console/ws")
            .into_client_request()
            .unwrap();
        req.headers_mut()
            .insert("cookie", HeaderValue::from_str(&admin).unwrap());
        req.headers_mut()
            .insert("origin", HeaderValue::from_static(ORIGIN));
        tokio_tungstenite::connect_async(req)
    };
    let mut open = Vec::new();
    for _ in 0..console::PER_SUBJECT {
        open.push(connect().await.unwrap().0);
    }
    match connect().await {
        Err(tokio_tungstenite::tungstenite::Error::Http(r)) => {
            assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE)
        }
        other => panic!("expected 503, got {:?}", other.map(|_| ())),
    }
    assert_eq!(h.state.console_subjects.lock().unwrap()[ALICE], 4);
    // Closing one console frees its slot.
    drop(open.pop());
    let freed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if h.state.console_subjects.lock().unwrap().get(ALICE) == Some(&3) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(freed.is_ok());
    assert!(connect().await.is_ok());
}

fn post_json(
    path: &str,
    cookie: Option<&str>,
    origin: Option<&str>,
    csrf: Option<&str>,
    body: serde_json::Value,
) -> Request<Body> {
    let mut req = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    for (key, value) in [
        ("cookie", cookie),
        ("origin", origin),
        ("x-csrf-token", csrf),
    ] {
        if let Some(value) = value {
            req = req.header(key, value);
        }
    }
    req.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn console_power_controls_are_admin_only_checked_and_reach_redfish_or_the_vm() {
    let h = harness().await;
    let admin = session(&h.state.sessions, ALICE, &[VIEWERS]).await;
    let (status, _, page) = send(&h, get("/vms/qrab/a/console", Some(&admin))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("data-endpoint=\"/vms/qrab/a/power\""));
    assert!(page.contains("data-csrf=\"correct\""));
    for button in ui::console_power() {
        assert!(
            page.contains(&format!("data-power=\"{}\"", button.action)),
            "{button:?}"
        );
    }
    for (action, label) in [
        ("GracefulRestart", "Restart guest"),
        ("PowerCycle", "Power cycle"),
        ("Nmi", "NMI"),
    ] {
        assert!(page.contains(&format!(
            "data-power=\"{action}\" data-label=\"{label}\" data-in-pod=\"true\""
        )));
    }

    let pause = serde_json::json!({"action": "Pause"});
    let path = "/vms/qrab/a/power";
    for (vm, cookie, origin, csrf, body, expected) in [
        (
            "a",
            None,
            Some(ORIGIN),
            Some("correct"),
            pause.clone(),
            StatusCode::UNAUTHORIZED,
        ),
        (
            "a",
            Some(admin.as_str()),
            Some(ORIGIN),
            None,
            pause.clone(),
            StatusCode::FORBIDDEN,
        ),
        (
            "a",
            Some(admin.as_str()),
            Some(ORIGIN),
            Some("wrong"),
            pause.clone(),
            StatusCode::FORBIDDEN,
        ),
        (
            "a",
            Some(admin.as_str()),
            Some("https://evil.example"),
            Some("correct"),
            pause.clone(),
            StatusCode::FORBIDDEN,
        ),
        (
            "a",
            Some(admin.as_str()),
            None,
            Some("correct"),
            pause.clone(),
            StatusCode::FORBIDDEN,
        ),
        (
            "b",
            Some(admin.as_str()),
            Some(ORIGIN),
            Some("correct"),
            pause.clone(),
            StatusCode::FORBIDDEN,
        ),
        (
            "b",
            Some(admin.as_str()),
            Some(ORIGIN),
            Some("correct"),
            serde_json::json!({"action": "on"}),
            StatusCode::FORBIDDEN,
        ),
        (
            "c",
            Some(admin.as_str()),
            Some(ORIGIN),
            Some("correct"),
            pause.clone(),
            StatusCode::NOT_FOUND,
        ),
        (
            "a",
            Some(admin.as_str()),
            Some(ORIGIN),
            Some("correct"),
            serde_json::json!({"action": "system_reset"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            "a",
            Some(admin.as_str()),
            Some(ORIGIN),
            Some("correct"),
            serde_json::json!({"action": "ForceRestart"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            "a",
            Some(admin.as_str()),
            Some(ORIGIN),
            Some("correct"),
            serde_json::json!({"action": "On"}),
            StatusCode::BAD_REQUEST,
        ),
        (
            "a",
            Some(admin.as_str()),
            Some(ORIGIN),
            Some("correct"),
            serde_json::json!({"action": "off", "confirm": false}),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let (status, _, _) = send(
            &h,
            post_json(
                &format!("/vms/qrab/{vm}/power"),
                cookie,
                origin,
                csrf,
                body.clone(),
            ),
        )
        .await;
        assert_eq!(status, expected, "{vm} {origin:?} {csrf:?} {body}");
    }
    assert!(h.bmc_a.lock().unwrap().is_empty() && h.bmc_b.lock().unwrap().is_empty());
    assert!(h.cluster.running.lock().unwrap().is_empty());

    let resets = [
        "Pause",
        "Resume",
        "GracefulRestart",
        "ForceRestart",
        "PowerCycle",
        "Nmi",
        "GracefulShutdown",
        "ForceOff",
    ];
    for reset in resets {
        let (status, _, text) = send(
            &h,
            post_json(
                path,
                Some(&admin),
                Some(ORIGIN),
                Some("correct"),
                serde_json::json!({"action": reset, "confirm": true}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{reset}: {text}");
    }
    for body in [
        serde_json::json!({"action": "off", "confirm": true}),
        serde_json::json!({"action": "on"}),
    ] {
        let (status, _, text) = send(
            &h,
            post_json(
                path,
                Some(&admin),
                Some(ORIGIN),
                Some("correct"),
                body.clone(),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}: {text}");
    }
    // Each in-pod action is the matching ResetType with the user's bearer.
    // Power on reaches the running VM's Pod as Redfish On: a powered-off
    // VM keeps its Pod and starts again in place.
    let sent: Vec<_> = h.bmc_a.lock().unwrap().clone();
    assert_eq!(
        sent,
        resets
            .into_iter()
            .chain(["On"])
            .map(|reset| (
                "POST".to_string(),
                "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset".to_string(),
                "Bearer id-token-of-alice".to_string(),
                format!(r#"{{"ResetType":"{reset}"}}"#)
            ))
            .collect::<Vec<_>>()
    );
    assert!(h.bmc_b.lock().unwrap().is_empty());
    assert_eq!(
        *h.cluster.running.lock().unwrap(),
        [("a".into(), "uid-a".into(), false)]
    );
    // A VM set to stop has no Pod to ask: Power on sets spec.running.
    h.cluster.vms.lock().unwrap()[0].spec.running = false;
    let (status, _, text) = send(
        &h,
        post_json(
            path,
            Some(&admin),
            Some(ORIGIN),
            Some("correct"),
            serde_json::json!({"action": "on"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{text}");
    assert_eq!(
        h.cluster.running.lock().unwrap().last(),
        Some(&("a".into(), "uid-a".into(), true))
    );
    assert_eq!(h.bmc_a.lock().unwrap().len(), resets.len() + 1);
}

#[tokio::test]
async fn console_power_state_is_admin_only_and_reads_the_vms_own_redfish() {
    let h = harness().await;
    let admin = session(&h.state.sessions, ALICE, &[VIEWERS]).await;
    let (status, headers, body) = send(&h, get("/vms/qrab/a/power", Some(&admin))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    let state: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(state["running"], true);
    assert_eq!(state["pod"], true);
    assert_eq!(state["phase"], "Running");
    assert_eq!(state["power_state"], "On");
    assert_eq!(state["health"], "OK");
    assert!(state["redfish_error"].is_null());
    // ResetActionInfo's values the console offers, in button order: no On, and
    // no PowerCycle where Redfish does not allow it.
    assert_eq!(
        state["reset_types"],
        serde_json::json!([
            "Resume",
            "Pause",
            "GracefulRestart",
            "ForceRestart",
            "Nmi",
            "GracefulShutdown"
        ])
    );
    let mut seen = h.bmc_a.lock().unwrap().clone();
    seen.sort();
    assert_eq!(
        seen,
        [
            (
                "GET".into(),
                "/redfish/v1/Systems/1".into(),
                "Bearer id-token-of-alice".into(),
                String::new()
            ),
            (
                "GET".into(),
                "/redfish/v1/Systems/1/ResetActionInfo".into(),
                "Bearer id-token-of-alice".into(),
                String::new()
            ),
        ]
    );
    for (vm, cookie, expected) in [
        ("a", None, StatusCode::UNAUTHORIZED),
        ("b", Some(admin.as_str()), StatusCode::FORBIDDEN),
        ("c", Some(admin.as_str()), StatusCode::NOT_FOUND),
    ] {
        let (status, _, _) = send(&h, get(&format!("/vms/qrab/{vm}/power"), cookie)).await;
        assert_eq!(status, expected, "{vm}");
    }
    assert!(h.bmc_b.lock().unwrap().is_empty());
}

/// Sets VM `a`'s components: Redfish-only VMs and VMs with neither.
fn set_components(h: &Harness, components: serde_json::Value) {
    h.cluster.vms.lock().unwrap()[0].spec.components = serde_json::from_value(components).unwrap();
}

/// The POSTed ResetTypes VM a's Redfish saw, in order.
fn resets_sent(h: &Harness) -> Vec<String> {
    h.bmc_a
        .lock()
        .unwrap()
        .iter()
        .filter(|(m, _, _, _)| m == "POST")
        .map(|(_, _, auth, body)| {
            assert_eq!(auth, "Bearer id-token-of-alice");
            body.clone()
        })
        .collect()
}

#[tokio::test]
async fn a_redfish_only_vm_is_read_and_power_managed_through_its_redfish() {
    let h = harness().await;
    // Redfish's own lists hold Alice; the console's are the dashboard's role.
    set_components(
        &h,
        serde_json::json!({"bmc": {"enabled": false},
            "redfish": {"enabled": true, "adminSubjects": [ALICE]}}),
    );
    let admin = session(&h.state.sessions, ALICE, &[]).await;
    let (status, _, body) = send(&h, get("/vms/qrab/a", Some(&admin))).await;
    assert_eq!(status, StatusCode::OK);
    // The panel comes from Redfish; there is no console without the BMC.
    assert!(
        body.contains("bmc-a") && body.contains("Recent QMP events"),
        "{body}"
    );
    assert!(body.contains("BMC off · Redfish on"));
    assert!(!body.contains("Open console") && !body.contains("Console: admins only"));
    // Power on and off sit beside the in-pod resets, since no console page has them.
    assert!(body.contains("power-title"));
    for action in ["on", "off", "ForceRestart", "Pause"] {
        assert!(body.contains(&format!("value=\"{action}\"")), "{action}");
    }
    let (status, _, text) = send(&h, get("/vms/qrab/a/power", Some(&admin))).await;
    assert_eq!(status, StatusCode::OK);
    let state: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(state["power_state"], "On");
    assert!(state["reset_types"].is_array() && state["redfish_error"].is_null());
    h.bmc_a.lock().unwrap().clear();

    let form = |body: &'static str| post("/vms/qrab/a/action", Some(&admin), Some(ORIGIN), body);
    let (status, _, _) = send(&h, form("csrf=correct&action=ForceRestart&confirm=yes")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (_, _, body) = send(&h, get("/vms/qrab/a", Some(&admin))).await;
    assert!(body.contains("Reset sent to a."), "{body}");
    // Power on while the VM is set to run and its Pod is up (it may be
    // powered off in place) is Redfish On, from the form and the JSON route.
    let (status, _, _) = send(&h, form("csrf=correct&action=on")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (_, _, body) = send(&h, get("/vms/qrab/a", Some(&admin))).await;
    assert!(body.contains("Power on sent to a."), "{body}");
    for action in [
        serde_json::json!({"action": "on"}),
        serde_json::json!({"action": "Nmi", "confirm": true}),
    ] {
        let (status, _, text) = send(
            &h,
            post_json(
                "/vms/qrab/a/power",
                Some(&admin),
                Some(ORIGIN),
                Some("correct"),
                action.clone(),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{action}: {text}");
    }
    assert_eq!(
        resets_sent(&h),
        ["ForceRestart", "On", "On", "Nmi"].map(|reset| format!(r#"{{"ResetType":"{reset}"}}"#))
    );
    assert!(h.cluster.running.lock().unwrap().is_empty());
    // Power off is the operator's stop, as with the BMC.
    let (status, _, _) = send(&h, form("csrf=correct&action=off")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = send(&h, form("csrf=correct&action=off&confirm=yes")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        *h.cluster.running.lock().unwrap(),
        [("a".into(), "uid-a".into(), false)]
    );
    assert!(h.bmc_b.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_console_stays_bmc_only_and_a_vm_without_redfish_powers_through_its_spec() {
    let h = harness().await;
    let admin = session(&h.state.sessions, ALICE, &[]).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    serve(listener, router(h.state.clone())).await;
    let socket_status = async || {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut req = format!("ws://{addr}/vms/qrab/a/console/ws")
            .into_client_request()
            .unwrap();
        req.headers_mut()
            .insert("cookie", HeaderValue::from_str(&admin).unwrap());
        req.headers_mut()
            .insert("origin", HeaderValue::from_static(ORIGIN));
        match tokio_tungstenite::connect_async(req).await {
            Ok(_) => 101,
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                response.status().as_u16()
            }
            Err(error) => panic!("{error}"),
        }
    };
    // With the BMC (and so Redfish), the console is open to the admin.
    assert_eq!(
        send(&h, get("/vms/qrab/a/console", Some(&admin))).await.0,
        StatusCode::OK
    );
    assert_eq!(socket_status().await, 101);
    for components in [
        serde_json::json!({"bmc": {"enabled": false},
            "redfish": {"enabled": true, "adminSubjects": [ALICE]}}),
        serde_json::json!({"bmc": {"enabled": false}}),
    ] {
        set_components(&h, components.clone());
        let (status, _, _) = send(&h, get("/vms/qrab/a/console", Some(&admin))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{components}");
        assert_eq!(socket_status().await, 403, "{components}");
    }
    h.bmc_a.lock().unwrap().clear();

    // Neither component: no Redfish call, Power on/off through spec.running,
    // and a clear refusal for in-pod actions.
    let (status, _, body) = send(&h, get("/vms/qrab/a", Some(&admin))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("BMC off · Redfish off"));
    assert!(body.contains("The Redfish component is disabled for this VM"));
    assert!(body.contains("components.redfish is not enabled for this VM"));
    assert!(body.contains("value=\"on\"") && body.contains("value=\"off\""));
    assert!(!body.contains("value=\"Pause\"") && !body.contains("Open console"));
    let (status, _, text) = send(&h, get("/vms/qrab/a/power", Some(&admin))).await;
    assert_eq!(status, StatusCode::OK);
    let state: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(state["pod"], true);
    assert!(state["power_state"].is_null() && state["redfish_error"].is_null());
    let (status, _, text) = send(
        &h,
        post_json(
            "/vms/qrab/a/power",
            Some(&admin),
            Some(ORIGIN),
            Some("correct"),
            serde_json::json!({"action": "Pause"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        text,
        "Pause needs the VM's Redfish service, and components.redfish is not enabled for this VM. Power on and Power off still work."
    );
    let form = |body: &'static str| post("/vms/qrab/a/action", Some(&admin), Some(ORIGIN), body);
    let (status, _, _) = send(&h, form("csrf=correct&action=Pause")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (_, _, body) = send(&h, get("/vms/qrab/a", Some(&admin))).await;
    assert!(body.contains("Pause needs the VM"), "{body}");
    // Power on with the Pod up still patches spec.running: no Redfish to ask.
    for body in [
        "csrf=correct&action=on",
        "csrf=correct&action=off&confirm=yes",
    ] {
        let (status, _, _) = send(&h, form(body)).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    }
    assert_eq!(
        *h.cluster.running.lock().unwrap(),
        [
            ("a".into(), "uid-a".into(), true),
            ("a".into(), "uid-a".into(), false)
        ]
    );
    assert!(h.bmc_a.lock().unwrap().is_empty() && h.bmc_b.lock().unwrap().is_empty());
}

#[tokio::test]
async fn console_page_and_socket_require_admin_and_exact_origin() {
    use futures::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    let h = harness().await;
    let admin = session(&h.state.sessions, ALICE, &[VIEWERS]).await;
    let viewer = session(&h.state.sessions, BOB, &[VIEWERS]).await;
    let (status, headers, body) = send(&h, get("/vms/qrab/a/console", Some(&admin))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("script-src 'self'")
    );
    assert!(body.contains("data-socket=\"/vms/qrab/a/console/ws\""));
    assert_eq!(
        send(&h, get("/vms/qrab/b/console", Some(&admin))).await.0,
        StatusCode::FORBIDDEN
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    serve(listener, router(h.state.clone())).await;
    let connect = |vm: &str, cookie: Option<&str>, origin: Option<&str>| {
        let mut req = format!("ws://{addr}/vms/qrab/{vm}/console/ws")
            .into_client_request()
            .unwrap();
        if let Some(c) = cookie {
            req.headers_mut()
                .insert("cookie", HeaderValue::from_str(c).unwrap());
        }
        if let Some(o) = origin {
            req.headers_mut()
                .insert("origin", HeaderValue::from_str(o).unwrap());
        }
        tokio_tungstenite::connect_async(req)
    };
    for (vm, cookie, origin, expected) in [
        ("a", None, Some(ORIGIN), 401),
        ("a", Some(admin.as_str()), Some("https://evil.example"), 403),
        ("a", Some(admin.as_str()), None, 403),
        ("b", Some(admin.as_str()), Some(ORIGIN), 403),
        ("b", Some(viewer.as_str()), Some(ORIGIN), 403),
        ("c", Some(admin.as_str()), Some(ORIGIN), 404),
    ] {
        match connect(vm, cookie, origin).await {
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status().as_u16(), expected, "{vm} {origin:?}")
            }
            other => panic!("{vm}: expected refusal, got {:?}", other.map(|_| ())),
        }
    }
    assert!(h.bmc_a.lock().unwrap().is_empty() && h.bmc_b.lock().unwrap().is_empty());
    let (mut socket, _) = connect("a", Some(&admin), Some(ORIGIN)).await.unwrap();
    let hello = socket.next().await.unwrap().unwrap();
    assert_eq!(hello, Message::Binary("RFB 003.008\n".into()));
    socket
        .send(Message::Binary(b"key".to_vec().into()))
        .await
        .unwrap();
    assert_eq!(
        socket.next().await.unwrap().unwrap(),
        Message::Binary(b"echo:key".to_vec().into())
    );
    // Signing out closes the proxied stream within about a second.
    let id: tower_sessions::session::Id = admin.split_once('=').unwrap().1.parse().unwrap();
    tower_sessions::SessionStore::delete(&h.state.sessions, &id)
        .await
        .unwrap();
    let closed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match socket.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert!(closed.is_ok(), "console outlived the session");
}

#[tokio::test]
async fn console_forwards_only_the_admin_bucket_features_the_vm_enables() {
    use futures::StreamExt;
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    let h = harness().await;
    let admin = session(&h.state.sessions, ALICE, &[]).await;
    let viewer = session(&h.state.sessions, BOB, &[VIEWERS]).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    serve(listener, router(h.state.clone())).await;
    let connect = |vm: &str, query: &str, cookie: &str| {
        let mut req = format!("ws://{addr}/vms/qrab/{vm}/console/ws{query}")
            .into_client_request()
            .unwrap();
        req.headers_mut()
            .insert("cookie", HeaderValue::from_str(cookie).unwrap());
        req.headers_mut()
            .insert("origin", HeaderValue::from_static(ORIGIN));
        tokio_tungstenite::connect_async(req)
    };
    let hello = Message::Binary("RFB 003.008\n".into());
    // The speaker toggle is rendered only when the VM enables audio.
    let toggle = async || {
        let (status, _, body) = send(&h, get("/vms/qrab/a/console", Some(&admin))).await;
        assert_eq!(status, StatusCode::OK);
        body.contains("id=\"audio\"")
    };
    assert!(!toggle().await);
    // Without spec.console.adminBucket nothing is forwarded.
    let (mut socket, _) = connect("a", "?features=audio", &admin).await.unwrap();
    assert_eq!(socket.next().await.unwrap().unwrap(), hello);
    drop(socket);
    h.cluster.vms.lock().unwrap()[0]
        .spec
        .console
        .as_mut()
        .unwrap()
        .admin_bucket = Some(qrab_operator::api::AdminBucket {
        audio: true,
        usb_redirect: false,
    });
    assert!(toggle().await);
    let (mut socket, _) = connect("a", "?features=audio,usb-redirect", &admin)
        .await
        .unwrap();
    assert_eq!(
        socket.next().await.unwrap().unwrap(),
        Message::Binary("query:features=audio".into())
    );
    assert_eq!(socket.next().await.unwrap().unwrap(), hello);
    drop(socket);
    let (mut socket, _) = connect("a", "?features=usb-redirect", &admin)
        .await
        .unwrap();
    assert_eq!(socket.next().await.unwrap().unwrap(), hello);
    drop(socket);
    for (vm, query, cookie, expected) in [
        ("a", "?features=microphone", &admin, 400),
        ("b", "?features=audio", &viewer, 403),
    ] {
        match connect(vm, query, cookie).await {
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status().as_u16(), expected, "{vm} {query}")
            }
            other => panic!(
                "{vm} {query}: expected refusal, got {:?}",
                other.map(|_| ())
            ),
        }
    }
}

#[tokio::test]
async fn login_uses_pkce_and_secure_cookies_and_callback_state_is_single_use() {
    let h = harness().await;
    let response = router(h.state.clone())
        .oneshot(get("/auth/login", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response.headers()["location"].to_str().unwrap();
    for part in [
        "code_challenge_method=S256",
        "nonce=",
        "state=",
        "groups_uuid",
        "profile",
        "client_id=qrab-dashboard",
    ] {
        assert!(location.contains(part), "{part}");
    }
    assert!(!location.contains("client-secret-never-sent"));
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.starts_with("__Host-qrab-dashboard="));
    for flag in ["Secure", "HttpOnly", "SameSite=Lax", "Path=/"] {
        assert!(cookie.contains(flag), "{flag}");
    }
    let pair = cookie.split(';').next().unwrap().to_owned();
    let id: tower_sessions::session::Id = pair.split_once('=').unwrap().1.parse().unwrap();
    let record = tower_sessions::SessionStore::load(&h.state.sessions, &id)
        .await
        .unwrap()
        .unwrap();
    let state = record.data["pending"]["state"].as_str().unwrap().to_owned();
    for attempt in ["wrong-state", state.as_str()] {
        let (status, _, _) = send(
            &h,
            get(
                &format!("/auth/callback?code=never-exchanged&state={attempt}"),
                Some(&pair),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let record = tower_sessions::SessionStore::load(&h.state.sessions, &id)
            .await
            .unwrap();
        assert!(
            record.is_none_or(|r| !r.data.contains_key("pending")),
            "pending login is replayable"
        );
    }
    let (status, _, _) = send(&h, get("/auth/callback?code=stolen&state=x", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn expired_sessions_are_signed_out() {
    let h = harness().await;
    let session = Session::new(None, Arc::new(h.state.sessions.clone()), None);
    session
        .insert(
            "user",
            auth::User {
                subject: ALICE.into(),
                name: "alice".into(),
                groups: vec![ADMINS.into()],
                csrf: "correct".into(),
                expires: auth::now(),
                id_token: "id-token-of-alice".into(),
            },
        )
        .await
        .unwrap();
    session.save().await.unwrap();
    let cookie = format!("__Host-qrab-dashboard={}", session.id().unwrap());
    assert_eq!(
        send(&h, get("/vms/qrab/a", Some(&cookie))).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(h.bmc_a.lock().unwrap().is_empty());
}

#[test]
fn public_origins_must_be_https_origins() {
    for bad in [
        "http://vms.example",
        "https://user@vms.example",
        "https://vms.example/path",
        "https://vms.example?x=1",
    ] {
        assert!(origin(bad, true).is_err(), "{bad}");
    }
    assert!(origin("http://localhost:8080", false).is_err());
    assert_eq!(
        origin("https://vms.yapplesauce.com", false).unwrap(),
        "https://vms.yapplesauce.com"
    );
}

#[tokio::test]
async fn detail_draws_a_server_side_svg_sparkline_per_redfish_history_metric() {
    let h = harness().await;
    let cookie = session(&h.state.sessions, ALICE, &[VIEWERS]).await;
    let (status, headers, body) = send(&h, get("/vms/qrab/a", Some(&cookie))).await;
    assert_eq!(status, StatusCode::OK);
    // Still the script-free page CSP: no client script and no inline styles.
    assert_eq!(
        headers["content-security-policy"],
        "default-src 'none'; style-src 'self'; img-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"
    );
    assert!(
        !body.contains("<script") && !body.contains("style="),
        "{body}"
    );
    assert!(body.contains("Metric history (last hour)"));
    let figure = |metric: &str| {
        let start = body
            .find(&format!("data-metric=\"{metric}\""))
            .unwrap_or_else(|| panic!("{metric}: {body}"));
        let end = start + body[start..].find("</figure>").unwrap();
        body[start..end].to_owned()
    };
    // One accessible chart per metric with data, plus decorative line keys;
    // One path per series, with gaps and visible isolated samples.
    let cpu = figure("container-cpu");
    assert_eq!(cpu.matches("role=\"img\"").count(), 1, "{cpu}");
    assert_eq!(cpu.matches("aria-hidden=\"true\"").count(), 2, "{cpu}");
    assert_eq!(cpu.matches("<path").count(), 2, "{cpu}");
    assert!(cpu.contains("viewBox=\"0 0 240 48\""), "{cpu}");
    assert!(
        cpu.contains("d=\"M0.0,24.0L120.0,1.0L240.0,35.5\""),
        "{cpu}"
    );
    assert!(
        cpu.contains("d=\"M0.0,46.9h1M240.0,46.8h-1\""),
        "Missing bmc samples must remain a gap, with both observed samples visible: {cpu}"
    );
    assert!(cpu.contains("role=\"img\""));
    assert!(
        cpu.contains("aria-label=\"Container CPU, last hour, peak 500 mcores; latest bmc 2 mcores, &#60;b&#62;manager&#60;/b&#62; 125 mcores.\""),
        "{cpu}"
    );
    assert!(!body.contains("<b>manager</b>"));
    let memory = figure("container-memory");
    assert_eq!(memory.matches("<path").count(), 1);
    assert!(memory.contains("512 MiB"), "{memory}");
    for metric in ["block-read", "block-write"] {
        let disk = figure(metric);
        assert_eq!(disk.matches("<path").count(), 1, "{disk}");
        assert!(disk.contains("2.0 KiB/s"), "{disk}");
    }
    // An empty answer and a failed one say so instead of drawing a line.
    let rx = figure("nic-rx");
    assert!(
        rx.contains("No data in the last hour.") && !rx.contains("<path"),
        "{rx}"
    );
    let tx = figure("nic-tx");
    assert!(
        tx.contains("<b>Network sent</b>")
            && tx.contains("Unavailable: Redfish answered HTTP 503")
            && !tx.contains("<svg"),
        "{tx}"
    );
}

#[tokio::test]
async fn central_bmc_page_and_bridge_preserve_vm_and_login_boundaries() {
    let h = harness().await;
    h.cluster.vms.lock().unwrap()[0]
        .spec
        .console
        .as_mut()
        .unwrap()
        .viewer_subjects
        .push(BOB.into());
    let admin = session(&h.state.sessions, ALICE, &[]).await;
    let viewer = session(&h.state.sessions, BOB, &[VIEWERS]).await;
    let base = "/vms/qrab/a/bmc";
    assert_eq!(send(&h, get(base, None)).await.0, StatusCode::UNAUTHORIZED);
    for cookie in [&admin, &viewer] {
        let (status, headers, html) = send(&h, get(base, Some(cookie))).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            headers["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("script-src 'self'")
        );
        assert!(html.contains("/assets/bmc.js") && html.contains("panel-agent"));
        assert!(!html.contains("id-token-of-alice"));
    }
    let (status, _, identity) = send(&h, get(&format!("{base}/session"), Some(&viewer))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(identity.contains("\"role\":\"viewer\"") && !identity.contains("id-token"));
    let resource = format!("{base}/redfish/v1/Systems/1");
    let (status, _, body) = send(&h, get(&resource, Some(&admin))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("bmc-a"));
    assert!(h.bmc_b.lock().unwrap().is_empty());
    assert_eq!(
        send(&h, get("/vms/qrab/missing/bmc", Some(&admin))).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(&h, get("/vms/other/a/bmc", Some(&admin))).await.0,
        StatusCode::NOT_FOUND
    );
    let action = format!("{resource}/Actions/ComputerSystem.Reset");
    for (cookie, origin, csrf, expected) in [
        (&viewer, ORIGIN, "correct", StatusCode::FORBIDDEN),
        (
            &admin,
            "https://other.example",
            "correct",
            StatusCode::FORBIDDEN,
        ),
        (&admin, ORIGIN, "wrong", StatusCode::FORBIDDEN),
        (&admin, ORIGIN, "correct", StatusCode::NO_CONTENT),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri(&action)
            .header("cookie", cookie)
            .header("origin", origin)
            .header("x-csrf-token", csrf)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"ResetType":"Pause"}"#))
            .unwrap();
        assert_eq!(send(&h, request).await.0, expected);
    }
    h.cluster.vms.lock().unwrap()[0]
        .status
        .as_mut()
        .unwrap()
        .pod_name = Some("b".into());
    let (status, headers, body) = send(&h, get(&resource, Some(&admin))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["error"]["message"], "The VM has no running Pod.");
    assert!(!body.contains("id-token-of-alice"));
    let (status, headers, body) = send(&h, get(&format!("{base}/session"), None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["error"]["message"].is_string()
    );
}

#[tokio::test]
async fn bmc_assets_scope_all_browser_connections_to_the_dashboard_vm() {
    let h = harness().await;
    let (status, _, js) = send(&h, get("/assets/bmc.js", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(js.contains("`${vmBase}/bmc${target.pathname}${target.search}`"));
    assert!(js.contains("`${vmBase}/bmc/session`"));
    assert!(js.contains("`${vmBase}/bmc/logout`"));
    assert!(js.contains("`${vmBase}/console/ws`") && js.contains("${location.host}${consolePath}"));
    assert!(!js.contains("${location.host}/api/console"));
    let (status, headers, keyboard) = send(&h, get("/assets/console-keyboard.js", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/javascript")
    );
    assert!(keyboard.contains("wireConsoleKeyboard"));
}

#[tokio::test]
async fn bmc_live_events_close_on_logout_and_release_the_stream_limit() {
    let h = harness().await;
    let cookie = session(&h.state.sessions, ALICE, &[]).await;
    let response = router(h.state.clone())
        .oneshot(get(
            "/vms/qrab/a/bmc/redfish/v1/EventService/SSE",
            Some(&cookie),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    assert_eq!(h.state.consoles.available_permits(), 15);
    let logout = Request::builder()
        .method("POST")
        .uri("/vms/qrab/a/bmc/logout")
        .header("cookie", &cookie)
        .header("origin", ORIGIN)
        .header("x-csrf-token", "correct")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&h, logout).await.0, StatusCode::NO_CONTENT);
    let body = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        http_body_util::BodyExt::collect(response.into_body()),
    )
    .await
    .unwrap()
    .unwrap()
    .to_bytes();
    assert!(body.is_empty());
    assert_eq!(h.state.consoles.available_permits(), 16);
    assert_eq!(
        send(&h, get("/vms/qrab/a/bmc/session", Some(&cookie)))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn rejected_optional_launch_values_are_validated_without_creating_resources() {
    let h = harness().await;
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (status, _, body) = send(
        &h,
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&name=web&cpus=2&memory_mib=512&max_cpus=1&disk0_id=logs&disk0_size_gib=0&user_data=PRIVATE_FORM_VALUE",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("Maximum vCPUs must be at least vCPUs."));
    assert!(body.contains("Disk 1 size (GiB) must be a whole number from 1 to 1024."));
    assert!(body.contains("name=\"max_cpus\"") && body.contains("value=\"1\""));
    assert!(body.contains("name=\"disk0_size_gib\"") && body.contains("value=\"0\""));
    for field in ["max_cpus", "disk0_size_gib"] {
        assert!(body.contains(&format!("href=\"#{field}\"")));
        assert!(body.contains(&format!("id=\"error-{field}\"")));
    }
    assert!(body.contains("aria-invalid=\"true\""));
    assert!(body.contains("autofocus"));
    assert!(!body.contains("PRIVATE_FORM_VALUE"));
    assert!(body.contains("User-data is cleared for privacy"));
    assert_eq!(h.cluster.mutations(), 0);
    assert!(h.kanidm.lock().unwrap().is_empty());
}

#[tokio::test]
async fn recovery_pages_preserve_status_and_safe_vm_context_while_power_errors_stay_plain() {
    let h = harness().await;
    for path in ["/vms/qrab/a", "/vms/qrab/a/bmc", "/vms/qrab/a/console"] {
        let (status, headers, body) = send(&h, get(path, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(
            headers["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );
        assert!(body.contains("Sign in to continue") && body.contains("Sign in with Kanidm"));
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("next", path)
            .finish();
        assert!(
            body.contains(&format!("href=\"/auth/login?{query}\"")),
            "{body}"
        );
        assert!(!body.contains("id-token"));
    }
    let cookie = session(&h.state.sessions, ALICE, &[]).await;
    let (status, _, missing) = send(&h, get("/vms/qrab/missing", Some(&cookie))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(missing.contains("VM not found"));
    let (status, _, invisible) = send(&h, get("/vms/qrab/c", Some(&cookie))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        missing, invisible,
        "Existence must not change the recovery page"
    );
    let (status, _, body) = send(&h, get("/not-a-page", None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("Page not found") && body.contains("All VMs"));
    let (status, _, body) = send(
        &h,
        post("/vms/qrab/a/action", Some(&cookie), None, "action=Pause"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("Access isn&#39;t available") && body.contains("href=\"/vms/qrab/a\""));
    let (status, headers, body) = send(&h, get("/vms/qrab/a/power", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        headers["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/plain")
    );
    assert_eq!(body, "Sign in again to continue.");
    assert_eq!(h.cluster.mutations(), 0);
}

#[tokio::test]
async fn vpn_network_summary_shows_routing_mode_without_secret_references() {
    let h = harness().await;
    h.cluster.vms.lock().unwrap()[0].spec.nics = serde_json::from_value(serde_json::json!([
        {"name":"eth0","vpn":{"secretName":"vpn-private-reference"}}
    ]))
    .unwrap();
    let cookie = session(&h.state.sessions, ALICE, &[]).await;
    let (status, _, body) = send(&h, get("/vms/qrab/a", Some(&cookie))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("VPN tunnel only") && body.contains("direct guest traffic is blocked"));
    assert!(!body.contains("vpn-private-reference") && !body.contains("default-allow"));
}

fn vpn_profile(name: &str, uid: &str, status: serde_json::Value) -> qrab_proton::crd::VpnProfile {
    serde_json::from_value(serde_json::json!({
        "apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "VpnProfile",
        "metadata": {"name": name, "namespace": "qrab", "uid": uid},
        "spec": {"proton": {"portForwarding": true, "netShield": 2}},
        "status": status,
    }))
    .unwrap()
}

fn vpn_catalog() -> Vec<u8> {
    use qrab_proton::catalog::{CompactCatalog, CompactServer};
    let server = |name: &str, country: &str, features: u32, status: u8, load: u8, score: f64| {
        CompactServer {
            name: name.into(),
            entry_country: if features & 1 != 0 {
                "IS".into()
            } else {
                country.into()
            },
            exit_country: country.into(),
            city: Some("Zurich".into()),
            tier: 2,
            features,
            status,
            load,
            score,
        }
    };
    qrab_proton::catalog::encode(&CompactCatalog {
        updated: 1_794_849_231,
        max_tier: 2,
        servers: vec![
            server("CH#1", "CH", 4, 1, 40, 2.0),
            server("CH#2", "CH", 0, 1, 10, 1.0),
            server("CH#3", "CH", 0, 0, 5, 0.1),
            server("IS-CH#1", "CH", 1, 1, 5, 0.5),
            server("SE#7", "SE", 4, 1, 80, 1.5),
        ],
    })
    .unwrap()
}

#[tokio::test]
async fn the_vpn_page_is_for_launchers_and_shows_profiles_and_servers() {
    let h = harness().await;
    *h.cluster.vpn_profiles.lock().unwrap() = vec![vpn_profile(
        "ch-p2p",
        "uid-ch",
        serde_json::json!({
            "server": "CH#1", "exitCountry": "CH", "city": "Zurich", "load": 12, "entryIp": "203.0.113.1",
            "forwardedPort": 41234, "publicKey": "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=",
            "attached": [{"vm": "web", "nic": "vpn", "anchor": "web-nic-vpn-kube01"}],
            "certificateExpires": 1_794_849_231,
            "conditions": [{"type": "Ready", "status": "True", "reason": "Configured", "message": "CH#1 (CH)"}],
        }),
    )];
    *h.cluster.vpn_catalog.lock().unwrap() = Some(vpn_catalog());
    // Not a launcher: the page and its nav link do not exist.
    let viewer = session(&h.state.sessions, BOB, &[VM_USERS]).await;
    let (status, _, _) = send(&h, get("/vpn", Some(&viewer))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, _, home) = send(&h, get("/images", Some(&viewer))).await;
    assert!(!home.contains("href=\"/vpn\""));
    let (status, _, _) = send(&h, get("/vpn", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (status, _, page) = send(&h, get("/vpn", Some(&cookie))).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains("href=\"/vpn\""), "launchers get the nav link");
    for text in [
        "ch-p2p",
        "CH#1",
        "41234",
        "web/vpn",
        "Ready",
        "2026-11-16 17:13 UTC",
    ] {
        assert!(page.contains(text), "{text}");
    }
    // The WireGuard public key is status, but not for the page.
    assert!(!page.contains("xTIBA5rbo"));
    // Servers: best score first; maintenance and Secure Core are hidden.
    let servers = &page[page.find("id=\"servers-title\"").unwrap()..];
    let at = |name: &str| servers.find(&format!("<code>{name}</code>")).unwrap();
    assert!(at("CH#2") < at("SE#7") && at("SE#7") < at("CH#1"));
    assert!(!servers.contains("<code>CH#3</code>") && !servers.contains("<code>IS-CH#1</code>"));
    // Filters: country, feature (Secure Core only when asked), load.
    let (_, _, page) = send(&h, get("/vpn?country=se", Some(&cookie))).await;
    assert!(page.contains("<code>SE#7</code>") && !page.contains("<code>CH#2</code>"));
    let (_, _, page) = send(&h, get("/vpn?feature=p2p&max_load=50", Some(&cookie))).await;
    assert!(
        page.contains("<code>CH#1</code>")
            && !page.contains("<code>SE#7</code>")
            && !page.contains("<code>CH#2</code>")
    );
    let (_, _, page) = send(&h, get("/vpn?feature=secureCore", Some(&cookie))).await;
    assert!(page.contains("<code>IS-CH#1</code>") && page.contains("via IS"));
    // No catalog yet: the page says what is missing.
    *h.cluster.vpn_catalog.lock().unwrap() = None;
    let (_, _, page) = send(&h, get("/vpn", Some(&cookie))).await;
    assert!(page.contains("qrab-proton login"));
}

#[tokio::test]
async fn vpn_profile_changes_are_checked_and_written_by_uid() {
    let h = harness().await;
    *h.cluster.vpn_profiles.lock().unwrap() = vec![
        vpn_profile("ch-p2p", "uid-ch", serde_json::json!({})),
        vpn_profile(
            "used",
            "uid-used",
            serde_json::json!({"attached": [{"vm": "web", "nic": "vpn", "anchor": "a"}]}),
        ),
    ];
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let post_ok = |path: &str, body: &str| post(path, Some(&cookie), Some(ORIGIN), body);
    // Every VPN form needs CSRF and the exact Origin, and a launcher.
    for (path, body) in [
        ("/vpn", "name=x"),
        ("/vpn/use", "profile=ch-p2p&server=SE%237"),
        ("/vpn/ch-p2p", "action=server&server=auto"),
        ("/vpn/ch-p2p/delete", "confirm_name=ch-p2p"),
    ] {
        let (status, _, _) = send(
            &h,
            post(
                path,
                Some(&cookie),
                Some(ORIGIN),
                &format!("{body}&csrf=wrong"),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        let (status, _, _) = send(
            &h,
            post(
                path,
                Some(&cookie),
                Some("https://evil.example"),
                &format!("{body}&csrf=correct"),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
    let viewer = session(&h.state.sessions, BOB, &[VM_USERS]).await;
    let (status, _, _) = send(
        &h,
        post(
            "/vpn/ch-p2p",
            Some(&viewer),
            Some(ORIGIN),
            "csrf=correct&action=server&server=auto",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Invalid input writes nothing.
    for (path, body) in [
        ("/vpn", "name=Bad_Name"),
        ("/vpn", "name=ok&country=Switzerland"),
        ("/vpn", "name=ok&net_shield=7"),
        ("/vpn", "name=ok&feature=warp"),
        ("/vpn/ch-p2p", "action=server&server=ch12; rm"),
        ("/vpn/ch-p2p/delete", "confirm_name=wrong"),
        ("/vpn/used/delete", "confirm_name=used"),
        ("/vpn/missing", "action=server&server=auto"),
    ] {
        let (status, _, body_out) = send(&h, post_ok(path, &format!("{body}&csrf=correct"))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path} {body}: {body_out}");
    }
    assert_eq!(h.cluster.mutations(), 0);

    // Create, choose a server, back to auto, change settings, delete.
    let ok = |status: StatusCode| assert_eq!(status, StatusCode::SEE_OTHER);
    let (status, _, _) = send(&h, post_ok("/vpn", "csrf=correct&name=se-games&country=se&feature=p2p&net_shield=2&port_forwarding=on&moderate_nat=on")).await;
    ok(status);
    let (status, _, _) = send(
        &h,
        post_ok("/vpn/use", "csrf=correct&profile=ch-p2p&server=SE%237"),
    )
    .await;
    ok(status);
    let (status, _, _) = send(
        &h,
        post_ok("/vpn/ch-p2p", "csrf=correct&action=server&server=auto"),
    )
    .await;
    ok(status);
    let (status, _, _) = send(
        &h,
        post_ok(
            "/vpn/ch-p2p",
            "csrf=correct&action=settings&city=Zurich&net_shield=0&vpn_accelerator=on",
        ),
    )
    .await;
    ok(status);
    let (status, _, _) = send(
        &h,
        post_ok("/vpn/ch-p2p/delete", "csrf=correct&confirm_name=ch-p2p"),
    )
    .await;
    ok(status);
    let writes = h.cluster.vpn_writes.lock().unwrap().clone();
    assert_eq!(writes.len(), 5);
    let (kind, name, created) = &writes[0];
    assert_eq!((kind.as_str(), name.as_str()), ("create", "se-games"));
    assert_eq!(created["metadata"]["namespace"], "qrab");
    assert_eq!(
        created["spec"]["proton"],
        serde_json::json!({"country": "SE", "features": ["p2p"], "netShield": 2, "portForwarding": true, "moderateNat": true, "vpnAccelerator": false})
    );
    assert_eq!(writes[1].2["spec"]["proton"]["server"], "SE#7");
    assert_eq!(writes[1].2["metadata"]["uid"], "uid-ch");
    assert!(
        writes[2].2["spec"]["proton"]["server"].is_null(),
        "auto clears the server"
    );
    let settings = &writes[3].2["spec"]["proton"];
    assert_eq!(
        (
            settings["city"].as_str(),
            settings["netShield"].as_u64(),
            settings["portForwarding"].as_bool()
        ),
        (Some("Zurich"), Some(0), Some(false))
    );
    assert!(settings["country"].is_null());
    assert_eq!(
        writes[4],
        (
            "delete".into(),
            "ch-p2p".into(),
            serde_json::json!({"uid": "uid-ch"})
        )
    );
}

#[tokio::test]
async fn a_launch_attaches_a_vpn_profile_and_the_vm_page_shows_its_port() {
    let h = harness().await;
    catalog_with_cirros(&h);
    *h.cluster.vpn_profiles.lock().unwrap() = vec![
        vpn_profile(
            "ch-p2p",
            "uid-ch",
            serde_json::json!({
                "server": "CH#496", "exitCountry": "CH", "city": "Zurich", "forwardedPort": 42077,
                "conditions": [{"type": "Ready", "status": "True", "reason": "Configured", "message": "CH#496 (CH)"}],
            }),
        ),
        vpn_profile("taken", "uid-taken", serde_json::json!({})),
    ];
    let mut other: VirtualMachine = serde_json::from_value(serde_json::json!({
        "apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "VirtualMachine",
        "metadata": {"name": "seedbox", "namespace": "qrab", "uid": "uid-seedbox"},
        "spec": {"stateVolumeClaim": "seedbox-state",
            "nics": [{"name": "eth0", "vpn": {"secretName": "vpn-taken"}}]},
    }))
    .unwrap();
    other.metadata.annotations = None;
    h.cluster.vms.lock().unwrap().push(other);
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (_, _, body) = send(&h, get("/launch", Some(&cookie))).await;
    assert!(body.contains("name=\"nic0_network\""), "{body}");
    assert!(body.contains("value=\"vpn:ch-p2p\"") && body.contains("CH#496 · CH · Zurich"));
    assert!(body.contains("(in use by seedbox/eth0)"));
    assert!(body.contains("name=\"nic1_enabled\""));
    let form = |network: &str| {
        let mut s = url::form_urlencoded::Serializer::new(String::new());
        s.extend_pairs([
            ("csrf", "correct"),
            ("name", "web"),
            ("cpus", "1"),
            ("memory_mib", "512"),
            ("os", "cirros:1"),
            ("nic0_enabled", "on"),
            ("nic0_network", network),
        ]);
        s.finish()
    };
    // A profile another VM uses is refused before anything is created.
    let (status, _, body) = send(
        &h,
        post("/launch", Some(&cookie), Some(ORIGIN), &form("vpn:taken")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("already used by seedbox/eth0"), "{body}");
    assert_eq!(h.cluster.mutations(), 0);
    let (status, _, _) = send(
        &h,
        post("/launch", Some(&cookie), Some(ORIGIN), &form("vpn:ch-p2p")),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let created = h.cluster.created_vms.lock().unwrap()[0].clone();
    assert_eq!(
        created["spec"]["nics"],
        serde_json::json!([{"name": "eth0", "securityGroups": ["vpn-egress"], "vpn": {"secretName": "vpn-ch-p2p"}}])
    );
    let mut vm: VirtualMachine = serde_json::from_value(created).unwrap();
    vm.metadata.uid = Some("uid-web".into());
    h.cluster.vms.lock().unwrap().push(vm);
    let (_, _, detail) = send(&h, get("/vms/qrab/web", Some(&cookie))).await;
    for cell in [
        "VPN tunnel only",
        "href=\"/vpn\"",
        "ch-p2p",
        "CH#496 · CH · Zurich",
        "Forwarded port 42077",
    ] {
        assert!(detail.contains(cell), "{cell}: {detail}");
    }
    assert!(!detail.contains("vpn-ch-p2p"));
}

fn terminating() -> Option<k8s_openapi::apimachinery::pkg::apis::meta::v1::Time> {
    Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(
        Default::default(),
    ))
}

#[tokio::test]
async fn deleting_a_launched_vm_deletes_only_its_own_access_groups() {
    use crate::cleanup::{FINALIZER, reconcile};
    let h = harness().await;
    h.cluster.vms.lock().unwrap().clear();
    let cookie = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (status, _, _) = send(
        &h,
        post(
            "/launch",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&name=web&cpus=1&memory_mib=512",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let created = h.cluster.created_vms.lock().unwrap()[0].clone();
    assert_eq!(
        created["metadata"]["finalizers"],
        serde_json::json!([FINALIZER])
    );
    let (admins_name, admins) = h.dir.lock().unwrap().launched("admins");
    let (_, viewers) = h.dir.lock().unwrap().launched("viewers");
    // Entries that are not this VM's own groups: a platform group whose
    // description imitates one, another VM's launch-ID group, and junk.
    const OPERATORS: &str = "11111111-1111-4111-8111-111111111111";
    const OTHER: &str = "22222222-2222-4222-8222-222222222222";
    const OTHER_NAME: &str = "qrab_vm_ffffffffffffffffffffffffffffffff_admins";
    {
        let mut dir = h.dir.lock().unwrap();
        for (name, uuid, description) in [
            (
                "qrab_operators",
                OPERATORS,
                "qrab VM qrab/web admins, launched by mallory",
            ),
            (
                OTHER_NAME,
                OTHER,
                "qrab VM qrab/db admins, launched by bob through qrab-dashboard",
            ),
        ] {
            dir.groups.insert(name.into(), uuid.into());
            dir.descriptions.insert(name.into(), description.into());
        }
    }
    let mut vm: VirtualMachine = serde_json::from_value(created).unwrap();
    vm.metadata.uid = Some("uid-web".into());
    vm.metadata.resource_version = Some("7".into());
    let console = vm.spec.console.as_mut().unwrap();
    console
        .admin_groups
        .extend([OPERATORS.into(), OTHER.into(), "not a group".into()]);
    // A live VM keeps its groups and its finalizer.
    h.cluster.vms.lock().unwrap().push(vm);
    reconcile(h.cluster.as_ref(), &h.state.kanidm, "qrab")
        .await
        .unwrap();
    assert!(h.cluster.finalizer_writes.lock().unwrap().is_empty());
    assert!(h.dir.lock().unwrap().groups.contains_key(&admins_name));
    // `kubectl delete`: one pass deletes its two groups, then drops the
    // finalizer, and the API server removes the VM.
    h.cluster.vms.lock().unwrap()[0].metadata.deletion_timestamp = terminating();
    reconcile(h.cluster.as_ref(), &h.state.kanidm, "qrab")
        .await
        .unwrap();
    {
        let dir = h.dir.lock().unwrap();
        assert!(!dir.groups.values().any(|g| *g == admins || *g == viewers));
        assert!(dir.groups.contains_key("qrab_operators"));
        assert!(dir.groups.contains_key(OTHER_NAME));
    }
    assert!(h.cluster.vms.lock().unwrap().is_empty());
    assert_eq!(
        *h.cluster.finalizer_writes.lock().unwrap(),
        [("web".to_string(), Vec::<String>::new())]
    );
}

#[tokio::test]
async fn older_dashboard_vms_get_the_finalizer_and_kanidm_failures_keep_it() {
    use crate::cleanup::{FINALIZER, reconcile};
    const ISCSI: &str = "qrab.yapplesauce.com/iscsi-state";
    const LEGACY: &str = "33333333-3333-4333-8333-333333333333";
    let h = harness().await;
    h.cluster.vms.lock().unwrap().clear();
    // Launched before the finalizer existed, with a pre-launch-ID group name.
    let mut legacy = vm(
        "old",
        serde_json::json!({"console": {"adminGroups": [LEGACY]}}),
        Some(ALICE),
    );
    legacy.metadata.resource_version = Some("1".into());
    legacy.metadata.finalizers = Some(vec![ISCSI.into()]);
    legacy.metadata.labels = Some(
        [(
            crate::access::MANAGED_BY.to_string(),
            crate::access::MANAGER.to_string(),
        )]
        .into(),
    );
    // A hand-made VM is left alone.
    let mut manual = vm("manual", serde_json::json!({}), None);
    manual.metadata.resource_version = Some("1".into());
    h.cluster.vms.lock().unwrap().extend([legacy, manual]);
    {
        let mut dir = h.dir.lock().unwrap();
        dir.groups
            .insert("qrab_vm_qrab_old_admins".into(), LEGACY.into());
        dir.descriptions.insert(
            "qrab_vm_qrab_old_admins".into(),
            "qrab VM qrab/old admins, launched by alice through qrab-dashboard".into(),
        );
    }
    reconcile(h.cluster.as_ref(), &h.state.kanidm, "qrab")
        .await
        .unwrap();
    assert_eq!(
        *h.cluster.finalizer_writes.lock().unwrap(),
        [(
            "old".to_string(),
            vec![ISCSI.to_string(), FINALIZER.to_string()]
        )]
    );
    // While Kanidm is unreachable the terminating VM keeps the finalizer.
    h.cluster.vms.lock().unwrap()[0].metadata.deletion_timestamp = terminating();
    let unreachable = kanidm::Kanidm::new("http://127.0.0.1:1", "t".into(), None, true).unwrap();
    reconcile(h.cluster.as_ref(), &unreachable, "qrab")
        .await
        .unwrap();
    assert_eq!(h.cluster.finalizer_writes.lock().unwrap().len(), 1);
    reconcile(h.cluster.as_ref(), &h.state.kanidm, "qrab")
        .await
        .unwrap();
    assert!(
        !h.dir
            .lock()
            .unwrap()
            .groups
            .contains_key("qrab_vm_qrab_old_admins")
    );
    // Only the dashboard's own finalizer was removed; the operator's remains.
    let vms = h.cluster.vms.lock().unwrap();
    let old = vms
        .iter()
        .find(|v| v.metadata.name.as_deref() == Some("old"))
        .unwrap();
    assert_eq!(
        old.metadata.finalizers.as_deref(),
        Some(&[ISCSI.to_string()][..])
    );
    assert_eq!(vms.len(), 2);
}
