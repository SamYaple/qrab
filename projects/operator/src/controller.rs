#[path = "storage_guard.rs"]
mod storage_guard;

use crate::{
    api::{VirtualMachine, VirtualMachineStatus},
    provision::{self, Attached, SecretStore, Step},
    resources::{self, Desired, GuestSize, ResizePlan, Settings},
    storage::{SharedStorage, StorageError},
    Error, Result,
};
use k8s_openapi::api::core::v1::{ConfigMap, PersistentVolumeClaim, Pod, Secret};
use kube::{
    api::{DeleteParams, Patch, PatchParams, PostParams, Preconditions},
    runtime::controller::Action,
    Api, Client, ResourceExt,
};
use serde_json::json;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub struct Context {
    pub client: Client,
    pub settings: Settings,
    /// Storage-service client for `spec.disks`; absent blocks provisioning.
    pub storage: Option<SharedStorage>,
}

fn state(
    vm: &VirtualMachine,
    pod: Option<&Pod>,
    phase: &str,
    ready: bool,
    reason: &str,
    message: &str,
) -> VirtualMachineStatus {
    VirtualMachineStatus {
        observed_generation: vm.metadata.generation.unwrap_or_default(),
        phase: phase.into(),
        ready,
        reason: reason.into(),
        message: message.into(),
        // Retain the last unreleased native-storage owner when its Pod is
        // absent; clearing it would turn an unknown loss into a fresh start.
        pod_name: pod.map(ResourceExt::name_any).or_else(|| {
            storage_guard::unreleased_uid(vm).and_then(|_| vm.status.as_ref()?.pod_name.clone())
        }),
        pod_uid: pod
            .and_then(ResourceExt::uid)
            .or_else(|| storage_guard::unreleased_uid(vm).map(str::to_owned)),
        node_name: pod
            .and_then(|p| p.spec.as_ref())
            .and_then(|s| s.node_name.clone())
            .or_else(|| {
                if pod.is_none() && storage_guard::unreleased_uid(vm).is_some() {
                    vm.status.as_ref()?.node_name.clone()
                } else {
                    None
                }
            }),
        state_volume_uid: vm.status.as_ref().and_then(|s| s.state_volume_uid.clone()),
        storage_released_pod_uid: vm
            .status
            .as_ref()
            .and_then(|s| s.storage_released_pod_uid.clone()),
        storage_recovery: vm.status.as_ref().and_then(|s| s.storage_recovery.clone()),
        disks: vm
            .status
            .as_ref()
            .map(|s| s.disks.clone())
            .unwrap_or_default(),
        resize: None,
    }
}

