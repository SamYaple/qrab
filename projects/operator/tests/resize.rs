use k8s_openapi::api::core::v1::Pod;
use qrab_operator::{
    api::{self, VirtualMachine},
    resources::{self, GuestSize, ResizePlan, Settings},
};
use serde_json::{json, Value};

fn settings() -> Settings {
    Settings {
        namespace: "qrab".into(),
        vm_image: format!("zot.yapplesauce.com/qrab/manager@sha256:{}", "a".repeat(64)),
        oidc_issuer: "https://idm.yaple.net/oauth2/openid/qrab-bmc".into(),
        oidc_client_id: "qrab-bmc".into(),
        idp_ip: None,
        redfish_oidc_issuer: "https://idm.yaple.net/oauth2/openid/qrab-redfish".into(),
        redfish_oidc_audience: "qrab-redfish".into(),
        bmc_bearer: None,
    }
}

fn vm() -> VirtualMachine {
    serde_json::from_value(json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine","metadata":{"name":"test-vm","namespace":"qrab","uid":"owner-one","generation":1},"spec":{"stateVolumeClaim":"test-state","console":{"publicUrl":"https://test-vm.yapplesauce.com","allowedSubjects":["6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b"]}}})).unwrap()
}

/// The stored, running Pod for a VM booted at its current spec size.
fn running(vm: &VirtualMachine) -> Pod {
    let mut pod = serde_json::to_value(resources::render(vm, &settings()).unwrap().pod).unwrap();
    pod["metadata"]["uid"] = json!("pod-one");
    pod["metadata"]["generation"] = json!(1);
    let limits = pod["spec"]["containers"][0]["resources"].clone();
    pod["status"] = json!({"phase":"Running","observedGeneration":1,"containerStatuses":[{"name":"manager","ready":true,"restartCount":0,"image":"x","imageID":"x","resources":limits}]});
    serde_json::from_value(pod).unwrap()
}

fn rerender(vm: &VirtualMachine, pod: &Pod) -> resources::Desired {
    resources::render_booted(vm, &settings(), &[], resources::boot_size(pod).unwrap()).unwrap()
}

fn config(desired: &resources::Desired) -> Value {
    serde_json::from_str(&desired.config.data.as_ref().unwrap()["vm.json"]).unwrap()
}

#[test]
fn vm_container_is_sized_from_spec_and_resizable_without_restart() {
    let desired = resources::render(&vm(), &settings()).unwrap();
    let pod = serde_json::to_value(&desired.pod).unwrap();
    let container = &pod["spec"]["containers"][0];
    assert_eq!(
        container["resources"],
        json!({"requests":{"cpu":"1","memory":"768Mi"},"limits":{"cpu":"2","memory":"1Gi"}})
    );
    assert_eq!(
        container["resizePolicy"],
        json!([{"resourceName":"cpu","restartPolicy":"NotRequired"},{"resourceName":"memory","restartPolicy":"NotRequired"}])
    );
    let size = pod["spec"]["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "size")
        .unwrap();
    assert_eq!(
        size["downwardAPI"]["items"][1]["resourceFieldRef"],
        json!({"containerName":"manager","resource":"limits.memory","divisor":"1Mi"})
    );
    assert_eq!(
        pod["metadata"]["annotations"][resources::BOOT_SIZE],
        r#"{"cpus":1,"memoryMiB":512}"#
    );
    // TCG guests refuse hot-added vCPUs; memory gets 4x virtio-mem headroom.
    assert_eq!(
        config(&desired)["hotplug"],
        json!({"max_cpus":1,"max_memory_mib":2048,"limits_dir":"/run/qrab-size","cpu_overhead_millis":1000,"memory_overhead_mib":512})
    );
    assert!(resources::pod_matches(&desired.pod, &desired));
    assert_eq!(
        GuestSize::from_limits(&container["resources"]["limits"]),
        Some(GuestSize {
            cpus: 1,
            memory_mib: 512
        })
    );
}

#[test]
fn memory_growth_patches_the_resize_subresource_and_keeps_the_boot_template() {
    let mut vm = vm();
    let pod = running(&vm);
    let booted = rerender(&vm, &pod);
    vm.spec.memory_mib = 1024;
    let desired = rerender(&vm, &pod);
    assert_eq!(desired.hash, booted.hash);
    assert_eq!(desired.config.data, booted.config.data);
    assert!(
        resources::pod_matches(&pod, &desired),
        "resize is not a template change"
    );
    assert_eq!(
        resources::plan_resize(&vm, &pod, &desired),
        ResizePlan::Grow(
            json!({"metadata":{"uid":"pod-one"},"spec":{"containers":[{"name":"manager","resources":{"requests":{"cpu":"1","memory":"1280Mi"},"limits":{"cpu":"2","memory":"1536Mi"}}}]}})
        )
    );
    // A fresh Pod for the same spec would boot at the new size instead.
    assert_ne!(
        resources::render(&vm, &settings()).unwrap().hash,
        booted.hash
    );
}

#[test]
fn shrink_vcpu_growth_on_tcg_beyond_maxima_and_unstarted_pods_restart() {
    let base = vm();
    let pod = running(&base);
    let plan = |change: &dyn Fn(&mut VirtualMachine), pod: &Pod| {
        let mut vm = base.clone();
        change(&mut vm);
        resources::plan_resize(&vm, pod, &rerender(&vm, pod))
    };
    assert_eq!(plan(&|_| {}, &pod), ResizePlan::Steady);
    assert!(
        matches!(plan(&|vm| vm.spec.cpus = 2, &pod), ResizePlan::Restart(m) if m.contains("maximum"))
    );
    assert!(
        matches!(plan(&|vm| vm.spec.memory_mib = 4096, &pod), ResizePlan::Restart(m) if m.contains("maximum"))
    );
    let mut pending = pod.clone();
    pending.status.as_mut().unwrap().phase = Some("Pending".into());
    assert!(
        matches!(plan(&|vm| vm.spec.memory_mib = 1024, &pending), ResizePlan::Restart(m) if m.contains("not started"))
    );
    let mut larger = base.clone();
    larger.spec.memory_mib = 1024;
    let grown = running(&larger);
    assert!(
        matches!(plan(&|vm| vm.spec.memory_mib = 768, &grown), ResizePlan::Restart(m) if m.contains("shrink"))
    );
    // Changing a ceiling is a boot-time change: the template hash moves.
    let mut ceiling = base.clone();
    ceiling.spec.max_memory_mib = Some(8192);
    assert_ne!(rerender(&ceiling, &pod).hash, rerender(&base, &pod).hash);
    assert_eq!(
        config(&rerender(&ceiling, &pod))["hotplug"]["max_memory_mib"],
        8192
    );
    ceiling.spec.max_memory_mib = Some(256);
    assert!(
        resources::render(&ceiling, &settings()).is_err(),
        "ceiling below memoryMiB"
    );
}

#[test]
fn kubelet_resize_conditions_are_reported_in_vm_status() {
    let mut vm = vm();
    let pod = running(&vm);
    let desired = rerender(&vm, &pod);
    let status = resources::resize_status(&pod, &desired, None);
    assert_eq!(
        (status.phase.as_str(), status.applied_memory_mib),
        ("Applied", Some(512))
    );
    assert_eq!(
        (
            status.boot_memory_mib,
            status.max_memory_mib,
            status.max_cpus
        ),
        (512, 2048, 1)
    );
    vm.spec.memory_mib = 1024;
    let desired = rerender(&vm, &pod);
    let ResizePlan::Grow(patch) = resources::plan_resize(&vm, &pod, &desired) else {
        panic!("growth expected");
    };
    let mut patched = serde_json::to_value(&pod).unwrap();
    patched["spec"]["containers"][0]["resources"] =
        patch["spec"]["containers"][0]["resources"].clone();
    patched["metadata"]["generation"] = json!(2);
    let observe = |pod: &Value, conditions: Value| {
        let mut pod = pod.clone();
        pod["status"]["observedGeneration"] = json!(2);
        pod["status"]["conditions"] = conditions;
        serde_json::from_value::<Pod>(pod).unwrap()
    };
    let proposed: Pod = serde_json::from_value(patched.clone()).unwrap();
    assert_eq!(
        resources::resize_status(&proposed, &desired, None).phase,
        "Proposed"
    );
    let deferred = observe(
        &patched,
        json!([{"type":"PodResizePending","status":"True","reason":"Deferred","message":"Node didn't have enough resource: memory"}]),
    );
    let status = resources::resize_status(&deferred, &desired, None);
    assert_eq!(
        (
            status.phase.as_str(),
            status.memory_mib,
            status.applied_memory_mib
        ),
        ("Deferred", 1024, Some(512))
    );
    assert!(status.message.contains("enough resource"));
    let progress = observe(
        &patched,
        json!([{"type":"PodResizeInProgress","status":"True"}]),
    );
    assert_eq!(
        resources::resize_status(&progress, &desired, None).phase,
        "InProgress"
    );
    let failed = observe(
        &patched,
        json!([{"type":"PodResizeInProgress","status":"True","reason":"Error","message":"cgroup write failed"}]),
    );
    assert_eq!(
        resources::resize_status(&failed, &desired, None).phase,
        "Error"
    );
    let rejected = resources::resize_status(
        &pod,
        &desired,
        Some("node didn't have enough allocatable resources: cpu".into()),
    );
    assert_eq!(rejected.phase, "Infeasible");
    let mut applied = patched.clone();
    applied["status"]["observedGeneration"] = json!(2);
    applied["status"]["containerStatuses"][0]["resources"] =
        patched["spec"]["containers"][0]["resources"].clone();
    let applied: Pod = serde_json::from_value(applied).unwrap();
    let status = resources::resize_status(&applied, &desired, None);
    assert_eq!(
        (status.phase.as_str(), status.applied_memory_mib),
        ("Applied", Some(1024))
    );
    assert_eq!(
        resources::plan_resize(&vm, &applied, &desired),
        ResizePlan::Steady
    );
}

/// Live resize of the rendered VM container in a disposable namespace. The
/// manager image, its ServiceAccount and probes are swapped for a sleeping
/// public image; resources, resizePolicy and the downward-API volume are kept.
#[tokio::test]
#[ignore = "requires a disposable namespace (QRAB_TEST_NAMESPACE) and a pullable image (QRAB_TEST_IMAGE)"]
async fn live_rendered_pod_grows_in_place_without_a_restart() {
    use kube::api::{DeleteParams, Patch, PatchParams, PostParams};
    let namespace = std::env::var("QRAB_TEST_NAMESPACE").unwrap();
    let image = std::env::var("QRAB_TEST_IMAGE").unwrap();
    let client = kube::Client::try_default().await.unwrap();
    let pods: kube::Api<Pod> = kube::Api::namespaced(client, &namespace);
    let mut settings = settings();
    settings.namespace = namespace;
    let mut vm = vm();
    let desired = resources::render(&vm, &settings).unwrap();
    let mut pod = serde_json::to_value(&desired.pod).unwrap();
    pod["metadata"]
        .as_object_mut()
        .unwrap()
        .remove("ownerReferences");
    let spec = pod["spec"].as_object_mut().unwrap();
    spec.remove("serviceAccountName");
    let config = spec["volumes"].as_array_mut().unwrap();
    config.retain(|v| v["name"] != "config" && v["name"] != "state");
    let container = spec["containers"][0].as_object_mut().unwrap();
    for field in ["startupProbe", "readinessProbe", "livenessProbe", "args"] {
        container.remove(field);
    }
    container.insert("image".into(), image.into());
    container.insert("imagePullPolicy".into(), "IfNotPresent".into());
    container.insert("command".into(), json!(["sleep", "3600"]));
    container["volumeMounts"]
        .as_array_mut()
        .unwrap()
        .retain(|m| m["name"] != "config" && m["name"] != "state");
    let created = pods
        .create(
            &PostParams::default(),
            &serde_json::from_value(pod).unwrap(),
        )
        .await
        .unwrap();
    let name = kube::ResourceExt::name_any(&created);
    let wait = |check: fn(&Pod) -> bool| {
        let pods = pods.clone();
        let name = name.clone();
        async move {
            for _ in 0..120 {
                let pod = pods.get(&name).await.unwrap();
                if check(&pod) {
                    return pod;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            panic!("timed out waiting for the resize test Pod");
        }
    };
    let running =
        wait(|p| p.status.as_ref().and_then(|s| s.phase.as_deref()) == Some("Running")).await;
    vm.spec.memory_mib = 1024;
    let desired =
        resources::render_booted(&vm, &settings, &[], resources::boot_size(&running).unwrap())
            .unwrap();
    let ResizePlan::Grow(patch) = resources::plan_resize(&vm, &running, &desired) else {
        panic!("growth expected");
    };
    pods.patch_subresource(
        "resize",
        &name,
        &PatchParams::default(),
        &Patch::Strategic(patch),
    )
    .await
    .unwrap();
    let grown = wait(|p| {
        serde_json::to_value(p).unwrap()["status"]["containerStatuses"][0]["resources"]["limits"]
            ["memory"]
            == "1536Mi"
    })
    .await;
    let status = resources::resize_status(&grown, &desired, None);
    let restarts = grown
        .status
        .as_ref()
        .unwrap()
        .container_statuses
        .as_ref()
        .unwrap()[0]
        .restart_count;
    pods.delete(&name, &DeleteParams::default()).await.unwrap();
    assert_eq!(
        (status.phase.as_str(), status.memory_mib, restarts),
        ("Applied", 1024, 0)
    );
    assert_eq!(
        resources::plan_resize(&vm, &grown, &desired),
        ResizePlan::Steady
    );
}

#[test]
fn kvm_is_opt_in_requests_the_device_and_grows_vcpus_in_place() {
    let tcg = vm();
    let mut explicit = vm();
    explicit.spec = serde_json::from_value(
        json!({"stateVolumeClaim":"test-state","accelerator":"tcg","console":{"publicUrl":"https://test-vm.yapplesauce.com","allowedSubjects":["6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b"]}}),
    )
    .unwrap();
    let default = resources::render(&tcg, &settings()).unwrap();
    assert_eq!(
        resources::render(&explicit, &settings()).unwrap().hash,
        default.hash
    );
    assert!(serde_json::to_value(&tcg.spec)
        .unwrap()
        .get("accelerator")
        .is_none());
    let mut kvm = vm();
    kvm.spec.accelerator = api::Accelerator::Kvm;
    let desired = resources::render(&kvm, &settings()).unwrap();
    assert_ne!(
        desired.hash, default.hash,
        "switching accelerator restarts the VM"
    );
    let pod = serde_json::to_value(&desired.pod).unwrap();
    let container = &pod["spec"]["containers"][0];
    for side in ["requests", "limits"] {
        assert_eq!(container["resources"][side][resources::KVM_RESOURCE], "1");
    }
    assert!(serde_json::to_string(&pod)
        .unwrap()
        .find("hostPath")
        .is_none());
    assert!(container["securityContext"]["privileged"] == false);
    assert_eq!(
        pod["spec"]["hostUsers"], true,
        "CDI device modes need the host user namespace"
    );
    let mut user_namespaced = pod["spec"].clone();
    user_namespaced["hostUsers"] = json!(false);
    assert!(resources::check_kvm_user_namespace(&user_namespaced).is_err());
    let tcg_pod = serde_json::to_value(&default.pod).unwrap();
    assert!(
        tcg_pod["spec"].get("hostUsers").is_none(),
        "TCG Pods are unchanged"
    );
    let mut tcg_user_namespaced = tcg_pod["spec"].clone();
    tcg_user_namespaced["hostUsers"] = json!(false);
    assert!(resources::check_kvm_user_namespace(&tcg_user_namespaced).is_ok());
    assert_eq!(config(&desired)["accelerator"], "kvm");
    assert_eq!(config(&desired)["hotplug"]["max_cpus"], 4);
    kvm.spec.max_cpus = Some(6);
    let pod = running(&kvm);
    kvm.spec.cpus = 3;
    let desired = rerender(&kvm, &pod);
    assert_eq!(config(&desired)["hotplug"]["max_cpus"], 6);
    let ResizePlan::Grow(patch) = resources::plan_resize(&kvm, &pod, &desired) else {
        panic!("KVM vCPU growth is in place");
    };
    assert_eq!(
        patch["spec"]["containers"][0]["resources"],
        json!({"requests":{"cpu":"3","memory":"768Mi",resources::KVM_RESOURCE:"1"},"limits":{"cpu":"4","memory":"1Gi",resources::KVM_RESOURCE:"1"}})
    );
    kvm.spec.max_cpus = None;
    kvm.spec.cpus = 5;
    assert!(
        matches!(resources::plan_resize(&kvm, &pod, &rerender(&kvm, &pod)), ResizePlan::Restart(m) if m.contains("maximum"))
    );
    let schema = &api::crd()["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]
        ["spec"]["properties"]["accelerator"];
    assert_eq!(schema["enum"], json!(["kvm", "tcg"]));
    assert!(
        schema.get("default").is_none(),
        "absent means tcg; stored CRs gain no field"
    );
}

#[test]
fn crd_validates_hotplug_ceilings_and_publishes_resize_status() {
    let crd = api::crd();
    let schema = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"];
    let rules: Vec<_> = schema["x-kubernetes-validations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["rule"].as_str().unwrap())
        .collect();
    assert!(rules.contains(&"!has(self.spec.maxCpus) || self.spec.cpus <= self.spec.maxCpus"));
    assert!(rules.contains(
        &"!has(self.spec.maxMemoryMiB) || self.spec.memoryMiB <= self.spec.maxMemoryMiB"
    ));
    let spec = &schema["properties"]["spec"]["properties"];
    for field in ["maxCpus", "maxMemoryMiB"] {
        assert!(spec[field].get("nullable").is_none());
        assert!(spec[field].get("default").is_none());
    }
    assert_eq!(spec["maxCpus"]["maximum"].as_f64(), Some(256.0));
    assert!(
        schema["properties"]["status"]["properties"]["resize"]["properties"]["phase"].is_object()
    );
}
