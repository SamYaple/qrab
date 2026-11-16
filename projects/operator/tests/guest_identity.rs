//! Guest workload identity: the exact ClusterStaticEntries a VM's
//! `spec.guestIdentity` renders, its validation, and their lifecycle against
//! a small in-memory API server (finalizer, workload removal and rename,
//! removal of guestIdentity, VM deletion, ownership and the orphan sweep).
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json, Router,
};
use qrab_operator::{
    api::VirtualMachine,
    controller::{reconcile, Context},
    guest_identity::{self, DEFAULT_DEVID_CA_CN, FINALIZER},
    resources::{self, Settings},
    Error,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    future::IntoFuture,
    sync::{Arc, Mutex},
};

const UID: &str = "0b7c4e6a-1d2e-4f30-8a4b-5c6d7e8f9a0b";
const OTHER_UID: &str = "11111111-2222-4333-8444-555555555555";
const VMS: &str = "/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/virtualmachines";
const VM: &str = "/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/virtualmachines/test-vm";
const PODS: &str = "/api/v1/namespaces/qrab/pods";
const POLICIES: &str = "/apis/cilium.io/v2/namespaces/qrab/ciliumnetworkpolicies";
const CONFIGS: &str = "/api/v1/namespaces/qrab/configmaps";
const MIGRATIONS: &str =
    "/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/virtualmachinemigrations";
const ENTRIES: &str = "/apis/spire.spiffe.io/v1alpha1/clusterstaticentries";

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
        guest_devid_ca_cn: DEFAULT_DEVID_CA_CN.into(),
        tpm_issuer: Default::default(),
    }
}

fn two_workloads() -> Value {
    json!({"workloads":[
        {"name":"web","selectors":["unix:uid:1000","systemd:id:web.service"]},
        {"name":"backup","selectors":["unix:user:backup"]}]})
}

fn vm_json(guest_identity: Option<Value>) -> Value {
    let mut vm = json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
        "metadata":{"name":"test-vm","namespace":"qrab","uid":UID,"generation":1,"resourceVersion":"1"},
        "spec":{"stateVolumeClaim":"state","components":{"bmc":{"enabled":false}}},
        "status":{"observedGeneration":1,"phase":"Pending","ready":false,"reason":"","message":"","stateVolumeUid":"pvc-uid"}});
    if let Some(identity) = guest_identity {
        vm["spec"]["guestIdentity"] = identity;
    }
    vm
}

fn vm(guest_identity: Option<Value>) -> VirtualMachine {
    serde_json::from_value(vm_json(guest_identity)).unwrap()
}

fn labels(uid: &str) -> Value {
    json!({"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"guest-identity",
        "app.kubernetes.io/instance":"test-vm","qrab.yapplesauce.com/owner-uid":uid,
        "qrab.yapplesauce.com/vm-namespace":"qrab"})
}

fn entry(name: &str, spec: Value) -> Value {
    json!({"apiVersion":"spire.spiffe.io/v1alpha1","kind":"ClusterStaticEntry",
        "metadata":{"name":name,"labels":labels(UID)},"spec":spec})
}

fn alias() -> Value {
    entry(
        &format!("qrab-guest-node-{UID}"),
        json!({"spiffeID":format!("spiffe://yapplesauce.com/qrab/guest-node/{UID}"),
            "parentID":"spiffe://yapplesauce.com/spire/server",
            "selectors":[format!("tpm_devid:subject:cn:qrab-vm-{UID}"),"tpm_devid:issuer:cn:qrab-devid-ca-1"],
            "className":"kubernetes-tpm"}),
    )
}

fn workload(name: &str, selectors: Value) -> Value {
    entry(
        &format!("qrab-guest-{UID}-{name}"),
        json!({"spiffeID":format!("spiffe://yapplesauce.com/qrab/guest/{UID}/{name}"),
            "parentID":format!("spiffe://yapplesauce.com/qrab/guest-node/{UID}"),
            "selectors":selectors,"x509SVIDTTL":"10m","className":"kubernetes-tpm"}),
    )
}

