//! The NIC reconcile loop against a small in-memory API server: NIC Pods and
//! relay policies come first, the VM waits as NicPending until every NIC Pod
//! is Ready, then boots with their IPs; security groups change in place and
//! removed NICs are cleaned up.
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
    let collection = [PODS, POLICIES, CONFIGS].contains(&path.as_str());
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
            let Some(object) = objects.get_mut(&path) else {
                return status(404, "NotFound");
            };
            if apply {
                let metadata = object["metadata"].clone();
                *object = body;
                object["metadata"]["uid"] = metadata["uid"].clone();
            } else {
                let mut patch = body;
                if let Some(metadata) = patch["metadata"].as_object_mut() {
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

#[tokio::test]
async fn vm_waits_for_ready_nic_pods_then_boots_with_their_relay_addresses() {
    let store = Store::default();
    store.0.lock().unwrap().extend([
        (VM.to_owned(), json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
            "metadata":{"name":"test-vm","namespace":"qrab","uid":"vm-uid","generation":1,"resourceVersion":"1"},
            "spec":{"stateVolumeClaim":"state","components":{"bmc":{"enabled":false}},
                "nics":[{"name":"lan","securityGroups":["web"]}]},
            "status":{"observedGeneration":1,"phase":"Pending","ready":false,"reason":"","message":"","stateVolumeUid":"pvc-uid"}})),
        ("/api/v1/namespaces/qrab/persistentvolumeclaims/state".to_owned(),
            json!({"apiVersion":"v1","kind":"PersistentVolumeClaim","metadata":{"name":"state","namespace":"qrab","uid":"pvc-uid"},
                "spec":{"accessModes":["ReadWriteOncePod"],"volumeMode":"Filesystem"},"status":{"phase":"Bound"}})),
    ]);
    let ctx = server(&store).await;
    let nic_pod = format!("{PODS}/test-vm-nic-lan");
    let policy = format!("{POLICIES}/test-vm-nic-lan-relay");
    let mac = qrab_operator::nic::mac("vm-uid", "lan");

    // First pass: the relay policy and NIC Pod exist, status records the NIC.
    store.step(&ctx).await;
    assert!(store.get(&policy).is_some());
    let pod = store.get(&nic_pod).unwrap();
    assert_eq!(pod["metadata"]["labels"]["sg.qrab.yapplesauce.com/web"], "");
    assert_eq!(
        store.status()["nics"],
        json!([{"name":"lan","mac":mac,"podName":"test-vm-nic-lan","ip":null,"ready":false}])
    );

    // The VM Pod is not created while the NIC Pod is not Ready.
    store.step(&ctx).await;
    assert_eq!(store.status()["reason"], "NicPending");
    assert!(store.get(&format!("{PODS}/test-vm")).is_none());
    assert!(store.get(&format!("{CONFIGS}/test-vm-config")).is_none());

    // Ready with an IP: status, then the VM's config carries the relay address.
    store.edit(&nic_pod, |pod| {
        pod["status"] = json!({"phase":"Running","podIP":"10.0.3.7","conditions":[{"type":"Ready","status":"True"}]});
    });
    store.step(&ctx).await;
    assert_eq!(store.status()["nics"][0]["ip"], "10.0.3.7");
    assert_eq!(store.status()["nics"][0]["ready"], true);
    store.step(&ctx).await;
    let config = store.get(&format!("{CONFIGS}/test-vm-config")).unwrap();
    let config: Value = serde_json::from_str(config["data"]["vm.json"].as_str().unwrap()).unwrap();
    assert_eq!(
        config["nics"],
        json!([{"id":"lan","mac":mac,"host":"10.0.3.7","port":7700}])
    );
    // Static addressing through the seed, even without spec.cloudInit.
    let network = config["seed"]["network_config"].as_str().unwrap();
    assert!(network.contains("\"10.0.3.7/32\"") && network.contains(&mac));
    assert!(store.get(&format!("{PODS}/test-vm")).is_some());
    assert_eq!(store.status()["reason"], "PodCreated");

    // Security groups change on the live NIC Pod without replacing it.
    store.edit(VM, |vm| {
        vm["spec"]["nics"][0]["securityGroups"] = json!(["db"]);
    });
    store.step(&ctx).await;
    let labels = store.get(&nic_pod).unwrap()["metadata"]["labels"].clone();
    assert_eq!(labels["sg.qrab.yapplesauce.com/db"], "");
    assert!(labels.get("sg.qrab.yapplesauce.com/web").is_none());
    assert_eq!(
        store.get(&nic_pod).unwrap()["metadata"]["uid"],
        "uid-test-vm-nic-lan"
    );

    // Removing the NIC removes its Pod and policy and clears the status.
    store.edit(VM, |vm| {
        vm["spec"].as_object_mut().unwrap().remove("nics");
    });
    store.step(&ctx).await;
    assert!(store.get(&nic_pod).is_none());
    assert!(store.get(&policy).is_none());
    assert_eq!(store.status()["nics"], json!([]));
}