/// Converge every `spec.disks` entry. `Err(action)` means this reconcile has
/// already published progress and must stop before rendering the Pod.
async fn provision_disks(
    vm: &VirtualMachine,
    ctx: &Context,
    vms: &Api<VirtualMachine>,
    pods: &Api<Pod>,
    pod: Option<&Pod>,
) -> Result<std::result::Result<Vec<Attached>, Action>> {
    if vm.spec.disks.is_empty() && vm.status.as_ref().is_none_or(|s| s.disks.is_empty()) {
        return Ok(Ok(Vec::new()));
    }
    let Some(api) = ctx.storage.as_deref() else {
        publish(
            vms,
            vm,
            state(
                vm,
                pod,
                "Blocked",
                false,
                "StorageServiceUnavailable",
                "spec.disks requires the operator's storage-service client (QRAB_STORAGE_ORIGIN)",
            ),
        )
        .await?;
        return Ok(Err(Action::requeue(Duration::from_secs(30))));
    };
    let uid = vm
        .uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    let launched = pod.is_some_and(|p| !storage_guard::never_launched(p));
    let secrets = SecretStore {
        api: Api::<Secret>::namespaced(ctx.client.clone(), &ctx.settings.namespace),
        vm,
    };
    // Disks dropped from spec: once no launched process mounts them, fence the
    // export and delete the credential. The zvol stays retained.
    let mut released = Vec::new();
    for gone in provision::removable(vm, pod, launched) {
        if let Err(error) = provision::release(api, &secrets, &uid, &gone).await {
            if provision::fence_in_progress(&error) {
                publish(
                    vms,
                    vm,
                    state(
                        vm,
                        pod,
                        "Pending",
                        false,
                        "StorageFenceInProgress",
                        &format!("Fencing removed disk {gone}: {error}"),
                    ),
                )
                .await?;
                return Ok(Err(Action::requeue(Duration::from_secs(15))));
            }
            return Err(error);
        }
        released.push(gone);
    }
    let (mut attached, mut statuses, mut wait, mut restart) = (Vec::new(), Vec::new(), None, None);
    for disk in &vm.spec.disks {
        let (step, status) =
            provision::converge(api, &secrets, &vm.name_any(), &uid, disk, launched).await?;
        statuses.extend(status);
        match step {
            Step::Ready(disk) => attached.push(disk),
            Step::Wait(message) => {
                wait.get_or_insert(message);
            }
            Step::Restart(message) => {
                restart.get_or_insert(message);
            }
        }
    }
    // Removed disks still mounted by a live process stay recorded until released.
    statuses.extend(
        vm.status
            .iter()
            .flat_map(|s| s.disks.iter())
            .filter(|d| !vm.spec.disks.iter().any(|s| s.id == d.id) && !released.contains(&d.id))
            .cloned(),
    );
    if vm.status.as_ref().map(|s| &s.disks) != Some(&statuses) {
        let mut status = vm.status.clone().unwrap_or_default();
        status.disks = statuses;
        publish(vms, vm, status).await?;
        return Ok(Err(Action::requeue(Duration::from_secs(1))));
    }
    if let (Some(message), Some(pod)) = (restart, pod) {
        if pod.metadata.deletion_timestamp.is_none() {
            delete(pods, pod).await?;
        }
        publish(
            vms,
            vm,
            state(vm, Some(pod), "Updating", false, "StorageFenced", &message),
        )
        .await?;
        return Ok(Err(Action::requeue(Duration::from_secs(2))));
    }
    if let Some(message) = wait {
        publish(
            vms,
            vm,
            state(vm, pod, "Pending", false, "StorageProvisioning", &message),
        )
        .await?;
        return Ok(Err(Action::requeue(Duration::from_secs(10))));
    }
    Ok(Ok(attached))
}

async fn publish(
    api: &Api<VirtualMachine>,
    vm: &VirtualMachine,
    status: VirtualMachineStatus,
) -> Result<()> {
    if vm.status.as_ref() != Some(&status) {
        api.patch_status(&vm.name_any(), &PatchParams::default(), &Patch::Merge(json!({"metadata":{"resourceVersion":vm.metadata.resource_version},"status":status}))).await?;
    }
    Ok(())
}

