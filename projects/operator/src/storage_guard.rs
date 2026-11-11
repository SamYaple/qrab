//! Keep native storage ownership until the old process has demonstrably exited.
//! A status recovery acknowledgment is an administrator statement that target
//! sessions were hard-fenced and CHAP rotated, never a nonce/timeout substitute.
use super::{delete, publish, state, Context};
use crate::{api::VirtualMachine, resources, Error, Result};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{ListParams, Patch, PatchParams},
    runtime::controller::Action,
    Api, Resource, ResourceExt,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};

pub const POD_FINALIZER: &str = "qrab.yapplesauce.com/iscsi-drain";
pub const VM_FINALIZER: &str = "qrab.yapplesauce.com/iscsi-state";
pub const OWNER_GATE: &str = "qrab.yapplesauce.com/iscsi-owner";

fn has_owner_gate(pod: &Pod) -> bool {
    pod.spec
        .as_ref()
        .and_then(|s| s.scheduling_gates.as_ref())
        .is_some_and(|g| g.iter().any(|g| g.name == OWNER_GATE))
}
fn never_launched(pod: &Pod) -> bool {
    // Gates cannot be added after creation, and our renderer never sets
    // nodeName directly. With this gate and no binding, no kubelet ran it.
    has_owner_gate(pod) && pod.spec.as_ref().is_some_and(|s| s.node_name.is_none())
}
pub(super) async fn open_owner_gate(
    vm: &VirtualMachine,
    pods: &Api<Pod>,
    pod: &Pod,
) -> Result<bool> {
    if !has_owner_gate(pod)
        || !vm.spec.running
        || vm.metadata.deletion_timestamp.is_some()
        || pod.metadata.deletion_timestamp.is_some()
        || vm.status.as_ref().and_then(|s| s.pod_uid.as_deref()) != pod.uid().as_deref()
    {
        return Ok(false);
    }
    let gates: Vec<_> = pod
        .spec
        .as_ref()
        .and_then(|s| s.scheduling_gates.as_ref())
        .into_iter()
        .flatten()
        .filter(|g| g.name != OWNER_GATE)
        .cloned()
        .collect();
    pods.patch(&pod.name_any(), &PatchParams::default(), &Patch::Merge(json!({"metadata":{"uid":pod.uid(),"resourceVersion":pod.resource_version()},"spec":{"schedulingGates":gates}}))).await?;
    Ok(true)
}

fn has_finalizer<K: Resource>(resource: &K, name: &str) -> bool {
    resource
        .meta()
        .finalizers
        .as_ref()
        .is_some_and(|v| v.iter().any(|s| s == name))
}

pub(super) fn active(vm: &VirtualMachine, pod: Option<&Pod>) -> bool {
    !vm.spec.iscsi_disks.is_empty()
        || has_finalizer(vm, VM_FINALIZER)
        || pod.is_some_and(|p| has_finalizer(p, POD_FINALIZER))
}

pub(super) fn unreleased_uid(vm: &VirtualMachine) -> Option<&str> {
    if !active(vm, None) {
        return None;
    }
    let status = vm.status.as_ref()?;
    let uid = status.pod_uid.as_deref()?;
    (status.storage_released_pod_uid.as_deref() != Some(uid)).then_some(uid)
}

#[derive(Debug, PartialEq, Eq)]
enum Exit {
    Clean,
    Active,
    Unknown,
}

fn exit(pod: &Pod) -> Exit {
    if never_launched(pod) {
        return Exit::Clean;
    }
    let Some(spec) = &pod.spec else {
        return Exit::Unknown;
    };
    let Some(status) = &pod.status else {
        return Exit::Unknown;
    };
    if spec.containers.is_empty() {
        return Exit::Unknown;
    }
    let names = spec
        .containers
        .iter()
        .map(|c| c.name.as_str())
        .chain(
            spec.init_containers
                .iter()
                .flatten()
                .map(|c| c.name.as_str()),
        )
        .chain(
            spec.ephemeral_containers
                .iter()
                .flatten()
                .map(|c| c.name.as_str()),
        );
    let statuses: Vec<_> = status
        .container_statuses
        .iter()
        .flatten()
        .chain(status.init_container_statuses.iter().flatten())
        .chain(status.ephemeral_container_statuses.iter().flatten())
        .collect();
    let mut running = false;
    for name in names {
        let Some(container) = statuses.iter().find(|s| s.name == name) else {
            return Exit::Unknown;
        };
        let Some(state) = &container.state else {
            return Exit::Unknown;
        };
        if let Some(done) = &state.terminated {
            if done.exit_code != 0 || done.signal.is_some_and(|n| n != 0) {
                return Exit::Unknown;
            }
        } else if state.running.is_some() || state.waiting.is_some() {
            running = true;
        } else {
            return Exit::Unknown;
        }
    }
    if running {
        Exit::Active
    } else {
        Exit::Clean
    }
}

fn recovered(vm: &VirtualMachine, uid: &str) -> bool {
    vm.status
        .as_ref()
        .and_then(|s| s.storage_recovery.as_deref())
        == Some(uid)
}
fn released(vm: &VirtualMachine, uid: &str) -> bool {
    vm.status
        .as_ref()
        .and_then(|s| s.storage_released_pod_uid.as_deref())
        == Some(uid)
}

