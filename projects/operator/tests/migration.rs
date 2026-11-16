//! Live migration through the VM reconcile against a small in-memory API
//! server: the target Pod, its own state PVC and the time-boxed policy appear,
//! the VM is handed over only when the source manager reports "migrated", and
//! each outcome cleans up after itself.
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json, Router,
};
use kube::runtime::controller::Action;
use qrab_operator::{
    api::VirtualMachine,
    controller::{reconcile, Context},
    resources::Settings,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    future::IntoFuture,
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
struct Store(Arc<Mutex<BTreeMap<String, Value>>>);

const VM: &str = "/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/virtualmachines/test-vm";
const PODS: &str = "/api/v1/namespaces/qrab/pods";
const POLICIES: &str = "/apis/cilium.io/v2/namespaces/qrab/ciliumnetworkpolicies";
const CONFIGS: &str = "/api/v1/namespaces/qrab/configmaps";
const POOLS: &str = "/apis/cilium.io/v2alpha1/ciliumpodippools";
const ROUTES: &str = "/apis/frrk8s.metallb.io/v1beta1/namespaces/frr-k8s-system/frrconfigurations";
const CILIUM_NODES: &str = "/apis/cilium.io/v2/ciliumnodes";
const NODES: &str = "/api/v1/nodes";
const MIGRATIONS: &str =
    "/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/virtualmachinemigrations";
const CLAIMS: &str = "/api/v1/namespaces/qrab/persistentvolumeclaims";

fn status(code: u16, reason: &str) -> Response {
    (
        StatusCode::from_u16(code).unwrap(),
        Json(json!({"apiVersion":"v1","kind":"Status","status":"Failure","reason":reason,"message":reason,"code":code})),
    )
        .into_response()
}

fn merge(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                if value.is_null() {
                    target.remove(key);
                } else {
                    merge(target.entry(key.clone()).or_insert(Value::Null), value);
                }
            }
        }
        (target, patch) => *target = patch.clone(),
    }
}

fn selected(object: &Value, selector: &str) -> bool {
    selector.split(',').filter(|s| !s.is_empty()).all(|term| {
        let (key, value) = term.split_once('=').unwrap();
        object["metadata"]["labels"][key] == value
    })
}

async fn api(
    State(store): State<Store>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let mut objects = store.0.lock().unwrap();
    let path = uri.path().trim_end_matches("/status").to_owned();
    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let collection = [
        PODS,
        POLICIES,
        CONFIGS,
        POOLS,
        ROUTES,
        CILIUM_NODES,
        NODES,
        MIGRATIONS,
        CLAIMS,
    ]
    .contains(&path.as_str());
    match (method, collection) {
        (Method::GET, true) => {
            let selector = url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
                .find(|(k, _)| k == "labelSelector")
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default();
            let items: Vec<_> = objects
                .iter()
                .filter(|(k, v)| k.rsplit_once('/').unwrap().0 == path && selected(v, &selector))
                .map(|(_, v)| v.clone())
                .collect();
            Json(json!({"apiVersion":"v1","kind":"List","metadata":{"resourceVersion":"1"},"items":items}))
                .into_response()
        }
        (Method::POST, true) => {
            let mut object = body;
            let name = object["metadata"]["name"].as_str().unwrap().to_owned();
            if objects.contains_key(&format!("{path}/{name}")) {
                return status(409, "AlreadyExists");
            }
            object["metadata"]["uid"] = json!(format!("uid-{name}"));
            object["metadata"]["resourceVersion"] = json!("1");
            objects.insert(format!("{path}/{name}"), object.clone());
            (StatusCode::CREATED, Json(object)).into_response()
        }
        (Method::GET, false) => match objects.get(&path) {
            Some(object) => Json(object.clone()).into_response(),
            None => status(404, "NotFound"),
        },
        (Method::PATCH, false) => {
            let apply = headers
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("apply-patch"));
            if apply && !objects.contains_key(&path) {
                let mut object = body;
                object["metadata"]["uid"] =
                    json!(format!("uid-{}", path.rsplit('/').next().unwrap()));
                objects.insert(path, object.clone());
                return Json(object).into_response();
            }
            let Some(object) = objects.get_mut(&path) else {
                return status(404, "NotFound");
            };
            if apply {
                let metadata = object["metadata"].clone();
                *object = body;
                object["metadata"]["uid"] = metadata["uid"].clone();
            } else {
                let mut patch = body;
                if let Some(metadata) = patch.get_mut("metadata").and_then(Value::as_object_mut) {
                    metadata.remove("resourceVersion");
                    metadata.remove("uid");
                }
                merge(object, &patch);
            }
            Json(object.clone()).into_response()
        }
        (Method::DELETE, false) => match objects.remove(&path) {
            Some(_) => {
                Json(json!({"apiVersion":"v1","kind":"Status","status":"Success","code":200}))
                    .into_response()
            }
            None => status(404, "NotFound"),
        },
        _ => status(405, "MethodNotAllowed"),
    }
}

