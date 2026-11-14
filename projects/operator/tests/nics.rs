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
const POOLS: &str = "/apis/cilium.io/v2alpha1/ciliumpodippools";
const ROUTES: &str = "/apis/frrk8s.metallb.io/v1beta1/namespaces/frr-k8s-system/frrconfigurations";
const CILIUM_NODES: &str = "/apis/cilium.io/v2/ciliumnodes";
const NODES: &str = "/api/v1/nodes";
const MIGRATIONS: &str =
    "/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/virtualmachinemigrations";

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

fn addressed() -> Settings {
    Settings {
        nic_addresses: Some(qrab_operator::address::Addressing {
            range: "10.244.128.0/29".parse().unwrap(),
            frr_namespace: "frr-k8s-system".into(),
        }),
        ..settings()
    }
}

fn base_route(node: &str, id: &str) -> Value {
    json!({"apiVersion":"frrk8s.metallb.io/v1beta1","kind":"FRRConfiguration",
        "metadata":{"name":node,"namespace":"frr-k8s-system","uid":format!("uid-{node}"),
            "labels":{"qrab.yapplesauce.com/nic-route-base":"true"}},
        "spec":{"nodeSelector":{"matchLabels":{"kubernetes.io/hostname":node}},
            "bgp":{"routers":[{"asn":64512,"id":id,"prefixes":["10.244.0.0/24"],"neighbors":[
                {"address":"10.203.0.12","asn":64512,
                 "toAdvertise":{"allowed":{"mode":"filtered","prefixes":["10.244.0.0/24"]}},
                 "toReceive":{"allowed":{"mode":"filtered","prefixes":[{"prefix":"10.244.128.0/17","ge":32,"le":32}]}}},
                {"address":"10.203.0.1","asn":64513,"toAdvertise":{"allowed":{"mode":"all"}}}]}]}}})
}

fn node(name: &str, ready: bool) -> Value {
    json!({"apiVersion":"v1","kind":"Node","metadata":{"name":name,"uid":format!("uid-{name}"),
        "labels":{"kubernetes.io/hostname":name}},
        "status":{"conditions":[{"type":"Ready","status":if ready {"True"} else {"Unknown"}}]}})
}

fn cilium_node(name: &str, pools: &[&str]) -> Value {
    let allocated: Vec<_> = pools
        .iter()
        .map(|p| json!({"pool":p,"cidrs":["10.244.0.0/24"]}))
        .collect();
    let requested: Vec<_> = pools
        .iter()
        .map(|p| json!({"pool":p,"needed":{"ipv4-addrs":1}}))
        .collect();
    json!({"apiVersion":"cilium.io/v2","kind":"CiliumNode","metadata":{"name":name,"uid":format!("uid-cn-{name}"),"resourceVersion":"7"},
        "spec":{"ipam":{"pools":{"allocated":allocated,"requested":requested}}}})
}

fn addressed_vm() -> Value {
    json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
        "metadata":{"name":"test-vm","namespace":"qrab","uid":"vm-uid","generation":1,"resourceVersion":"1"},
        "spec":{"stateVolumeClaim":"state","components":{"bmc":{"enabled":false}},
            "nics":[{"name":"lan"}]},
        "status":{"observedGeneration":1,"phase":"Pending","ready":false,"reason":"","message":"","stateVolumeUid":"pvc-uid"}})
}

fn running_on<'a>(node: &'a str, ip: &'a str) -> impl FnOnce(&mut Value) + 'a {
    move |pod| {
        pod["spec"]["nodeName"] = json!(node);
        pod["status"] =
            json!({"phase":"Running","podIP":ip,"conditions":[{"type":"Ready","status":"True"}]});
    }
}