async fn vm_finalizer(vms: &Api<VirtualMachine>, vm: &VirtualMachine, add: bool) -> Result<()> {
    let mut finalizers = vm.finalizers().to_vec();
    finalizers.retain(|s| s != VM_FINALIZER);
    if add {
        finalizers.push(VM_FINALIZER.into());
    }
    vms.patch(&vm.name_any(), &PatchParams::default(), &Patch::Merge(json!({"metadata":{"uid":vm.uid(),"resourceVersion":vm.resource_version(),"finalizers":finalizers}}))).await?;
    Ok(())
}
async fn pod_finalizer(pods: &Api<Pod>, pod: &Pod, add: bool) -> Result<()> {
    let mut finalizers = pod.finalizers().to_vec();
    finalizers.retain(|s| s != POD_FINALIZER);
    if add {
        finalizers.push(POD_FINALIZER.into());
    }
    pods.patch(&pod.name_any(), &PatchParams::default(), &Patch::Merge(json!({"metadata":{"uid":pod.uid(),"resourceVersion":pod.resource_version(),"finalizers":finalizers}}))).await?;
    Ok(())
}
async fn blocked(
    vms: &Api<VirtualMachine>,
    vm: &VirtualMachine,
    pod: Option<&Pod>,
    message: &str,
) -> Result<Option<Action>> {
    publish(
        vms,
        vm,
        state(vm, pod, "Blocked", false, "StorageFenceRequired", message),
    )
    .await?;
    Ok(Some(Action::requeue(Duration::from_secs(30))))
}
async fn record_release(
    vms: &Api<VirtualMachine>,
    vm: &VirtualMachine,
    pod: Option<&Pod>,
    uid: &str,
    manual: bool,
) -> Result<Option<Action>> {
    let mut status = state(
        vm,
        pod,
        "Stopping",
        false,
        "StorageReleased",
        if manual {
            "Administrator acknowledged hard target fencing and CHAP rotation"
        } else {
            "All VM containers exited cleanly; QEMU has closed its storage sessions"
        },
    );
    status.storage_released_pod_uid = Some(uid.into());
    // Retain a manual acknowledgment until this Pod is gone. This distinguishes
    // a hard target fence from a stale clean release after an in-place restart.
    if !manual {
        status.storage_recovery = None;
    }
    publish(vms, vm, status).await?;
    // A separate reconcile observes the durable status before finalizer removal.
    Ok(Some(Action::requeue(Duration::from_secs(1))))
}