fn settings() -> Settings {
    Settings {
        namespace: "qrab".into(),
        vm_image: format!("zot.yapplesauce.com/qrab/manager@sha256:{}", "a".repeat(64)),
        oidc_issuer: "https://idm.example/oidc".into(),
        oidc_client_id: "qrab".into(),
        idp_ip: None,
        redfish_oidc_issuer: "https://idm.example/redfish".into(),
        redfish_oidc_audience: "qrab-redfish".into(),
        bmc_bearer: None,
        nic_addresses: None,
        guest_devid_ca_cn: qrab_operator::guest_identity::DEFAULT_DEVID_CA_CN.into(),
    }
}

async fn server(store: &Store) -> Arc<Context> {
    server_with(store, settings()).await
}

async fn server_with(store: &Store, settings: Settings) -> Arc<Context> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = kube::Client::try_from(kube::Config::new(
        format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap(),
    ))
    .unwrap();
    tokio::spawn(
        axum::serve(
            listener,
            Router::new().fallback(api).with_state(store.clone()),
        )
        .into_future(),
    );
    Arc::new(Context {
        client,
        settings,
        metrics: None,
        storage: None,
    })
}

impl Store {
    fn get(&self, path: &str) -> Option<Value> {
        self.0.lock().unwrap().get(path).cloned()
    }
    fn edit(&self, path: &str, change: impl FnOnce(&mut Value)) {
        change(self.0.lock().unwrap().get_mut(path).unwrap());
    }
    async fn step(&self, ctx: &Arc<Context>) -> Action {
        let vm: VirtualMachine = serde_json::from_value(self.get(VM).unwrap()).unwrap();
        reconcile(Arc::new(vm), ctx.clone()).await.unwrap()
    }
    fn status(&self) -> Value {
        self.get(VM).unwrap()["status"].clone()
    }
}

const SOURCE: &str = "/api/v1/namespaces/qrab/pods/test-vm";
const TARGET: &str = "/api/v1/namespaces/qrab/pods/test-vm-maaaa11";
const POLICY: &str = "/apis/cilium.io/v2/namespaces/qrab/ciliumnetworkpolicies/qrab-migration-move";
const GUARD: &str =
    "/apis/cilium.io/v2/namespaces/qrab/ciliumnetworkpolicies/test-vm-migration-guard";
const TARGET_CLAIM: &str = "/api/v1/namespaces/qrab/persistentvolumeclaims/test-vm-state-aaaa11";

fn node(name: &str) -> Value {
    json!({"apiVersion":"v1","kind":"Node","metadata":{"name":name},
        "status":{"conditions":[{"type":"Ready","status":"True"}]}})
}

fn migration(name: &str, uid: &str, created: &str) -> Value {
    json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachineMigration",
        "metadata":{"name":name,"namespace":"qrab","uid":uid,"creationTimestamp":created,"resourceVersion":"1"},
        "spec":{"vmName":"test-vm"}})
}

/// A running VM on node-a whose Pod is Ready, and two other Ready nodes.
async fn running() -> (Store, Arc<Context>) {
    running_with(json!({}), []).await
}

/// `running` for a VM whose spec also carries `spec`, with extra objects.
async fn running_with<const N: usize>(
    spec: Value,
    objects: [(String, Value); N],
) -> (Store, Arc<Context>) {
    let store = Store::default();
    store.0.lock().unwrap().extend(objects);
    store.0.lock().unwrap().extend([
        (VM.to_owned(), json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
            "metadata":{"name":"test-vm","namespace":"qrab","uid":"vm-uid","generation":1,"resourceVersion":"1"},
            "spec":{"stateVolumeClaim":"state","components":{"bmc":{"enabled":false}}},
            "status":{"observedGeneration":1,"phase":"Pending","ready":false,"reason":"","message":"","stateVolumeUid":"pvc-uid"}})),
        (format!("{CLAIMS}/state"),
            json!({"apiVersion":"v1","kind":"PersistentVolumeClaim","metadata":{"name":"state","namespace":"qrab","uid":"pvc-uid"},
                "spec":{"accessModes":["ReadWriteOncePod"],"volumeMode":"Filesystem","storageClassName":"zfs-block","resources":{"requests":{"storage":"1Gi"}}},"status":{"phase":"Bound"}})),
        (format!("{NODES}/node-a"), node("node-a")),
        (format!("{NODES}/node-b"), node("node-b")),
        (format!("{NODES}/node-c"), node("node-c")),
    ]);
    store.edit(VM, |vm| merge(&mut vm["spec"], &spec));
    let ctx = server(&store).await;
    store.step(&ctx).await;
    assert!(store.get(SOURCE).is_some(), "the VM Pod is created");
    store.edit(SOURCE, |pod| {
        pod["spec"]["nodeName"] = json!("node-a");
        pod["status"] = json!({"phase":"Running","podIP":"10.244.1.5","conditions":[{"type":"Ready","status":"True"}],
            "containerStatuses":[{"name":"manager","image":"x","imageID":"x","ready":true,"restartCount":0,"state":{"running":{}}}]});
    });
    store.step(&ctx).await;
    assert_eq!(store.status()["phase"], "Running");
    (store, ctx)
}

