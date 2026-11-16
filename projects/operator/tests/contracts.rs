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
        redfish_oidc_issuer: "https://idm.yaple.net/oauth2/openid/qrab-redfish".into(),
        redfish_oidc_audience: "qrab-redfish".into(),
        bmc_bearer: None,
        nic_addresses: None,
        guest_devid_ca_cn: qrab_operator::guest_identity::DEFAULT_DEVID_CA_CN.into(),
        tpm_issuer: Default::default(),
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
    stored["spec"]["initContainers"][0]["readinessProbe"]["httpGet"]["scheme"] = json!("HTTP");
    stored["spec"]["initContainers"][0]["terminationMessagePath"] = json!("/dev/termination-log");
    stored["spec"]["containers"][0]["env"][0]["valueFrom"]["fieldRef"]["apiVersion"] = json!("v1");
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
    vm.spec.console.as_mut().unwrap().allowed_subjects.clear();
    assert!(resources::render(&vm, &settings()).is_err());
    vm = self::vm();
    vm.spec.console.as_mut().unwrap().public_url = Some("http://test-vm.yapplesauce.com".into());
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
    // Subjects, groups or a recorded creator may grant console access, so no
    // single list is required; validation demands at least one grant.
    let console = &spec["console"]["properties"];
    assert!(console["allowedSubjects"].get("minItems").is_none());
    assert_eq!(console["allowedSubjects"]["maxItems"], 128);
    let redfish = &spec["components"]["properties"]["redfish"]["properties"];
    for groups in [
        &console["adminGroups"],
        &console["viewerGroups"],
        &redfish["adminGroups"],
        &redfish["viewerGroups"],
    ] {
        assert_eq!(groups["maxItems"], 16);
    }
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

fn with_iscsi() -> VirtualMachine {
    let mut machine = vm();
    machine.spec.storage_node = Some("kube03".into());
    machine.spec.iscsi_disks=serde_json::from_value(json!([{"id":"qrab-probe","portal":"10.203.0.1:3260","target":"iqn.2026-11.com.yapplesauce:qrab.disk.demo","lun":0,"username":"qrab-demo","passwordSecret":{"name":"qrab-demo-iscsi"}}])).unwrap();
    machine
}