/// Runs before normal spec validation: deletion must still drain a VM whose
/// newer desired spec is invalid. Returns None only when normal reconcile is safe.
pub(super) async fn reconcile(
    vm: &VirtualMachine,
    pod: Option<&Pod>,
    vms: &Api<VirtualMachine>,
    pods: &Api<Pod>,
) -> Result<Option<Action>> {
    if !active(vm, pod) {
        return Ok(None);
    }
    let deleting = vm.metadata.deletion_timestamp.is_some();
    if !has_finalizer(vm, VM_FINALIZER) && !deleting {
        vm_finalizer(vms, vm, true).await?;
        return Ok(Some(Action::requeue(Duration::from_secs(1))));
    }
    if let Some(pod) = pod {
        if !resources::owned(pod, vm) {
            return Err(Error::Ownership(
                "storage guard refuses a Pod belonging to another VM UID".into(),
            ));
        }
        if !has_finalizer(pod, POD_FINALIZER) && pod.metadata.deletion_timestamp.is_none() {
            pod_finalizer(pods, pod, true).await?;
            return Ok(Some(Action::requeue(Duration::from_secs(1))));
        }
        let uid = pod
            .uid()
            .ok_or_else(|| Error::Invalid("VM Pod has no UID".into()))?;
        if let Some(old) = unreleased_uid(vm).filter(|old| *old != uid) {
            if recovered(vm, old) {
                return record_release(vms, vm, None, old, true).await;
            }
            return blocked(vms,vm,None,"The previous Pod vanished without a clean storage release; hard-fence the target, rotate CHAP and acknowledge its UID in status.storageRecovery").await;
        }
        if vm.status.as_ref().and_then(|s| s.pod_uid.as_deref()) != Some(uid.as_str()) {
            let mut observed = state(
                vm,
                Some(pod),
                "Pending",
                false,
                "StorageOwnerObserved",
                "Tracking this Pod UID before any replacement can be considered",
            );
            observed.storage_recovery = None;
            publish(vms, vm, observed).await?;
            return Ok(Some(Action::requeue(Duration::from_secs(1))));
        }
        if recovered(vm, &uid) && pod.metadata.deletion_timestamp.is_none() {
            delete(pods, pod).await?;
            publish(vms,vm,state(vm,Some(pod),"Stopping",false,"StorageRecoveryStopping","Administrator hard-fenced storage; terminating the old process before replacement")).await?;
            return Ok(Some(Action::requeue(Duration::from_secs(2))));
        }
        if released(vm, &uid)
            && !recovered(vm, &uid)
            && (exit(pod) != Exit::Clean
                || (has_owner_gate(pod)
                    && vm.spec.running
                    && !deleting
                    && pod.metadata.deletion_timestamp.is_none()))
        {
            let mut status=state(vm,Some(pod),"Stopping",false,"StorageReleaseInvalidated","This Pod has a new or uncertain process incarnation; the old clean release is no longer valid");
            status.storage_released_pod_uid = None;
            publish(vms, vm, status).await?;
            return Ok(Some(Action::requeue(Duration::from_secs(1))));
        }
        if (deleting || !vm.spec.running) && pod.metadata.deletion_timestamp.is_none() {
            delete(pods, pod).await?;
            publish(
                vms,
                vm,
                state(
                    vm,
                    Some(pod),
                    "Stopping",
                    false,
                    if deleting {
                        "DeletingVM"
                    } else {
                        "PowerOffRequested"
                    },
                    "Waiting for the VM process to drain native storage",
                ),
            )
            .await?;
            return Ok(Some(Action::requeue(Duration::from_secs(2))));
        }
        if !released(vm, &uid) {
            if recovered(vm, &uid) {
                return record_release(vms, vm, Some(pod), &uid, true).await;
            }
            if exit(pod) == Exit::Clean
                && (!never_launched(pod)
                    || pod.metadata.deletion_timestamp.is_some()
                    || deleting
                    || !vm.spec.running)
            {
                return record_release(vms, vm, Some(pod), &uid, false).await;
            }
            if pod.metadata.deletion_timestamp.is_some() {
                if exit(pod) == Exit::Active {
                    publish(
                        vms,
                        vm,
                        state(
                            vm,
                            Some(pod),
                            "Stopping",
                            false,
                            "StorageDraining",
                            "Waiting for all VM containers to terminate cleanly",
                        ),
                    )
                    .await?;
                    return Ok(Some(Action::requeue(Duration::from_secs(2))));
                }
                return blocked(vms,vm,Some(pod),"The old Pod exit is failed or unknown; hard-fence the target, rotate CHAP and acknowledge this Pod UID in status.storageRecovery").await;
            }
            if pod
                .status
                .as_ref()
                .and_then(|s| s.phase.as_deref())
                .is_some_and(|p| p == "Failed" || p == "Succeeded")
            {
                return blocked(vms,vm,Some(pod),"The VM did not exit cleanly; automatic storage reattachment is blocked until explicit administrator fencing recovery").await;
            }
        }
        if pod.metadata.deletion_timestamp.is_some() {
            if has_finalizer(pod, POD_FINALIZER) {
                pod_finalizer(pods, pod, false).await?;
            }
            return Ok(Some(Action::requeue(Duration::from_secs(2))));
        }
        return Ok(None);
    }
    if let Some(old) = unreleased_uid(vm) {
        if recovered(vm, old) {
            return record_release(vms, vm, None, old, true).await;
        }
        return blocked(vms,vm,None,"The old Pod is missing without a recorded clean release; hard-fence the target, rotate CHAP and acknowledge its UID in status.storageRecovery").await;
    }
    if vm.status.as_ref().is_some_and(|s| {
        s.storage_recovery.is_some() && s.storage_recovery == s.storage_released_pod_uid
    }) {
        let mut status = state(
            vm,
            None,
            "Stopping",
            false,
            "StorageRecoveryComplete",
            "The fenced old Pod is gone; consumed its administrator acknowledgment",
        );
        status.storage_recovery = None;
        publish(vms, vm, status).await?;
        return Ok(Some(Action::requeue(Duration::from_secs(1))));
    }
    if deleting {
        if has_finalizer(vm, VM_FINALIZER) {
            vm_finalizer(vms, vm, false).await?;
        }
        return Ok(Some(Action::requeue(Duration::from_secs(2))));
    }
    Ok(None)
}

