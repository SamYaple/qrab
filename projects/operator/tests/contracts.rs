use kube::ResourceExt;
use qrab_operator::{
    api::{self, VirtualMachine},
    resources::{self, Settings},
};
use serde_json::{json, Value};

fn settings() -> Settings {
    Settings {
        namespace: "qrab".into(),
        vm_image: format!("zot.yapplesauce.com/qrab/manager@sha256:{}", "a".repeat(64)),
        oidc_issuer: "https://idm.yaple.net/oauth2/openid/qrab-bmc".into(),
        oidc_client_id: "qrab-bmc".into(),
        idp_ip: Some("192.168.0.251".into()),
    }
}
fn vm() -> VirtualMachine {
    serde_json::from_value(json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine","metadata":{"name":"test-vm","namespace":"qrab","uid":"owner-one","generation":1},"spec":{"stateVolumeClaim":"test-state","console":{"publicUrl":"https://test-vm.yapplesauce.com","allowedSubjects":["6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b"]}}})).unwrap()
}

#[test]
fn vm_pods_require_persistent_uefi_tpm_and_keep_api_credentials_out() {
    let desired = resources::render(&vm(), &settings()).unwrap();
    let config: Value = serde_json::from_str(&desired.config.data.unwrap()["vm.json"]).unwrap();
    assert!(config["uefi"].is_object() && config["tpm"].is_object());
    assert_eq!(config["state_id"], "owner-one");
    assert_eq!(config["state_dir"], "/var/lib/qrab");
    for field in ["kernel", "initrd", "kernel_args", "network"] {
        assert!(config.get(field).is_none());
    }
    let pod = serde_json::to_value(desired.pod).unwrap();
    assert_eq!(pod["spec"]["automountServiceAccountToken"], false);
    assert_eq!(
        pod["spec"]["securityContext"]["fsGroupChangePolicy"],
        "OnRootMismatch"
    );
    assert_eq!(
        pod["spec"]["containers"][0]["securityContext"]["capabilities"]["drop"],
        json!(["ALL"])
    );
    assert_eq!(
        pod["spec"]["volumes"][1]["persistentVolumeClaim"]["claimName"],
        "test-state"
    );
    assert!(pod["spec"]["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v.get("projected").is_none() && v.get("hostPath").is_none()));
}

#[test]
fn owner_uid_prevents_adopting_an_unrelated_resource_with_the_same_name() {
    let owner = vm();
    let desired = resources::render(&owner, &settings()).unwrap();
    assert!(resources::owned(&desired.pod, &owner));
    let mut replacement = vm();
    replacement.metadata.uid = Some("owner-two".into());
    assert!(!resources::owned(&desired.pod, &replacement));
    let mut unowned = desired.pod;
    unowned.metadata.owner_references = None;
    assert!(!resources::owned(&unowned, &owner));
}

#[test]
fn api_server_defaults_and_canonical_quantities_do_not_restart_a_vm() {
    let desired = resources::render(&vm(), &settings()).unwrap();
    let mut stored = serde_json::to_value(&desired.pod).unwrap();
    for name in ["hostNetwork", "hostPID", "hostIPC"] {
        stored["spec"].as_object_mut().unwrap().remove(name);
    }
    stored["spec"]["nodeName"] = json!("kube03");
    stored["spec"]["dnsPolicy"] = json!("ClusterFirst");
    stored["spec"]["containers"][0]["startupProbe"]["httpGet"]["scheme"] = json!("HTTP");
    stored["spec"]["tolerations"].as_array_mut().unwrap().push(json!({"key":"node.kubernetes.io/not-ready","operator":"Exists","effect":"NoExecute","tolerationSeconds":300}));
    assert_eq!(
        stored["spec"]["containers"][0]["resources"]["limits"]["memory"],
        "1Gi"
    );
    let stored = serde_json::from_value(stored).unwrap();
    assert!(resources::pod_matches(&stored, &desired));
}

#[test]
fn mutable_image_or_security_or_selector_drift_is_detected() {
    let desired = resources::render(&vm(), &settings()).unwrap();
    let mut stored = desired.pod.clone();
    stored.spec.as_mut().unwrap().containers[0].image = Some(format!(
        "zot.yapplesauce.com/qrab/manager@sha256:{}",
        "b".repeat(64)
    ));
    assert!(!resources::pod_matches(&stored, &desired));
    let mut stored = desired.pod.clone();
    stored
        .spec
        .as_mut()
        .unwrap()
        .automount_service_account_token = Some(true);
    assert!(!resources::pod_matches(&stored, &desired));
    let mut stored = desired.pod.clone();
    stored.spec.as_mut().unwrap().host_network = Some(true);
    assert!(!resources::pod_matches(&stored, &desired));
    let mut stored = desired.pod.clone();
    stored.labels_mut().remove("app.kubernetes.io/instance");
    assert!(!resources::pod_matches(&stored, &desired));
    let mut stored = desired.pod.clone();
    stored.spec.as_mut().unwrap().containers[0]
        .volume_mounts
        .as_mut()
        .unwrap()[1]
        .sub_path = Some("fresh-identity".into());
    assert!(!resources::pod_matches(&stored, &desired));
    let mut stored = desired.pod.clone();
    stored.spec.as_mut().unwrap().containers[0]
        .volume_mounts
        .as_mut()
        .unwrap()[1]
        .read_only = Some(true);
    assert!(!resources::pod_matches(&stored, &desired));
}

#[test]
fn metadata_and_status_updates_do_not_change_the_template_but_restart_nonce_does() {
    let mut vm = vm();
    let first = resources::render(&vm, &settings()).unwrap();
    vm.metadata.generation = Some(30);
    vm.metadata.resource_version = Some("100".into());
    assert_eq!(
        resources::render(&vm, &settings()).unwrap().hash,
        first.hash
    );
    vm.spec.restart_nonce = "restart-once".into();
    let next = resources::render(&vm, &settings()).unwrap();
    assert_ne!(next.hash, first.hash);
    assert_eq!(
        next.config.data, first.config.data,
        "restart must preserve the persistent state identity"
    );
    vm.spec.memory_mib = 768;
    assert_ne!(resources::render(&vm, &settings()).unwrap().hash, next.hash);
}

#[test]
fn invalid_unsafe_or_incomplete_specs_are_rejected_before_pod_creation() {
    let mut vm = vm();
    vm.spec.console.allowed_subjects.clear();
    assert!(resources::render(&vm, &settings()).is_err());
    vm = self::vm();
    vm.spec.console.public_url = "http://test-vm.yapplesauce.com".into();
    assert!(resources::render(&vm, &settings()).is_err());
    vm = self::vm();
    vm.spec.image = Some("docker.io/untrusted/vm:latest".into());
    assert!(resources::render(&vm, &settings()).is_err());
    vm = self::vm();
    vm.spec.boot_disk = "relative/path".into();
    assert!(resources::render(&vm, &settings()).is_err());
    vm = self::vm();
    vm.spec.state_volume_claim = "../other-volume".into();
    assert!(resources::render(&vm, &settings()).is_err());
    vm = self::vm();
    vm.spec.memory_mib = 0;
    assert!(resources::render(&vm, &settings()).is_err());
}

#[test]
fn crd_is_namespaced_and_schema_bounds_and_status_are_published() {
    let crd = api::crd();
    assert_eq!(crd["spec"]["scope"], "Namespaced");
    assert!(crd["spec"]["versions"][0]["subresources"]["status"].is_object());
    let spec = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"];
    assert_eq!(spec["memoryMiB"]["minimum"].as_f64(), Some(256.0));
    assert_eq!(spec["cpus"]["maximum"].as_f64(), Some(256.0));
    assert_eq!(
        spec["stateVolumeClaim"]["x-kubernetes-validations"][0]["rule"],
        "self == oldSelf"
    );
    assert_eq!(
        spec["console"]["properties"]["allowedSubjects"]["minItems"],
        1
    );
}

#[test]
fn stopped_status_explicitly_clears_pod_links_in_a_merge_patch() {
    let status = api::VirtualMachineStatus::default();
    let wire = serde_json::to_value(status).unwrap();
    for field in ["podName", "podUid", "nodeName"] {
        assert!(wire.get(field).is_some_and(Value::is_null));
    }
}

#[tokio::test]
#[ignore = "requires a Kubernetes namespace with the qrab-manager identity and create/use permission; server dry run only"]
async fn live_api_defaulting_preserves_the_declared_vm_template() {
    let client = kube::Client::try_default().await.unwrap();
    let mut settings = settings();
    settings.namespace = std::env::var("QRAB_TEST_NAMESPACE").unwrap_or_else(|_| "qrab".into());
    let desired = resources::render(&vm(), &settings).unwrap();
    let pods: kube::Api<k8s_openapi::api::core::v1::Pod> =
        kube::Api::namespaced(client, &settings.namespace);
    let stored = pods
        .create(
            &kube::api::PostParams {
                dry_run: true,
                ..Default::default()
            },
            &desired.pod,
        )
        .await
        .unwrap();
    assert!(
        resources::pod_matches(&stored, &desired),
        "API-server defaults must not cause a controller restart loop"
    );
}
