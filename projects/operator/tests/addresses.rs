//! Rendering and fencing decisions of stable NIC addresses.
use k8s_openapi::api::core::v1::Node;
use kube::api::DynamicObject;
use qrab_operator::address::{
    fence_patch, render_pool, render_route, selects, stranded, SKIP_MASQUERADE,
};
use qrab_operator::api::VirtualMachine;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn object(value: Value) -> DynamicObject {
    serde_json::from_value(value).unwrap()
}

fn pool() -> DynamicObject {
    object(
        json!({"apiVersion":"cilium.io/v2alpha1","kind":"CiliumPodIPPool",
        "metadata":{"name":"qrab-nic-10-244-128-9","labels":{"qrab.yapplesauce.com/nic":"lan"}}}),
    )
}

fn base(routers: Value, selector: Value) -> DynamicObject {
    object(
        json!({"apiVersion":"frrk8s.metallb.io/v1beta1","kind":"FRRConfiguration",
        "metadata":{"name":"kube01","namespace":"frr-k8s-system"},
        "spec":{"nodeSelector":selector,"bgp":{"routers":routers}}}),
    )
}

fn node(labels: Value, ready: &str) -> Node {
    serde_json::from_value(json!({"metadata":{"name":"kube01","labels":labels},
        "status":{"conditions":[{"type":"Ready","status":ready}]}}))
    .unwrap()
}

#[test]
fn routes_keep_the_base_vrf_and_neighbor_settings_but_only_send_the_nic_address() {
    let base = base(
        json!([{"asn":64512,"vrf":"red","id":"10.203.0.11","neighbors":[
            {"address":"10.203.0.1","asn":64513,"holdTime":"9s","toReceive":{"allowed":{"mode":"all"}}}]}]),
        json!({"matchLabels":{"kubernetes.io/hostname":"kube01"}}),
    );
    let route = render_route(&base, &pool(), "frr").unwrap();
    assert_eq!(route.metadata.namespace.as_deref(), Some("frr"));
    assert_eq!(
        route.data["spec"]["bgp"]["routers"],
        json!([{"asn":64512,"vrf":"red","prefixes":["10.244.128.9/32"],"neighbors":[
            {"address":"10.203.0.1","asn":64513,"holdTime":"9s",
             "toAdvertise":{"allowed":{"mode":"filtered","prefixes":["10.244.128.9/32"]}}}]}])
    );
}

#[test]
fn ambiguous_bases_are_refused() {
    let selector = json!({"matchLabels":{"kubernetes.io/hostname":"kube01"}});
    let two = base(json!([{"asn":1},{"asn":2}]), selector.clone());
    assert!(render_route(&two, &pool(), "frr").is_err());
    let expressions = base(
        json!([{"asn":1}]),
        json!({"matchExpressions":[{"key":"kubernetes.io/hostname","operator":"In","values":["kube01"]}]}),
    );
    assert!(render_route(&expressions, &pool(), "frr").is_err());
    let host = node(json!({"kubernetes.io/hostname":"kube01"}), "True");
    assert!(!selects(&expressions, &host));
    assert!(selects(&base(json!([{"asn":1}]), selector.clone()), &host));
    assert!(!selects(
        &base(json!([{"asn":1}]), selector),
        &node(json!({"kubernetes.io/hostname":"kube02"}), "True")
    ));
}

#[test]
fn only_dead_or_missing_nodes_without_a_bound_pod_lose_the_pool() {
    let held = |name: &str| {
        object(json!({"apiVersion":"cilium.io/v2","kind":"CiliumNode",
            "metadata":{"name":name,"resourceVersion":"3"},
            "spec":{"ipam":{"pools":{
                "allocated":[{"pool":"default"},{"pool":"qrab-nic-10-244-128-9"}],
                "requested":[{"pool":"qrab-nic-10-244-128-9"}]}}}}))
    };
    let cilium_nodes = [held("kube01"), held("gone"), held("bound")];
    let nodes = [
        node(json!({}), "False"),
        serde_json::from_value(json!({"metadata":{"name":"bound"},"status":{"conditions":[{"type":"Ready","status":"False"}]}})).unwrap(),
    ];
    let bound = BTreeSet::from(["bound".to_owned()]);
    let names: Vec<_> = stranded("qrab-nic-10-244-128-9", &cilium_nodes, &nodes, &bound)
        .iter()
        .map(|cn| cn.metadata.name.clone().unwrap())
        .collect();
    assert_eq!(names, ["kube01", "gone"]);
    assert!(stranded("other", &cilium_nodes, &nodes, &bound).is_empty());
    assert_eq!(
        fence_patch(&cilium_nodes[0], "qrab-nic-10-244-128-9"),
        json!({"metadata":{"resourceVersion":"3"},"spec":{"ipam":{"pools":{
            "allocated":[{"pool":"default"}],"requested":[]}}}})
    );
}

#[test]
fn nic_pools_never_masquerade_their_guest() {
    let vm: VirtualMachine = serde_json::from_value(json!({
        "apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
        "metadata":{"name":"test-vm","namespace":"qrab","uid":"vm-uid"},
        "spec":{"stateVolumeClaim":"state","nics":[{"name":"lan"}]}}))
    .unwrap();
    let pool = render_pool(&vm, "lan", "10.244.128.9".parse().unwrap()).unwrap();
    let annotations = pool.metadata.annotations.unwrap();
    assert_eq!(
        annotations.get(SKIP_MASQUERADE).map(String::as_str),
        Some("true")
    );
    assert_eq!(SKIP_MASQUERADE, "ipam.cilium.io/skip-masquerade");
}
