//! HTTP and WebSocket tests against an in-memory cluster, a fake Kanidm and
//! fake BMCs on loopback addresses.
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
const KANIDM_TOKEN: &str = "service-account-token";

#[derive(Default)]
struct FakeCluster {
    vms: Mutex<Vec<VirtualMachine>>,
    pods: Mutex<HashMap<String, Pod>>,
    pvcs: Mutex<HashMap<String, PersistentVolumeClaim>>,
    created_vms: Mutex<Vec<serde_json::Value>>,
    deleted: Mutex<Vec<(String, String)>>,
    fail_vm_create: std::sync::atomic::AtomicBool,
    /// Appears after the first read misses it (a race with the preflight).
    racing_pvc: Mutex<Option<PersistentVolumeClaim>>,
    /// ConfigMap qrab-base-images, key images.json.
    catalog: Mutex<Option<String>>,
    fail_catalog: std::sync::atomic::AtomicBool,
}

impl FakeCluster {
    fn mutations(&self) -> usize {
        self.created_vms.lock().unwrap().len()
            + self.deleted.lock().unwrap().len()
            + self.pvcs.lock().unwrap().len()
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

/// A fake BMC whose status names `label`. Its guest strings carry markup.
fn fake_bmc(label: &'static str, log: Log) -> axum::Router {
    let handler = move |req: Request| {
        let log = log.clone();
        async move {
            let (parts, body) = req.into_parts();
            let bytes = axum::body::to_bytes(body, 4096).await.unwrap();
            let req = Request::from_parts(parts.clone(), Body::empty());
            record(&log, &req, &String::from_utf8_lossy(&bytes));
            if parts
                .headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                != Some("Bearer id-token-of-alice")
            {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            match parts.uri.path() {
                "/api/status" => Json(serde_json::json!({"phase": "running", "ready": true,
                    "run_state": label, "qemu_version": [11, 1, 3], "observed_unix_ms": 1}))
                .into_response(),
                "/api/agent" => Json(
                    serde_json::json!({"status": {"enabled": true, "connected": true,
                    "version": "<script>alert(1)</script>"}, "freeze": "thawed"}),
                )
                .into_response(),
                "/api/events" => Json(
                    serde_json::json!({"events": [{"seq": 1, "received_unix_ms": 1,
                    "event": "RESET", "data": {"reason": "<img src=x onerror=alert(2)>"}}],
                    "next": 1, "latest": 1, "missed": 0, "rejected": 0, "qmp_dropped": 0}),
                )
                .into_response(),
                "/api/config" => Json(
                    serde_json::json!({"name": "vm", "memory_mib": 512, "cpus": 1,
                    "accelerator": "tcg", "boot": "uefi", "console": true, "tpm": true,
                    "guest_network": false, "disks": [], "startup_timeout_secs": 60,
                    "shutdown_grace_secs": 30, "redacted": false}),
                )
                .into_response(),
                "/api/action" => StatusCode::NO_CONTENT.into_response(),
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }
    };
    let console = |headers: HeaderMap, ws: WebSocketUpgrade| async move {
        if headers.get("authorization").and_then(|v| v.to_str().ok())
            != Some("Bearer id-token-of-alice")
            || headers.contains_key("origin")
            || headers.contains_key("cookie")
        {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        ws.on_upgrade(|mut socket| async move {
            use axum::extract::ws::Message;
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
    members: HashMap<String, Vec<String>>,
    /// Lookups miss even existing groups.
    blind: bool,
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
                (Method::GET, ["v1", "group", id]) => match dir.groups.get(*id) {
                    Some(_) if dir.blind => StatusCode::NOT_FOUND.into_response(),
                    Some(uuid) => {
                        Json(serde_json::json!({"attrs": {"name": [id], "uuid": [uuid]}}))
                            .into_response()
                    }
                    None => StatusCode::NOT_FOUND.into_response(),
                },
                (Method::POST, ["v1", "group"]) => {
                    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    let name = v["attrs"]["name"][0].as_str().unwrap().to_owned();
                    let id = uuid::Uuid::new_v4().to_string();
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
    // Two BMCs on the same port at different loopback addresses.
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
        storage_node: Some("kube03".into()),
        cluster: cluster.clone(),
        kanidm: kanidm::Kanidm::new(&kanidm_url, KANIDM_TOKEN.into(), None, true).unwrap(),
        bmc: bmc::Bmc::new(port).unwrap(),
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
                id_token: "id-token-of-alice".into(),
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
    assert!(body.contains("250m") && body.contains("300 MiB"));
    assert!(!body.contains("href=\"/launch\""));
    assert!(!body.contains("id-token-of-alice"));
    let launcher = session(&h.state.sessions, ALICE, &[LAUNCHERS]).await;
    let (_, _, body) = send(&h, get("/", Some(&launcher))).await;
    assert!(body.contains("href=\"/launch\""));
    // No BMC is contacted for the list.
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
async fn detail_calls_only_the_selected_vms_bmc_with_the_bearer_and_escapes_guest_text() {
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
    let seen = h.bmc_a.lock().unwrap().clone();
    let mut paths: Vec<_> = seen.iter().map(|(m, p, _, _)| format!("{m} {p}")).collect();
    paths.sort();
    assert_eq!(
        paths,
        [
            "GET /api/agent",
            "GET /api/config",
            "GET /api/events?limit=20",
            "GET /api/status"
        ]
    );
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
    // A viewer learns why, and can refresh a sign-in that predates a new grant.
    assert!(
        body.contains("Console: admins only")
            && body.contains("href=\"/auth/login?next=/vms/qrab/b\"")
    );
    assert_eq!(h.bmc_b.lock().unwrap().len(), 4);
    assert_eq!(h.bmc_a.lock().unwrap().len(), 4);
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
        ("/vms/qrab/a/action", "action=pause".to_string()),
        ("/vms/qrab/a/delete", "confirm_name=a".to_string()),
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
async fn power_actions_are_admin_only_confirmed_and_sent_with_the_bearer() {
    let h = harness().await;
    let cookie = session(&h.state.sessions, ALICE, &[VIEWERS]).await;
    let (status, _, _) = send(
        &h,
        post(
            "/vms/qrab/b/action",
            Some(&cookie),
            Some(ORIGIN),
            "csrf=correct&action=pause",
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
            "csrf=correct&action=pause",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    for bad in ["action=qmp", "action=reset", "action=powerdown&confirm=no"] {
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
            "csrf=correct&action=reset&confirm=yes",
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
            "/api/action".into(),
            "Bearer id-token-of-alice".into(),
            "\"reset\"".into()
        )]
    );
    let (_, _, body) = send(&h, get("/vms/qrab/a", Some(&cookie))).await;
    assert!(body.contains("Reset sent to a."));
}

#[tokio::test]
async fn delete_is_admin_only_confirmed_and_keeps_the_pvc() {
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
    assert!(body.contains("a-state was kept"));
}

fn catalog_with_cirros(h: &Harness) {
    let broken = serde_json::json!({"name": "broken", "generation": 1});
    *h.cluster.catalog.lock().unwrap() =
        Some(serde_json::json!([crate::images::tests::cirros_json(), broken]).to_string());
}

#[tokio::test]
async fn the_images_page_lists_the_catalog_for_any_signed_in_user() {
    let h = harness().await;
    assert_eq!(
        send(&h, get("/images", None)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let viewer = session(&h.state.sessions, BOB, &[]).await;
    let (status, _, body) = send(&h, get("/images", Some(&viewer))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("No base images are staged yet."));
    assert!(body.contains("href=\"/images\""));
    catalog_with_cirros(&h);
    let (status, _, body) = send(&h, get("/images", Some(&viewer))).await;
    assert_eq!(status, StatusCode::OK);
    for text in [
        "CirrOS 0.6.3",
        "<code>cirros g1</code>",
        "112 MiB",
        "<code>0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef</code>",
        "https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img",
        "data-label=\"Raw SHA-256\"",
        "How images get here",
        "contrib/images/stage-base-image.sh",
        "1 staged",
    ] {
        assert!(body.contains(text), "{text}");
    }
    assert!(!body.contains("broken"));
    // A catalog the dashboard cannot read is an upstream failure, not an empty list.
    h.cluster
        .fail_catalog
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        send(&h, get("/images", Some(&viewer))).await.0,
        StatusCode::BAD_GATEWAY
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
    let admins = h.dir.lock().unwrap().groups["qrab_vm_qrab_web_admins"].clone();
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
        let admins = dir.groups["qrab_vm_qrab_web_admins"].clone();
        let viewers = dir.groups["qrab_vm_qrab_web_viewers"].clone();
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
    assert_eq!(
        calls[..3],
        [
            "GET /v1/group/qrab_vm_qrab_web_admins",
            "GET /v1/group/qrab_vm_qrab_web_viewers",
            "POST /v1/group"
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
    // The form's Redfish field is ignored: nothing admits 8081 for these VMs.
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
    assert!(body.contains("Kanidm group qrab_vm_qrab_web_admins deleted"));
    assert!(body.contains("PersistentVolumeClaim qrab/web-state: kept"));
    assert!(h.dir.lock().unwrap().groups.is_empty());
    // An existing group is never adopted.
    h.cluster
        .fail_vm_create
        .store(false, std::sync::atomic::Ordering::SeqCst);
    h.dir
        .lock()
        .unwrap()
        .groups
        .insert("qrab_vm_qrab_db_viewers".into(), BOB.into());
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
    assert!(body.contains("Read Kanidm group qrab_vm_qrab_web_admins"));
    assert!(body.contains("Kanidm group qrab_vm_qrab_web_admins deleted"));
    assert!(h.dir.lock().unwrap().groups.is_empty());
    let log = h.kanidm.lock().unwrap().clone();
    let lookups = log
        .iter()
        .filter(|(m, p, _, _)| m == "GET" && p == "/v1/group/qrab_vm_qrab_web_admins")
        .count();
    assert_eq!(lookups, 3, "preflight plus one retry after creation");
    assert!(
        log.iter()
            .any(|(m, p, _, _)| m == "DELETE" && p == "/v1/group/qrab_vm_qrab_web_admins")
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
    assert!(list.contains("data-label=\"IP\"") && list.contains("10.244.1.9"));
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
async fn oversized_bmc_responses_are_refused() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    serve(
        listener,
        axum::Router::new()
            .route(
                "/api/status",
                axum::routing::get(|| async {
                    let mut big = String::from("{\"phase\":\"");
                    big.push_str(&"x".repeat(bmc::MAX_BODY));
                    big.push_str("\"}");
                    ([("content-type", "application/json")], big)
                }),
            )
            .route(
                "/api/config",
                axum::routing::get(|| async { Json(serde_json::json!({"name": "ok"})) }),
            ),
    )
    .await;
    let client = bmc::Bmc::new(port).unwrap();
    let ip = "127.0.0.1".parse().unwrap();
    let status: Result<bmc::Status, _> = client.get(ip, bmc::STATUS, "t").await;
    assert_eq!(status.unwrap_err(), bmc::BmcError::Invalid);
    let config: bmc::Config = client.get(ip, bmc::CONFIG, "t").await.unwrap();
    assert_eq!(config.name, "ok");
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
