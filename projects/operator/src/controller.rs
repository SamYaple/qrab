#[path = "storage_guard.rs"]
pub(crate) mod storage_guard;

#[cfg(test)]
mod retirement_tests;

use crate::{
    address,
    api::{NicStatus, VirtualMachine, VirtualMachineStatus},
    guest_identity,
    nic::{self, NicEndpoint},
    power,
    provision::{self, Attached, SecretStore, Step},
    resources::{self, Desired, GuestSize, ResizePlan, Settings},
    storage::{SharedStorage, StorageError},
    Error, Result,
};
use k8s_openapi::api::core::v1::{ConfigMap, PersistentVolumeClaim, Pod, Secret};
use kube::{
    api::{DeleteParams, DynamicObject, ListParams, Patch, PatchParams, PostParams, Preconditions},
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
    /// Exact-VM authenticated metrics reader; absent only in isolated controller fixtures.
    pub metrics: Option<power::Client>,
    /// Evicts a deleted VM's guest SPIRE agent; absent leaves it to expire.
    pub evictor: Option<Arc<dyn guest_identity::GuestEvictor>>,
}

pub(crate) fn state(
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
        nics: vm
            .status
            .as_ref()
            .map(|s| s.nics.clone())
            .unwrap_or_default(),
        state_volume_claim: vm
            .status
            .as_ref()
            .and_then(|s| s.state_volume_claim.clone()),
        placement_node: vm.status.as_ref().and_then(|s| s.placement_node.clone()),
        power_state: None,
        conditions: Vec::new(),
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

/// Converge the NIC anchor Pods of `spec.nics`, remove those of dropped
/// NICs and record `status.nics`. Each NIC claims, announces, fences and
/// releases a stable address pool, so guest NICs need stable addressing
/// (`QRAB_NIC_ADDRESS_RANGE`). Each anchor is pinned to the VM Pod's node and
/// created once that Pod is scheduled. Returns every NIC's pool address,
/// `Ok(None)` while one is missing, or `Err(action)` after publishing a
/// change.
///
/// The VM Pod does not wait for anchors: the guest's address is the NIC's
/// pool address, and the manager waits for qrab-netd before QEMU starts.
/// `status.nics[].ready` is the anchor's readiness (qrab-netd's plumbed
/// condition). A relay CiliumNetworkPolicy left by the removed stream
/// datapath is deleted.
async fn reconcile_nics(
    vm: &VirtualMachine,
    ctx: &Context,
    vms: &Api<VirtualMachine>,
    pods: &Api<Pod>,
    vm_pod: Option<&Pod>,
) -> Result<std::result::Result<Option<Vec<NicEndpoint>>, Action>> {
    let vm_node = vm_pod.and_then(|p| p.spec.as_ref()?.node_name.as_deref());
    let addressing = ctx.settings.nic_addresses.as_ref();
    if vm.spec.nics.is_empty()
        && vm.status.as_ref().is_none_or(|s| s.nics.is_empty())
        && !address::has_finalizer(vm)
    {
        return Ok(Ok(Some(Vec::new())));
    }
    let uid = vm
        .uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    // Recorded before the first claim, so deletion always finds the pools.
    if addressing.is_some() && !vm.spec.nics.is_empty() && !address::has_finalizer(vm) {
        address::vm_finalizer(vms, vm, true).await?;
        return Ok(Err(Action::requeue(Duration::from_secs(1))));
    }
    let mut addresses = if addressing.is_some() || address::has_finalizer(vm) {
        Some(address::Addresses::load(&ctx.client, &frr_namespace(ctx)).await?)
    } else {
        None
    };
    let policies: Api<DynamicObject> = Api::namespaced_with(
        ctx.client.clone(),
        &ctx.settings.namespace,
        &nic::policy_resource(),
    );
    let selector = format!(
        "app.kubernetes.io/component={},{}={uid}",
        nic::COMPONENT,
        nic::OWNER_UID
    );
    let nic_pods = pods
        .list(&ListParams::default().labels(&selector))
        .await?
        .items;
    let (mut endpoints, mut statuses) = (Vec::new(), Vec::new());
    for spec in &vm.spec.nics {
        let pool = match (addressing, addresses.as_mut()) {
            (Some(addressing), Some(addresses)) => Some(match addresses.of(&uid, &spec.name) {
                Some(pool) => pool.clone(),
                None => addresses.claim(&addressing.range, vm, &spec.name).await?,
            }),
            _ => None,
        };
        let address = pool
            .as_ref()
            .and_then(|p| address::pool_address(&p.name_any()));
        // The anchor template; None while it waits for the VM Pod's node.
        let pool_name = pool.as_ref().map(|p| p.name_any()).ok_or_else(|| {
            Error::Invalid("guest NICs need stable NIC addresses (QRAB_NIC_ADDRESS_RANGE)".into())
        })?;
        let template = vm_node
            .map(|node| nic::render_anchor(vm, &ctx.settings, spec, &pool_name, node))
            .transpose()?
            .map(|a| (a.pod, a.hash));
        // The stream datapath's relay policy, from before tap was the only one.
        let relay = nic::legacy_relay_policy_name(vm, &spec.name);
        match policies.get_opt(&relay).await? {
            Some(actual) if !resources::owned_meta(&actual.metadata, vm) => {
                return Err(Error::Ownership(format!(
                    "CiliumNetworkPolicy {relay} belongs to another owner"
                )))
            }
            Some(actual) => {
                policies
                    .delete(
                        &relay,
                        &DeleteParams {
                            preconditions: Some(Preconditions {
                                uid: actual.uid(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )
                    .await?;
            }
            None => {}
        }
        // The anchor on the VM Pod's node; others are retired below.
        let name = vm_node.map(|node| nic::anchor_name(vm, &spec.name, node));
        let mut endpoint = None;
        let live = match &name {
            Some(name) => pods.get_opt(name).await?,
            None => None,
        };
        let name = name.unwrap_or_default();
        match &live {
            Some(actual) if !resources::owned(actual, vm) => {
                return Err(Error::Ownership(format!(
                    "Pod {name} is not owned by this VM UID"
                )))
            }
            Some(actual) if actual.metadata.deletion_timestamp.is_some() => {}
            Some(actual)
                if template
                    .as_ref()
                    .is_some_and(|(pod, hash)| !nic::template_matches(actual, pod, hash))
                    || matches!(
                        actual.status.as_ref().and_then(|s| s.phase.as_deref()),
                        Some("Failed" | "Succeeded")
                    ) =>
            {
                delete(pods, actual).await?;
            }
            Some(actual) => {
                if let Some(patch) = nic::security_group_patch(actual, spec) {
                    pods.patch(&name, &PatchParams::default(), &Patch::Merge(patch))
                        .await?;
                }
                endpoint = nic::endpoint(actual, &spec.name)
                    .filter(|e| address.is_none_or(|a| e.ip == std::net::IpAddr::V4(a)));
            }
            // Anchors hold no VM state; a missing one is simply recreated.
            None => {
                if let Some((pod, _)) = &template {
                    pods.create(&PostParams::default(), pod).await?;
                }
            }
        }
        // A live migration's target anchor holds its address through a
        // handoff: move the pool to its node before the source anchor goes.
        if let (Some(addresses), Some(pool), Some(ip), Some(actual)) =
            (&addresses, &pool, address, &live)
        {
            let annotated = actual.annotations().contains_key(nic::HANDOFF_FROM);
            let holds =
                actual.status.as_ref().and_then(|s| s.pod_ip.as_deref()) == Some(&ip.to_string());
            if let Some(node) = actual
                .spec
                .as_ref()
                .and_then(|s| s.node_name.as_deref())
                .filter(|_| annotated && holds && endpoint.is_some())
            {
                addresses.adopt(pool, node, ip).await?;
            }
        }
        if vm_node.is_some() {
            retire_other_anchors(ctx, pods, vm, pods_of(&nic_pods, &spec.name), &name).await?;
        }
        let anchor_ready = endpoint.is_some();
        let endpoint = address.map(|a| NicEndpoint {
            name: spec.name.clone(),
            ip: std::net::IpAddr::V4(a),
        });
        if let (Some(addresses), Some(pool), Some(ip)) = (&addresses, &pool, address) {
            let holder = live.as_ref().filter(|p| {
                p.status.as_ref().and_then(|s| s.pod_ip.as_deref()) == Some(&ip.to_string())
            });
            // The anchor a live migration took the address over from keeps
            // announcing it while it retires (make before break).
            let previous = holder
                .and_then(|h| h.annotations().get(nic::HANDOFF_FROM))
                .and_then(|from| from.split_once('/'))
                .and_then(|(_, name)| pods_of(&nic_pods, &spec.name).find(|p| p.name_any() == name))
                .filter(|p| p.metadata.deletion_timestamp.is_none());
            addresses.route(pool, holder, previous).await?;
            if holder.is_none() {
                let bound = pods_of(&nic_pods, &spec.name)
                    .filter_map(|p| p.spec.as_ref()?.node_name.clone())
                    .collect();
                addresses.fence(pool, &bound).await?;
            }
        }
        statuses.push(NicStatus {
            name: spec.name.clone(),
            mac: nic::mac(&uid, &spec.name),
            pod_name: name,
            ip: endpoint.as_ref().map(|e| e.ip.to_string()),
            ready: anchor_ready,
            address: address.map(|a| a.to_string()),
        });
        endpoints.extend(endpoint);
    }
    // Dropped NICs. VM deletion is handled by `finalize_nics`.
    let wanted = |labels: &std::collections::BTreeMap<String, String>| {
        labels
            .get(nic::NIC_LABEL)
            .is_some_and(|n| vm.spec.nics.iter().any(|s| &s.name == n))
    };
    for stale in &nic_pods {
        if !wanted(stale.labels())
            && resources::owned(stale, vm)
            && stale.metadata.deletion_timestamp.is_none()
        {
            delete(pods, stale).await?;
        }
    }
    for stale in policies
        .list(&ListParams::default().labels(&selector))
        .await?
    {
        if !wanted(stale.labels()) && resources::owned_meta(&stale.metadata, vm) {
            policies
                .delete(
                    &stale.name_any(),
                    &DeleteParams {
                        preconditions: Some(Preconditions {
                            uid: stale.uid(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )
                .await?;
        }
    }
    if let Some(addresses) = &addresses {
        let mut kept = 0;
        for pool in addresses.owned_by(&uid) {
            let nic = pool
                .labels()
                .get(nic::NIC_LABEL)
                .cloned()
                .unwrap_or_default();
            if wanted(pool.labels()) {
                kept += 1;
            } else if pods_of(&nic_pods, &nic).next().is_none() {
                addresses.release(pool).await?;
            } else {
                kept += 1;
            }
        }
        if kept == 0 && vm.spec.nics.is_empty() && address::has_finalizer(vm) {
            address::vm_finalizer(vms, vm, false).await?;
            return Ok(Err(Action::requeue(Duration::from_secs(1))));
        }
    }
    if vm.status.as_ref().map(|s| &s.nics) != Some(&statuses) {
        let mut status = vm.status.clone().unwrap_or_default();
        status.nics = statuses;
        publish(vms, vm, status).await?;
        return Ok(Err(Action::requeue(Duration::from_secs(1))));
    }
    Ok(Ok(
        (endpoints.len() == vm.spec.nics.len()).then_some(endpoints)
    ))
}

/// Anchors of one NIC other than `current` (the VM Pod's node's): one an
/// active migration owns is left to it; any other is annotated with a
/// retire time on first sight and deleted once that passes, so the new
/// node's announcement converges while the old node still forwards.
async fn retire_other_anchors<'a>(
    ctx: &Context,
    pods: &Api<Pod>,
    vm: &VirtualMachine,
    anchors: impl Iterator<Item = &'a Pod>,
    current: &str,
) -> Result<()> {
    let migrations: Api<crate::api::VirtualMachineMigration> =
        Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    for anchor in anchors {
        if anchor.name_any() == current
            || anchor.metadata.deletion_timestamp.is_some()
            || !resources::owned(anchor, vm)
        {
            continue;
        }
        if let Some(migration) = anchor.annotations().get(nic::MIGRATION_ANNOTATION) {
            let active = migrations.get_opt(migration).await?.is_some_and(|m| {
                !crate::migration::terminal(m.status.as_ref().map_or("", |s| s.phase.as_str()))
            });
            if active {
                continue;
            }
        }
        match anchor
            .annotations()
            .get(nic::RETIRE_ANNOTATION)
            .and_then(|at| at.parse::<i64>().ok())
        {
            Some(at) if now >= at => delete(pods, anchor).await?,
            Some(_) => {}
            None => {
                let at = (now + nic::RETIRE_DELAY_SECS).to_string();
                let identity = pod_preconditions(anchor)?;
                pods.patch(
                    &anchor.name_any(),
                    &PatchParams::default(),
                    &Patch::Merge(json!({"metadata":{"uid":identity.uid,"resourceVersion":identity.resource_version,"annotations":{nic::RETIRE_ANNOTATION: at}}})),
                )
                .await?;
            }
        }
    }
    Ok(())
}

fn pods_of<'a>(pods: &'a [Pod], nic: &'a str) -> impl Iterator<Item = &'a Pod> + 'a {
    pods.iter()
        .filter(move |p| p.labels().get(nic::NIC_LABEL).map(String::as_str) == Some(nic))
}

fn frr_namespace(ctx: &Context) -> String {
    ctx.settings
        .nic_addresses
        .as_ref()
        .map_or(address::DEFAULT_FRR_NAMESPACE, |a| a.frr_namespace.as_str())
        .to_owned()
}

enum Finalized {
    /// No address pools to release.
    Absent,
    /// NIC Pods are still terminating.
    Waiting,
    /// The VM changed; reconcile again before touching it.
    Patched,
}

/// On VM deletion: stop the NIC Pods, then release their addresses and drop
/// the VM's address finalizer.
async fn finalize_nics(
    vm: &VirtualMachine,
    ctx: &Context,
    vms: &Api<VirtualMachine>,
    pods: &Api<Pod>,
) -> Result<Finalized> {
    if !address::has_finalizer(vm) {
        return Ok(Finalized::Absent);
    }
    let uid = vm
        .uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    let selector = format!(
        "app.kubernetes.io/component={},{}={uid}",
        nic::COMPONENT,
        nic::OWNER_UID
    );
    let nic_pods = pods
        .list(&ListParams::default().labels(&selector))
        .await?
        .items;
    if !nic_pods.is_empty() {
        for pod in &nic_pods {
            if pod.metadata.deletion_timestamp.is_none() && resources::owned(pod, vm) {
                delete(pods, pod).await?;
            }
        }
        return Ok(Finalized::Waiting);
    }
    let addresses = address::Addresses::load(&ctx.client, &frr_namespace(ctx)).await?;
    for pool in addresses.owned_by(&uid) {
        addresses.release(pool).await?;
    }
    address::vm_finalizer(vms, vm, false).await?;
    Ok(Finalized::Patched)
}

/// Release address pools whose VM is gone (deleted past its finalizer) once
/// no NIC Pod of theirs remains.
async fn sweep_addresses(ctx: &Context) -> Result<()> {
    if ctx.settings.nic_addresses.is_none() {
        return Ok(());
    }
    let addresses = address::Addresses::load(&ctx.client, &frr_namespace(ctx)).await?;
    if addresses.all().is_empty() {
        return Ok(());
    }
    let vms: Api<VirtualMachine> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let live: std::collections::BTreeSet<String> = vms
        .list(&ListParams::default())
        .await?
        .items
        .iter()
        .filter_map(ResourceExt::uid)
        .collect();
    let pods: Api<Pod> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    for pool in addresses.all() {
        let Some(uid) = pool.labels().get(nic::OWNER_UID) else {
            continue;
        };
        if live.contains(uid) {
            continue;
        }
        let selector = format!(
            "app.kubernetes.io/component={},{}={uid}",
            nic::COMPONENT,
            nic::OWNER_UID
        );
        if pods
            .list(&ListParams::default().labels(&selector))
            .await?
            .items
            .is_empty()
        {
            tracing::warn!(pool=%pool.name_any(), %uid, "releasing the NIC address of a VM that is gone");
            addresses.release(pool).await?;
        }
    }
    Ok(())
}

pub(crate) async fn publish(
    api: &Api<VirtualMachine>,
    vm: &VirtualMachine,
    status: VirtualMachineStatus,
) -> Result<()> {
    if vm.status.as_ref() != Some(&status) {
        api.patch_status(&vm.name_any(), &PatchParams::default(), &Patch::Merge(json!({"metadata":{"resourceVersion":vm.metadata.resource_version},"status":status}))).await?;
    }
    Ok(())
}

fn pod_preconditions(pod: &Pod) -> Result<Preconditions> {
    if pod.metadata.uid.as_deref().is_none_or(str::is_empty)
        || pod
            .metadata
            .resource_version
            .as_deref()
            .is_none_or(str::is_empty)
    {
        return Err(Error::Ownership(format!(
            "Pod {} lacks UID/resourceVersion for a conditional mutation",
            pod.name_any()
        )));
    }
    Ok(Preconditions {
        uid: pod.uid(),
        resource_version: pod.resource_version(),
    })
}

async fn delete(pods: &Api<Pod>, pod: &Pod) -> Result<()> {
    pods.delete(
        &pod.name_any(),
        &DeleteParams {
            preconditions: Some(pod_preconditions(pod)?),
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
        let (actual_json, desired_json) = (
            resources::data(&actual).get("vm.json").cloned(),
            resources::data(&desired.config).get("vm.json").cloned(),
        );
        let changed = actual_json != desired_json;
        // A NIC change reaches the running manager through the refreshed
        // ConfigMap; only the rest of the config needs a new VM Pod.
        let boot_changed = changed
            && match (&actual_json, &desired_json) {
                (Some(actual), Some(desired)) => {
                    resources::boot_identity(actual).ok() != resources::boot_identity(desired).ok()
                }
                _ => true,
            };
        if !changed
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
    let incarnation = resources::Incarnation::of(vm);
    let pod = pods.get_opt(&incarnation.pod).await?;
    let nics_finalized = if vm.metadata.deletion_timestamp.is_some() {
        finalize_nics(vm, ctx, vms, &pods).await?
    } else {
        Finalized::Absent
    };
    if matches!(nics_finalized, Finalized::Patched) {
        return Ok(Action::requeue(Duration::from_secs(1)));
    }
    let guest_finalized = if vm.metadata.deletion_timestamp.is_some() {
        guest_identity::finalize(vm, &ctx.client, vms, ctx.evictor.as_deref()).await?
    } else {
        guest_identity::Finalized::Absent
    };
    if guest_finalized == guest_identity::Finalized::Released {
        return Ok(Action::requeue(Duration::from_secs(1)));
    }
    if let Some(action) = storage_guard::reconcile(vm, pod.as_ref(), vms, &pods, ctx).await? {
        return Ok(action);
    }
    if vm.metadata.deletion_timestamp.is_some() {
        return Ok(match nics_finalized {
            Finalized::Waiting => Action::requeue(Duration::from_secs(2)),
            _ if guest_finalized == guest_identity::Finalized::Waiting => {
                Action::requeue(Duration::from_secs(5))
            }
            _ => Action::await_change(),
        });
    }
    resources::validate(vm, &ctx.settings)?;
    reconcile_redfish_policy(vm, ctx).await?;
    reconcile_redfish_metrics_policy(vm, ctx).await?;
    reconcile_redfish_prometheus_policy(vm, ctx).await?;
    if guest_identity::reconcile(vm, &ctx.client, &ctx.settings.guest_devid_ca_cn, vms).await? {
        return Ok(Action::requeue(Duration::from_secs(1)));
    }
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
    // NIC anchors stay up while the VM is off; their addresses are stable.
    let nics = match reconcile_nics(vm, ctx, vms, &pods, pod.as_ref()).await? {
        Ok(nics) => nics,
        Err(action) => return Ok(action),
    };
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
        let mut stopped = state(
            vm,
            None,
            "Stopped",
            false,
            "PowerOff",
            "Set spec.running=true to start this VM",
        );
        power::stopped(&mut stopped, vm, &power::now());
        publish(vms, vm, stopped).await?;
        return Ok(Action::await_change());
    }
    let Some(claim) = claims.get_opt(&incarnation.state_claim).await? else {
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
    // The dashboard's state claim goes with its VM (docs/dashboard.md).
    if let Some(patch) = resources::state_claim_adoption(&claim, vm) {
        claims
            .patch(
                &incarnation.state_claim,
                &PatchParams::default(),
                &Patch::Merge(patch),
            )
            .await?;
        tracing::info!(vm=%vm.name_any(), claim=%incarnation.state_claim, "the VM now owns its state claim");
    }
    let Some(nics) = nics else {
        publish(
            vms,
            vm,
            state(
                vm,
                pod.as_ref(),
                "Pending",
                false,
                "NicPending",
                "Waiting for the NIC addresses",
            ),
        )
        .await?;
        return Ok(Action::requeue(Duration::from_secs(3)));
    };
    let provisioned = match provision_disks(vm, ctx, vms, &pods, pod.as_ref()).await? {
        Ok(provisioned) => provisioned,
        Err(action) => return Ok(action),
    };
    // A live Pod keeps the config it booted with; only its resources follow the spec.
    let boot = pod
        .as_ref()
        .and_then(resources::boot_size)
        .unwrap_or_else(|| GuestSize::of(vm));
    let desired = resources::render_attached(vm, &ctx.settings, &provisioned, boot, &nics)?;
    if let Some(pod) = &pod {
        // An active live migration owns both of the VM's Pods.
        let inputs = crate::migration::Inputs {
            provisioned: &provisioned,
            boot,
            nics: &nics,
        };
        if let Some(action) = crate::migration::drive(ctx, vm, vms, &pods, pod, &inputs).await? {
            return Ok(action);
        }
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
        if crate::migration::handed_over(pod) {
            publish(vms, vm, state(vm, Some(pod), "Blocked", false, "HandedOver",
                "This Pod live-migrated its guest to a target no VirtualMachineMigration adopted; find the running target before restarting the VM")).await?;
            return Ok(Action::requeue(Duration::from_secs(30)));
        }
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
        // A guest powered off in place keeps its Pod Ready so Redfish can
        // power it on again; only its metrics export tells the two apart.
        let reading = if ready && vm.spec.components.redfish_enabled() {
            Some(match &ctx.metrics {
                Some(client) => client.read(vm, pod).await,
                None => Err("workload metrics identity unavailable".into()),
            })
        } else {
            None
        };
        if let Some(Err(error)) = &reading {
            tracing::debug!(vm=%vm.name_any(), %error, "guest power state unknown");
        }
        let mut status = power::pod_status(vm, pod, ready, reading.as_ref(), &power::now());
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
        // An outage, timeout or another VM's long fence holding the target
        // lock: retry without rewriting this VM's status.
        Err(Error::Storage(StorageError::Unavailable(message))) => {
            tracing::warn!(vm=%vm.name_any(), %message, "storage service temporarily unavailable");
            Ok(Action::requeue(Duration::from_secs(15)))
        }
        Err(error @ (Error::Invalid(_) | Error::Ownership(_) | Error::Storage(_))) => {
            let reason = match &error {
                Error::Ownership(_) => "OwnershipConflict",
                Error::Storage(StorageError::Refused(_)) => "StorageRefused",
                Error::Storage(StorageError::Unavailable(_)) => "StorageUnavailable",
                _ => "InvalidSpec",
            };
            let mut blocked = state(&vm, None, "Blocked", false, reason, &error.to_string());
            // Refusing an operation does not prove the current execution is
            // gone. Forgetting a migrated Pod here falls back to the primary
            // name on retry and can create a second consumer of its state.
            if let Some(current) = &vm.status {
                blocked.pod_name = current.pod_name.clone();
                blocked.pod_uid = current.pod_uid.clone();
                blocked.node_name = current.node_name.clone();
            }
            publish(&vms, &vm, blocked).await?;
            Ok(Action::requeue(Duration::from_secs(30)))
        }
        Err(error) => Err(error),
    }
}

pub fn error_policy(vm: Arc<VirtualMachine>, error: &Error, _ctx: Arc<Context>) -> Action {
    tracing::warn!(vm=%vm.name_any(), %error, "VM reconciliation failed");
    Action::requeue(Duration::from_secs(5))
}

/// Periodically collect clean storage Pods, NIC addresses and guest identity
/// entries after a forcibly removed parent.
pub async fn sweep_orphans(ctx: Arc<Context>) -> Result<()> {
    storage_guard::sweep_orphans(ctx.clone()).await?;
    sweep_addresses(&ctx).await?;
    guest_identity::sweep(&ctx.client, &ctx.settings.namespace).await
}

/// Event egress is reconciled independently of NIC presence and NIC GC labels.
async fn reconcile_redfish_policy(vm: &VirtualMachine, ctx: &Context) -> Result<()> {
    reconcile_vm_policy(
        vm,
        ctx,
        resources::redfish_policy_name(vm),
        resources::redfish_policy(vm, &ctx.settings)?,
    )
    .await
}

/// The Prometheus-only ingress of the Redfish metrics port, like event egress.
async fn reconcile_redfish_metrics_policy(vm: &VirtualMachine, ctx: &Context) -> Result<()> {
    reconcile_vm_policy(
        vm,
        ctx,
        resources::redfish_metrics_policy_name(vm),
        resources::redfish_metrics_policy(vm, &ctx.settings)?,
    )
    .await
}

/// Egress to Prometheus for Redfish's metric history, like event egress.
async fn reconcile_redfish_prometheus_policy(vm: &VirtualMachine, ctx: &Context) -> Result<()> {
    reconcile_vm_policy(
        vm,
        ctx,
        resources::redfish_prometheus_policy_name(vm),
        resources::redfish_prometheus_policy(vm, &ctx.settings)?,
    )
    .await
}

async fn reconcile_vm_policy(
    vm: &VirtualMachine,
    ctx: &Context,
    name: String,
    desired: Option<DynamicObject>,
) -> Result<()> {
    let policies: Api<DynamicObject> = Api::namespaced_with(
        ctx.client.clone(),
        &ctx.settings.namespace,
        &nic::policy_resource(),
    );
    let actual = policies.get_opt(&name).await?;
    if actual
        .as_ref()
        .is_some_and(|p| !resources::owned_meta(&p.metadata, vm))
    {
        return Err(Error::Ownership(format!(
            "CiliumNetworkPolicy {name} belongs to another owner"
        )));
    }
    match (actual, desired) {
        (None, Some(want)) => {
            policies.create(&PostParams::default(), &want).await?;
        }
        (Some(actual), Some(mut want)) if !nic::policy_matches(&actual, &want) => {
            // Bind the write to the exact object whose ownership we checked.
            want.metadata.uid = actual.metadata.uid.clone();
            want.metadata.resource_version = actual.metadata.resource_version.clone();
            policies
                .patch(
                    &name,
                    &PatchParams::apply("qrab-operator").force(),
                    &Patch::Apply(&want),
                )
                .await?;
        }
        (Some(actual), None) => {
            policies
                .delete(
                    &name,
                    &DeleteParams {
                        preconditions: Some(Preconditions {
                            uid: actual.uid(),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                )
                .await?;
        }
        _ => {}
    }
    Ok(())
}