#[test]
fn a_vm_with_two_workloads_renders_its_alias_and_two_workload_entries() {
    let rendered: Vec<Value> =
        guest_identity::render(&vm(Some(two_workloads())), DEFAULT_DEVID_CA_CN)
            .unwrap()
            .iter()
            .map(|e| serde_json::to_value(e).unwrap())
            .collect();
    assert_eq!(
        rendered,
        vec![
            alias(),
            workload("backup", json!(["unix:user:backup"])),
            workload("web", json!(["unix:uid:1000", "systemd:id:web.service"])),
        ]
    );
}

#[test]
fn a_vm_without_guest_identity_renders_no_entries_and_an_empty_one_only_its_alias() {
    assert!(guest_identity::render(&vm(None), DEFAULT_DEVID_CA_CN)
        .unwrap()
        .is_empty());
    let alias_only = guest_identity::render(&vm(Some(json!({}))), "qrab DevID CA 2").unwrap();
    assert_eq!(alias_only.len(), 1);
    assert_eq!(
        serde_json::to_value(&alias_only[0]).unwrap()["spec"]["selectors"][1],
        "tpm_devid:issuer:cn:qrab DevID CA 2"
    );
}

fn manager_config(desired: &resources::Desired) -> Value {
    serde_json::from_str(&desired.config.data.clone().unwrap()["vm.json"]).unwrap()
}

#[test]
fn only_a_vm_with_guest_identity_asks_its_manager_for_the_vtpm_identity_and_relay() {
    let plain = resources::render(&vm(None), &settings()).unwrap();
    let config = manager_config(&plain);
    assert!(config.get("guest_identity").is_none());
    assert!(config.get("spire_relay").is_none());

    let with = resources::render(&vm(Some(two_workloads())), &settings()).unwrap();
    let config = manager_config(&with);
    assert_eq!(
        config["guest_identity"],
        json!({"enabled":true,
            "issuer_url":"https://qrab-tpm-issuer.qrab-tpm-issuer.svc:8443",
            "issuer_id":"spiffe://yapplesauce.com/kubernetes/qrab-tpm-issuer/qrab-tpm-issuer"})
    );
    assert_eq!(config["spire_relay"], json!({"enabled":true}));
    assert_ne!(with.hash, plain.hash, "the disk and port need a new QEMU");
    // Apart from those two keys the manager config is the plain one.
    let mut stripped = config.clone();
    stripped.as_object_mut().unwrap().remove("guest_identity");
    stripped.as_object_mut().unwrap().remove("spire_relay");
    assert_eq!(stripped, manager_config(&plain));

    // The issuer location follows the operator's settings; workloads do not
    // change the manager config.
    let mut moved = settings();
    moved.tpm_issuer = guest_identity::TpmIssuer {
        url: "https://issuer.example:9443".into(),
        id: "spiffe://yapplesauce.com/kubernetes/elsewhere/issuer".into(),
    };
    let config = manager_config(&resources::render(&vm(Some(json!({}))), &moved).unwrap());
    assert_eq!(
        config["guest_identity"]["issuer_url"],
        "https://issuer.example:9443"
    );
    assert_eq!(
        config["guest_identity"]["issuer_id"],
        "spiffe://yapplesauce.com/kubernetes/elsewhere/issuer"
    );
    assert_eq!(
        manager_config(&resources::render(&vm(Some(json!({}))), &settings()).unwrap()),
        manager_config(&with)
    );
}

fn invalid(identity: Value) -> String {
    let vm = vm(Some(identity));
    match resources::validate(&vm, &settings()) {
        Err(Error::Invalid(message)) => message,
        other => panic!("{other:?}"),
    }
}