#[tokio::test]
async fn a_nic_keeps_its_claimed_address_and_its_route_follows_the_pod() {
    let store = Store::default();
    store.0.lock().unwrap().extend([
        (VM.to_owned(), addressed_vm()),
        // Claimed outside qrab's labels: the create collides and moves on.
        (
            format!("{POOLS}/qrab-nic-10-244-128-1"),
            json!({"metadata":{"name":"qrab-nic-10-244-128-1"}}),
        ),
        (
            format!("{ROUTES}/kube01"),
            base_route("kube01", "10.203.0.11"),
        ),
        (
            format!("{ROUTES}/kube02"),
            base_route("kube02", "10.203.0.12"),
        ),
        (format!("{NODES}/kube01"), node("kube01", true)),
        (format!("{NODES}/kube02"), node("kube02", true)),
        (
            format!("{CILIUM_NODES}/kube01"),
            cilium_node("kube01", &["default"]),
        ),
        (
            format!("{CILIUM_NODES}/kube02"),
            cilium_node("kube02", &["default"]),
        ),
    ]);
    let ctx = server_with(&store, addressed()).await;
    let nic_pod = format!("{PODS}/test-vm-nic-lan");
    let pool = format!("{POOLS}/qrab-nic-10-244-128-2");
    let route = format!("{ROUTES}/qrab-nic-10-244-128-2");

    // The VM holds the address finalizer before anything is claimed.
    store.step(&ctx).await;
    assert_eq!(
        store.get(VM).unwrap()["metadata"]["finalizers"],
        json!(["qrab.yapplesauce.com/nic-address"])
    );
    assert!(store.get(&pool).is_none());

    // The lowest free address is claimed and the NIC Pod pinned to its pool.
    store.step(&ctx).await;
    let claimed = store.get(&pool).unwrap();
    assert_eq!(
        claimed["spec"],
        json!({"ipv4":{"cidrs":["10.244.128.2/32"],"maskSize":32}})
    );
    assert_eq!(
        claimed["metadata"]["finalizers"],
        json!(["qrab.yapplesauce.com/nic-address"])
    );
    assert_eq!(
        claimed["metadata"]["labels"]["qrab.yapplesauce.com/owner-uid"],
        "vm-uid"
    );
    let annotations = store.get(&nic_pod).unwrap()["metadata"]["annotations"].clone();
    assert_eq!(
        annotations["ipam.cilium.io/ip-pool"],
        "qrab-nic-10-244-128-2"
    );
    assert_eq!(annotations["ipam.cilium.io/require-pool-match"], "true");
    assert_eq!(store.status()["nics"][0]["address"], "10.244.128.2");
    assert!(
        store.get(&route).is_none(),
        "no route before a Pod holds the address"
    );

    // A Pod with some other address is not this NIC's endpoint.
    store.edit(&nic_pod, running_on("kube01", "10.244.0.9"));
    store.step(&ctx).await;
    assert_eq!(store.status()["nics"][0]["ready"], false);
    assert!(store.get(&route).is_none());

    // Holding the address on kube01: announced from kube01 only, to the base's
    // neighbors, with no receive filters of its own.
    store.edit(&nic_pod, running_on("kube01", "10.244.128.2"));
    store.step(&ctx).await;
    assert_eq!(store.status()["nics"][0]["ip"], "10.244.128.2");
    let announced = store.get(&route).unwrap();
    assert_eq!(
        announced["spec"],
        json!({
        "nodeSelector":{"matchLabels":{"kubernetes.io/hostname":"kube01"}},
        "bgp":{"routers":[{"asn":64512,"prefixes":["10.244.128.2/32"],"neighbors":[
            {"address":"10.203.0.12","asn":64512,"toAdvertise":{"allowed":{"mode":"filtered","prefixes":["10.244.128.2/32"]}}},
            {"address":"10.203.0.1","asn":64513,"toAdvertise":{"allowed":{"mode":"filtered","prefixes":["10.244.128.2/32"]}}}]}]}})
    );

    // The Pod comes back on kube02 with the same address: the route follows.
    store.edit(&nic_pod, running_on("kube02", "10.244.128.2"));
    store.step(&ctx).await;
    assert_eq!(
        store.get(&route).unwrap()["spec"]["nodeSelector"]["matchLabels"]["kubernetes.io/hostname"],
        "kube02"
    );

    // Rescheduled to kube02 but stuck: kube01 still holds the pool. A Ready
    // kube01 is left alone; once it is not Ready, the pool is fenced off it.
    store.edit(&format!("{CILIUM_NODES}/kube01"), |cn| {
        *cn = cilium_node("kube01", &["default", "qrab-nic-10-244-128-2"]);
    });
    store.edit(&nic_pod, |pod| {
        pod["spec"]["nodeName"] = json!("kube02");
        pod["status"] = json!({"phase":"Pending"});
    });
    store.step(&ctx).await;
    assert!(
        store.get(&route).is_none(),
        "withdrawn while no Pod holds the address"
    );
    let pools_of = |store: &Store, name: &str| -> Vec<String> {
        let cn = store.get(&format!("{CILIUM_NODES}/{name}")).unwrap();
        ["allocated", "requested"]
            .iter()
            .flat_map(|k| {
                cn["spec"]["ipam"]["pools"][k]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| e["pool"].as_str().unwrap().to_owned())
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    assert!(pools_of(&store, "kube01").contains(&"qrab-nic-10-244-128-2".to_owned()));
    store.edit(&format!("{NODES}/kube01"), |n| *n = node("kube01", false));
    store.step(&ctx).await;
    assert_eq!(pools_of(&store, "kube01"), ["default", "default"]);
    assert_eq!(pools_of(&store, "kube02"), ["default", "default"]);

    // Dropping the NIC: Pod first, then the route and pool, then the finalizer.
    store.edit(&nic_pod, running_on("kube02", "10.244.128.2"));
    store.step(&ctx).await;
    assert!(store.get(&route).is_some());
    store.edit(VM, |vm| {
        vm["spec"].as_object_mut().unwrap().remove("nics");
    });
    store.step(&ctx).await;
    assert!(store.get(&nic_pod).is_none());
    assert!(store.get(&pool).is_some(), "kept until the Pod is gone");
    store.step(&ctx).await;
    assert!(store.get(&pool).is_none());
    assert!(store.get(&route).is_none());
    assert!(
        store
            .get(&format!("{POOLS}/qrab-nic-10-244-128-1"))
            .is_some(),
        "foreign pools are untouched"
    );
    assert_eq!(store.get(VM).unwrap()["metadata"]["finalizers"], json!([]));
}

#[tokio::test]
async fn deleting_a_vm_stops_its_nic_pods_then_releases_their_addresses() {
    let store = Store::default();
    store.0.lock().unwrap().extend([
        (VM.to_owned(), addressed_vm()),
        (
            format!("{ROUTES}/kube01"),
            base_route("kube01", "10.203.0.11"),
        ),
        (format!("{NODES}/kube01"), node("kube01", true)),
    ]);
    let ctx = server_with(&store, addressed()).await;
    let nic_pod = format!("{PODS}/test-vm-nic-lan");
    let pool = format!("{POOLS}/qrab-nic-10-244-128-1");
    let route = format!("{ROUTES}/qrab-nic-10-244-128-1");
    store.step(&ctx).await;
    store.step(&ctx).await;
    store.edit(&nic_pod, running_on("kube01", "10.244.128.1"));
    store.step(&ctx).await;
    assert!(store.get(&route).is_some());

    store.edit(VM, |vm| {
        vm["metadata"]["deletionTimestamp"] = json!("2026-11-14T00:00:00Z")
    });
    store.step(&ctx).await;
    assert!(store.get(&nic_pod).is_none());
    assert!(store.get(&pool).is_some());
    store.step(&ctx).await;
    assert!(store.get(&pool).is_none());
    assert!(store.get(&route).is_none());
    assert_eq!(store.get(VM).unwrap()["metadata"]["finalizers"], json!([]));
}