async fn delete(pods: &Api<Pod>, pod: &Pod) -> Result<()> {
    pods.delete(
        &pod.name_any(),
        &DeleteParams {
            preconditions: Some(Preconditions {
                uid: pod.uid(),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

async fn ensure_config(
    api: &Api<ConfigMap>,
    vm: &VirtualMachine,
    desired: &Desired,
) -> Result<bool> {
    let name = desired.config.name_any();
    if let Some(actual) = api.get_opt(&name).await? {
        if !resources::owned(&actual, vm) {
            return Err(Error::Ownership(format!(
                "ConfigMap {name} belongs to another owner"
            )));
        }
        let boot_changed = resources::data(&actual).get("vm.json")
            != resources::data(&desired.config).get("vm.json");
        if !boot_changed
            && desired
                .config
                .labels()
                .iter()
                .all(|(k, v)| actual.labels().get(k) == Some(v))
        {
            return Ok(false);
        }
        let mut applied = desired.config.clone();
        applied.metadata.uid = Some(
            actual
                .uid()
                .ok_or_else(|| Error::Ownership(format!("ConfigMap {name} has no UID")))?,
        );
        applied.metadata.resource_version =
            Some(actual.resource_version().ok_or_else(|| {
                Error::Ownership(format!("ConfigMap {name} has no resourceVersion"))
            })?);
        api.patch(
            &name,
            &PatchParams::apply("qrab-operator").force(),
            &Patch::Apply(&applied),
        )
        .await?;
        return Ok(boot_changed);
    } else {
        api.create(&PostParams::default(), &desired.config).await?;
    }
    Ok(true)
}

async fn synchronize(
    vm: &VirtualMachine,
    ctx: &Context,
    vms: &Api<VirtualMachine>,
) -> Result<Action> {
    let pods: Api<Pod> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let configs: Api<ConfigMap> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let claims: Api<PersistentVolumeClaim> =
        Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let pod = pods.get_opt(&vm.name_any()).await?;
    if let Some(action) = storage_guard::reconcile(vm, pod.as_ref(), vms, &pods, ctx).await? {
        return Ok(action);
    }
    if vm.metadata.deletion_timestamp.is_some() {
        return Ok(Action::await_change());
    }
    resources::validate(vm, &ctx.settings)?;
    if let Some(pod) = &pod {
        if !resources::owned(pod, vm) {
            return Err(Error::Ownership(format!(
                "Pod {} is not owned by this VM UID",
                pod.name_any()
            )));
        }
        if pod.metadata.deletion_timestamp.is_some() {
            publish(
                vms,
                vm,
                state(
                    vm,
                    Some(pod),
                    "Stopping",
                    false,
                    "PodTerminating",
                    "Waiting for the previous VM process to exit",
                ),
            )
            .await?;
            return Ok(Action::requeue(Duration::from_secs(2)));
        }
    }
    if !vm.spec.running {
        if let Some(pod) = &pod {
            delete(&pods, pod).await?;
            publish(
                vms,
                vm,
                state(
                    vm,
                    Some(pod),
                    "Stopping",
                    false,
                    "PowerOffRequested",
                    "Removing the VM Pod; persistent identity is retained",
                ),
            )
            .await?;
            return Ok(Action::requeue(Duration::from_secs(2)));
        }
        publish(
            vms,
            vm,
            state(
                vm,
                None,
                "Stopped",
                false,
                "PowerOff",
                "Set spec.running=true to start this VM",
            ),
        )
        .await?;
        return Ok(Action::await_change());
    }
    let Some(claim) = claims.get_opt(&vm.spec.state_volume_claim).await? else {
        publish(
            vms,
            vm,
            state(
                vm,
                pod.as_ref(),
                "Pending",
                false,
                "StateVolumeMissing",
                "Create the referenced filesystem PVC before starting this VM",
            ),
        )
        .await?;
        return Ok(Action::requeue(Duration::from_secs(10)));
    };
    let spec = claim
        .spec
        .as_ref()
        .ok_or_else(|| Error::Invalid("state PVC has no spec".into()))?;
    if spec
        .volume_mode
        .as_deref()
        .is_some_and(|m| m != "Filesystem")
        || !spec.access_modes.as_ref().is_some_and(|m| {
            m.iter()
                .any(|v| v == "ReadWriteOncePod" || v == "ReadWriteOnce")
                && !m.iter().any(|v| v == "ReadWriteMany")
        })
    {
        return Err(Error::Invalid("state PVC must be a writable single-node filesystem volume (ReadWriteOncePod preferred)".into()));
    }
    if claim.status.as_ref().and_then(|s| s.phase.as_deref()) == Some("Lost") {
        return Err(Error::Invalid(
            "state PVC is Lost; recover the existing identity volume rather than replacing it"
                .into(),
        ));
    }
    if claim.metadata.deletion_timestamp.is_some() {
        publish(
            vms,
            vm,
            state(
                vm,
                pod.as_ref(),
                "Pending",
                false,
                "StateVolumeDeleting",
                "The state PVC is being deleted",
            ),
        )
        .await?;
        return Ok(Action::requeue(Duration::from_secs(5)));
    }
    // Create the consumer even while Pending: WaitForFirstConsumer storage
    // classes cannot bind until the Pod exists and a node has been selected.
    let claim_uid = claim
        .uid()
        .ok_or_else(|| Error::Invalid("state PVC has no UID".into()))?;
    match vm
        .status
        .as_ref()
        .and_then(|s| s.state_volume_uid.as_deref())
    {
        Some(bound) if bound != claim_uid => {
            return Err(Error::Invalid(
                "the state PVC was replaced; refusing to initialize a new TPM identity for this VM"
                    .into(),
            ))
        }
        None => {
            let mut binding = state(
                vm,
                pod.as_ref(),
                "Pending",
                false,
                "StateVolumeAccepted",
                "Bound this VM to the state PVC UID",
            );
            binding.state_volume_uid = Some(claim_uid);
            publish(vms, vm, binding).await?;
            return Ok(Action::requeue(Duration::from_secs(1)));
        }
        _ => {}
    }
    let provisioned = match provision_disks(vm, ctx, vms, &pods, pod.as_ref()).await? {
        Ok(provisioned) => provisioned,
        Err(action) => return Ok(action),
    };
    // A live Pod keeps the config it booted with; only its resources follow the spec.
    let boot = pod
        .as_ref()
        .and_then(resources::boot_size)
        .unwrap_or_else(|| GuestSize::of(vm));
    let desired = resources::render_booted(vm, &ctx.settings, &provisioned, boot)?;
    if let Some(pod) = &pod {
        if !resources::pod_matches(pod, &desired) {
            delete(&pods, pod).await?;
            publish(
                vms,
                vm,
                state(
                    vm,
                    Some(pod),
                    "Updating",
                    false,
                    "TemplateChanged",
                    "Replacing the VM Pod while retaining firmware and TPM identity",
                ),
            )
            .await?;
            return Ok(Action::requeue(Duration::from_secs(2)));
        }
        let phase = pod
            .status
            .as_ref()
            .and_then(|s| s.phase.as_deref())
            .unwrap_or("Pending");
        if phase == "Failed" || phase == "Succeeded" {
            let finished = pod
                .status
                .as_ref()
                .and_then(|s| s.container_statuses.as_ref())
                .into_iter()
                .flatten()
                .filter_map(|s| s.state.as_ref()?.terminated.as_ref()?.finished_at.as_ref())
                .map(|t| t.0.as_second().max(0) as u64)
                .max()
                .unwrap_or(0);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now < finished.saturating_add(10) {
                let message = pod
                    .status
                    .as_ref()
                    .and_then(|s| s.container_statuses.as_ref())
                    .into_iter()
                    .flatten()
                    .find_map(|s| s.state.as_ref()?.terminated.as_ref()?.message.clone())
                    .unwrap_or_else(|| {
                        "The VM process exited; retrying because spec.running=true".into()
                    });
                publish(
                    vms,
                    vm,
                    state(vm, Some(pod), "Failed", false, "VMExited", &message),
                )
                .await?;
                return Ok(Action::requeue(Duration::from_secs(10)));
            }
            delete(&pods, pod).await?;
            publish(
                vms,
                vm,
                state(
                    vm,
                    Some(pod),
                    "Restarting",
                    false,
                    "VMExited",
                    "Creating another instance because spec.running=true",
                ),
            )
            .await?;
            return Ok(Action::requeue(Duration::from_secs(2)));
        }
        if ensure_config(&configs, vm, &desired).await? {
            delete(&pods, pod).await?;
            publish(
                vms,
                vm,
                state(
                    vm,
                    Some(pod),
                    "Updating",
                    false,
                    "ConfigChanged",
                    "Restored the managed configuration; replacing the VM instance",
                ),
            )
            .await?;
            return Ok(Action::requeue(Duration::from_secs(2)));
        }
        let mut rejected = None;
        let mut current = pod.clone();
        match resources::plan_resize(vm, pod, &desired) {
            ResizePlan::Steady => {}
            ResizePlan::Restart(reason) => {
                delete(&pods, pod).await?;
                publish(
                    vms,
                    vm,
                    state(
                        vm,
                        Some(pod),
                        "Updating",
                        false,
                        "ResizeRequiresRestart",
                        reason,
                    ),
                )
                .await?;
                return Ok(Action::requeue(Duration::from_secs(2)));
            }
            ResizePlan::Grow(patch) => match pods
                .patch_subresource(
                    "resize",
                    &pod.name_any(),
                    &PatchParams::default(),
                    &Patch::Strategic(patch),
                )
                .await
            {
                Ok(updated) => current = updated,
                // Requests beyond node allocatable are refused synchronously.
                Err(kube::Error::Api(status)) if status.code == 403 => {
                    rejected = Some(status.message.clone());
                }
                Err(error) => return Err(error.into()),
            },
        }
        // Grown provisioned disks reach the running guest through the
        // projected capacity annotation; the manager applies block_resize.
        if let Some(want) = desired.pod.annotations().get(resources::DISK_CAPACITY) {
            if current.annotations().get(resources::DISK_CAPACITY) != Some(want) {
                let mut annotations = serde_json::Map::new();
                annotations.insert(resources::DISK_CAPACITY.into(), want.clone().into());
                current = pods
                    .patch(
                        &current.name_any(),
                        &PatchParams::default(),
                        &Patch::Merge(json!({"metadata":{"uid":current.uid(),"resourceVersion":current.resource_version(),"annotations":annotations}})),
                    )
                    .await?;
            }
        }
        let pod = &current;
        // The guard recorded the owner UID first; only a validated,
        // converged spec/config may now be made schedulable.
        if storage_guard::open_owner_gate(vm, &pods, pod).await? {
            return Ok(Action::requeue(Duration::from_secs(1)));
        }
        let ready = pod
            .status
            .as_ref()
            .and_then(|s| s.conditions.as_ref())
            .is_some_and(|c| c.iter().any(|c| c.type_ == "Ready" && c.status == "True"));
        let mut status = state(
            vm,
            Some(pod),
            if ready { "Running" } else { "Pending" },
            ready,
            if ready { "VMRunning" } else { "PodNotReady" },
            if ready {
                "QEMU, swtpm and the enabled BMC/Redfish components are live; guest networking is disabled"
            } else {
                "Waiting for the VM Pod to become ready"
            },
        );
        let resize = resources::resize_status(pod, &desired, rejected);
        let settled = resize.phase == "Applied";
        status.resize = Some(resize);
        publish(vms, vm, status).await?;
        if !settled {
            return Ok(Action::requeue(Duration::from_secs(5)));
        }
    } else {
        ensure_config(&configs, vm, &desired).await?;
        let pod = pods.create(&PostParams::default(), &desired.pod).await?;
        publish(
            vms,
            vm,
            state(
                vm,
                Some(&pod),
                "Pending",
                false,
                "PodCreated",
                "Starting UEFI, TPM2 and the authenticated BMC",
            ),
        )
        .await?;
    }
    Ok(Action::requeue(Duration::from_secs(15)))
}

pub async fn reconcile(event: Arc<VirtualMachine>, ctx: Arc<Context>) -> Result<Action> {
    let vms: Api<VirtualMachine> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    // Act on a fresh spec, including after queued child events or CR deletion.
    let Some(vm) = vms.get_opt(&event.name_any()).await? else {
        return Ok(Action::await_change());
    };
    match synchronize(&vm, &ctx, &vms).await {
        Ok(action) => Ok(action),
        Err(error @ (Error::Invalid(_) | Error::Ownership(_) | Error::Storage(_))) => {
            let reason = match &error {
                Error::Ownership(_) => "OwnershipConflict",
                Error::Storage(StorageError::Refused(_)) => "StorageRefused",
                Error::Storage(StorageError::Unavailable(_)) => "StorageUnavailable",
                _ => "InvalidSpec",
            };
            publish(
                &vms,
                &vm,
                state(&vm, None, "Blocked", false, reason, &error.to_string()),
            )
            .await?;
            Ok(Action::requeue(Duration::from_secs(30)))
        }
        Err(error) => Err(error),
    }
}

pub fn error_policy(vm: Arc<VirtualMachine>, error: &Error, _ctx: Arc<Context>) -> Action {
    tracing::warn!(vm=%vm.name_any(), %error, "VM reconciliation failed");
    Action::requeue(Duration::from_secs(5))
}

/// Periodically collect clean storage Pods after a forcibly removed parent.
pub async fn sweep_orphans(ctx: Arc<Context>) -> Result<()> {
    storage_guard::sweep_orphans(ctx).await
}