#[test]
fn native_iscsi_uses_secret_files_and_derived_initiator_without_block_devices() {
    let machine = with_iscsi();
    let desired = resources::render(&machine, &settings()).unwrap();
    let config: Value =
        serde_json::from_str(&desired.config.data.as_ref().unwrap()["vm.json"]).unwrap();
    let disk = &config["iscsi_disks"][0];
    assert_eq!(
        disk["initiator_name"],
        "iqn.2026-11.com.yapplesauce:qrab.vm.owner-one"
    );
    assert_eq!(disk["password_file"], "/run/qrab-auth/qrab-probe/password");
    assert!(disk.get("password").is_none());
    let pod = serde_json::to_value(&desired.pod).unwrap();
    assert_eq!(
        pod["spec"]["nodeSelector"]["kubernetes.io/hostname"],
        "kube03"
    );
    let auth = pod["spec"]["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "iscsi-auth-qrab-probe")
        .unwrap();
    assert_eq!(auth["secret"]["defaultMode"], 288);
    assert!(pod["spec"]["containers"][0].get("volumeDevices").is_none());
    assert!(resources::pod_matches(&desired.pod, &desired));
    let mut scheduled = desired.pod.clone();
    scheduled.spec.as_mut().unwrap().scheduling_gates = None;
    assert!(
        resources::pod_matches(&scheduled, &desired),
        "removing the ownership gate must not restart a VM"
    );
    let mut stored = desired.pod.clone();
    stored.spec.as_mut().unwrap().containers[0]
        .volume_mounts
        .as_mut()
        .unwrap()
        .iter_mut()
        .find(|m| m.name == "iscsi-auth-qrab-probe")
        .unwrap()
        .read_only = Some(false);
    assert!(!resources::pod_matches(&stored, &desired));
}

#[test]
fn native_disk_configuration_requires_pinning_and_valid_unique_endpoints() {
    let mut machine = with_iscsi();
    machine.spec.storage_node = None;
    assert!(resources::render(&machine, &settings()).is_err());
    machine = with_iscsi();
    machine.spec.iscsi_disks[0].portal = "iscsi://user:password@host".into();
    assert!(resources::render(&machine, &settings()).is_err());
    machine = with_iscsi();
    machine.spec.iscsi_disks[0].boot_index = Some(1);
    assert!(resources::render(&machine, &settings()).is_err());
    machine = with_iscsi();
    machine
        .spec
        .iscsi_disks
        .push(machine.spec.iscsi_disks[0].clone());
    assert!(resources::render(&machine, &settings()).is_err());
}

#[test]
fn boot_media_cannot_expose_host_secrets_or_tpm_state_to_the_guest() {
    for path in [
        "/run/qrab-auth/data/password",
        "/var/lib/qrab/identity/tpm/tpm2-00.permall",
        "/opt/qrab/demo/../../../run/qrab-auth/data/password",
    ] {
        let mut machine = with_iscsi();
        machine.spec.boot_disk = path.into();
        assert!(resources::render(&machine, &settings()).is_err());
    }
}

fn with_provisioned() -> (VirtualMachine, Vec<qrab_operator::provision::Attached>) {
    let mut machine = with_iscsi();
    machine.spec.disks = serde_json::from_value(
        json!([{"id":"root","sizeBytes":67108864,"baseImage":{"name":"debian","generation":3},"bootIndex":0}]),
    )
    .unwrap();
    let attached = vec![qrab_operator::provision::Attached {
        id: "root".into(),
        portal: "10.203.0.1:3260".into(),
        target: "iqn.2026-11.com.yapplesauce:qrab.disk.owner-one.root".into(),
        username: "qrab-ownerone-root".into(),
        secret_name: "test-vm-disk-root".into(),
        generation: 1,
        capacity_bytes: 67108864,
        read_only: false,
        boot_index: Some(0),
    }];
    (machine, attached)
}

#[test]
fn provisioned_disks_render_beside_hand_declared_ones_with_owned_secret_files() {
    let (machine, attached) = with_provisioned();
    // A provisioned disk is never rendered before the storage service exports it.
    assert!(resources::render(&machine, &settings()).is_err());
    let desired = resources::render_with(&machine, &settings(), &attached).unwrap();
    let config: Value =
        serde_json::from_str(&desired.config.data.as_ref().unwrap()["vm.json"]).unwrap();
    let disk = &config["iscsi_disks"][1];
    assert_eq!(config["iscsi_disks"][0]["id"], "qrab-probe");
    assert_eq!(
        disk["target"],
        "iqn.2026-11.com.yapplesauce:qrab.disk.owner-one.root"
    );
    assert_eq!(
        disk["initiator_name"],
        "iqn.2026-11.com.yapplesauce:qrab.vm.owner-one"
    );
    assert_eq!(disk["password_file"], "/run/qrab-auth/root/password");
    assert_eq!(disk["boot_index"], 0);
    assert!(!config.to_string().contains("password\":"));
    let pod = serde_json::to_value(&desired.pod).unwrap();
    let volume = pod["spec"]["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "iscsi-auth-root")
        .unwrap();
    assert_eq!(volume["secret"]["secretName"], "test-vm-disk-root");
    assert_eq!(volume["secret"]["items"][0]["key"], "password");
    assert_eq!(
        pod["metadata"]["finalizers"][0],
        "qrab.yapplesauce.com/iscsi-drain"
    );
    assert!(resources::pod_matches(&desired.pod, &desired));
}

#[test]
fn rotation_replaces_the_instance_growth_is_online_and_legacy_hashes_are_unchanged() {
    use kube::ResourceExt;
    let legacy = resources::render(&with_iscsi(), &settings()).unwrap();
    let (mut machine, attached) = with_provisioned();
    let base = resources::render_with(&machine, &settings(), &attached).unwrap();
    let mut rotated = attached.clone();
    rotated[0].generation = 2;
    let changed = resources::render_with(&machine, &settings(), &rotated).unwrap();
    assert!(!resources::pod_matches(&base.pod, &changed));
    // Growth keeps the template; only the projected capacity annotation moves.
    let mut grown = attached.clone();
    grown[0].capacity_bytes *= 2;
    let bigger = resources::render_with(&machine, &settings(), &grown).unwrap();
    assert!(resources::pod_matches(&base.pod, &bigger));
    assert_eq!(
        base.pod.annotations()[resources::DISK_CAPACITY],
        r#"{"root":67108864}"#
    );
    assert_eq!(
        bigger.pod.annotations()[resources::DISK_CAPACITY],
        r#"{"root":134217728}"#
    );
    let pod = serde_json::to_value(&base.pod).unwrap();
    let size = pod["spec"]["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "size")
        .unwrap();
    assert!(size["downwardAPI"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| {
            i["path"] == "disk_capacity"
                && i["fieldRef"]["fieldPath"]
                    == "metadata.annotations['qrab.yapplesauce.com/disk-capacity']"
        }));
    let legacy_pod = serde_json::to_value(&legacy.pod).unwrap();
    assert!(!legacy_pod.to_string().contains("disk_capacity"));
    machine.spec.disks.clear();
    assert_eq!(
        resources::render_with(&machine, &settings(), &[])
            .unwrap()
            .hash,
        legacy.hash
    );
}

#[test]
fn provisioned_disk_specs_are_bounded_unique_and_pinned() {
    let (machine, attached) = with_provisioned();
    for edit in [
        json!({"sizeBytes": 1048576}),
        json!({"sizeBytes": 67108865}),
        json!({"id": "qrab-probe"}),
        json!({"bootIndex": 1}),
        json!({"baseImage": {"name": "../x", "generation": 1}}),
        json!({"baseImage": {"name": "debian", "generation": 0}}),
    ] {
        let mut value = serde_json::to_value(&machine).unwrap();
        for (key, v) in edit.as_object().unwrap() {
            value["spec"]["disks"][0][key] = v.clone();
        }
        let bad: VirtualMachine = serde_json::from_value(value).unwrap();
        let mut renamed = attached.clone();
        renamed[0].id = bad.spec.disks[0].id.clone();
        assert!(
            resources::render_with(&bad, &settings(), &renamed).is_err(),
            "{edit}"
        );
    }
    let mut unpinned = machine.clone();
    unpinned.spec.storage_node = None;
    unpinned.spec.iscsi_disks.clear();
    assert!(resources::render_with(&unpinned, &settings(), &attached).is_err());
    let crd = api::crd();
    let disks = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["disks"];
    assert_eq!(disks["x-kubernetes-list-map-keys"], json!(["id"]));
    assert!(disks["items"]["x-kubernetes-validations"]
        .to_string()
        .contains("self.sizeBytes >= oldSelf.sizeBytes"));
}

const VIEWER: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

fn with_components(bmc: bool, redfish: bool) -> VirtualMachine {
    let mut machine = vm();
    machine.spec.components = serde_json::from_value(json!({
        "bmc":{"enabled":bmc},
        "redfish":{"enabled":redfish,"adminSubjects":["6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b"],"viewerSubjects":[VIEWER]}
    }))
    .unwrap();
    if !bmc {
        machine.spec.console = None;
    }
    machine
}

fn env(container: &Value) -> std::collections::BTreeMap<String, Value> {
    container["env"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let value = e
                .get("value")
                .cloned()
                .unwrap_or_else(|| e["valueFrom"].clone());
            (e["name"].as_str().unwrap().to_owned(), value)
        })
        .collect()
}

#[test]
fn existing_specs_keep_the_bmc_and_gain_its_redfish_sidecar() {
    let spec: api::VirtualMachineSpec = serde_json::from_value(
        json!({"stateVolumeClaim":"s","console":{"publicUrl":"https://x.example","allowedSubjects":[]}}),
    )
    .unwrap();
    assert!(spec.components.bmc.enabled);
    assert_eq!(spec.components.redfish.enabled, None);
    assert!(spec.components.redfish_enabled());
    // Objects stored under the old CRD carry its `enabled: false` default.
    let mut stored = vm();
    stored.spec.components.redfish.enabled = Some(false);
    for machine in [vm(), stored] {
        let pod =
            serde_json::to_value(resources::render(&machine, &settings()).unwrap().pod).unwrap();
        let names: Vec<_> = pod["spec"]["initContainers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["bmc", "redfish"]);
        let bmc = env(&pod["spec"]["initContainers"][0]);
        assert!(!bmc.contains_key("BMC_VIEWER_SUBJECTS"));
        // The BMC's /redfish proxy reaches Redfish over the Pod's loopback.
        assert_eq!(bmc["BMC_REDFISH_URL"], "https://127.0.0.1:8444");
        assert_eq!(bmc["BMC_WORKLOAD_LISTEN"], "0.0.0.0:8443");
        assert_eq!(
            bmc["BMC_WORKLOAD_PEERS"],
            "spiffe://yapplesauce.com/kubernetes/qrab/qrab-dashboard"
        );
        assert_eq!(
            bmc["BMC_REDFISH_SPIFFE_ID"],
            "spiffe://yapplesauce.com/kubernetes/qrab/vm/owner-one/redfish"
        );
        let redfish = env(&pod["spec"]["initContainers"][1]);
        assert_eq!(
            redfish["REDFISH_ADMIN_SUBJECTS"],
            "6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b"
        );
        assert!(!redfish.contains_key("REDFISH_VIEWER_SUBJECTS"));
    }
}

#[test]
fn console_roles_carry_over_to_redfish_with_admin_winning() {
    const ADMIN_GROUP: &str = "11111111-1111-4111-8111-111111111111";
    const VIEWER_GROUP: &str = "22222222-2222-4222-8222-222222222222";
    const REDFISH_ONLY: &str = "44444444-4444-4444-8444-444444444444";
    let console_admin = "6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b";
    let mut machine = vm();
    let console = machine.spec.console.as_mut().unwrap();
    console.viewer_subjects = vec![VIEWER.into()];
    console.admin_groups = vec![ADMIN_GROUP.into()];
    console.viewer_groups = vec![VIEWER_GROUP.into()];
    // Redfish's own lists add to the console's; a console admin listed as a
    // Redfish viewer (in another case) stays an admin.
    let redfish = &mut machine.spec.components.redfish;
    redfish.admin_subjects = vec![REDFISH_ONLY.into()];
    redfish.viewer_subjects = vec![console_admin.to_uppercase()];
    redfish.viewer_groups = vec![ADMIN_GROUP.into()];
    let pod = serde_json::to_value(resources::render(&machine, &settings()).unwrap().pod).unwrap();
    let redfish = env(&pod["spec"]["initContainers"][1]);
    assert_eq!(
        redfish["REDFISH_ADMIN_SUBJECTS"],
        format!("{REDFISH_ONLY},{console_admin}")
    );
    assert_eq!(redfish["REDFISH_VIEWER_SUBJECTS"], VIEWER);
    assert_eq!(redfish["REDFISH_ADMIN_GROUPS"], ADMIN_GROUP);
    assert_eq!(redfish["REDFISH_VIEWER_GROUPS"], VIEWER_GROUP);
    // Without the BMC the console's lists grant nothing in Redfish.
    machine.spec.components.bmc.enabled = false;
    machine.spec.components.redfish.enabled = Some(true);
    let pod = serde_json::to_value(resources::render(&machine, &settings()).unwrap().pod).unwrap();
    let redfish = env(&pod["spec"]["initContainers"][0]);
    assert_eq!(redfish["REDFISH_ADMIN_SUBJECTS"], REDFISH_ONLY);
    assert_eq!(
        redfish["REDFISH_VIEWER_SUBJECTS"],
        console_admin.to_uppercase()
    );
    assert_eq!(redfish["REDFISH_VIEWER_GROUPS"], ADMIN_GROUP);
    assert!(!redfish.contains_key("REDFISH_ADMIN_GROUPS"));
}

#[test]
fn the_crd_refuses_turning_redfish_off_beside_the_bmc() {
    let crd = api::crd();
    let components = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["components"];
    let rules = components["x-kubernetes-validations"].to_string();
    assert!(rules.contains("!self.redfish.enabled"), "{rules}");
    assert!(rules.contains("!self.bmc.enabled"), "{rules}");
    // Attached below the root, without oldSelf, so unchanged stored
    // components ratchet through unrelated updates such as spec.running.
    assert!(!rules.contains("oldSelf"), "{rules}");
    assert!(
        crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["x-kubernetes-validations"]
            .to_string()
            .contains("console is required")
    );
    // No default may make an absent `enabled` look explicitly false.
    let enabled = &components["properties"]["redfish"]["properties"]["enabled"];
    assert!(enabled.get("default").is_none() && enabled.get("nullable").is_none());
    for default in [
        &components["default"]["redfish"],
        &components["properties"]["redfish"]["default"],
    ] {
        assert!(default.get("enabled").is_none(), "{default}");
    }
}

#[test]
fn operator_renders_exactly_the_enabled_sidecars() {
    for (bmc, redfish) in [(true, false), (true, true), (false, true), (false, false)] {
        let machine = with_components(bmc, redfish);
        let desired = resources::render(&machine, &settings()).unwrap();
        assert!(
            resources::pod_matches(&desired.pod, &desired),
            "{bmc} {redfish}"
        );
        let config: Value =
            serde_json::from_str(&desired.config.data.as_ref().unwrap()["vm.json"]).unwrap();
        assert_eq!(config["control_socket"], true);
        assert_eq!(config["control_dir"], resources::CONTROL_DIR);
        let pod = serde_json::to_value(&desired.pod).unwrap();
        // The manager always owns the VM, with no port and deployment metadata.
        let manager = &pod["spec"]["containers"][0];
        assert_eq!(pod["spec"]["containers"].as_array().unwrap().len(), 1);
        // Unqualified `kubectl exec`/`logs` reach the VM owner.
        assert_eq!(
            pod["metadata"]["annotations"][resources::DEFAULT_CONTAINER],
            "manager"
        );
        assert!(pod["metadata"]["annotations"][resources::HASH].is_string());
        assert_eq!(manager["command"], json!(["qrab-manager"]));
        assert_eq!(
            manager["args"],
            json!(["run", "--config", "/etc/qrab/vm.json"])
        );
        assert!(manager.get("ports").is_none());
        assert_eq!(
            manager["readinessProbe"]["exec"]["command"],
            json!(["qrab-manager", "live", "--config", "/etc/qrab/vm.json"])
        );
        let manager_env = env(manager);
        assert_eq!(
            manager_env["QRAB_NODE_NAME"]["fieldRef"]["fieldPath"],
            "spec.nodeName"
        );
        assert_eq!(
            manager_env["QRAB_POD_NAME"]["fieldRef"]["fieldPath"],
            "metadata.name"
        );
        assert_eq!(
            manager_env["QRAB_POD_NAMESPACE"]["fieldRef"]["fieldPath"],
            "metadata.namespace"
        );
        assert_eq!(manager_env["QRAB_IMAGE"], manager["image"]);
        let sidecars: Vec<Value> = pod["spec"]["initContainers"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let names: Vec<_> = sidecars
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        // The BMC always brings Redfish, whatever redfish.enabled says.
        let redfish = redfish || bmc;
        let expected: Vec<_> = [("bmc", bmc), ("redfish", redfish)]
            .into_iter()
            .filter_map(|(name, on)| on.then_some(name))
            .collect();
        assert_eq!(names, expected);
        for sidecar in &sidecars {
            // Each service has its own cgroup budget; it is never subtracted
            // from the guest's manager container allocation.
            assert_eq!(
                sidecar["resources"],
                json!({
                    "requests":{"cpu":"20m","memory":"32Mi"},
                    "limits":{"cpu":"500m","memory":"128Mi"}
                })
            );
            assert_eq!(
                manager["resources"],
                resources::GuestSize::of(&machine).container_resources(machine.spec.accelerator)
            );
            assert_eq!(sidecar["restartPolicy"], "Always");
            assert_eq!(sidecar["image"], manager["image"]);
            assert_eq!(
                sidecar["args"],
                json!(["--socket", resources::CONTROL_SOCKET])
            );
            // Control plus attested Workload API: no QMP/VNC runtime, state, Secrets or config.
            assert_eq!(
                sidecar["volumeMounts"],
                json!([{"name":"control","mountPath":"/run/qrab-control"},{"name":"workload-socket","mountPath":"/run/spire","readOnly":true}])
            );
            assert!(sidecar.get("startupProbe").is_none());
            assert_eq!(sidecar["securityContext"], manager["securityContext"]);
        }
        let ports: Vec<_> = sidecars
            .iter()
            .flat_map(|c| c["ports"].as_array().unwrap().clone())
            .map(|p| p["containerPort"].as_u64().unwrap())
            .collect();
        let expected_ports: Vec<u64> = [
            (8080, bmc),
            (8443, bmc),
            (8081, redfish),
            (8444, redfish),
            (9181, redfish),
        ]
        .into_iter()
        .filter_map(|(port, on)| on.then_some(port))
        .collect();
        assert_eq!(ports, expected_ports);
        if redfish {
            let redfish = env(sidecars.iter().find(|c| c["name"] == "redfish").unwrap());
            assert_eq!(redfish["REDFISH_OIDC_AUDIENCE"], "qrab-redfish");
            assert_eq!(redfish["REDFISH_WORKLOAD_LISTEN"], "0.0.0.0:8444");
            let expected_peers = if bmc {
                "spiffe://yapplesauce.com/kubernetes/qrab/qrab-dashboard,spiffe://yapplesauce.com/kubernetes/qrab/vm/owner-one/bmc"
            } else {
                "spiffe://yapplesauce.com/kubernetes/qrab/qrab-dashboard"
            };
            assert_eq!(redfish["REDFISH_WORKLOAD_PEERS"], expected_peers);
            assert_eq!(
                redfish["QRAB_EXPECTED_SPIFFE_ID"],
                "spiffe://yapplesauce.com/kubernetes/qrab/vm/owner-one/redfish"
            );
            assert_eq!(
                redfish["REDFISH_OIDC_ISSUER"],
                "https://idm.yaple.net/oauth2/openid/qrab-redfish"
            );
            assert_eq!(
                redfish["REDFISH_ADMIN_SUBJECTS"],
                "6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b"
            );
            assert_eq!(redfish["REDFISH_VIEWER_SUBJECTS"], VIEWER);
        }
        assert_eq!(pod["spec"]["hostAliases"].is_null(), !bmc && !redfish);
        assert!(pod["spec"]["volumes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "control" && v["emptyDir"]["medium"] == "Memory"));
    }
}

#[test]
fn bmc_viewers_are_rendered_and_role_lists_are_validated() {
    let mut machine = vm();
    machine.spec.console.as_mut().unwrap().viewer_subjects = vec![VIEWER.into()];
    let pod = serde_json::to_value(resources::render(&machine, &settings()).unwrap().pod).unwrap();
    assert_eq!(
        env(&pod["spec"]["initContainers"][0])["BMC_VIEWER_SUBJECTS"],
        VIEWER
    );
    let mut overlap = machine.clone();
    overlap.spec.console.as_mut().unwrap().viewer_subjects =
        vec!["6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b".into()];
    assert!(resources::render(&overlap, &settings()).is_err());
    let mut no_console = vm();
    no_console.spec.console = None;
    assert!(resources::render(&no_console, &settings()).is_err());
    let mut empty_redfish = with_components(false, true);
    empty_redfish.spec.components.redfish.admin_subjects.clear();
    empty_redfish
        .spec
        .components
        .redfish
        .viewer_subjects
        .clear();
    assert!(resources::render(&empty_redfish, &settings()).is_err());
    let mut bad_redfish = with_components(false, true);
    bad_redfish.spec.components.redfish.viewer_subjects = vec!["not-a-uuid".into()];
    assert!(resources::render(&bad_redfish, &settings()).is_err());
}

#[test]
fn role_groups_and_the_creator_reach_both_sidecars() {
    const ADMIN_GROUP: &str = "11111111-1111-4111-8111-111111111111";
    const VIEWER_GROUP: &str = "22222222-2222-4222-8222-222222222222";
    const CREATOR: &str = "33333333-3333-4333-8333-333333333333";
    let admin = "6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b";
    let mut machine = with_components(true, true);
    let console = machine.spec.console.as_mut().unwrap();
    console.admin_groups = vec![ADMIN_GROUP.into()];
    console.viewer_groups = vec![VIEWER_GROUP.into()];
    let redfish = &mut machine.spec.components.redfish;
    redfish.admin_groups = vec![ADMIN_GROUP.into()];
    redfish.viewer_groups = vec![VIEWER_GROUP.into()];
    machine
        .metadata
        .annotations
        .get_or_insert_with(Default::default)
        .insert(resources::CREATOR.into(), CREATOR.to_uppercase());
    let pod = serde_json::to_value(resources::render(&machine, &settings()).unwrap().pod).unwrap();
    let bmc = env(&pod["spec"]["initContainers"][0]);
    assert_eq!(bmc["BMC_ADMIN_GROUPS"], ADMIN_GROUP);
    assert_eq!(bmc["BMC_VIEWER_GROUPS"], VIEWER_GROUP);
    assert_eq!(
        bmc["BMC_VIEWER_SUBJECTS"], CREATOR,
        "the creator is a viewer"
    );
    let redfish = env(&pod["spec"]["initContainers"][1]);
    assert_eq!(redfish["REDFISH_ADMIN_GROUPS"], ADMIN_GROUP);
    assert_eq!(redfish["REDFISH_VIEWER_GROUPS"], VIEWER_GROUP);
    assert!(redfish["REDFISH_VIEWER_SUBJECTS"]
        .as_str()
        .unwrap()
        .ends_with(CREATOR));

    // A creator who is already an admin is not demoted or duplicated.
    let mut creator_admin = machine.clone();
    creator_admin
        .metadata
        .annotations
        .as_mut()
        .unwrap()
        .insert(resources::CREATOR.into(), admin.into());
    let pod =
        serde_json::to_value(resources::render(&creator_admin, &settings()).unwrap().pod).unwrap();
    assert!(!env(&pod["spec"]["initContainers"][0]).contains_key("BMC_VIEWER_SUBJECTS"));

    // Groups alone, or the creator alone, are enough for the console.
    let mut groups_only = machine.clone();
    groups_only.metadata.annotations = None;
    groups_only
        .spec
        .console
        .as_mut()
        .unwrap()
        .allowed_subjects
        .clear();
    let pod =
        serde_json::to_value(resources::render(&groups_only, &settings()).unwrap().pod).unwrap();
    assert!(!env(&pod["spec"]["initContainers"][0]).contains_key("BMC_ALLOWED_SUBJECTS"));
    let mut creator_only = with_components(true, false);
    creator_only
        .spec
        .console
        .as_mut()
        .unwrap()
        .allowed_subjects
        .clear();
    creator_only
        .metadata
        .annotations
        .get_or_insert_with(Default::default)
        .insert(resources::CREATOR.into(), CREATOR.into());
    assert!(resources::render(&creator_only, &settings()).is_ok());
    creator_only.metadata.annotations = None;
    assert!(resources::render(&creator_only, &settings()).is_err());

    // Malformed creators and overlapping or malformed groups are refused.
    let mut bad = machine.clone();
    bad.metadata
        .annotations
        .as_mut()
        .unwrap()
        .insert(resources::CREATOR.into(), "kanidm:someone".into());
    assert!(resources::render(&bad, &settings()).is_err());
    let mut overlap = machine.clone();
    overlap.spec.console.as_mut().unwrap().viewer_groups = vec![ADMIN_GROUP.into()];
    assert!(resources::render(&overlap, &settings()).is_err());
    let mut malformed = machine.clone();
    malformed.spec.components.redfish.admin_groups = vec!["qrab_vm_qrab_demo_admins".into()];
    assert!(resources::render(&malformed, &settings()).is_err());
}

#[test]
fn unexpected_or_missing_sidecars_replace_the_pod() {
    let desired = resources::render(&with_components(true, true), &settings()).unwrap();
    let sidecars = |pod: &mut k8s_openapi::api::core::v1::Pod| {
        pod.spec
            .as_mut()
            .unwrap()
            .init_containers
            .as_mut()
            .unwrap()
            .clone()
    };
    let mut extra = desired.pod.clone();
    let mut injected = sidecars(&mut extra)[0].clone();
    injected.name = "injected".into();
    extra
        .spec
        .as_mut()
        .unwrap()
        .init_containers
        .as_mut()
        .unwrap()
        .push(injected);
    assert!(!resources::pod_matches(&extra, &desired));
    let mut missing = desired.pod.clone();
    missing
        .spec
        .as_mut()
        .unwrap()
        .init_containers
        .as_mut()
        .unwrap()
        .pop();
    assert!(!resources::pod_matches(&missing, &desired));
    let mut widened = desired.pod.clone();
    widened
        .spec
        .as_mut()
        .unwrap()
        .init_containers
        .as_mut()
        .unwrap()[1]
        .volume_mounts
        .as_mut()
        .unwrap()
        .push(serde_json::from_value(json!({"name":"runtime","mountPath":"/run/qrab"})).unwrap());
    assert!(!resources::pod_matches(&widened, &desired));
}

fn bearer_settings() -> Settings {
    Settings {
        bmc_bearer: Some(resources::BearerSettings {
            issuer: "https://idm.example/oauth2/openid/qrab-dashboard".into(),
            audience: "qrab-dashboard".into(),
        }),
        ..settings()
    }
}

#[test]
fn bmc_bearer_settings_reach_every_bmc_and_public_url_is_optional() {
    // With bearer settings, every BMC gets them next to its browser login.
    let pod =
        serde_json::to_value(resources::render(&vm(), &bearer_settings()).unwrap().pod).unwrap();
    let bmc = env(&pod["spec"]["initContainers"][0]);
    assert_eq!(
        bmc["BMC_BEARER_ISSUER"],
        "https://idm.example/oauth2/openid/qrab-dashboard"
    );
    assert_eq!(bmc["BMC_BEARER_AUDIENCE"], "qrab-dashboard");
    assert_eq!(bmc["BMC_PUBLIC_URL"], "https://test-vm.yapplesauce.com");
    assert_eq!(bmc["BMC_OIDC_CLIENT_ID"], "qrab-bmc");
    let hosts = pod["spec"]["hostAliases"][0]["hostnames"].clone();
    assert_eq!(hosts, json!(["idm.example", "idm.yaple.net"]));
    // Without them nothing changes.
    let pod = serde_json::to_value(resources::render(&vm(), &settings()).unwrap().pod).unwrap();
    let bmc = env(&pod["spec"]["initContainers"][0]);
    assert!(!bmc.contains_key("BMC_BEARER_ISSUER") && !bmc.contains_key("BMC_BEARER_AUDIENCE"));

    // A dashboard-only BMC has no public URL and no browser OIDC client.
    let mut dashboard_only = vm();
    dashboard_only.spec.console.as_mut().unwrap().public_url = None;
    let pod = serde_json::to_value(
        resources::render(&dashboard_only, &bearer_settings())
            .unwrap()
            .pod,
    )
    .unwrap();
    let bmc = env(&pod["spec"]["initContainers"][0]);
    for absent in ["BMC_PUBLIC_URL", "BMC_OIDC_ISSUER", "BMC_OIDC_CLIENT_ID"] {
        assert!(!bmc.contains_key(absent), "{absent}");
    }
    assert_eq!(bmc["BMC_BEARER_AUDIENCE"], "qrab-dashboard");
    assert_eq!(
        bmc["BMC_ALLOWED_SUBJECTS"],
        "6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b"
    );
    // Its Redfish sidecar still resolves Redfish's own issuer.
    assert_eq!(
        pod["spec"]["hostAliases"][0]["hostnames"],
        json!(["idm.example", "idm.yaple.net"])
    );
    // It is refused without the operator's bearer settings, whose absence
    // would leave the BMC with no way in.
    assert!(resources::validate(&dashboard_only, &settings()).is_err());
    assert!(resources::render(&dashboard_only, &settings()).is_err());
    // Without a BMC, publicUrl is not needed either way.
    let mut no_bmc = dashboard_only.clone();
    no_bmc.spec.components.bmc.enabled = false;
    assert!(resources::render(&no_bmc, &settings()).is_ok());
    // Access rules still apply to a dashboard-only BMC.
    let mut ungranted = dashboard_only.clone();
    ungranted
        .spec
        .console
        .as_mut()
        .unwrap()
        .allowed_subjects
        .clear();
    assert!(resources::render(&ungranted, &bearer_settings()).is_err());
    let mut http = vm();
    http.spec.console.as_mut().unwrap().public_url = Some("http://test-vm.yapplesauce.com".into());
    assert!(resources::render(&http, &bearer_settings()).is_err());

    // The CRD no longer requires publicUrl; a manifest may omit it.
    let crd = api::crd();
    let console = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["console"];
    assert!(!console["required"]
        .as_array()
        .is_some_and(|r| r.contains(&json!("publicUrl"))));
    let spec: api::VirtualMachineSpec = serde_json::from_value(
        json!({"stateVolumeClaim":"s","console":{"allowedSubjects":["6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b"]}}),
    )
    .unwrap();
    assert!(spec.console.unwrap().public_url.is_none());
}

#[test]
fn redfish_trusts_the_bmc_login_and_dashboard_clients_only_when_they_exist() {
    const BMC_PAIR: &str = "https://idm.yaple.net/oauth2/openid/qrab-bmc|qrab-bmc";
    const DASHBOARD_PAIR: &str = "https://idm.example/oauth2/openid/qrab-dashboard|qrab-dashboard";
    let render = |machine: &VirtualMachine, settings: &Settings| {
        let pod = serde_json::to_value(resources::render(machine, settings).unwrap().pod).unwrap();
        let redfish = env(pod["spec"]["initContainers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "redfish")
            .unwrap());
        // Redfish's own client stays the primary source in every case.
        assert_eq!(
            redfish["REDFISH_OIDC_ISSUER"],
            "https://idm.yaple.net/oauth2/openid/qrab-redfish"
        );
        assert_eq!(redfish["REDFISH_OIDC_AUDIENCE"], "qrab-redfish");
        let hosts = pod["spec"]["hostAliases"][0]["hostnames"].clone();
        (
            redfish
                .get("REDFISH_TRUSTED_TOKENS")
                .map(|v| v.as_str().unwrap().to_owned()),
            hosts,
        )
    };
    let browser = with_components(true, true);
    let mut dashboard_only = browser.clone();
    dashboard_only.spec.console.as_mut().unwrap().public_url = None;
    let mut no_bmc = vm();
    no_bmc.spec.components = with_components(false, true).spec.components;
    assert!(no_bmc.spec.console.as_ref().unwrap().public_url.is_some());
    for (machine, settings, pairs, hosts) in [
        (
            &browser,
            settings(),
            Some(BMC_PAIR.to_owned()),
            json!(["idm.yaple.net"]),
        ),
        (
            &browser,
            bearer_settings(),
            Some(format!("{BMC_PAIR},{DASHBOARD_PAIR}")),
            json!(["idm.example", "idm.yaple.net"]),
        ),
        (
            &dashboard_only,
            bearer_settings(),
            Some(DASHBOARD_PAIR.to_owned()),
            json!(["idm.example", "idm.yaple.net"]),
        ),
        // A publicUrl without a BMC is no browser login.
        (&no_bmc, settings(), None, json!(["idm.yaple.net"])),
        // Without a BMC, Redfish still trusts the dashboard and resolves its issuer.
        (
            &no_bmc,
            bearer_settings(),
            Some(DASHBOARD_PAIR.to_owned()),
            json!(["idm.example", "idm.yaple.net"]),
        ),
    ] {
        assert_eq!(render(machine, &settings), (pairs, hosts));
    }
}

fn networked() -> VirtualMachine {
    let mut vm = vm();
    vm.spec.nics = serde_json::from_value(json!([
        {"name":"lan","securityGroups":["web","ssh"]},
        {"name":"backup"}
    ]))
    .unwrap();
    vm
}

#[test]
fn nic_macs_are_deterministic_local_unicast_and_distinct() {
    use qrab_operator::nic;
    let lan = nic::mac("owner-one", "lan");
    assert_eq!(lan, nic::mac("owner-one", "lan"));
    assert_ne!(lan, nic::mac("owner-one", "backup"));
    assert_ne!(lan, nic::mac("owner-two", "lan"));
    let bytes: Vec<u8> = lan
        .split(':')
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect();
    assert_eq!(bytes.len(), 6);
    assert_eq!(bytes[0], 0x02);
}

#[test]
fn nics_are_restricted_anchors_pinned_to_the_vm_node_and_gated_on_qrab_netd() {
    use qrab_operator::nic;
    let vm = networked();
    let pool = "qrab-nic-10-244-128-9";
    let anchor = nic::render_anchor(&vm, &settings(), &vm.spec.nics[0], pool, "kube02").unwrap();
    assert!(resources::owned(&anchor.pod, &vm));
    let pod = serde_json::to_value(&anchor.pod).unwrap();
    // The node is in the name: a migration's target anchor already has the
    // name the VM's reconcile renders once the VM lives there.
    assert_eq!(pod["metadata"]["name"], "test-vm-nic-lan-kube02");
    // Nothing per Pod, so a second anchor of this NIC gets the same Cilium
    // identity. Not part of the BMC Service, the dashboard's or the VM's own
    // policies.
    assert_eq!(
        pod["metadata"]["labels"],
        json!({"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm-nic",
            "app.kubernetes.io/instance":"test-vm","qrab.yapplesauce.com/owner-uid":"owner-one",
            "qrab.yapplesauce.com/nic":"lan",
            "sg.qrab.yapplesauce.com/web":"","sg.qrab.yapplesauce.com/ssh":""})
    );
    let annotations = &pod["metadata"]["annotations"];
    assert_eq!(annotations["ipam.cilium.io/ip-pool"], pool);
    assert_eq!(annotations["ipam.cilium.io/require-pool-match"], "true");
    assert_eq!(annotations["qrab.yapplesauce.com/nic-datapath"], "tap");
    assert_eq!(annotations[qrab_operator::resources::HASH], anchor.hash);
    let spec = &pod["spec"];
    assert_eq!(spec["serviceAccountName"], "qrab-manager");
    assert_eq!(spec["restartPolicy"], "Always");
    assert_eq!(
        spec["tolerations"][0]["key"],
        "node-role.kubernetes.io/control-plane"
    );
    assert_eq!(
        spec["affinity"],
        json!({"nodeAffinity":{"requiredDuringSchedulingIgnoredDuringExecution":{"nodeSelectorTerms":[
            {"matchFields":[{"key":"metadata.name","operator":"In","values":["kube02"]}]}]}}})
    );
    assert_eq!(
        spec["readinessGates"],
        json!([{"conditionType":"qrab.yapplesauce.com/nic-plumbed"}])
    );
    for (key, value) in [
        ("hostNetwork", false),
        ("hostPID", false),
        ("hostIPC", false),
        ("automountServiceAccountToken", false),
    ] {
        assert_eq!(spec[key], value, "{key}");
    }
    assert_eq!(spec["securityContext"]["runAsNonRoot"], true);
    assert_eq!(
        spec["securityContext"]["seccompProfile"]["type"],
        "RuntimeDefault"
    );
    let containers = spec["containers"].as_array().unwrap();
    assert_eq!(containers.len(), 1);
    let container = &containers[0];
    assert_eq!(container["image"], settings().vm_image);
    assert_eq!(container["command"], json!(["qrab-manager"]));
    assert_eq!(container["args"], json!(["anchor"]));
    assert_eq!(
        container["securityContext"],
        json!({"allowPrivilegeEscalation":false,"privileged":false,"readOnlyRootFilesystem":true,"capabilities":{"drop":["ALL"]}})
    );
    // The Pod IP is the guest's: no ports, no probes, no volumes.
    for key in [
        "ports",
        "readinessProbe",
        "livenessProbe",
        "startupProbe",
        "volumeMounts",
    ] {
        assert!(container.get(key).is_none(), "{key}");
    }
    assert!(spec.get("volumes").is_none());
    // Another node is another template.
    let elsewhere = nic::render_anchor(&vm, &settings(), &vm.spec.nics[0], pool, "kube03").unwrap();
    assert_ne!(elsewhere.hash, anchor.hash);
    assert!(
        nic::anchor_matches(&anchor.pod, &anchor) && !nic::anchor_matches(&anchor.pod, &elsewhere)
    );
    // A live migration's target anchor is the target node's normal anchor plus
    // the handoff source and owning migration, outside the template hash: once
    // the VM lives there, its reconcile keeps it instead of replacing it.
    let target = nic::handoff_target(elsewhere, "qrab", "test-vm-nic-lan-kube02", "move-1");
    let annotations = target.pod.metadata.annotations.clone().unwrap();
    assert_eq!(
        annotations["ipam.cilium.io/handoff-from"],
        "qrab/test-vm-nic-lan-kube02"
    );
    assert_eq!(annotations[nic::MIGRATION_ANNOTATION], "move-1");
    let normal = nic::render_anchor(&vm, &settings(), &vm.spec.nics[0], pool, "kube03").unwrap();
    assert!(nic::anchor_matches(&target.pod, &normal));
    assert_eq!(
        target.pod.metadata.name.as_deref(),
        Some("test-vm-nic-lan-kube03")
    );
    // Security groups are reconciled in place; the template is unchanged.
    let mut changed = vm.clone();
    changed.spec.nics[0].security_groups = serde_json::from_value(json!(["db"])).unwrap();
    let regrouped =
        nic::render_anchor(&changed, &settings(), &changed.spec.nics[0], pool, "kube02").unwrap();
    assert_eq!(regrouped.hash, anchor.hash);
    assert!(nic::anchor_matches(&anchor.pod, &regrouped));
    let patch = nic::security_group_patch(&anchor.pod, &changed.spec.nics[0]).unwrap();
    assert_eq!(
        patch["metadata"]["labels"],
        json!({"sg.qrab.yapplesauce.com/web":null,"sg.qrab.yapplesauce.com/ssh":null,"sg.qrab.yapplesauce.com/db":""})
    );
    assert!(nic::security_group_patch(&regrouped.pod, &changed.spec.nics[0]).is_none());
    // An image change replaces the anchor.
    let mut image = vm.clone();
    image.spec.image = Some(format!(
        "zot.yapplesauce.com/qrab/manager@sha256:{}",
        "b".repeat(64)
    ));
    let moved =
        nic::render_anchor(&image, &settings(), &image.spec.nics[0], pool, "kube02").unwrap();
    assert!(!nic::anchor_matches(&anchor.pod, &moved));
    // The removed stream datapath's relay policy, which the operator deletes.
    assert_eq!(
        nic::legacy_relay_policy_name(&vm, "lan"),
        "test-vm-nic-lan-relay"
    );
}

#[test]
fn vm_config_nics_carry_only_their_id_and_mac() {
    use qrab_operator::nic::NicEndpoint;
    let vm = networked();
    let boot = resources::GuestSize::of(&vm);
    let endpoints =
        [("lan", "10.244.128.9"), ("backup", "10.244.128.10")].map(|(name, ip)| NicEndpoint {
            name: name.into(),
            ip: ip.parse().unwrap(),
        });
    let desired = resources::render_attached(&vm, &settings(), &[], boot, &endpoints).unwrap();
    let config: Value = serde_json::from_str(&desired.config.data.unwrap()["vm.json"]).unwrap();
    assert_eq!(
        config["nics"],
        json!([
            {"id":"lan","mac":qrab_operator::nic::mac("owner-one","lan")},
            {"id":"backup","mac":qrab_operator::nic::mac("owner-one","backup")}
        ])
    );
    // The guest configures a static /32 per NIC from the seed.
    assert!(config["seed"]["network_config"]
        .as_str()
        .unwrap()
        .contains("10.244.128.9/32"));
    // The CRD refuses NICs without kvm, which brings qrab-netd's socket. The
    // rule guards absent `nics` and `accelerator` fields with `has`.
    let crd = qrab_operator::api::crd();
    let rules = crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["x-kubernetes-validations"]
        .as_array()
        .unwrap();
    let kvm = rules
        .iter()
        .find(|r| {
            r["message"]
                .as_str()
                .unwrap()
                .starts_with("guest NICs need accelerator kvm")
        })
        .unwrap();
    assert_eq!(
        kvm["rule"],
        "!has(self.spec.nics) || size(self.spec.nics) == 0 || (has(self.spec.accelerator) && self.spec.accelerator == 'kvm')"
    );
}

#[test]
fn vm_config_carries_nics_only_once_every_nic_has_an_address() {
    use qrab_operator::nic::NicEndpoint;
    let vm = networked();
    let boot = resources::GuestSize::of(&vm);
    assert!(resources::render(&vm, &settings()).is_err());
    let lan = NicEndpoint {
        name: "lan".into(),
        ip: "10.0.3.7".parse().unwrap(),
    };
    assert!(
        resources::render_attached(&vm, &settings(), &[], boot, std::slice::from_ref(&lan))
            .is_err()
    );
    let endpoints = [
        lan,
        NicEndpoint {
            name: "backup".into(),
            ip: "fd00::9".parse().unwrap(),
        },
    ];
    let desired = resources::render_attached(&vm, &settings(), &[], boot, &endpoints).unwrap();
    let config: Value =
        serde_json::from_str(&desired.config.data.clone().unwrap()["vm.json"]).unwrap();
    assert_eq!(
        config["nics"],
        json!([
            {"id":"lan","mac":qrab_operator::nic::mac("owner-one","lan")},
            {"id":"backup","mac":qrab_operator::nic::mac("owner-one","backup")}
        ])
    );
    // Without cloudInit the VM still gets a seed: meta-data plus static
    // network-config for each NIC, gateway and DNS through qrab-netd.
    let seed = &config["seed"];
    assert_eq!(
        seed["meta_data"],
        resources::seed_meta_data("owner-one", "test-vm")
    );
    assert!(seed.get("user_data").is_none());
    let lan_mac = qrab_operator::nic::mac("owner-one", "lan");
    let backup_mac = qrab_operator::nic::mac("owner-one", "backup");
    assert_eq!(
        seed["network_config"],
        resources::seed_network_config(&[
            resources::NicAddress {
                mac: lan_mac.clone(),
                address_cidr: "10.0.3.7/32".into(),
                gateway: Some("169.254.1.1".into()),
                nameservers: vec!["169.254.1.1".into()],
                dhcp: false,
            },
            resources::NicAddress {
                mac: backup_mac,
                address_cidr: "fd00::9/128".into(),
                gateway: None,
                nameservers: vec![],
                dhcp: false,
            },
        ])
    );
    let network = seed["network_config"].as_str().unwrap();
    assert!(network.contains(&format!("macaddress: \"{lan_mac}\"")));
    assert!(network.contains("- \"10.0.3.7/32\"\n"));
    assert!(network.contains("via: \"169.254.1.1\"\n        on-link: true\n"));
    // cloudInit keeps its hostname and user-data alongside the NICs.
    let mut configured = vm.clone();
    configured.spec.cloud_init =
        serde_json::from_value(json!({"hostname":"web-1","userData":"#cloud-config\n"})).unwrap();
    let with_user =
        resources::render_attached(&configured, &settings(), &[], boot, &endpoints).unwrap();
    let with_user: Value =
        serde_json::from_str(&with_user.config.data.unwrap()["vm.json"]).unwrap();
    assert_eq!(with_user["seed"]["user_data"], "#cloud-config\n");
    assert_eq!(
        with_user["seed"]["meta_data"],
        resources::seed_meta_data("owner-one", "web-1")
    );
    assert_eq!(with_user["seed"]["network_config"], seed["network_config"]);
    // NIC changes reach the running manager through the ConfigMap; they are
    // not a new VM template (the manager plugs NICs while the guest runs).
    let mut moved = endpoints.clone();
    moved[0].ip = "10.0.3.8".parse().unwrap();
    let replaced = resources::render_attached(&vm, &settings(), &[], boot, &moved).unwrap();
    assert_eq!(replaced.hash, desired.hash);
    assert_ne!(replaced.config.data, desired.config.data);
    let mut one = vm.clone();
    one.spec.nics.truncate(1);
    let fewer = resources::render_attached(&one, &settings(), &[], boot, &endpoints[..1]).unwrap();
    assert_eq!(fewer.hash, desired.hash);
    let fewer: Value = serde_json::from_str(&fewer.config.data.unwrap()["vm.json"]).unwrap();
    assert_eq!(fewer["nics"].as_array().unwrap().len(), 1);
    // Without NICs nothing changes for existing VMs.
    let plain = resources::render(&self::vm(), &settings()).unwrap();
    let config: Value = serde_json::from_str(&plain.config.data.unwrap()["vm.json"]).unwrap();
    assert!(config.get("nics").is_none() && config.get("seed").is_none());
}

#[test]
fn invalid_nics_are_refused() {
    let mut cases = Vec::new();
    for nics in [
        json!([{"name":"lan"},{"name":"lan"}]),
        json!([{"name":"Lan"}]),
        json!([{"name":"-lan"}]),
        json!([{"name":"a-very-long-nic-name"}]),
        json!([{"name":"a"},{"name":"b"},{"name":"c"},{"name":"d"},{"name":"e"}]),
        json!([{"name":"lan","securityGroups":["web","web"]}]),
        json!([{"name":"lan","securityGroups":["Web"]}]),
        json!([{"name":"lan","securityGroups":["a","b","c","d","e","f","g","h","i"]}]),
    ] {
        let mut vm = vm();
        vm.spec.nics = serde_json::from_value(nics.clone()).unwrap();
        cases.push((nics, resources::validate(&vm, &settings())));
    }
    for (nics, result) in cases {
        assert!(
            matches!(result, Err(qrab_operator::Error::Invalid(_))),
            "{nics} accepted"
        );
    }
    let mut vm = vm();
    vm.spec.nics =
        serde_json::from_value(json!([{"name":"lan","securityGroups":["web"]},{"name":"b2"}]))
            .unwrap();
    resources::validate(&vm, &settings()).unwrap();
    let crd = api::crd();
    let nics = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["nics"];
    assert_eq!(nics["maxItems"], 4);
    assert_eq!(nics["x-kubernetes-list-map-keys"], json!(["name"]));
    assert_eq!(nics["items"]["properties"]["securityGroups"]["maxItems"], 8);
}

#[test]
fn redfish_push_is_opt_in_with_separate_state_and_exact_https_egress() {
    let mut machine = with_components(true, true);
    assert!(resources::redfish_policy(&machine, &settings())
        .unwrap()
        .is_none());
    let plain = resources::render(&machine, &settings()).unwrap();
    let plain_json = serde_json::to_value(&plain.pod).unwrap();
    let plain_redfish = plain_json["spec"]["initContainers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "redfish")
        .unwrap();
    assert!(!env(plain_redfish).contains_key("REDFISH_EVENT_DESTINATIONS"));
    assert!(!env(plain_redfish).contains_key("REDFISH_SUBSCRIPTIONS_FILE"));
    machine.spec.components.redfish.event_destinations = vec![
        "https://events.example/receiver".into(),
        "https://192.0.2.20:443/hooks".into(),
        "https://[fd00::20]/hooks".into(),
    ];
    machine.spec.components.redfish.state_volume_claim = Some("test-redfish-state".into());
    let desired = resources::render(&machine, &settings()).unwrap();
    let pod = serde_json::to_value(&desired.pod).unwrap();
    let redfish = pod["spec"]["initContainers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "redfish")
        .unwrap();
    let configured = env(redfish);
    assert_eq!(
        configured["REDFISH_EVENT_DESTINATIONS"],
        "https://events.example/receiver,https://192.0.2.20/hooks,https://[fd00::20]/hooks"
    );
    assert_eq!(
        configured["REDFISH_SUBSCRIPTIONS_FILE"],
        "/var/lib/qrab-redfish/subscriptions-owner-one.json"
    );
    assert_eq!(configured["QRAB_RESOURCE_SCOPE"], "container");
    assert_eq!(
        redfish["volumeMounts"],
        json!([{"name":"control","mountPath":"/run/qrab-control"},{"name":"workload-socket","mountPath":"/run/spire","readOnly":true},{"name":"redfish-state","mountPath":"/var/lib/qrab-redfish"}])
    );
    assert!(pod["spec"]["volumes"].as_array().unwrap().iter().any(|v|v==&json!({"name":"redfish-state","persistentVolumeClaim":{"claimName":"test-redfish-state"}})));
    for container in pod["spec"]["containers"]
        .as_array()
        .unwrap()
        .iter()
        .chain(pod["spec"]["initContainers"].as_array().unwrap())
    {
        if container["name"] != "redfish" {
            assert!(!container["volumeMounts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["name"] == "redfish-state"));
        }
    }
    assert_eq!(redfish["resources"], plain_redfish["resources"]);
    assert_eq!(
        pod["spec"]["containers"][0]["resources"],
        plain_json["spec"]["containers"][0]["resources"]
    );
    assert!(resources::pod_matches(&desired.pod, &desired));
    assert!(!resources::pod_matches(&plain.pod, &desired));
    let policy = resources::redfish_policy(&machine, &settings())
        .unwrap()
        .unwrap();
    assert!(resources::owned_meta(&policy.metadata, &machine));
    let policy = serde_json::to_value(policy).unwrap();
    assert_eq!(policy["metadata"]["name"], "test-vm-redfish-events");
    assert_eq!(
        policy["spec"]["enableDefaultDeny"],
        json!({"ingress":false,"egress":false})
    );
    assert_eq!(
        policy["spec"]["endpointSelector"]["matchLabels"]["qrab.yapplesauce.com/owner-uid"],
        "owner-one"
    );
    let egress = policy["spec"]["egress"].as_array().unwrap();
    assert_eq!(egress.len(), 3);
    assert_eq!(
        egress[0]["toEndpoints"][0]["matchLabels"]["k8s:k8s-app"],
        "kube-dns"
    );
    assert_eq!(
        egress[1]["toFQDNs"],
        json!([{"matchName":"events.example"}])
    );
    assert_eq!(
        egress[1]["toPorts"],
        json!([{"ports":[{"port":"443","protocol":"TCP"}]}])
    );
    assert_eq!(
        egress[2]["toCIDR"],
        json!(["192.0.2.20/32", "fd00::20/128"])
    );
    assert!(!policy.to_string().contains("toEntities"));
}

#[test]
fn redfish_event_configuration_rejects_unsafe_or_ambiguous_destinations() {
    let mut machine = with_components(false, true);
    for destination in [
        "http://events.example/hook",
        "https://user:pass@events.example/hook",
        "https://events.example:8443/hook",
        "https://events.example/hook?token=secret",
        "https://events.example/hook#fragment",
        "https://events.example/a,b",
        "https://127.0.0.1/hook",
        "https://0.0.0.0/hook",
        "https://[::1]/hook",
    ] {
        machine.spec.components.redfish.event_destinations = vec![destination.into()];
        assert!(
            resources::render(&machine, &settings()).is_err(),
            "accepted {destination}"
        );
    }
    machine.spec.components.redfish.event_destinations =
        vec!["https://events.example/hook".into(); 33];
    assert!(resources::render(&machine, &settings()).is_err());
    machine.spec.components.redfish.event_destinations =
        vec!["https://events.example/hook".into(); 2];
    assert!(resources::render(&machine, &settings()).is_err());
    machine.spec.components.redfish.event_destinations.clear();
    machine.spec.components.redfish.state_volume_claim =
        Some(machine.spec.state_volume_claim.clone());
    assert!(resources::render(&machine, &settings()).is_err());
    machine.spec.components.redfish.state_volume_claim = Some("bad_claim".into());
    assert!(resources::render(&machine, &settings()).is_err());
    machine.spec.components.redfish.state_volume_claim = Some("separate-redfish-state".into());
    // Only switching the BMC off as well turns Redfish off.
    machine.spec.components.bmc.enabled = false;
    machine.spec.components.redfish.enabled = Some(false);
    assert!(resources::render(&machine, &settings()).is_err());
}

#[test]
fn approved_media_and_firmware_catalogs_are_read_only_manager_only_and_default_off() {
    let mut machine = with_components(true, true);
    let plain = resources::render(&machine, &settings()).unwrap();
    let plain_json = serde_json::to_value(&plain.pod).unwrap();
    let plain_env = env(&plain_json["spec"]["containers"][0]);
    assert!(
        !plain_env.contains_key("QRAB_MEDIA_ROOT") && !plain_env.contains_key("QRAB_FIRMWARE_ROOT")
    );
    machine.spec.media_volume_claim = Some("approved-media".into());
    machine.spec.firmware_catalog_volume_claim = Some("approved-firmware".into());
    machine.spec.components.redfish.state_volume_claim = Some("redfish-persistence".into());
    let desired = resources::render(&machine, &settings()).unwrap();
    let pod = serde_json::to_value(&desired.pod).unwrap();
    let manager = &pod["spec"]["containers"][0];
    let manager_env = env(manager);
    assert_eq!(manager_env["QRAB_MEDIA_ROOT"], "/var/lib/qrab-media");
    assert_eq!(manager_env["QRAB_FIRMWARE_ROOT"], "/var/lib/qrab-firmware");
    for (name, claim, path) in [
        ("media-catalog", "approved-media", "/var/lib/qrab-media"),
        (
            "firmware-catalog",
            "approved-firmware",
            "/var/lib/qrab-firmware",
        ),
    ] {
        assert!(pod["spec"]["volumes"].as_array().unwrap().iter().any(|v| v
            == &json!({"name":name,"persistentVolumeClaim":{"claimName":claim,"readOnly":true}})));
        assert!(manager["volumeMounts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == &json!({"name":name,"mountPath":path,"readOnly":true})));
        for sidecar in pod["spec"]["initContainers"].as_array().unwrap() {
            assert!(!sidecar["volumeMounts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["name"] == name));
        }
    }
    assert!(resources::pod_matches(&desired.pod, &desired));
    assert!(!resources::pod_matches(&plain.pod, &desired));
    let mut widened = desired.pod.clone();
    widened.spec.as_mut().unwrap().containers[0]
        .volume_mounts
        .as_mut()
        .unwrap()
        .iter_mut()
        .find(|m| m.name == "firmware-catalog")
        .unwrap()
        .read_only = Some(false);
    assert!(!resources::pod_matches(&widened, &desired));
    machine.spec.firmware_catalog_volume_claim = machine.spec.media_volume_claim.clone();
    assert!(resources::render(&machine, &settings()).is_err());
    machine.spec.firmware_catalog_volume_claim = Some(machine.spec.state_volume_claim.clone());
    assert!(resources::render(&machine, &settings()).is_err());
    machine.spec.firmware_catalog_volume_claim = Some("redfish-persistence".into());
    assert!(resources::render(&machine, &settings()).is_err());
    machine.spec.firmware_catalog_volume_claim = Some("bad_claim".into());
    assert!(resources::render(&machine, &settings()).is_err());
}

#[test]
fn periodic_redfish_telemetry_is_explicit_and_bounded() {
    let mut machine = with_components(true, true);
    let default =
        serde_json::to_value(resources::render(&machine, &settings()).unwrap().pod).unwrap();
    let redfish = default["spec"]["initContainers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "redfish")
        .unwrap();
    assert!(!env(redfish).contains_key("REDFISH_TELEMETRY_INTERVAL_SECS"));
    machine.spec.components.redfish.telemetry_interval_seconds = Some(7);
    let configured = resources::render(&machine, &settings()).unwrap();
    let pod = serde_json::to_value(&configured.pod).unwrap();
    let redfish = pod["spec"]["initContainers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "redfish")
        .unwrap();
    assert_eq!(env(redfish)["REDFISH_TELEMETRY_INTERVAL_SECS"], "7");
    assert!(resources::pod_matches(&configured.pod, &configured));
    for seconds in [0, 4, 3601] {
        machine.spec.components.redfish.telemetry_interval_seconds = Some(seconds);
        assert!(resources::render(&machine, &settings()).is_err());
    }
    machine.spec.components.redfish.telemetry_interval_seconds = Some(5);
    // Only switching the BMC off as well turns Redfish off.
    machine.spec.components.bmc.enabled = false;
    machine.spec.components.redfish.enabled = Some(false);
    assert!(resources::render(&machine, &settings()).is_err());
}

#[test]
fn secure_boot_capability_is_explicit_without_inventing_enabled_state_or_resources() {
    let mut machine = vm();
    assert!(!machine.spec.secure_boot_capable);
    let plain = resources::render(&machine, &settings()).unwrap();
    let plain_config: Value =
        serde_json::from_str(&plain.config.data.as_ref().unwrap()["vm.json"]).unwrap();
    assert_eq!(
        plain_config["uefi"],
        json!({"boot_disk":"/opt/qrab/demo/boot.img"})
    );
    assert_eq!(plain_config["restart_on_exit"], true);
    machine.spec.secure_boot_capable = true;
    let secure = resources::render(&machine, &settings()).unwrap();
    let config: Value =
        serde_json::from_str(&secure.config.data.as_ref().unwrap()["vm.json"]).unwrap();
    assert_eq!(
        config["uefi"],
        json!({"boot_disk":"/opt/qrab/demo/boot.img","secure_boot_capable":true,"code":"/opt/qrab/firmware/OVMF_CODE_4M.secboot.fd"})
    );
    assert_eq!(config["restart_on_exit"], true);
    assert!(config["uefi"].get("secure_boot_enabled").is_none());
    let plain_pod = serde_json::to_value(&plain.pod).unwrap();
    let secure_pod = serde_json::to_value(&secure.pod).unwrap();
    assert_eq!(
        plain_pod["spec"]["containers"][0]["resources"],
        secure_pod["spec"]["containers"][0]["resources"]
    );
    assert_eq!(
        plain_pod["spec"]["initContainers"],
        secure_pod["spec"]["initContainers"]
    );
    assert!(resources::pod_matches(&secure.pod, &secure));
    assert_ne!(
        plain.hash, secure.hash,
        "changing firmware capability must replace the booted instance"
    );
}

#[test]
fn approved_boot_media_requires_its_explicit_read_only_pvc_without_extra_resources() {
    let mut machine = with_components(true, true);
    let original = resources::render(&machine, &settings()).unwrap();
    let original_pod = serde_json::to_value(&original.pod).unwrap();
    let image_config: Value =
        serde_json::from_str(&original.config.data.as_ref().unwrap()["vm.json"]).unwrap();
    assert_eq!(image_config["uefi"]["boot_disk"], "/opt/qrab/demo/boot.img");
    for name in [
        "signed-boot.img",
        "install.iso",
        "Boot_2026-11.efi.img",
        "a.img",
    ] {
        machine.spec.boot_disk = format!("/var/lib/qrab-media/{name}");
        assert!(
            resources::render(&machine, &settings()).is_err(),
            "media boot accepted without a claim"
        );
        machine.spec.media_volume_claim = Some("approved-media".into());
        let desired = resources::render(&machine, &settings()).unwrap();
        let config: Value =
            serde_json::from_str(&desired.config.data.as_ref().unwrap()["vm.json"]).unwrap();
        assert_eq!(config["uefi"]["boot_disk"], machine.spec.boot_disk);
        let pod = serde_json::to_value(&desired.pod).unwrap();
        assert_eq!(
            pod["spec"]["containers"][0]["resources"],
            original_pod["spec"]["containers"][0]["resources"]
        );
        assert_eq!(
            pod["spec"]["initContainers"],
            original_pod["spec"]["initContainers"]
        );
        assert!(pod["spec"]["containers"][0]["volumeMounts"].as_array().unwrap().iter().any(|m|m==&json!({"name":"media-catalog","mountPath":"/var/lib/qrab-media","readOnly":true})));
        assert!(pod["spec"]["volumes"].as_array().unwrap().iter().any(|v|v==&json!({"name":"media-catalog","persistentVolumeClaim":{"claimName":"approved-media","readOnly":true}})));
        assert!(resources::pod_matches(&desired.pod, &desired));
        machine.spec.media_volume_claim = None;
    }
    machine.spec.media_volume_claim = Some("approved-media".into());
    machine.spec.boot_disk = format!("/var/lib/qrab-media/{}.img", "a".repeat(124));
    assert!(
        resources::render(&machine, &settings()).is_ok(),
        "128-byte basename must be accepted"
    );
    machine.spec.boot_disk = format!("/var/lib/qrab-media/{}.img", "a".repeat(125));
    assert!(resources::render(&machine, &settings()).is_err());
}

#[test]
fn approved_boot_media_cannot_select_identity_secrets_or_ambiguous_paths() {
    let mut machine = vm();
    machine.spec.media_volume_claim = Some("approved-media".into());
    for path in [
        "/var/lib/qrab/identity/boot.img",
        "/run/qrab-auth/password.img",
        "/run/secrets/boot.img",
        "/opt/qrab/demo/other.img",
        "/var/lib/qrab-firmware/code.img",
        "/var/lib/qrab-media/../qrab/boot.img",
        "/var/lib/qrab-media/nested/boot.img",
        "/var/lib/qrab-media/.boot.img",
        "/var/lib/qrab-media/a..img",
        "/var/lib/qrab-media/a..b.iso",
        "/var/lib/qrab-media/-boot.img",
        "/var/lib/qrab-media/a.qcow2",
        "/var/lib/qrab-media/boot.IMG",
        "/var/lib/qrab-media/boot.img/",
        "/var/lib/qrab-media/a%2eb.iso",
        "/var/lib/qrab-media/boot image.img",
        "/var/lib/qrab-media/é.img",
        "/var/lib/qrab-media/a.img\n",
        "var/lib/qrab-media/boot.img",
        "/var/lib/qrab-media//boot.img",
    ] {
        machine.spec.boot_disk = path.into();
        assert!(
            resources::render(&machine, &settings()).is_err(),
            "unsafe boot path accepted: {path:?}"
        );
    }
    machine.spec.boot_disk = "/var/lib/qrab-media/signed-boot.img".into();
    for claim in [None, Some(""), Some("invalid_claim"), Some("test-state")] {
        machine.spec.media_volume_claim = claim.map(str::to_owned);
        assert!(
            resources::render(&machine, &settings()).is_err(),
            "unsafe media claim accepted: {claim:?}"
        );
    }
    machine.spec.media_volume_claim = Some("approved-media".into());
    machine.spec.firmware_catalog_volume_claim = Some("approved-media".into());
    assert!(
        resources::render(&machine, &settings()).is_err(),
        "firmware and media PVC roles must stay distinct"
    );
}

#[test]
fn boot_media_schema_and_dashboard_admission_preserve_the_same_narrow_boundary() {
    let crd = api::crd();
    let spec = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"];
    let boot = &spec["properties"]["bootDisk"];
    assert!(boot.get("enum").is_none());
    assert_eq!(boot["default"], "/opt/qrab/demo/boot.img");
    assert_eq!(boot["maxLength"], 148);
    assert_eq!(boot["pattern"],"^(/opt/qrab/demo/boot[.]img|/var/lib/qrab-media/[A-Za-z0-9][A-Za-z0-9._-]{0,123}[.](img|iso))$");
    let rule = spec["x-kubernetes-validations"][0]["rule"]
        .as_str()
        .unwrap();
    assert!(rule.contains("has(self.mediaVolumeClaim)"));
    assert!(rule.contains("self.mediaVolumeClaim != ''"));
    assert!(rule.contains("self.mediaVolumeClaim != self.stateVolumeClaim"));
    assert!(rule.contains("!self.bootDisk.contains('..')"));
    let dashboard = api::dashboard_update_policy();
    let unchanged = dashboard["spec"]["validations"][0]["expression"]
        .as_str()
        .unwrap();
    assert!(unchanged.contains("object.spec.bootDisk == oldObject.spec.bootDisk"));
    assert!(unchanged.contains("object.spec.mediaVolumeClaim == oldObject.spec.mediaVolumeClaim"));
    assert!(!unchanged.contains("spec.running"));
    assert_eq!(dashboard["spec"]["failurePolicy"], "Fail");
}

#[test]
fn redfish_metrics_port_is_declared_and_admitted_only_from_prometheus_and_the_operator() {
    let machine = with_components(true, true);
    let pod = serde_json::to_value(&resources::render(&machine, &settings()).unwrap().pod).unwrap();
    let sidecar = |pod: &Value, name: &str| {
        pod["spec"]["initContainers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .cloned()
            .unwrap()
    };
    let redfish = sidecar(&pod, "redfish");
    assert_eq!(
        redfish["ports"],
        json!([{"name":"redfish","containerPort":8081},{"name":"redfish-mtls","containerPort":8444},{"name":"metrics","containerPort":9181}])
    );
    // Probes stay on the API listener; the metrics listener serves only /metrics.
    assert_eq!(redfish["readinessProbe"]["httpGet"]["port"], "redfish");
    assert_eq!(env(&redfish)["REDFISH_METRICS_LISTEN"], "0.0.0.0:9181");
    assert_eq!(env(&redfish)["REDFISH_METRICS_PEERS"], "spiffe://yapplesauce.com/kubernetes/qrab/qrab-operator,spiffe://yapplesauce.com/kubernetes/monitoring/qrab-telemetry");
    assert!(!sidecar(&pod, "bmc")["ports"]
        .to_string()
        .contains("metrics"));

    let policy = resources::redfish_metrics_policy(&machine, &settings())
        .unwrap()
        .unwrap();
    assert_eq!(policy.name_any(), "test-vm-redfish-metrics");
    assert_eq!(policy.metadata.namespace.as_deref(), Some("qrab"));
    let owners = policy.metadata.owner_references.as_ref().unwrap();
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].uid, "owner-one");
    assert_eq!(owners[0].controller, Some(true));
    let spec = &policy.data["spec"];
    assert_eq!(
        spec["endpointSelector"],
        json!({"matchLabels":{"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm","qrab.yapplesauce.com/owner-uid":"owner-one"}})
    );
    // Additive: other ports and other policies are unaffected.
    assert_eq!(
        spec["enableDefaultDeny"],
        json!({"ingress":false,"egress":false})
    );
    assert!(spec.get("egress").is_none());
    let metrics_port = json!([{"ports":[{"port":"9181","protocol":"TCP"}]}]);
    // The operator reads only the guest power state (qrab_vm_power_state).
    assert_eq!(
        spec["ingress"],
        json!([{"fromEndpoints":[
            {"matchLabels":{"k8s:io.kubernetes.pod.namespace":"monitoring","app.kubernetes.io/name":"prometheus"}},
            {"matchLabels":{"k8s:io.kubernetes.pod.namespace":"qrab","app.kubernetes.io/name":"qrab-operator"}}],
            "toPorts":metrics_port}])
    );
    let deny = spec["ingressDeny"].as_array().unwrap();
    assert_eq!(deny.len(), 2);
    assert!(deny.iter().all(|rule| rule["toPorts"] == metrics_port));
    let entities = deny[0]["fromEntities"].as_array().unwrap();
    for entity in ["world", "host", "remote-node", "ingress", "kube-apiserver"] {
        assert!(entities.iter().any(|e| e == entity), "{entity}");
    }
    assert_eq!(
        deny[1]["fromEndpoints"],
        json!([
            {"matchExpressions":[{"key":"k8s:io.kubernetes.pod.namespace","operator":"NotIn","values":["monitoring","qrab"]}]},
            {"matchExpressions":[{"key":"k8s:io.kubernetes.pod.namespace","operator":"In","values":["monitoring"]},
                {"key":"app.kubernetes.io/name","operator":"NotIn","values":["prometheus"]}]},
            {"matchExpressions":[{"key":"k8s:io.kubernetes.pod.namespace","operator":"In","values":["qrab"]},
                {"key":"app.kubernetes.io/name","operator":"NotIn","values":["qrab-operator"]}]}
        ])
    );
    // Sharing the monitoring namespace still denies its other Pods.
    let shared = Settings {
        namespace: "monitoring".into(),
        ..settings()
    };
    let policy = resources::redfish_metrics_policy(&machine, &shared)
        .unwrap()
        .unwrap();
    assert_eq!(
        policy.data["spec"]["ingressDeny"][1]["fromEndpoints"],
        json!([
            {"matchExpressions":[{"key":"k8s:io.kubernetes.pod.namespace","operator":"NotIn","values":["monitoring"]}]},
            {"matchExpressions":[{"key":"k8s:io.kubernetes.pod.namespace","operator":"In","values":["monitoring"]},
                {"key":"app.kubernetes.io/name","operator":"NotIn","values":["prometheus","qrab-operator"]}]}
        ])
    );
    // Without Redfish (which the BMC would bring) nothing listens, and no
    // policy is kept.
    let plain = with_components(false, false);
    assert!(resources::redfish_metrics_policy(&plain, &settings())
        .unwrap()
        .is_none());
    let pod = serde_json::to_value(&resources::render(&plain, &settings()).unwrap().pod).unwrap();
    assert!(!pod.to_string().contains("9181"));
}

#[test]
fn admin_bucket_is_default_off_and_renders_only_enabled_features() {
    let rendered = |machine: &VirtualMachine| {
        let desired = resources::render(machine, &settings()).unwrap();
        let config: Value =
            serde_json::from_str(&desired.config.data.as_ref().unwrap()["vm.json"]).unwrap();
        let pod = serde_json::to_value(&desired.pod).unwrap();
        let bmc = pod["spec"]["initContainers"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["name"] == "bmc")
            .map(env);
        (desired.hash, config, bmc)
    };
    let plain = vm();
    let (plain_hash, config, bmc) = rendered(&plain);
    assert!(config.get("admin_bucket").is_none());
    assert!(!bmc.unwrap().contains_key("BMC_ADMIN_BUCKET"));
    // An all-false bucket is the same as none: no restart, nothing rendered.
    let mut off = vm();
    off.spec.console.as_mut().unwrap().admin_bucket = Some(api::AdminBucket::default());
    let (off_hash, config, bmc) = rendered(&off);
    assert_eq!(off_hash, plain_hash);
    assert!(config.get("admin_bucket").is_none());
    assert!(!bmc.unwrap().contains_key("BMC_ADMIN_BUCKET"));
    for (bucket, names) in [
        (json!({"audio":true}), "audio"),
        (json!({"usbRedirect":true}), "usb-redirect"),
        (
            json!({"audio":true,"usbRedirect":true}),
            "audio,usb-redirect",
        ),
    ] {
        let mut machine = vm();
        machine.spec.console.as_mut().unwrap().admin_bucket =
            Some(serde_json::from_value(bucket.clone()).unwrap());
        let (hash, config, bmc) = rendered(&machine);
        assert_ne!(hash, plain_hash, "{bucket}");
        assert_eq!(
            config["admin_bucket"],
            json!({"audio":bucket["audio"] == true,"usb_redirect":bucket["usbRedirect"] == true})
        );
        assert_eq!(bmc.unwrap()["BMC_ADMIN_BUCKET"], names);
    }
    // Without the BMC that gates them, the features render nowhere.
    let mut headless = vm();
    headless.spec.console.as_mut().unwrap().admin_bucket = Some(api::AdminBucket {
        audio: true,
        usb_redirect: true,
    });
    headless.spec.components.bmc.enabled = false;
    let (_, config, bmc) = rendered(&headless);
    assert!(config.get("admin_bucket").is_none() && bmc.is_none());
    let crd = api::crd();
    let bucket = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["console"]["properties"]["adminBucket"]["properties"];
    for field in ["audio", "usbRedirect"] {
        assert_eq!(bucket[field]["default"], false, "{field}");
    }
    assert!(serde_json::from_value::<api::AdminBucket>(json!({"microphone":true})).is_err());
}

#[test]
fn redfish_history_uses_workload_tls_9091_and_denies_raw_prometheus_9090() {
    let machine = with_components(false, true);
    let pod = serde_json::to_value(&resources::render(&machine, &settings()).unwrap().pod).unwrap();
    let redfish = pod["spec"]["initContainers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "redfish")
        .cloned()
        .unwrap();
    let env = env(&redfish);
    assert_eq!(
        env["REDFISH_PROMETHEUS_URL"],
        "https://qrab-prometheus-history.monitoring.svc:9091"
    );
    // History queries are scoped to the Pod's own downward-API identity.
    assert_eq!(
        env["QRAB_POD_NAME"],
        json!({"fieldRef":{"fieldPath":"metadata.name"}})
    );
    assert_eq!(
        env["QRAB_POD_NAMESPACE"],
        json!({"fieldRef":{"fieldPath":"metadata.namespace"}})
    );

    let policy = resources::redfish_prometheus_policy(&machine, &settings())
        .unwrap()
        .unwrap();
    assert_eq!(policy.name_any(), "test-vm-redfish-prometheus");
    assert_eq!(policy.metadata.namespace.as_deref(), Some("qrab"));
    let owners = policy.metadata.owner_references.as_ref().unwrap();
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].uid, "owner-one");
    assert_eq!(owners[0].controller, Some(true));
    let spec = &policy.data["spec"];
    assert_eq!(
        spec["endpointSelector"],
        json!({"matchLabels":{"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm","qrab.yapplesauce.com/owner-uid":"owner-one"}})
    );
    // Additive egress only: no ingress, no default deny, one peer, one port.
    assert_eq!(
        spec["enableDefaultDeny"],
        json!({"ingress":false,"egress":false})
    );
    assert!(spec.get("ingress").is_none() && spec.get("ingressDeny").is_none());
    assert_eq!(
        spec["egress"],
        json!([{"toEndpoints":[{"matchLabels":{"k8s:io.kubernetes.pod.namespace":"monitoring","app.kubernetes.io/name":"prometheus"}}],
            "toPorts":[{"ports":[{"port":"9091","protocol":"TCP"}]}]}])
    );
    assert_eq!(
        spec["egressDeny"],
        json!([{"toEndpoints":[{"matchLabels":{"k8s:io.kubernetes.pod.namespace":"monitoring","app.kubernetes.io/name":"prometheus"}}],
        "toPorts":[{"ports":[{"port":"9090","protocol":"TCP"}]}]}])
    );
    // The BMC brings Redfish, so it gets the policy too; neither means none.
    assert!(
        resources::redfish_prometheus_policy(&with_components(true, false), &settings())
            .unwrap()
            .is_some()
    );
    let plain = with_components(false, false);
    assert!(resources::redfish_prometheus_policy(&plain, &settings())
        .unwrap()
        .is_none());
    let pod = serde_json::to_value(&resources::render(&plain, &settings()).unwrap().pod).unwrap();
    assert!(!pod.to_string().contains("REDFISH_PROMETHEUS_URL"));
}

#[test]
fn kvm_vms_with_nic_addresses_get_nic_root_ports() {
    let addressed = Settings {
        nic_addresses: Some(qrab_operator::address::Addressing {
            range: "10.244.128.0/29".parse().unwrap(),
            frr_namespace: "frr-k8s-system".into(),
        }),
        ..settings()
    };
    let config = |vm: &VirtualMachine, settings: &Settings| -> Value {
        let desired = resources::render(vm, settings).unwrap();
        serde_json::from_str(&desired.config.data.unwrap()["vm.json"]).unwrap()
    };
    let mut kvm = vm();
    kvm.spec.accelerator = qrab_operator::api::Accelerator::Kvm;
    assert_eq!(config(&kvm, &addressed)["nic_slots"], 4);
    // Without stable addresses no VM can have NICs, and TCG VMs never do.
    assert!(config(&kvm, &settings()).get("nic_slots").is_none());
    assert!(config(&vm(), &addressed).get("nic_slots").is_none());
}

#[test]
fn a_vpn_nic_anchor_mounts_its_tunnel_config_and_the_guest_uses_dhcp() {
    use qrab_operator::nic::{self, NicEndpoint};
    let mut vm = networked();
    vm.spec.nics[0].vpn = Some(qrab_operator::api::NicVpn {
        secret_name: "vpn-proton".into(),
    });
    let pool = "qrab-nic-10-244-128-7";
    let anchor = nic::render_anchor(&vm, &settings(), &vm.spec.nics[0], pool, "kube02").unwrap();
    let pod = serde_json::to_value(&anchor.pod).unwrap();
    assert_eq!(
        pod["metadata"]["annotations"][qrab_netd::vpn::VPN_ANNOTATION],
        "wireguard"
    );
    assert_eq!(
        pod["spec"]["volumes"],
        json!([{"name":"vpn","secret":{"secretName":"vpn-proton","items":[{"key":"wg.conf","path":"wg.conf"}],"defaultMode":0o400}}])
    );
    assert_eq!(
        pod["spec"]["containers"][0]["volumeMounts"],
        json!([{"name":"vpn","mountPath":"/run/qrab-vpn","readOnly":true}])
    );
    // The anchor stays restricted: no added capability, no privilege.
    assert_eq!(
        pod["spec"]["containers"][0]["securityContext"]["capabilities"]["drop"],
        json!(["ALL"])
    );
    // The plain NIC's anchor carries neither.
    let plain = nic::render_anchor(&vm, &settings(), &vm.spec.nics[1], pool, "kube02").unwrap();
    let plain = serde_json::to_value(&plain.pod).unwrap();
    assert!(plain["spec"].get("volumes").is_none());
    assert!(plain["metadata"]["annotations"]
        .get(qrab_netd::vpn::VPN_ANNOTATION)
        .is_none());
    // The guest configures the VPN NIC by DHCP (only the Secret knows its address).
    let endpoints = [
        NicEndpoint {
            name: "lan".into(),
            ip: "10.0.3.7".parse().unwrap(),
        },
        NicEndpoint {
            name: "backup".into(),
            ip: "10.0.3.8".parse().unwrap(),
        },
    ];
    let desired = resources::render_attached(
        &vm,
        &settings(),
        &[],
        resources::GuestSize::of(&vm),
        &endpoints,
    )
    .unwrap();
    let config: Value = serde_json::from_str(&desired.config.data.unwrap()["vm.json"]).unwrap();
    let network = config["seed"]["network_config"].as_str().unwrap();
    let lan = &network[network.find("  eth0:").unwrap()..network.find("  eth1:").unwrap()];
    assert!(
        lan.contains("    dhcp4: true\n") && !lan.contains("addresses"),
        "{lan}"
    );
    assert!(network[network.find("  eth1:").unwrap()..].contains("- \"10.0.3.8/32\""));
    // Only Secrets named vpn-* can be referenced.
    let crd = include_str!("../../../contrib/kubernetes/operator/crd.json");
    assert!(crd.contains(r#""pattern": "^vpn-[a-z0-9]([-a-z0-9]{0,243}[a-z0-9])?$""#));
}