fn terminated(pod: &mut Value, code: i32, message: &str) {
    pod["status"]["phase"] = json!(if code == 0 { "Succeeded" } else { "Failed" });
    pod["status"]["containerStatuses"] = json!([{"name":"manager","image":"x","imageID":"x","ready":false,"restartCount":0,
        "state":{"terminated":{"exitCode":code,"message":message,"reason":"Completed"}}}]);
}

fn config(store: &Store, name: &str) -> Value {
    serde_json::from_str(
        store.get(&format!("{CONFIGS}/{name}")).unwrap()["data"]["vm.json"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

/// Reconcile until `done` holds, at most `limit` times.
async fn until(store: &Store, ctx: &Arc<Context>, limit: usize, done: impl Fn(&Store) -> bool) {
    for _ in 0..limit {
        if done(store) {
            return;
        }
        store.step(ctx).await;
    }
    assert!(
        done(store),
        "condition not reached; VM status {}",
        store.status()
    );
}

fn moving(store: &Store) -> Value {
    store.get(&format!("{MIGRATIONS}/move")).unwrap()["status"].clone()
}

#[tokio::test]
async fn a_vm_moves_to_another_node_and_its_target_takes_over_only_after_the_handover() {
    let (store, ctx) = running().await;
    // The standing guard denies the migration port to everything but this VM's Pods.
    let guard = store.get(GUARD).expect("guard policy");
    assert_eq!(
        guard["spec"]["ingressDeny"][0]["toPorts"][0]["ports"][0]["port"],
        "7800"
    );
    assert!(store.get(POLICY).is_none(), "no window before a migration");

    store.0.lock().unwrap().insert(
        format!("{MIGRATIONS}/move"),
        migration(
            "move",
            "aaaa1111-2222-3333-4444-555566667777",
            "2026-11-14T17:00:00Z",
        ),
    );
    store.step(&ctx).await;
    let status = moving(&store);
    assert_eq!(
        (
            status["phase"].as_str(),
            status["sourceNode"].as_str(),
            status["targetNode"].as_str()
        ),
        (Some("TargetStarting"), Some("node-a"), Some("node-b"))
    );
    assert_eq!(store.status()["phase"], "Migrating");

    until(&store, &ctx, 6, |s| s.get(TARGET).is_some()).await;
    // Only the target reaches the source, on the migration port.
    let window = store.get(POLICY).expect("window policy");
    assert_eq!(
        window["specs"][0]["endpointSelector"]["matchLabels"]["qrab.yapplesauce.com/pod"],
        "test-vm"
    );
    assert_eq!(
        window["specs"][0]["ingress"][0]["fromEndpoints"][0]["matchLabels"]
            ["qrab.yapplesauce.com/pod"],
        "test-vm-maaaa11"
    );
    assert_eq!(
        window["specs"][1]["egress"][0]["toEndpoints"][0]["matchLabels"]
            ["qrab.yapplesauce.com/pod"],
        "test-vm"
    );
    // Its own empty state PVC and node, and the source's address to dial.
    let claim = store.get(TARGET_CLAIM).expect("target state PVC");
    assert_eq!(claim["spec"]["storageClassName"], "zfs-block");
    let target = store.get(TARGET).unwrap();
    assert_eq!(
        target["spec"]["nodeSelector"]["kubernetes.io/hostname"],
        "node-b"
    );
    let volume = target["spec"]["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "state")
        .unwrap()
        .clone();
    assert_eq!(
        volume["persistentVolumeClaim"]["claimName"],
        "test-vm-state-aaaa11"
    );
    let incoming = config(&store, "test-vm-maaaa11-config");
    assert_eq!(
        incoming["incoming"],
        json!({"migration":"aaaa1111-2222-3333-4444-555566667777","source":"10.244.1.5:7800"})
    );
    assert_eq!(
        config(&store, "test-vm-config")["incoming"],
        Value::Null,
        "the source config is untouched"
    );

    store.edit(TARGET, |pod| {
        pod["spec"]["nodeName"] = json!("node-b");
        pod["status"] = json!({"phase":"Running","podIP":"10.244.2.9","conditions":[{"type":"Ready","status":"False"}],
            "containerStatuses":[{"name":"manager","image":"x","imageID":"x","ready":false,"restartCount":0,"state":{"running":{}}}]});
    });
    until(&store, &ctx, 4, |s| moving(s)["phase"] == "Migrating").await;
    assert_eq!(moving(&store)["targetPodUid"], "uid-test-vm-maaaa11");

    // An ordinary exit 0 is not a handover; only "migrated" is.
    store.edit(SOURCE, |pod| terminated(pod, 0, "migrated\n"));
    store.edit(TARGET, |pod| {
        pod["status"]["conditions"] = json!([{"type":"Ready","status":"True"}])
    });
    until(&store, &ctx, 3, |s| moving(s)["phase"] == "Succeeded").await;
    let vm = store.status();
    assert_eq!(
        (
            vm["podName"].as_str(),
            vm["podUid"].as_str(),
            vm["nodeName"].as_str()
        ),
        (
            Some("test-vm-maaaa11"),
            Some("uid-test-vm-maaaa11"),
            Some("node-b")
        )
    );
    assert_eq!(
        (
            vm["stateVolumeClaim"].as_str(),
            vm["placementNode"].as_str()
        ),
        (Some("test-vm-state-aaaa11"), Some("node-b"))
    );
    assert_eq!(
        config(&store, "test-vm-maaaa11-config")["incoming"],
        Value::Null,
        "incoming is stripped on handover"
    );

    // Cleanup: the source Pod and the window go; the user's own state PVC stays.
    until(&store, &ctx, 6, |s| moving(s)["cleaned"] == true).await;
    assert!(store.get(SOURCE).is_none());
    assert!(store.get(POLICY).is_none());
    assert!(store.get(&format!("{CLAIMS}/state")).is_some());
    // The VM now runs its target Pod unchanged.
    until(&store, &ctx, 4, |s| s.status()["phase"] == "Running").await;
    assert_eq!(
        store.get(TARGET).unwrap()["metadata"]["uid"],
        "uid-test-vm-maaaa11",
        "the adopted Pod was not replaced"
    );
    assert!(
        store.get(SOURCE).is_none(),
        "no second boot under the old name"
    );
    // The adopted target is no longer a disposable target: deleting the
    // finished migration leaves the VM's Pod, PVC and ConfigMap alone.
    assert!(store.get(TARGET).unwrap()["metadata"]["annotations"]
        .get("qrab.yapplesauce.com/migration-target")
        .is_none());
    store
        .0
        .lock()
        .unwrap()
        .remove(&format!("{MIGRATIONS}/move"));
    for _ in 0..4 {
        store.step(&ctx).await;
    }
    assert_eq!(
        store.get(TARGET).unwrap()["metadata"]["uid"],
        "uid-test-vm-maaaa11"
    );
    assert!(store.get(TARGET_CLAIM).is_some());
    assert!(store
        .get(&format!("{CONFIGS}/test-vm-maaaa11-config"))
        .is_some());
    assert_eq!(store.status()["phase"], "Running");
}

#[tokio::test]
async fn a_target_that_stops_fails_the_migration_and_leaves_the_source_running() {
    let (store, ctx) = running().await;
    store.0.lock().unwrap().insert(
        format!("{MIGRATIONS}/move"),
        migration(
            "move",
            "aaaa1111-2222-3333-4444-555566667777",
            "2026-11-14T17:00:00Z",
        ),
    );
    until(&store, &ctx, 8, |s| s.get(TARGET).is_some()).await;
    store.edit(TARGET, |pod| {
        pod["spec"]["nodeName"] = json!("node-b");
        terminated(pod, 1, "migration: the source refused");
    });
    until(&store, &ctx, 4, |s| moving(s)["phase"] == "Failed").await;
    assert!(moving(&store)["message"]
        .as_str()
        .unwrap()
        .contains("stopped before taking over"));
    assert!(moving(&store).get("reason").is_none(), "unclassified");
    until(&store, &ctx, 6, |s| moving(s)["cleaned"] == true).await;
    assert!(store.get(TARGET).is_none());
    assert!(store.get(POLICY).is_none());
    assert!(store.get(TARGET_CLAIM).is_none());
    assert_eq!(store.get(SOURCE).unwrap()["metadata"]["uid"], "uid-test-vm");
    assert_eq!(store.status()["podName"], "test-vm");
    store.step(&ctx).await;
    assert_eq!(store.status()["phase"], "Running");
}

#[tokio::test]
async fn a_powered_off_vm_fails_its_migration_as_powered_off_and_cleans_up() {
    // The VM Pod stays Ready while its QEMU is powered off; only the
    // target learns it, from the source's refusal.
    let (store, ctx) = running().await;
    store.0.lock().unwrap().insert(
        format!("{MIGRATIONS}/move"),
        migration(
            "move",
            "aaaa1111-2222-3333-4444-555566667777",
            "2026-11-14T17:00:00Z",
        ),
    );
    until(&store, &ctx, 8, |s| s.get(TARGET).is_some()).await;
    assert!(store
        .get(&format!("{CONFIGS}/test-vm-maaaa11-config"))
        .is_some());
    store.edit(TARGET, |pod| {
        pod["spec"]["nodeName"] = json!("node-b");
        terminated(pod, 1, "source-powered-off");
    });
    until(&store, &ctx, 4, |s| moving(s)["phase"] == "Failed").await;
    let status = moving(&store);
    assert_eq!(status["reason"], "PoweredOff");
    let message = status["message"].as_str().unwrap();
    assert!(message.contains("powered off"), "{message}");
    assert!(message.contains("power it on"), "{message}");
    // Terminal: cleaned up like any failed move, and never retried.
    until(&store, &ctx, 6, |s| moving(s)["cleaned"] == true).await;
    assert!(store.get(TARGET).is_none());
    assert!(store.get(POLICY).is_none());
    assert!(store.get(TARGET_CLAIM).is_none());
    assert!(store
        .get(&format!("{CONFIGS}/test-vm-maaaa11-config"))
        .is_none());
    for _ in 0..3 {
        store.step(&ctx).await;
    }
    assert!(store.get(TARGET).is_none(), "no second target is started");
    assert_eq!(moving(&store)["phase"], "Failed");
    assert_eq!(moving(&store)["reason"], "PoweredOff");
    assert_eq!(store.status()["podName"], "test-vm");
    assert_eq!(store.status()["phase"], "Running");
}

#[tokio::test]
async fn a_vm_reported_powered_off_fails_its_migration_before_any_target() {
    let (store, ctx) = running().await;
    store.edit(VM, |vm| vm["status"]["powerState"] = json!("Off"));
    store.0.lock().unwrap().insert(
        format!("{MIGRATIONS}/move"),
        migration(
            "move",
            "aaaa1111-2222-3333-4444-555566667777",
            "2026-11-14T17:00:00Z",
        ),
    );
    store.step(&ctx).await;
    assert_eq!(moving(&store)["phase"], "Failed");
    assert_eq!(moving(&store)["reason"], "PoweredOff");
    assert!(store.get(TARGET).is_none());
    assert!(store.get(TARGET_CLAIM).is_none());
    assert!(store.get(POLICY).is_none());
}

fn media_claim(modes: Value) -> (String, Value) {
    (
        format!("{CLAIMS}/media"),
        json!({"apiVersion":"v1","kind":"PersistentVolumeClaim","metadata":{"name":"media","namespace":"qrab","uid":"media-uid"},
            "spec":{"accessModes":modes,"volumeMode":"Filesystem","storageClassName":"zfs-block","resources":{"requests":{"storage":"1Gi"}}},"status":{"phase":"Bound"}}),
    )
}

#[tokio::test]
async fn a_claim_the_target_node_cannot_mount_refuses_the_migration_before_any_target() {
    let (store, ctx) = running_with(
        json!({"mediaVolumeClaim":"media"}),
        [media_claim(json!(["ReadWriteOncePod"]))],
    )
    .await;
    store.0.lock().unwrap().insert(
        format!("{MIGRATIONS}/move"),
        migration(
            "move",
            "aaaa1111-2222-3333-4444-555566667777",
            "2026-11-14T17:00:00Z",
        ),
    );
    store.step(&ctx).await;
    let status = moving(&store);
    assert_eq!(status["phase"], "Failed");
    assert_eq!(status["reason"], "UnsharedClaim");
    let message = status["message"].as_str().unwrap();
    assert!(message.contains("media catalog claim media"), "{message}");
    assert!(store.get(TARGET).is_none());
    assert!(store.get(TARGET_CLAIM).is_none());
    assert_eq!(store.status()["podName"], "test-vm");
}

#[tokio::test]
async fn a_read_only_many_media_claim_goes_along_to_the_target() {
    let (store, ctx) = running_with(
        json!({"mediaVolumeClaim":"media"}),
        [media_claim(json!(["ReadOnlyMany"]))],
    )
    .await;
    store.0.lock().unwrap().insert(
        format!("{MIGRATIONS}/move"),
        migration(
            "move",
            "aaaa1111-2222-3333-4444-555566667777",
            "2026-11-14T17:00:00Z",
        ),
    );
    until(&store, &ctx, 8, |s| s.get(TARGET).is_some()).await;
    let target = store.get(TARGET).unwrap();
    let claims: Vec<_> = target["spec"]["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v["persistentVolumeClaim"]["claimName"].as_str())
        .collect();
    assert!(claims.contains(&"media"), "{claims:?}");
    assert!(!claims.contains(&"state"), "{claims:?}");
}

#[tokio::test]
async fn a_source_that_exits_without_handing_over_fails_the_migration() {
    let (store, ctx) = running().await;
    store.0.lock().unwrap().insert(
        format!("{MIGRATIONS}/move"),
        migration(
            "move",
            "aaaa1111-2222-3333-4444-555566667777",
            "2026-11-14T17:00:00Z",
        ),
    );
    until(&store, &ctx, 8, |s| s.get(TARGET).is_some()).await;
    store.edit(SOURCE, |pod| terminated(pod, 0, ""));
    until(&store, &ctx, 3, |s| moving(s)["phase"] == "Failed").await;
    assert_eq!(
        store.status()["podName"],
        "test-vm",
        "storage stays with the source"
    );
}

#[tokio::test]
async fn a_second_concurrent_migration_is_refused() {
    let (store, ctx) = running().await;
    store.0.lock().unwrap().extend([
        (
            format!("{MIGRATIONS}/move"),
            migration(
                "move",
                "aaaa1111-2222-3333-4444-555566667777",
                "2026-11-14T17:00:00Z",
            ),
        ),
        (
            format!("{MIGRATIONS}/again"),
            migration(
                "again",
                "bbbb1111-2222-3333-4444-555566667777",
                "2026-11-14T17:00:05Z",
            ),
        ),
    ]);
    store.step(&ctx).await;
    assert_eq!(moving(&store)["phase"], "TargetStarting");
    let again = store.get(&format!("{MIGRATIONS}/again")).unwrap()["status"].clone();
    assert_eq!(again["phase"], "Failed");
    assert!(again["message"]
        .as_str()
        .unwrap()
        .contains("another migration"));
}

#[tokio::test]
async fn the_requested_node_must_differ_and_be_ready() {
    let (store, ctx) = running().await;
    let mut same = migration(
        "move",
        "aaaa1111-2222-3333-4444-555566667777",
        "2026-11-14T17:00:00Z",
    );
    same["spec"]["targetNode"] = json!("node-a");
    store
        .0
        .lock()
        .unwrap()
        .insert(format!("{MIGRATIONS}/move"), same);
    store.step(&ctx).await;
    assert_eq!(moving(&store)["phase"], "Failed");
    assert!(store.get(TARGET).is_none());
}

#[tokio::test]
async fn a_handed_over_pod_without_its_migration_blocks_instead_of_booting_again() {
    let (store, ctx) = running().await;
    store.edit(SOURCE, |pod| terminated(pod, 0, "migrated"));
    store.step(&ctx).await;
    store.step(&ctx).await;
    assert_eq!(store.status()["reason"], "HandedOver");
    assert_eq!(
        store.get(SOURCE).unwrap()["metadata"]["uid"],
        "uid-test-vm",
        "not replaced"
    );
}

fn labelled(mut node: Value) -> Value {
    let name = node["metadata"]["name"].clone();
    node["metadata"]["labels"] = json!({"kubernetes.io/hostname": name});
    node
}

/// A node's base BGP configuration, which NIC routes are derived from.
fn base_route(node: &str, id: &str) -> Value {
    json!({"apiVersion":"frrk8s.metallb.io/v1beta1","kind":"FRRConfiguration",
        "metadata":{"name":node,"namespace":"frr-k8s-system","uid":format!("uid-{node}"),
            "labels":{"qrab.yapplesauce.com/nic-route-base":"true"}},
        "spec":{"nodeSelector":{"matchLabels":{"kubernetes.io/hostname":node}},
            "bgp":{"routers":[{"asn":64512,"id":id,"prefixes":["10.244.0.0/24"],"neighbors":[
                {"address":"10.203.0.1","asn":64513,"toAdvertise":{"allowed":{"mode":"all"}}}]}]}}})
}

/// The tap NIC's anchor on `node` reports Ready (qrab-netd plumbed it) with
/// the NIC's pool address.
fn plumb(store: &Store, anchor: &str, node: &str, ip: &str) {
    store.edit(&format!("{PODS}/{anchor}"), |pod| {
        pod["spec"]["nodeName"] = json!(node);
        pod["status"] = json!({"phase":"Running","podIP":ip,"conditions":[
            {"type":"Ready","status":"True"},{"type":"qrab.yapplesauce.com/nic-plumbed","status":"True"}]});
    });
}

/// A running tap-NIC VM on node-a (anchor plumbed) and a migration `move`
/// created for it; returns the store, the context and the source anchor path.
async fn nic_vm_migrating() -> (Store, Arc<Context>, String) {
    let store = Store::default();
    let mut settings = settings();
    settings.nic_addresses = Some(qrab_operator::address::Addressing {
        range: "10.244.128.0/17".parse().unwrap(),
        frr_namespace: "frr-k8s-system".into(),
    });
    store.0.lock().unwrap().extend([
        (VM.to_owned(), json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
            "metadata":{"name":"test-vm","namespace":"qrab","uid":"vm-uid","generation":1,"resourceVersion":"1"},
            "spec":{"stateVolumeClaim":"state","accelerator":"kvm","components":{"bmc":{"enabled":false}},"nics":[{"name":"lan"}]},
            "status":{"observedGeneration":1,"phase":"Pending","ready":false,"reason":"","message":"","stateVolumeUid":"pvc-uid"}})),
        (format!("{CLAIMS}/state"),
            json!({"apiVersion":"v1","kind":"PersistentVolumeClaim","metadata":{"name":"state","namespace":"qrab","uid":"pvc-uid"},
                "spec":{"accessModes":["ReadWriteOncePod"],"volumeMode":"Filesystem","storageClassName":"zfs-block","resources":{"requests":{"storage":"1Gi"}}},"status":{"phase":"Bound"}})),
        (format!("{ROUTES}/node-a"), base_route("node-a", "10.203.0.11")),
        (format!("{ROUTES}/node-b"), base_route("node-b", "10.203.0.12")),
        (format!("{NODES}/node-a"), labelled(node("node-a"))),
        (format!("{NODES}/node-b"), {
            let mut node = labelled(node("node-b"));
            node["status"]["addresses"] = json!([{"type":"InternalIP","address":"10.203.0.12"}]);
            node["status"]["allocatable"] = json!({"qrab.yapplesauce.com/kvm":"32"});
            node
        }),
    ]);
    let ctx = server_with(&store, settings).await;
    until(&store, &ctx, 10, |s| s.get(SOURCE).is_some()).await;
    store.edit(SOURCE, |pod| {
        pod["spec"]["nodeName"] = json!("node-a");
        pod["status"] = json!({"phase":"Running","podIP":"10.244.1.5","conditions":[{"type":"Ready","status":"True"}],
            "containerStatuses":[{"name":"manager","image":"x","imageID":"x","ready":true,"restartCount":0,"state":{"running":{}}}]});
    });
    let source_anchor = format!("{PODS}/test-vm-nic-lan-node-a");
    until(&store, &ctx, 6, |s| s.get(&source_anchor).is_some()).await;
    let ip = store.status()["nics"][0]["address"]
        .as_str()
        .unwrap()
        .to_owned();
    plumb(&store, "test-vm-nic-lan-node-a", "node-a", &ip);
    until(&store, &ctx, 6, |s| s.status()["phase"] == "Running").await;

    store.0.lock().unwrap().insert(
        format!("{MIGRATIONS}/move"),
        migration(
            "move",
            "aaaa1111-2222-3333-4444-555566667777",
            "2026-11-14T17:00:00Z",
        ),
    );
    (store, ctx, source_anchor)
}

#[tokio::test]
async fn deleting_a_migration_mid_flight_removes_its_target_objects() {
    let (store, ctx, source_anchor) = nic_vm_migrating().await;
    let target_anchor = format!("{PODS}/test-vm-nic-lan-node-b");
    until(&store, &ctx, 8, |s| s.get(&target_anchor).is_some()).await;
    let ip = store.status()["nics"][0]["address"]
        .as_str()
        .unwrap()
        .to_owned();
    plumb(&store, "test-vm-nic-lan-node-b", "node-b", &ip);
    until(&store, &ctx, 6, |s| s.get(TARGET).is_some()).await;
    assert_eq!(moving(&store)["phase"], "TargetStarting");
    assert_eq!(
        store.get(TARGET).unwrap()["metadata"]["annotations"]
            ["qrab.yapplesauce.com/migration-target"],
        "move"
    );
    let target_config = format!("{CONFIGS}/test-vm-maaaa11-config");
    for leftover in [TARGET, TARGET_CLAIM, POLICY, &target_config, &target_anchor] {
        assert!(
            store.get(leftover).is_some(),
            "{leftover} exists before the deletion"
        );
    }
    // The user deletes the migration while its target starts.
    store
        .0
        .lock()
        .unwrap()
        .remove(&format!("{MIGRATIONS}/move"));
    until(&store, &ctx, 6, |s| {
        [
            TARGET,
            TARGET_CLAIM,
            POLICY,
            target_config.as_str(),
            target_anchor.as_str(),
        ]
        .iter()
        .all(|p| s.get(p).is_none())
    })
    .await;
    // The source and its anchor are untouched and the VM keeps running there.
    assert!(store.get(SOURCE).is_some());
    assert!(store.get(&source_anchor).is_some());
    assert!(store.get(&format!("{CLAIMS}/state")).is_some());
    until(&store, &ctx, 4, |s| s.status()["phase"] == "Running").await;
    assert_eq!(store.status()["podName"], "test-vm");
}

#[tokio::test]
async fn a_nic_vm_gets_standby_target_anchors_before_its_target_and_loses_them_on_failure() {
    let store = Store::default();
    let mut settings = settings();
    settings.nic_addresses = Some(qrab_operator::address::Addressing {
        range: "10.244.128.0/17".parse().unwrap(),
        frr_namespace: "frr-k8s-system".into(),
    });
    store.0.lock().unwrap().extend([
        (VM.to_owned(), json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
            "metadata":{"name":"test-vm","namespace":"qrab","uid":"vm-uid","generation":1,"resourceVersion":"1"},
            "spec":{"stateVolumeClaim":"state","accelerator":"kvm","components":{"bmc":{"enabled":false}},"nics":[{"name":"lan"}]},
            "status":{"observedGeneration":1,"phase":"Pending","ready":false,"reason":"","message":"","stateVolumeUid":"pvc-uid"}})),
        (format!("{CLAIMS}/state"),
            json!({"apiVersion":"v1","kind":"PersistentVolumeClaim","metadata":{"name":"state","namespace":"qrab","uid":"pvc-uid"},
                "spec":{"accessModes":["ReadWriteOncePod"],"volumeMode":"Filesystem","storageClassName":"zfs-block","resources":{"requests":{"storage":"1Gi"}}},"status":{"phase":"Bound"}})),
        (format!("{ROUTES}/node-a"), base_route("node-a", "10.203.0.11")),
        (format!("{ROUTES}/node-b"), base_route("node-b", "10.203.0.12")),
        (format!("{NODES}/node-a"), labelled(node("node-a"))),
        (format!("{NODES}/node-b"), {
            let mut node = labelled(node("node-b"));
            node["status"]["addresses"] = json!([{"type":"InternalIP","address":"10.203.0.12"}]);
            node["status"]["allocatable"] = json!({"qrab.yapplesauce.com/kvm":"32"});
            node
        }),
    ]);
    let ctx = server_with(&store, settings).await;
    until(&store, &ctx, 10, |s| s.get(SOURCE).is_some()).await;
    store.edit(SOURCE, |pod| {
        pod["spec"]["nodeName"] = json!("node-a");
        pod["status"] = json!({"phase":"Running","podIP":"10.244.1.5","conditions":[{"type":"Ready","status":"True"}],
            "containerStatuses":[{"name":"manager","image":"x","imageID":"x","ready":true,"restartCount":0,"state":{"running":{}}}]});
    });
    let source_anchor = format!("{PODS}/test-vm-nic-lan-node-a");
    until(&store, &ctx, 6, |s| s.get(&source_anchor).is_some()).await;
    let ip = store.status()["nics"][0]["address"]
        .as_str()
        .unwrap()
        .to_owned();
    plumb(&store, "test-vm-nic-lan-node-a", "node-a", &ip);
    until(&store, &ctx, 6, |s| s.status()["phase"] == "Running").await;

    store.0.lock().unwrap().insert(
        format!("{MIGRATIONS}/move"),
        migration(
            "move",
            "aaaa1111-2222-3333-4444-555566667777",
            "2026-11-14T17:00:00Z",
        ),
    );
    let target_anchor = format!("{PODS}/test-vm-nic-lan-node-b");
    until(&store, &ctx, 8, |s| s.get(&target_anchor).is_some()).await;
    let anchor = store.get(&target_anchor).unwrap();
    assert_eq!(
        anchor["metadata"]["annotations"]["ipam.cilium.io/handoff-from"],
        "qrab/test-vm-nic-lan-node-a"
    );
    assert_eq!(
        anchor["metadata"]["annotations"]["qrab.yapplesauce.com/migration"],
        "move"
    );
    assert_eq!(
        anchor["spec"]["affinity"]["nodeAffinity"]
            ["requiredDuringSchedulingIgnoredDuringExecution"]["nodeSelectorTerms"][0]
            ["matchFields"][0]["values"],
        json!(["node-b"])
    );
    // No target VM Pod until its anchor is plumbed.
    for _ in 0..3 {
        store.step(&ctx).await;
    }
    assert!(store.get(TARGET).is_none());
    assert_eq!(
        moving(&store)["message"],
        "Waiting for the target NIC anchors"
    );
    // The VM's own reconcile leaves the migration's anchor alone meanwhile.
    assert!(
        store.get(&target_anchor).unwrap()["metadata"]["annotations"]
            .get("qrab.yapplesauce.com/retire-after")
            .is_none()
    );
    plumb(&store, "test-vm-nic-lan-node-b", "node-b", &ip);
    until(&store, &ctx, 6, |s| s.get(TARGET).is_some()).await;
    assert_eq!(
        config(&store, "test-vm-maaaa11-config")["incoming"]["node_ip"],
        "10.203.0.12",
        "the source node forwards to the target node during the switch"
    );
    // A failed migration removes its standby anchor and keeps the source's.
    store.edit(TARGET, |pod| {
        pod["spec"]["nodeName"] = json!("node-b");
        terminated(pod, 1, "migration: the source refused");
    });
    until(&store, &ctx, 8, |s| moving(s)["cleaned"] == true).await;
    assert!(store.get(&target_anchor).is_none());
    assert!(store.get(&source_anchor).is_some());
}
