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
    vm.spec.console.as_mut().unwrap().public_url = "http://test-vm.yapplesauce.com".into();
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
    assert_eq!(pod["spec"]["volumes"][5]["secret"]["defaultMode"], 288);
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
        .unwrap()[5]
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
fn existing_specs_keep_the_bmc_and_omit_redfish() {
    let spec: api::VirtualMachineSpec = serde_json::from_value(
        json!({"stateVolumeClaim":"s","console":{"publicUrl":"https://x.example","allowedSubjects":[]}}),
    )
    .unwrap();
    assert!(spec.components.bmc.enabled);
    assert!(!spec.components.redfish.enabled);
    let pod = serde_json::to_value(resources::render(&vm(), &settings()).unwrap().pod).unwrap();
    let names: Vec<_> = pod["spec"]["initContainers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["bmc"]);
    assert!(!env(&pod["spec"]["initContainers"][0]).contains_key("BMC_VIEWER_SUBJECTS"));
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
        let expected: Vec<_> = [("bmc", bmc), ("redfish", redfish)]
            .into_iter()
            .filter_map(|(name, on)| on.then_some(name))
            .collect();
        assert_eq!(names, expected);
        for sidecar in &sidecars {
            assert_eq!(sidecar["restartPolicy"], "Always");
            assert_eq!(sidecar["image"], manager["image"]);
            assert_eq!(
                sidecar["args"],
                json!(["--socket", resources::CONTROL_SOCKET])
            );
            // Only the control socket volume: no QMP/VNC runtime, state, Secrets or config.
            assert_eq!(
                sidecar["volumeMounts"],
                json!([{"name":"control","mountPath":"/run/qrab-control"}])
            );
            assert!(sidecar.get("startupProbe").is_none());
            assert_eq!(sidecar["securityContext"], manager["securityContext"]);
        }
        let ports: Vec<_> = sidecars
            .iter()
            .flat_map(|c| c["ports"].as_array().unwrap().clone())
            .map(|p| p["containerPort"].as_u64().unwrap())
            .collect();
        let expected_ports: Vec<u64> = [(8080, bmc), (8081, redfish)]
            .into_iter()
            .filter_map(|(port, on)| on.then_some(port))
            .collect();
        assert_eq!(ports, expected_ports);
        if redfish {
            let redfish = env(sidecars.iter().find(|c| c["name"] == "redfish").unwrap());
            assert_eq!(redfish["REDFISH_OIDC_AUDIENCE"], "qrab-redfish");
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