#[test]
fn invalid_workload_names_and_selectors_are_rejected() {
    resources::validate(&vm(Some(two_workloads())), &settings()).unwrap();
    for name in ["Web", "web_1", "-web", "web-", "", &"a".repeat(41)] {
        let message = invalid(json!({"workloads":[{"name":name,"selectors":["unix:uid:1"]}]}));
        assert!(message.contains("workload names"), "{name}: {message}");
    }
    let message = invalid(json!({"workloads":[
        {"name":"web","selectors":["unix:uid:1"]},{"name":"web","selectors":["unix:uid:2"]}]}));
    assert!(message.contains("unique"), "{message}");
    for selectors in [
        json!([]),
        json!(["k8s:ns:qrab"]),
        json!(["tpm_devid:subject:cn:x"]),
        json!(["unix:uid"]),
        json!(["unix:uid:"]),
        json!(["unix:path:/a b"]),
        json!(["unix:uid:1", "unix:uid:1"]),
        json!((0..9).map(|i| format!("unix:uid:{i}")).collect::<Vec<_>>()),
    ] {
        let message = invalid(json!({"workloads":[{"name":"web","selectors":selectors}]}));
        assert!(message.contains("selectors"), "{selectors}: {message}");
    }
    let many: Vec<_> = (0..17)
        .map(|i| json!({"name":format!("w{i}"),"selectors":["unix:uid:1"]}))
        .collect();
    assert!(invalid(json!({ "workloads": many })).contains("at most 16"));
    // The UID becomes part of SPIFFE IDs and entry names.
    let mut odd = vm(Some(two_workloads()));
    odd.metadata.uid = Some("vm-uid".into());
    assert!(matches!(
        guest_identity::render(&odd, DEFAULT_DEVID_CA_CN),
        Err(Error::Invalid(m)) if m.contains("UUID")
    ));
    assert!(matches!(
        guest_identity::render(&vm(Some(two_workloads())), "bad:cn"),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn the_crd_bounds_guest_identity() {
    let crd: Value = serde_json::from_str(include_str!(
        "../../../contrib/kubernetes/operator/crd.json"
    ))
    .unwrap();
    let schema = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["guestIdentity"];
    assert!(schema.get("nullable").is_none());
    let workloads = &schema["properties"]["workloads"];
    assert_eq!(workloads["maxItems"], 16);
    assert_eq!(workloads["x-kubernetes-list-type"], "map");
    assert_eq!(workloads["x-kubernetes-list-map-keys"], json!(["name"]));
    let item = &workloads["items"]["properties"];
    assert_eq!(
        item["name"]["pattern"],
        "^[a-z0-9]([a-z0-9-]{0,38}[a-z0-9])?$"
    );
    assert_eq!(item["selectors"]["minItems"], 1);
    assert_eq!(item["selectors"]["maxItems"], 8);
    assert_eq!(item["selectors"]["x-kubernetes-list-type"], "set");
    assert_eq!(
        item["selectors"]["items"]["pattern"],
        "^(unix|systemd):[a-z_]{1,32}:[!-~]{1,200}$"
    );
    assert_eq!(
        serde_json::to_value(qrab_operator::api::crd()).unwrap(),
        crd,
        "regenerate crd.json with `cargo run -p qrab-operator -- crd`"
    );
}

#[derive(Clone, Default)]
struct Store {
    objects: Arc<Mutex<BTreeMap<String, Value>>>,
    entry_requests: Arc<Mutex<usize>>,
}

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
    if uri.path().starts_with(ENTRIES) {
        *store.entry_requests.lock().unwrap() += 1;
    }
    let mut objects = store.objects.lock().unwrap();
    let path = uri.path().trim_end_matches("/status").to_owned();
    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let collection = [VMS, PODS, POLICIES, CONFIGS, MIGRATIONS, ENTRIES].contains(&path.as_str());
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

async fn server(store: &Store) -> Arc<Context> {
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
        settings: settings(),
        metrics: None,
        storage: None,
    })
}

impl Store {
    fn with(objects: impl IntoIterator<Item = (String, Value)>) -> Self {
        let store = Self::default();
        store.objects.lock().unwrap().extend(objects);
        store
    }
    fn get(&self, path: &str) -> Option<Value> {
        self.objects.lock().unwrap().get(path).cloned()
    }
    fn edit(&self, path: &str, change: impl FnOnce(&mut Value)) {
        change(self.objects.lock().unwrap().get_mut(path).unwrap());
    }
    async fn step(&self, ctx: &Arc<Context>) {
        let vm: VirtualMachine = serde_json::from_value(self.get(VM).unwrap()).unwrap();
        reconcile(Arc::new(vm), ctx.clone()).await.unwrap();
    }
    /// The entries' names and specs.
    fn entries(&self) -> BTreeMap<String, Value> {
        self.objects
            .lock()
            .unwrap()
            .iter()
            .filter_map(|(k, v)| {
                let name = k.strip_prefix(&format!("{ENTRIES}/"))?;
                Some((name.to_owned(), v["spec"].clone()))
            })
            .collect()
    }
    fn finalizers(&self) -> Value {
        self.get(VM).unwrap()["metadata"]["finalizers"].clone()
    }
}

fn state_pvc() -> (String, Value) {
    (
        "/api/v1/namespaces/qrab/persistentvolumeclaims/state".to_owned(),
        json!({"apiVersion":"v1","kind":"PersistentVolumeClaim","metadata":{"name":"state","namespace":"qrab","uid":"pvc-uid"},
            "spec":{"accessModes":["ReadWriteOncePod"],"volumeMode":"Filesystem"},"status":{"phase":"Bound"}}),
    )
}

/// An entry of another VM, which no step may touch.
fn foreign() -> (String, Value) {
    let name = format!("qrab-guest-node-{OTHER_UID}");
    let mut value = alias();
    value["metadata"]["name"] = json!(name);
    value["metadata"]["labels"] = labels(OTHER_UID);
    (format!("{ENTRIES}/{name}"), value)
}

fn spec(value: Value) -> Value {
    value["spec"].clone()
}

#[tokio::test]
async fn entries_follow_the_workloads_and_go_with_the_vm() {
    let store = Store::with([
        (VM.to_owned(), vm_json(Some(two_workloads()))),
        state_pvc(),
        foreign(),
    ]);
    let ctx = server(&store).await;
    let alias_name = format!("qrab-guest-node-{UID}");
    let web = format!("qrab-guest-{UID}-web");
    let backup = format!("qrab-guest-{UID}-backup");

    // The finalizer is recorded before the first entry.
    store.step(&ctx).await;
    assert_eq!(store.finalizers(), json!([FINALIZER]));
    assert_eq!(store.entries().len(), 1);
    store.step(&ctx).await;
    let entries = store.entries();
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[&alias_name], spec(alias()));
    assert_eq!(
        entries[&web],
        spec(workload(
            "web",
            json!(["unix:uid:1000", "systemd:id:web.service"])
        ))
    );
    assert_eq!(
        entries[&backup],
        spec(workload("backup", json!(["unix:user:backup"])))
    );
    assert_eq!(
        store.get(&format!("{ENTRIES}/{web}")).unwrap()["metadata"]["labels"],
        labels(UID)
    );

    // A changed selector is applied in place; a removed workload's entry goes.
    store.edit(VM, |vm| {
        vm["spec"]["guestIdentity"] =
            json!({"workloads":[{"name":"web","selectors":["unix:uid:1001"]}]})
    });
    store.step(&ctx).await;
    let entries = store.entries();
    assert!(!entries.contains_key(&backup));
    assert_eq!(
        entries[&web],
        spec(workload("web", json!(["unix:uid:1001"])))
    );
    assert_eq!(entries.len(), 3);

    // A renamed workload: the old entry goes, the new one comes.
    store.edit(VM, |vm| {
        vm["spec"]["guestIdentity"]["workloads"][0]["name"] = json!("api")
    });
    store.step(&ctx).await;
    let entries = store.entries();
    assert!(!entries.contains_key(&web));
    assert_eq!(
        entries[&format!("qrab-guest-{UID}-api")],
        spec(workload("api", json!(["unix:uid:1001"])))
    );
    assert_eq!(entries.len(), 3);

    // Deleting the VM deletes its entries, then releases it.
    store.edit(VM, |vm| {
        vm["metadata"]["deletionTimestamp"] = json!("2026-11-16T00:00:00Z")
    });
    store.step(&ctx).await;
    assert_eq!(
        store.entries().into_keys().collect::<Vec<_>>(),
        vec![foreign().0.rsplit('/').next().unwrap().to_owned()]
    );
    assert_eq!(store.finalizers(), json!([]));
}