/// A forced parent deletion bypasses the normal finalizer path. Clean orphans
/// can be collected; failed/unknown ones stay as visible fencing tombstones.
pub(super) async fn sweep_orphans(ctx: Arc<Context>) -> Result<()> {
    let pods: Api<Pod> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let vms: Api<VirtualMachine> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    for pod in pods
        .list(&ListParams::default().labels("app.kubernetes.io/name=qrab"))
        .await?
    {
        if !has_finalizer(&pod, POD_FINALIZER) {
            continue;
        }
        let Some(owner) = pod.owner_references().iter().find(|o| {
            o.controller == Some(true)
                && o.kind == "VirtualMachine"
                && o.api_version == VirtualMachine::api_version(&())
        }) else {
            continue;
        };
        if pod.labels().get("qrab.yapplesauce.com/owner-uid") != Some(&owner.uid) {
            continue;
        }
        if vms
            .get_opt(&owner.name)
            .await?
            .is_some_and(|vm| vm.uid().as_deref() == Some(owner.uid.as_str()))
        {
            continue;
        }
        if pod.metadata.deletion_timestamp.is_none() {
            delete(&pods, &pod).await?;
        } else if exit(&pod) == Exit::Clean {
            pod_finalizer(&pods, &pod, false).await?;
        } else {
            tracing::warn!(pod=%pod.name_any(),uid=?pod.uid(),"Orphan storage Pod remains fenced by its finalizer; administrator target fence/CHAP rotation and explicit finalizer removal are required");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{api::VirtualMachineStatus, resources::Settings};
    use axum::{
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::get,
        Json, Router,
    };
    use serde_json::Value;
    use std::{future::IntoFuture, sync::Mutex};

    fn vm() -> VirtualMachine {
        let mut vm:VirtualMachine=serde_json::from_value(json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine","metadata":{"name":"test-vm","namespace":"qrab","uid":"vm-uid","resourceVersion":"1","finalizers":[VM_FINALIZER]},"spec":{"running":false,"stateVolumeClaim":"state","console":{"publicUrl":"https://vm.example","allowedSubjects":["6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b"]}}})).unwrap();
        vm.status = Some(VirtualMachineStatus {
            pod_name: Some("test-vm".into()),
            pod_uid: Some("pod-uid".into()),
            node_name: Some("node-one".into()),
            state_volume_uid: Some("pvc-uid".into()),
            ..Default::default()
        });
        vm
    }
    fn pod(code: Option<i32>) -> Pod {
        let state = code
            .map(|code| json!({"terminated":{"exitCode":code,"reason":"Completed"}}))
            .unwrap_or(json!({"running":{}}));
        serde_json::from_value(json!({"apiVersion":"v1","kind":"Pod","metadata":{"name":"test-vm","namespace":"qrab","uid":"pod-uid","resourceVersion":"1","labels":{"app.kubernetes.io/name":"qrab","qrab.yapplesauce.com/owner-uid":"vm-uid"},"finalizers":[POD_FINALIZER],"ownerReferences":[{"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine","name":"test-vm","uid":"vm-uid","controller":true}]},"spec":{"containers":[{"name":"manager"}]},"status":{"phase":if code.is_some(){"Succeeded"}else{"Running"},"containerStatuses":[{"name":"manager","image":"image","imageID":"image","ready":false,"restartCount":0,"state":state}]}})).unwrap()
    }
    fn mark_deleting<K: Resource + serde::Serialize + serde::de::DeserializeOwned>(
        resource: &mut K,
    ) {
        let mut value = serde_json::to_value(&resource).unwrap();
        value["metadata"]["deletionTimestamp"] = json!("2026-11-11T00:00:00Z");
        *resource = serde_json::from_value(value).unwrap();
    }

    #[test]
    fn clean_release_needs_every_container_terminated_without_failure_or_signal() {
        assert_eq!(exit(&pod(Some(0))), Exit::Clean);
        assert_eq!(exit(&pod(Some(1))), Exit::Unknown);
        assert_eq!(exit(&pod(None)), Exit::Active);
        let mut missing = pod(Some(0));
        missing.status.as_mut().unwrap().container_statuses = None;
        assert_eq!(exit(&missing), Exit::Unknown);
        let mut signaled = pod(Some(0));
        signaled
            .status
            .as_mut()
            .unwrap()
            .container_statuses
            .as_mut()
            .unwrap()[0]
            .state
            .as_mut()
            .unwrap()
            .terminated
            .as_mut()
            .unwrap()
            .signal = Some(9);
        assert_eq!(exit(&signaled), Exit::Unknown);
        let mut sidecar = pod(Some(0));
        sidecar
            .spec
            .as_mut()
            .unwrap()
            .containers
            .push(serde_json::from_value(json!({"name":"another"})).unwrap());
        assert_eq!(exit(&sidecar), Exit::Unknown);
    }

    #[test]
    fn nonce_and_other_uids_are_not_fencing_acknowledgments() {
        let mut vm = vm();
        vm.spec.restart_nonce = "pod-uid".into();
        assert!(!recovered(&vm, "pod-uid"));
        vm.status.as_mut().unwrap().storage_recovery = Some("different-pod".into());
        assert!(!recovered(&vm, "pod-uid"));
        vm.status.as_mut().unwrap().storage_recovery = Some("pod-uid".into());
        assert!(recovered(&vm, "pod-uid"));
    }

    #[test]
    fn unknown_loss_retains_owner_until_release_and_does_not_change_legacy_status() {
        let mut vm = vm();
        let status = state(
            &vm,
            None,
            "Blocked",
            false,
            "StorageFenceRequired",
            "unknown loss",
        );
        assert_eq!(status.pod_uid.as_deref(), Some("pod-uid"));
        vm.status.as_mut().unwrap().storage_released_pod_uid = Some("pod-uid".into());
        assert_eq!(
            state(&vm, None, "Stopped", false, "PowerOff", "").pod_uid,
            None
        );
        vm.metadata.finalizers = None;
        vm.status.as_mut().unwrap().storage_released_pod_uid = None;
        assert_eq!(
            state(&vm, None, "Stopped", false, "PowerOff", "").pod_uid,
            None
        );
    }

    #[derive(Clone)]
    struct Store(Arc<Mutex<Objects>>);
    struct Objects {
        vm: Option<VirtualMachine>,
        pod: Option<Pod>,
        calls: Vec<&'static str>,
        config: Option<k8s_openapi::api::core::v1::ConfigMap>,
    }
    fn not_found() -> Response {
        (StatusCode::NOT_FOUND,Json(json!({"apiVersion":"v1","kind":"Status","status":"Failure","reason":"NotFound","message":"not found","code":404}))).into_response()
    }
    async fn get_vm(State(store): State<Store>) -> Response {
        match &store.0.lock().unwrap().vm {
            Some(vm) => Json(serde_json::to_value(vm).unwrap()).into_response(),
            None => not_found(),
        }
    }
    async fn get_pod(State(store): State<Store>) -> Response {
        match &store.0.lock().unwrap().pod {
            Some(pod) => Json(serde_json::to_value(pod).unwrap()).into_response(),
            None => not_found(),
        }
    }
    fn settings() -> Settings {
        Settings {
            namespace: "qrab".into(),
            vm_image: format!("zot.yapplesauce.com/qrab/manager@sha256:{}", "a".repeat(64)),
            oidc_issuer: "https://idm.example/oidc".into(),
            oidc_client_id: "qrab".into(),
            idp_ip: None,
        }
    }
    async fn get_claim() -> Response {
        Json(json!({"apiVersion":"v1","kind":"PersistentVolumeClaim","metadata":{"name":"state","namespace":"qrab","uid":"pvc-uid"},"spec":{"accessModes":["ReadWriteOncePod"],"volumeMode":"Filesystem"},"status":{"phase":"Bound"}})).into_response()
    }
    async fn get_config(State(store): State<Store>) -> Response {
        match &store.0.lock().unwrap().config {
            Some(config) => Json(serde_json::to_value(config).unwrap()).into_response(),
            None => not_found(),
        }
    }
    async fn list_pods(State(store): State<Store>) -> Response {
        let objects = store.0.lock().unwrap();
        Json(json!({"apiVersion":"v1","kind":"PodList","metadata":{"resourceVersion":"1"},"items":objects.pod.iter().collect::<Vec<_>>() })).into_response()
    }
    async fn patch_status(State(store): State<Store>, Json(value): Json<Value>) -> Response {
        let mut objects = store.0.lock().unwrap();
        let old = objects
            .vm
            .as_ref()
            .unwrap()
            .status
            .as_ref()
            .and_then(|s| s.storage_released_pod_uid.clone());
        let new: VirtualMachineStatus = serde_json::from_value(value["status"].clone()).unwrap();
        if new.reason == "StorageOwnerObserved" {
            objects.calls.push("observe-owner");
        }
        if new.storage_released_pod_uid != old && new.storage_released_pod_uid.is_some() {
            objects.calls.push("record-release");
        }
        objects.vm.as_mut().unwrap().status = Some(new);
        Json(serde_json::to_value(objects.vm.as_ref().unwrap()).unwrap()).into_response()
    }
    async fn patch_vm(State(store): State<Store>, Json(value): Json<Value>) -> Response {
        let mut objects = store.0.lock().unwrap();
        let finalizers: Vec<String> =
            serde_json::from_value(value["metadata"]["finalizers"].clone()).unwrap();
        objects.vm.as_mut().unwrap().metadata.finalizers = Some(finalizers.clone());
        let output = serde_json::to_value(objects.vm.as_ref().unwrap()).unwrap();
        if !finalizers.iter().any(|v| v == VM_FINALIZER) {
            objects.calls.push("remove-vm-finalizer");
            objects.vm = None;
        }
        Json(output).into_response()
    }
    async fn delete_pod(State(store): State<Store>) -> Response {
        let mut objects = store.0.lock().unwrap();
        objects.calls.push("delete-pod");
        mark_deleting(objects.pod.as_mut().unwrap());
        Json(json!({"apiVersion":"v1","kind":"Status","status":"Success","code":200}))
            .into_response()
    }
    async fn patch_pod(State(store): State<Store>, Json(value): Json<Value>) -> Response {
        let mut objects = store.0.lock().unwrap();
        if value.get("spec").is_some() {
            objects.calls.push("remove-launch-gate");
            objects
                .pod
                .as_mut()
                .unwrap()
                .spec
                .as_mut()
                .unwrap()
                .scheduling_gates =
                Some(serde_json::from_value(value["spec"]["schedulingGates"].clone()).unwrap());
            return Json(serde_json::to_value(objects.pod.as_ref().unwrap()).unwrap())
                .into_response();
        }
        let finalizers: Vec<String> =
            serde_json::from_value(value["metadata"]["finalizers"].clone()).unwrap();
        objects.pod.as_mut().unwrap().metadata.finalizers = Some(finalizers.clone());
        let output = serde_json::to_value(objects.pod.as_ref().unwrap()).unwrap();
        if !finalizers.iter().any(|v| v == POD_FINALIZER) {
            objects.calls.push("remove-pod-finalizer");
            objects.pod = None;
        }
        Json(output).into_response()
    }
    async fn server(
        vm: VirtualMachine,
        pod: Option<Pod>,
    ) -> (
        Store,
        Arc<Context>,
        tokio::task::JoinHandle<std::io::Result<()>>,
    ) {
        let config = resources::render(&vm, &settings()).ok().map(|d| d.config);
        let store = Store(Arc::new(Mutex::new(Objects {
            vm: Some(vm),
            pod,
            calls: Vec::new(),
            config,
        })));
        let app=Router::new()
            .route("/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/virtualmachines/test-vm",get(get_vm).patch(patch_vm))
            .route("/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/virtualmachines/test-vm/status",axum::routing::patch(patch_status))
            .route("/api/v1/namespaces/qrab/pods/test-vm",get(get_pod).delete(delete_pod).patch(patch_pod))
            .route("/api/v1/namespaces/qrab/pods",get(list_pods))
            .route("/api/v1/namespaces/qrab/persistentvolumeclaims/state",get(get_claim))
            .route("/api/v1/namespaces/qrab/configmaps/test-vm-config",get(get_config))
            .with_state(store.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = kube::Client::try_from(kube::Config::new(
            format!("http://{}", listener.local_addr().unwrap())
                .parse()
                .unwrap(),
        ))
        .unwrap();
        let ctx = Arc::new(Context {
            client,
            settings: Settings {
                namespace: "qrab".into(),
                vm_image: format!("zot.yapplesauce.com/qrab/manager@sha256:{}", "a".repeat(64)),
                oidc_issuer: "https://idm.example/oidc".into(),
                oidc_client_id: "qrab".into(),
                idp_ip: None,
            },
        });
        (
            store,
            ctx,
            tokio::spawn(axum::serve(listener, app).into_future()),
        )
    }
    async fn step(store: &Store, ctx: Arc<Context>) {
        let vm = store.0.lock().unwrap().vm.clone().unwrap();
        super::super::reconcile(Arc::new(vm), ctx).await.unwrap();
    }

    #[tokio::test]
    async fn deletion_records_release_before_dropping_pod_then_parent_finalizers() {
        let mut vm = vm();
        mark_deleting(&mut vm);
        let (store, ctx, server) = server(vm, Some(pod(None))).await;
        step(&store, ctx.clone()).await;
        assert_eq!(store.0.lock().unwrap().calls, vec!["delete-pod"]);
        {
            let mut objects = store.0.lock().unwrap();
            let mut done = pod(Some(0));
            mark_deleting(&mut done);
            objects.pod = Some(done);
        }
        step(&store, ctx.clone()).await;
        assert_eq!(
            store.0.lock().unwrap().calls,
            vec!["delete-pod", "record-release"]
        );
        step(&store, ctx.clone()).await;
        assert!(store.0.lock().unwrap().pod.is_none());
        step(&store, ctx).await;
        assert_eq!(
            store.0.lock().unwrap().calls,
            vec![
                "delete-pod",
                "record-release",
                "remove-pod-finalizer",
                "remove-vm-finalizer"
            ]
        );
        server.abort();
    }

    #[tokio::test]
    async fn failed_exit_waits_for_exact_admin_fence_acknowledgment() {
        let mut vm = vm();
        mark_deleting(&mut vm);
        let mut done = pod(Some(137));
        mark_deleting(&mut done);
        let (store, ctx, server) = server(vm, Some(done)).await;
        step(&store, ctx.clone()).await;
        assert!(store.0.lock().unwrap().calls.is_empty());
        assert_eq!(
            store
                .0
                .lock()
                .unwrap()
                .vm
                .as_ref()
                .unwrap()
                .status
                .as_ref()
                .unwrap()
                .reason,
            "StorageFenceRequired"
        );
        store
            .0
            .lock()
            .unwrap()
            .vm
            .as_mut()
            .unwrap()
            .status
            .as_mut()
            .unwrap()
            .storage_recovery = Some("pod-uid".into());
        step(&store, ctx.clone()).await;
        assert_eq!(store.0.lock().unwrap().calls, vec!["record-release"]);
        step(&store, ctx).await;
        assert_eq!(
            store.0.lock().unwrap().calls,
            vec!["record-release", "remove-pod-finalizer"]
        );
        server.abort();
    }

    #[tokio::test]
    async fn vanished_unknown_owner_stays_blocked_across_reconciles_until_manual_recovery() {
        let (store, ctx, server) = server(vm(), None).await;
        step(&store, ctx.clone()).await;
        step(&store, ctx.clone()).await;
        let status = store
            .0
            .lock()
            .unwrap()
            .vm
            .as_ref()
            .unwrap()
            .status
            .clone()
            .unwrap();
        assert_eq!(status.pod_uid.as_deref(), Some("pod-uid"));
        assert_eq!(status.reason, "StorageFenceRequired");
        assert!(store.0.lock().unwrap().calls.is_empty());
        store
            .0
            .lock()
            .unwrap()
            .vm
            .as_mut()
            .unwrap()
            .status
            .as_mut()
            .unwrap()
            .storage_recovery = Some("pod-uid".into());
        step(&store, ctx.clone()).await;
        step(&store, ctx).await;
        assert_eq!(
            store
                .0
                .lock()
                .unwrap()
                .vm
                .as_ref()
                .unwrap()
                .status
                .as_ref()
                .unwrap()
                .pod_uid,
            None
        );
        assert_eq!(store.0.lock().unwrap().calls, vec!["record-release"]);
        server.abort();
    }

    #[tokio::test]
    async fn orphan_sweep_only_collects_a_genuine_owned_clean_tombstone() {
        let mut done = pod(Some(0));
        mark_deleting(&mut done);
        let (store, ctx, server) = server(vm(), Some(done)).await;
        store.0.lock().unwrap().vm = None;
        sweep_orphans(ctx).await.unwrap();
        assert_eq!(store.0.lock().unwrap().calls, vec!["remove-pod-finalizer"]);
        server.abort();
    }

    #[tokio::test]
    async fn orphan_sweep_keeps_failed_or_foreign_tombstones() {
        let mut failed = pod(Some(137));
        mark_deleting(&mut failed);
        let (store, ctx, server) = server(vm(), Some(failed)).await;
        store.0.lock().unwrap().vm = None;
        sweep_orphans(ctx.clone()).await.unwrap();
        assert!(store.0.lock().unwrap().calls.is_empty());
        {
            let mut objects = store.0.lock().unwrap();
            let pod = objects.pod.as_mut().unwrap();
            pod.metadata.deletion_timestamp = None;
            pod.metadata.owner_references.as_mut().unwrap()[0].kind = "Deployment".into();
        }
        sweep_orphans(ctx).await.unwrap();
        assert!(store.0.lock().unwrap().calls.is_empty());
        server.abort();
    }

    fn gated_pod() -> Pod {
        let mut pod = pod(None);
        pod.status = None;
        pod.spec.as_mut().unwrap().scheduling_gates = Some(
            serde_json::from_value(
                json!([{"name":OWNER_GATE},{"name":"example.com/another-gate"}]),
            )
            .unwrap(),
        );
        pod
    }

    #[tokio::test]
    async fn owner_uid_is_persisted_before_opening_only_our_launch_gate() {
        let mut vm = vm();
        vm.spec.running = true;
        vm.status.as_mut().unwrap().pod_uid = None;
        let mut actual = resources::render(&vm, &settings()).unwrap().pod;
        actual.metadata.uid = Some("pod-uid".into());
        actual.metadata.resource_version = Some("1".into());
        actual
            .spec
            .as_mut()
            .unwrap()
            .scheduling_gates
            .as_mut()
            .unwrap()
            .push(serde_json::from_value(json!({"name":"example.com/another-gate"})).unwrap());
        let (store, ctx, server) = server(vm, Some(actual)).await;
        step(&store, ctx.clone()).await;
        assert_eq!(store.0.lock().unwrap().calls, vec!["observe-owner"]);
        assert!(has_owner_gate(
            store.0.lock().unwrap().pod.as_ref().unwrap()
        ));
        step(&store, ctx).await;
        assert_eq!(
            store.0.lock().unwrap().calls,
            vec!["observe-owner", "remove-launch-gate"]
        );
        let gates = store
            .0
            .lock()
            .unwrap()
            .pod
            .as_ref()
            .unwrap()
            .spec
            .as_ref()
            .unwrap()
            .scheduling_gates
            .clone()
            .unwrap();
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].name, "example.com/another-gate");
        server.abort();
    }

    #[tokio::test]
    async fn a_gated_unscheduled_pod_can_be_cancelled_without_a_fence() {
        let mut vm = vm();
        mark_deleting(&mut vm);
        let (store, ctx, server) = server(vm, Some(gated_pod())).await;
        step(&store, ctx.clone()).await;
        step(&store, ctx.clone()).await;
        step(&store, ctx.clone()).await;
        step(&store, ctx).await;
        assert_eq!(
            store.0.lock().unwrap().calls,
            vec![
                "delete-pod",
                "record-release",
                "remove-pod-finalizer",
                "remove-vm-finalizer"
            ]
        );
        server.abort();
    }

    #[tokio::test]
    async fn a_current_active_pod_with_admin_recovery_is_terminated_not_reported_healthy() {
        let mut vm = vm();
        vm.spec.running = true;
        vm.status.as_mut().unwrap().storage_recovery = Some("pod-uid".into());
        let (store, ctx, server) = server(vm, Some(pod(None))).await;
        step(&store, ctx.clone()).await;
        assert_eq!(store.0.lock().unwrap().calls, vec!["delete-pod"]);
        assert!(store
            .0
            .lock()
            .unwrap()
            .pod
            .as_ref()
            .unwrap()
            .metadata
            .deletion_timestamp
            .is_some());
        step(&store, ctx.clone()).await;
        assert_eq!(
            store.0.lock().unwrap().calls,
            vec!["delete-pod", "record-release"]
        );
        step(&store, ctx.clone()).await;
        assert!(store.0.lock().unwrap().pod.is_none());
        step(&store, ctx).await;
        assert!(store
            .0
            .lock()
            .unwrap()
            .vm
            .as_ref()
            .unwrap()
            .status
            .as_ref()
            .unwrap()
            .storage_recovery
            .is_none());
        assert_eq!(
            store.0.lock().unwrap().calls,
            vec!["delete-pod", "record-release", "remove-pod-finalizer"]
        );
        server.abort();
    }

    #[tokio::test]
    async fn same_uid_new_active_incarnation_invalidates_an_old_clean_release() {
        let mut vm = vm();
        vm.spec.running = true;
        vm.status.as_mut().unwrap().storage_released_pod_uid = Some("pod-uid".into());
        let (store, ctx, server) = server(vm, Some(pod(None))).await;
        step(&store, ctx.clone()).await;
        assert!(store
            .0
            .lock()
            .unwrap()
            .vm
            .as_ref()
            .unwrap()
            .status
            .as_ref()
            .unwrap()
            .storage_released_pod_uid
            .is_none());
        mark_deleting(store.0.lock().unwrap().pod.as_mut().unwrap());
        step(&store, ctx).await;
        assert!(store.0.lock().unwrap().pod.is_some());
        assert!(store.0.lock().unwrap().calls.is_empty());
        server.abort();
    }

    #[test]
    fn another_gate_or_an_already_bound_pod_does_not_prove_no_launch() {
        let mut pod = gated_pod();
        assert_eq!(exit(&pod), Exit::Clean);
        pod.spec.as_mut().unwrap().node_name = Some("node-one".into());
        assert_eq!(exit(&pod), Exit::Unknown);
        pod.spec.as_mut().unwrap().node_name = None;
        pod.spec
            .as_mut()
            .unwrap()
            .scheduling_gates
            .as_mut()
            .unwrap()
            .retain(|g| g.name != OWNER_GATE);
        assert_eq!(exit(&pod), Exit::Unknown);
    }

    #[tokio::test]
    async fn config_apply_does_not_clobber_a_foreign_replacement_after_ownership_get() {
        let vm = vm();
        let settings = Settings {
            namespace: "qrab".into(),
            vm_image: format!("zot.yapplesauce.com/qrab/manager@sha256:{}", "a".repeat(64)),
            oidc_issuer: "https://idm.example/oidc".into(),
            oidc_client_id: "qrab".into(),
            idp_ip: None,
        };
        let desired = resources::render(&vm, &settings).unwrap();
        let mut old = desired.config.clone();
        old.metadata.uid = Some("old-config".into());
        old.metadata.resource_version = Some("1".into());
        old.data
            .as_mut()
            .unwrap()
            .insert("vm.json".into(), "drift".into());
        let overwritten = Arc::new(Mutex::new(false));
        let written = overwritten.clone();
        let app=Router::new().route("/api/v1/namespaces/qrab/configmaps/test-vm-config",get(move|| {let old=old.clone();async move {Json(old)}}).patch(move|body:axum::body::Bytes| {
            let written=written.clone();async move {
                let patch:Value=serde_json::from_slice(&body).unwrap();
                // GET returned the owned UID1; it was deleted and UID2 with
                // foreign ownership replaced it before this apply arrived.
                if patch["metadata"]["uid"]=="old-config" && patch["metadata"]["resourceVersion"]=="1" {
                    (StatusCode::CONFLICT,Json(json!({"apiVersion":"v1","kind":"Status","status":"Failure","reason":"Conflict","message":"ConfigMap was replaced","code":409}))).into_response()
                } else {
                    *written.lock().unwrap()=true;
                    Json(patch).into_response()
                }
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = kube::Client::try_from(kube::Config::new(
            format!("http://{}", listener.local_addr().unwrap())
                .parse()
                .unwrap(),
        ))
        .unwrap();
        let server = tokio::spawn(axum::serve(listener, app).into_future());
        let api = Api::namespaced(client, "qrab");
        let error = super::super::ensure_config(&api, &vm, &desired)
            .await
            .unwrap_err();
        assert!(
            matches!(error,Error::Kubernetes(kube::Error::Api(response)) if response.code==409)
        );
        assert!(!*overwritten.lock().unwrap());
        server.abort();
    }

    fn rendered_gated_pod(vm: &VirtualMachine) -> Pod {
        let mut pod = resources::render(vm, &settings()).unwrap().pod;
        pod.metadata.uid = Some("pod-uid".into());
        pod.metadata.resource_version = Some("1".into());
        pod
    }

    #[tokio::test]
    async fn a_stale_gated_template_is_replaced_without_ever_opening_it() {
        let mut vm = vm();
        vm.spec.running = true;
        let pod = rendered_gated_pod(&vm);
        vm.spec.restart_nonce = "new-template".into();
        let (store, ctx, server) = server(vm, Some(pod)).await;
        step(&store, ctx).await;
        assert_eq!(store.0.lock().unwrap().calls, vec!["delete-pod"]);
        assert!(has_owner_gate(
            store.0.lock().unwrap().pod.as_ref().unwrap()
        ));
        server.abort();
    }

    #[tokio::test]
    async fn invalid_desired_spec_never_opens_an_existing_launch_gate() {
        let mut vm = vm();
        vm.spec.running = true;
        let pod = rendered_gated_pod(&vm);
        vm.spec.console.allowed_subjects = vec!["invalid-uuid".into()];
        let (store, ctx, server) = server(vm, Some(pod)).await;
        step(&store, ctx).await;
        assert!(store.0.lock().unwrap().calls.is_empty());
        assert!(has_owner_gate(
            store.0.lock().unwrap().pod.as_ref().unwrap()
        ));
        assert_eq!(
            store
                .0
                .lock()
                .unwrap()
                .vm
                .as_ref()
                .unwrap()
                .status
                .as_ref()
                .unwrap()
                .reason,
            "InvalidSpec"
        );
        server.abort();
    }
}