#[tokio::test]
async fn removing_guest_identity_deletes_the_entries_and_the_finalizer() {
    let store = Store::with([
        (VM.to_owned(), vm_json(Some(two_workloads()))),
        state_pvc(),
        foreign(),
    ]);
    let ctx = server(&store).await;
    store.step(&ctx).await;
    store.step(&ctx).await;
    assert_eq!(store.entries().len(), 4);
    store.edit(VM, |vm| {
        vm["spec"].as_object_mut().unwrap().remove("guestIdentity");
    });
    store.step(&ctx).await;
    assert_eq!(store.entries().len(), 1);
    assert_eq!(store.finalizers(), json!([]));
}

#[tokio::test]
async fn a_vm_without_guest_identity_never_touches_entries() {
    let store = Store::with([(VM.to_owned(), vm_json(None)), state_pvc(), foreign()]);
    let ctx = server(&store).await;
    store.step(&ctx).await;
    store.step(&ctx).await;
    assert_eq!(*store.entry_requests.lock().unwrap(), 0);
    assert!(store.finalizers().is_null());
}

#[tokio::test]
async fn an_unowned_entry_of_the_same_name_is_not_taken_over() {
    let alias_path = format!("{ENTRIES}/qrab-guest-node-{UID}");
    let mut squatter = alias();
    squatter["metadata"]
        .as_object_mut()
        .unwrap()
        .remove("labels");
    squatter["spec"]["parentID"] = json!("spiffe://yapplesauce.com/elsewhere");
    let store = Store::with([
        (VM.to_owned(), vm_json(Some(two_workloads()))),
        state_pvc(),
        (alias_path.clone(), squatter.clone()),
    ]);
    let ctx = server(&store).await;
    store.step(&ctx).await;
    store.step(&ctx).await;
    let vm = store.get(VM).unwrap();
    assert_eq!(vm["status"]["phase"], "Blocked");
    assert_eq!(vm["status"]["reason"], "OwnershipConflict");
    assert_eq!(store.get(&alias_path).unwrap(), squatter);
    assert!(store.get(&format!("{PODS}/test-vm")).is_none());
}

#[tokio::test]
async fn the_sweep_deletes_only_entries_of_vanished_vms_in_its_namespace() {
    let mut elsewhere = foreign();
    elsewhere.0 = format!("{ENTRIES}/qrab-guest-node-{}", "2".repeat(36));
    elsewhere.1["metadata"]["labels"]["qrab.yapplesauce.com/vm-namespace"] = json!("other");
    let store = Store::with([
        (VM.to_owned(), vm_json(Some(two_workloads()))),
        state_pvc(),
        foreign(),
        elsewhere.clone(),
    ]);
    let ctx = server(&store).await;
    store.step(&ctx).await;
    store.step(&ctx).await;
    assert_eq!(store.entries().len(), 5);
    guest_identity::sweep(&ctx.client, "qrab").await.unwrap();
    let entries = store.entries();
    assert_eq!(entries.len(), 4);
    assert!(!entries.contains_key(foreign().0.rsplit('/').next().unwrap()));
    assert!(store.get(&elsewhere.0).is_some());
}
