//! Live migration of a running VM to another node (`VirtualMachineMigration`).
//!
//! The VM's own reconcile drives it, because only that reconcile holds the
//! rendered inputs (exported disks, NIC addresses, boot size). While a
//! migration is active the VM's Pod is left alone; the steps are:
//!
//! 1. **Pending**: pick the target node and name the target Pod and its own
//!    state PVC.
//! 2. **TargetStarting/Migrating**: open the migration port from the target to
//!    the source only, create the PVC, the target's ConfigMap (the VM's config
//!    plus `incoming`) and the target Pod, record its UID before ungating it,
//!    then wait.
//! 3. When the source's manager exits with the termination message
//!    `migrated`, the target owns the guest and its disks: strip `incoming`,
//!    and point VM status at the target Pod, its PVC and node (**Succeeded**).
//!    Anything else ending first fails the migration and leaves the source.
//!    A target ending with `source-powered-off` (the VM's QEMU is powered
//!    off in its Pod) fails it with reason `PoweredOff`.
//! 4. Clean up: the source Pod, the policy and the replaced PVC after success;
//!    the target Pod, policy and PVC after failure.
//!
//! A standing guard policy per VM denies the migration port to every endpoint
//! that is not one of the VM's own Pods, whatever other policies allow.
use crate::{
    api::{
        VirtualMachine, VirtualMachineMigration, VirtualMachineMigrationStatus,
        VirtualMachineStatus,
    },
    controller::{storage_guard, Context},
    nic::{NicEndpoint, OWNER_UID},
    provision::Attached,
    resources::{self, Desired, GuestSize, Incarnation, KVM_RESOURCE, MIGRATION_PORT, POD_LABEL},
    Error, Result,
};
use k8s_openapi::{
    api::core::v1::{ConfigMap, Node, PersistentVolumeClaim, Pod},
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::{
    api::{DeleteParams, DynamicObject, ListParams, Patch, PatchParams, PostParams, Preconditions},
    core::{ApiResource, GroupVersionKind},
    runtime::controller::Action,
    Api, Resource, ResourceExt,
};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A migration that has not finished within this bound fails.
pub const DEADLINE_SECS: i64 = 600;
/// Termination message of a source manager that handed its guest over.
pub const MIGRATED: &str = "migrated";
/// Termination message of a target manager whose source refused because the
/// VM is powered off in its Pod (QEMU stopped, manager still serving).
pub const SOURCE_POWERED_OFF: &str = "source-powered-off";
/// Failure reason of a migration whose VM is powered off.
pub const POWERED_OFF: &str = "PoweredOff";
/// The migration that created a target, until operator adoption removes it.
/// This is provenance, not proof that the guest never ran: runtime handover
/// can finish before the operator observes and adopts the target.
pub const TARGET_ANNOTATION: &str = "qrab.yapplesauce.com/migration-target";
const STATE_COMPONENT: &str = "vm-state";

/// What the VM reconcile rendered for its current Pod.
pub struct Inputs<'a> {
    pub provisioned: &'a [Attached],
    pub boot: GuestSize,
    pub nics: &'a [NicEndpoint],
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub(crate) fn terminal(phase: &str) -> bool {
    matches!(phase, "Succeeded" | "Failed")
}

fn phase(migration: &VirtualMachineMigration) -> &str {
    migration.status.as_ref().map_or("", |s| s.phase.as_str())
}

fn suffix(migration: &VirtualMachineMigration) -> Result<String> {
    let uid = migration
        .uid()
        .ok_or_else(|| Error::Invalid("migration has no UID".into()))?;
    Ok(uid.replace('-', "").chars().take(6).collect())
}

pub fn target_pod_name(vm: &str, suffix: &str) -> String {
    format!("{vm}-m{suffix}")
}

pub fn target_claim_name(vm: &str, suffix: &str) -> String {
    format!("{vm}-state-{suffix}")
}

pub fn policy_name(migration: &str) -> String {
    format!("qrab-migration-{migration}")
}

pub fn guard_name(vm: &str) -> String {
    format!("{vm}-migration-guard")
}

fn policies(ctx: &Context) -> Api<DynamicObject> {
    let resource = ApiResource::from_gvk_with_plural(
        &GroupVersionKind::gvk("cilium.io", "v2", "CiliumNetworkPolicy"),
        "ciliumnetworkpolicies",
    );
    Api::namespaced_with(ctx.client.clone(), &ctx.settings.namespace, &resource)
}

fn owner(vm: &VirtualMachine) -> Result<Value> {
    let mut owner = vm
        .controller_owner_ref(&())
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    owner.block_owner_deletion = Some(false);
    Ok(serde_json::to_value(owner)?)
}

fn uid(vm: &VirtualMachine) -> Result<String> {
    vm.uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))
}

fn pod_selector(uid: &str, pod: &str) -> Value {
    json!({"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm",OWNER_UID:uid,POD_LABEL:pod})
}

fn port() -> Value {
    json!([{"ports":[{"port":MIGRATION_PORT.to_string(),"protocol":"TCP"}]}])
}

/// The standing per-VM guard: the migration port refuses every entity and
/// every endpoint except this VM's own VM Pods. A deny always wins, so no
/// other policy can open it; without a second Pod nothing can use it.
pub fn guard(vm: &VirtualMachine, namespace: &str) -> Result<Value> {
    let uid = uid(vm)?;
    let vm_pods =
        json!({"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm",OWNER_UID:uid});
    Ok(
        json!({"apiVersion":"cilium.io/v2","kind":"CiliumNetworkPolicy",
        "metadata":{"name":guard_name(&vm.name_any()),"namespace":namespace,
            "labels":{"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm-migration",OWNER_UID:uid},
            "ownerReferences":[owner(vm)?]},
        "spec":{"description":format!("The live-migration port of VM {} admits only its own Pods",vm.name_any()),
            "endpointSelector":{"matchLabels":vm_pods},
            "enableDefaultDeny":{"ingress":false,"egress":false},
            "ingressDeny":[
                {"fromEntities":["world","host","remote-node","kube-apiserver","ingress","health","init","unmanaged"],"toPorts":port()},
                {"fromEndpoints":[
                    {"matchExpressions":[{"key":"k8s:io.kubernetes.pod.namespace","operator":"NotIn","values":[namespace]}]},
                    {"matchExpressions":[{"key":"app.kubernetes.io/component","operator":"NotIn","values":["vm"]}]},
                    {"matchExpressions":[{"key":OWNER_UID,"operator":"NotIn","values":[uid]}]}],
                 "toPorts":port()}]}}),
    )
}

/// The time-boxed allow: target to source on the migration port, for the
/// migration's lifetime only. Additive, so it never enables default deny.
pub fn window(
    vm: &VirtualMachine,
    namespace: &str,
    migration: &str,
    source: &str,
    target: &str,
) -> Result<Value> {
    let uid = uid(vm)?;
    let additive = json!({"ingress":false,"egress":false});
    Ok(
        json!({"apiVersion":"cilium.io/v2","kind":"CiliumNetworkPolicy",
        "metadata":{"name":policy_name(migration),"namespace":namespace,
            "labels":{"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm-migration",OWNER_UID:uid},
            "ownerReferences":[owner(vm)?]},
        "specs":[
            {"description":format!("Migration {migration}: {target} streams VM {} from {source}",vm.name_any()),
             "endpointSelector":{"matchLabels":pod_selector(&uid,source)},
             "enableDefaultDeny":additive,
             "ingress":[{"fromEndpoints":[{"matchLabels":pod_selector(&uid,target)}],"toPorts":port()}]},
            {"description":format!("Migration {migration}: {target} reaches {source}"),
             "endpointSelector":{"matchLabels":pod_selector(&uid,target)},
             "enableDefaultDeny":additive,
             "egress":[{"toEndpoints":[{"matchLabels":pod_selector(&uid,source)}],"toPorts":port()}]}]}),
    )
}

/// The target's ConfigMap: the VM's config for the target incarnation plus
/// `incoming`. The template hash excludes `incoming`, so once the target takes
/// over, the VM's normal render matches its Pod.
pub fn incoming_config(
    desired: &Desired,
    migration: &str,
    source: &str,
    node_ip: Option<&str>,
) -> Result<ConfigMap> {
    let mut config = desired.config.clone();
    let data = config.data.get_or_insert_with(Default::default);
    let mut vm: Value = serde_json::from_str(
        data.get("vm.json")
            .ok_or_else(|| Error::Invalid("rendered config has no vm.json".into()))?,
    )?;
    vm["incoming"] = json!({"migration":migration,"source":format!("{source}:{MIGRATION_PORT}")});
    // Where the source node forwards the guest's NIC traffic during the switch.
    if let Some(node_ip) = node_ip {
        vm["incoming"]["node_ip"] = json!(node_ip);
    }
    data.insert("vm.json".into(), serde_json::to_string_pretty(&vm)?);
    Ok(config)
}

fn ready(pod: &Pod) -> bool {
    pod.status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .is_some_and(|c| c.iter().any(|c| c.type_ == "Ready" && c.status == "True"))
}

/// How the manager container of `pod` ended, if it has.
pub(crate) fn manager_exit(pod: &Pod) -> Option<(i32, String)> {
    pod.status
        .as_ref()?
        .container_statuses
        .as_ref()?
        .iter()
        .find(|c| c.name == "manager")?
        .state
        .as_ref()?
        .terminated
        .as_ref()
        .map(|t| (t.exit_code, t.message.clone().unwrap_or_default()))
}

/// A migration target that never took over and whose process is gone (or
/// never ran): it holds no disk session, whatever its exit code.
pub(crate) fn abandoned_target(pod: &Pod) -> bool {
    pod.annotations().contains_key(TARGET_ANNOTATION)
        && (manager_exit(pod).is_some() || storage_guard::never_launched(pod))
}

/// The Pod's manager handed its guest to a migration target. Its disks belong
/// to that target now: nothing may fence them or boot the VM here again.
pub fn handed_over(pod: &Pod) -> bool {
    manager_exit(pod).is_some_and(|(code, message)| code == 0 && message.trim() == MIGRATED)
}

/// Why a stopped target failed the migration: the reason (empty when not
/// classified) and the message.
const POWERED_OFF_MESSAGE: &str = "the VM is powered off, so there is no running guest to live-migrate; power it on and create a new migration";

/// Reason of a migration refused because the target could not mount one of
/// the VM's claims while the source still holds it.
pub const UNSHARED_CLAIM: &str = "UnsharedClaim";

/// The claims a VM Pod mounts besides its state claim, by role: the Redfish
/// state claim and the media and firmware catalogs. The target Pod mounts the
/// same claims, on another node, while the source still runs.
///
/// ```
/// let vm: qrab_operator::api::VirtualMachine = serde_json::from_value(serde_json::json!({
///     "apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "VirtualMachine",
///     "metadata": {"name": "vm"},
///     "spec": {"stateVolumeClaim": "state", "mediaVolumeClaim": "media"}})).unwrap();
/// assert_eq!(qrab_operator::migration::shared_claims(&vm), [("media catalog", "media")]);
/// ```
pub fn shared_claims(vm: &VirtualMachine) -> Vec<(&'static str, &str)> {
    let redfish = &vm.spec.components.redfish;
    [
        (
            "Redfish state",
            redfish
                .state_volume_claim
                .as_deref()
                .filter(|_| vm.spec.components.redfish_enabled()),
        ),
        ("media catalog", vm.spec.media_volume_claim.as_deref()),
        (
            "firmware catalog",
            vm.spec.firmware_catalog_volume_claim.as_deref(),
        ),
    ]
    .into_iter()
    .filter_map(|(role, claim)| Some((role, claim?)))
    .collect()
}

/// Whether two Pods on different nodes can mount `claim` at once.
pub fn shareable(claim: &PersistentVolumeClaim) -> bool {
    claim
        .spec
        .as_ref()
        .and_then(|s| s.access_modes.as_ref())
        .is_some_and(|modes| {
            modes
                .iter()
                .any(|m| m == "ReadWriteMany" || m == "ReadOnlyMany")
        })
}

fn target_failure(target: &str, pod: &Pod) -> (&'static str, String) {
    match manager_exit(pod) {
        Some((_, message)) if message.trim() == SOURCE_POWERED_OFF => {
            (POWERED_OFF, POWERED_OFF_MESSAGE.into())
        }
        exit => {
            let detail = exit
                .map(|(code, message)| format!(" (exit {code}: {})", message.trim()))
                .unwrap_or_default();
            (
                "",
                format!("the target {target} stopped before taking over{detail}"),
            )
        }
    }
}

fn failed(pod: &Pod) -> bool {
    manager_exit(pod).is_some()
        || pod
            .status
            .as_ref()
            .and_then(|s| s.phase.as_deref())
            .is_some_and(|p| p == "Failed" || p == "Succeeded")
}

/// Pick the destination: the requested node, else the first Ready,
/// schedulable node other than the source (with KVM when the VM needs it).
pub fn choose_node(
    nodes: &[Node],
    source: &str,
    requested: Option<&str>,
    kvm: bool,
) -> std::result::Result<String, String> {
    let usable = |node: &&Node| {
        let ready = node
            .status
            .as_ref()
            .and_then(|s| s.conditions.as_ref())
            .is_some_and(|c| c.iter().any(|c| c.type_ == "Ready" && c.status == "True"));
        let schedulable = !node
            .spec
            .as_ref()
            .and_then(|s| s.unschedulable)
            .unwrap_or(false);
        let accelerated = !kvm
            || node
                .status
                .as_ref()
                .and_then(|s| s.allocatable.as_ref())
                .and_then(|a| a.get(KVM_RESOURCE))
                .is_some_and(|q| q.0 != "0");
        ready && schedulable && accelerated && node.name_any() != source
    };
    if requested == Some(source) {
        return Err("the target node is the source node".into());
    }
    let mut candidates: Vec<_> = nodes
        .iter()
        .filter(usable)
        .map(ResourceExt::name_any)
        .collect();
    candidates.sort();
    match requested {
        Some(node) if candidates.iter().any(|c| c == node) => Ok(node.into()),
        Some(node) => Err(format!(
            "node {node} is not Ready, schedulable and able to run this VM"
        )),
        None => candidates
            .into_iter()
            .next()
            .ok_or_else(|| "no other Ready node can run this VM".into()),
    }
}

async fn set_status(
    api: &Api<VirtualMachineMigration>,
    migration: &VirtualMachineMigration,
    status: &VirtualMachineMigrationStatus,
) -> Result<()> {
    api.patch_status(
        &migration.name_any(),
        &PatchParams::default(),
        &Patch::Merge(
            json!({"metadata":{"resourceVersion":migration.resource_version()},"status":status}),
        ),
    )
    .await?;
    Ok(())
}

async fn fail(
    api: &Api<VirtualMachineMigration>,
    migration: &VirtualMachineMigration,
    message: &str,
) -> Result<Option<Action>> {
    fail_because(api, migration, "", message).await
}

async fn fail_because(
    api: &Api<VirtualMachineMigration>,
    migration: &VirtualMachineMigration,
    reason: &str,
    message: &str,
) -> Result<Option<Action>> {
    tracing::warn!(migration=%migration.name_any(), %reason, %message, "live migration failed");
    let mut status = migration.status.clone().unwrap_or_default();
    status.phase = "Failed".into();
    status.reason = reason.into();
    status.message = message.into();
    status.completed_at = Some(now());
    set_status(api, migration, &status).await?;
    Ok(Some(Action::requeue(Duration::from_secs(1))))
}

fn child_identity(metadata: &ObjectMeta, vm: &VirtualMachine) -> Result<Preconditions> {
    if !resources::owned_meta(metadata, vm)
        || metadata.namespace != vm.namespace()
        || metadata.uid.as_deref().is_none_or(str::is_empty)
        || metadata
            .resource_version
            .as_deref()
            .is_none_or(str::is_empty)
    {
        return Err(Error::Ownership(format!(
            "migration child {} lacks this VM's ownership or object identity",
            metadata.name.as_deref().unwrap_or("<unnamed>")
        )));
    }
    Ok(Preconditions {
        uid: metadata.uid.clone(),
        resource_version: metadata.resource_version.clone(),
    })
}

async fn delete_pod(
    pods: &Api<Pod>,
    vm: &VirtualMachine,
    name: &str,
    expected_uid: Option<&str>,
) -> Result<bool> {
    let Some(pod) = pods.get_opt(name).await? else {
        return Ok(true);
    };
    let identity = child_identity(&pod.metadata, vm)?;
    if expected_uid.is_some_and(|uid| pod.metadata.uid.as_deref() != Some(uid)) {
        return Err(Error::Ownership(format!(
            "Pod {name} is another migration incarnation"
        )));
    }
    if pod.metadata.deletion_timestamp.is_none() {
        match pods
            .delete(name, &DeleteParams::default().preconditions(identity))
            .await
        {
            Ok(_) => {}
            Err(kube::Error::Api(status)) if status.code == 404 => return Ok(true),
            Err(error) => return Err(error.into()),
        }
        return Ok(false);
    }
    // A process that has stopped (or never started) holds no disk session.
    if manager_exit(&pod).is_some() || storage_guard::never_launched(&pod) {
        let finalizers: Vec<_> = pod
            .finalizers()
            .iter()
            .filter(|f| *f != storage_guard::POD_FINALIZER)
            .cloned()
            .collect();
        pods.patch(name, &PatchParams::default(), &Patch::Merge(json!({"metadata":{"uid":pod.uid(),"resourceVersion":pod.resource_version(),"finalizers":finalizers}}))).await?;
    }
    Ok(false)
}

async fn delete_policy(ctx: &Context, vm: &VirtualMachine, name: &str) -> Result<()> {
    let api = policies(ctx);
    let Some(policy) = api.get_opt(name).await? else {
        return Ok(());
    };
    let identity = child_identity(&policy.metadata, vm)?;
    match api
        .delete(name, &DeleteParams::default().preconditions(identity))
        .await
    {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(status)) if status.code == 404 => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn delete_config(configs: &Api<ConfigMap>, vm: &VirtualMachine, name: &str) -> Result<()> {
    let Some(config) = configs.get_opt(name).await? else {
        return Ok(());
    };
    let identity = child_identity(&config.metadata, vm)?;
    match configs
        .delete(name, &DeleteParams::default().preconditions(identity))
        .await
    {
        Ok(_) => Ok(()),
        Err(kube::Error::Api(status)) if status.code == 404 => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn state_claim_identity(
    claim: &PersistentVolumeClaim,
    vm: &VirtualMachine,
) -> Result<Preconditions> {
    let identity = child_identity(&claim.metadata, vm)?;
    if claim.metadata.deletion_timestamp.is_some() {
        return Err(Error::Ownership(format!(
            "PVC {} is being deleted",
            claim.name_any()
        )));
    }
    if claim
        .labels()
        .get("app.kubernetes.io/component")
        .map(String::as_str)
        != Some(STATE_COMPONENT)
        || claim.labels().get(OWNER_UID).map(String::as_str) != vm.metadata.uid.as_deref()
    {
        return Err(Error::Ownership(format!(
            "PVC {} is not this VM's migration state",
            claim.name_any()
        )));
    }
    Ok(identity)
}

/// Delete `name` only when it is a state PVC the operator created for this VM.
async fn delete_state_claim(
    claims: &Api<PersistentVolumeClaim>,
    name: &str,
    vm: &VirtualMachine,
) -> Result<()> {
    let Some(claim) = claims.get_opt(name).await? else {
        return Ok(());
    };
    let labels = claim.labels();
    if labels
        .get("app.kubernetes.io/component")
        .map(String::as_str)
        == Some(STATE_COMPONENT)
        && labels.get(OWNER_UID).map(String::as_str) == vm.metadata.uid.as_deref()
        && claim.metadata.deletion_timestamp.is_none()
    {
        let identity = state_claim_identity(&claim, vm)?;
        match claims
            .delete(name, &DeleteParams::default().preconditions(identity))
            .await
        {
            Ok(_) => {}
            Err(kube::Error::Api(status)) if status.code == 404 => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Finish a terminal migration's cleanup; true once nothing is left.
async fn clean(
    ctx: &Context,
    vm: &VirtualMachine,
    api: &Api<VirtualMachineMigration>,
    pods: &Api<Pod>,
    migration: &VirtualMachineMigration,
) -> Result<bool> {
    let claims: Api<PersistentVolumeClaim> =
        Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let status = migration.status.clone().unwrap_or_default();
    let uid = uid(vm)?;
    let current = Incarnation::of(vm);
    delete_policy(ctx, vm, &policy_name(&migration.name_any())).await?;
    // A late failure cannot undo authoritative adoption. Retire the old
    // source just as on success; keep every resource of the current target.
    let target_is_current = status.target_pod.as_deref() == Some(current.pod.as_str());
    let gone = if status.phase == "Succeeded" || target_is_current {
        let source_gone = match &status.source_pod {
            Some(source) if source != &current.pod => {
                delete_pod(pods, vm, source, status.source_pod_uid.as_deref()).await?
            }
            _ => true,
        };
        if source_gone {
            if let Some(previous) = status
                .previous_state_volume_claim
                .as_ref()
                .filter(|name| *name != &current.state_claim)
            {
                delete_state_claim(&claims, previous, vm).await?;
            }
        }
        source_gone
    } else {
        let target_gone = match &status.target_pod {
            Some(target) if target != &current.pod => {
                delete_pod(pods, vm, target, status.target_pod_uid.as_deref()).await?
            }
            _ => true,
        };
        // The target's standby anchors; the source's stay active.
        let selector = format!(
            "app.kubernetes.io/component={},{}={uid}",
            crate::nic::COMPONENT,
            OWNER_UID
        );
        for anchor in pods
            .list(&ListParams::default().labels(&selector))
            .await?
            .items
        {
            if anchor.annotations().get(crate::nic::MIGRATION_ANNOTATION)
                == Some(&migration.name_any())
            {
                delete_pod(pods, vm, &anchor.name_any(), anchor.metadata.uid.as_deref()).await?;
            }
        }
        if target_gone {
            if let Some(claim) = status
                .target_state_volume_claim
                .as_ref()
                .filter(|name| *name != &current.state_claim)
            {
                delete_state_claim(&claims, claim, vm).await?;
            }
            if let Some(target) = &status.target_pod {
                let configs: Api<ConfigMap> =
                    Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
                delete_config(
                    &configs,
                    vm,
                    &Incarnation {
                        pod: target.clone(),
                        state_claim: String::new(),
                        node: None,
                    }
                    .config_name(),
                )
                .await?;
            }
        }
        target_gone
    };
    if gone {
        let mut status = status;
        status.cleaned = true;
        set_status(api, migration, &status).await?;
    }
    Ok(gone)
}

/// Remove what migrations that no longer exist (deleted mid-flight) left: their
/// target Pods (released once their manager exits), the targets' ConfigMaps and
/// state PVCs, their standby anchors and time-boxed policies. The VM's own Pod
/// and its live anchors are never touched, and nothing while the source has
/// handed its guest over (a deleted migration's target may run it: the VM
/// blocks as `HandedOver` instead).
async fn collect_abandoned(
    ctx: &Context,
    vm: &VirtualMachine,
    pods: &Api<Pod>,
    source: &Pod,
    existing: &[String],
) -> Result<()> {
    if handed_over(source) {
        return Ok(());
    }
    let uid = uid(vm)?;
    let gone = |name: Option<&String>| name.is_some_and(|n| !existing.contains(n));
    let current = vm.status.as_ref().and_then(|s| s.pod_name.clone());
    let owned = format!("{OWNER_UID}={uid}");
    let claims: Api<PersistentVolumeClaim> =
        Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let configs: Api<ConfigMap> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    for pod in pods
        .list(&ListParams::default().labels(&owned))
        .await?
        .items
    {
        let name = pod.name_any();
        if Some(&name) == current.as_ref() || Some(&name) == source.metadata.name.as_ref() {
            continue;
        }
        let target = pod.annotations().get(TARGET_ANNOTATION);
        let standby = pod.annotations().get(crate::nic::MIGRATION_ANNOTATION);
        // A standby anchor on the VM's node is its live anchor after a move.
        let live_anchor = standby.is_some()
            && pod.spec.as_ref().and_then(|s| s.node_name.as_deref())
                == source.spec.as_ref().and_then(|s| s.node_name.as_deref());
        if gone(target) {
            let claim = pod.spec.as_ref().and_then(|s| {
                s.volumes.iter().flatten().find_map(|v| {
                    v.persistent_volume_claim
                        .as_ref()
                        .filter(|_| v.name == "state")
                        .map(|c| c.claim_name.clone())
                })
            });
            tracing::warn!(pod=%name, migration=?target, "removing the target of a deleted migration");
            delete_pod(pods, vm, &name, pod.metadata.uid.as_deref()).await?;
            // Now, while the Pod still names them: PVC protection keeps the
            // claim until the Pod is gone.
            if let Some(claim) = claim.filter(|c| {
                Some(c)
                    != vm
                        .status
                        .as_ref()
                        .and_then(|s| s.state_volume_claim.as_ref())
            }) {
                delete_state_claim(&claims, &claim, vm).await?;
            }
            let config = Incarnation {
                pod: name.clone(),
                state_claim: String::new(),
                node: None,
            }
            .config_name();
            delete_config(&configs, vm, &config).await?;
        } else if gone(standby) && !live_anchor {
            tracing::warn!(pod=%name, migration=?standby, "removing a standby anchor of a deleted migration");
            delete_pod(pods, vm, &name, pod.metadata.uid.as_deref()).await?;
        }
    }
    let windows = policies(ctx)
        .list(
            &ListParams::default()
                .labels(&format!("app.kubernetes.io/component=vm-migration,{owned}")),
        )
        .await?
        .items;
    for window in windows {
        let migration = window
            .name_any()
            .strip_prefix("qrab-migration-")
            .map(str::to_owned);
        if window.name_any() != guard_name(&vm.name_any()) && gone(migration.as_ref()) {
            delete_policy(ctx, vm, &window.name_any()).await?;
        }
    }
    Ok(())
}

async fn ensure_guard(ctx: &Context, vm: &VirtualMachine) -> Result<()> {
    let guard = guard(vm, &ctx.settings.namespace)?;
    let api = policies(ctx);
    let name = guard_name(&vm.name_any());
    let actual = api.get_opt(&name).await?;
    if let Some(actual) = &actual {
        child_identity(&actual.metadata, vm)?;
        if actual.data["spec"] == guard["spec"] {
            return Ok(());
        }
    }
    let mut object: DynamicObject = serde_json::from_value(guard)?;
    if let Some(actual) = actual {
        object.metadata.uid = actual.metadata.uid;
        object.metadata.resource_version = actual.metadata.resource_version;
    }
    api.patch(
        &name,
        &PatchParams::apply("qrab-operator").force(),
        &Patch::Apply(&object),
    )
    .await?;
    Ok(())
}

/// VM status is the durable adoption receipt. Completion publication on the
/// Migration can conflict after this write; retry must finish that receipt,
/// rather than classify the adopted target as a replacement source.
fn adoption_published(
    vm: &VirtualMachine,
    status: &VirtualMachineMigrationStatus,
    current: &Pod,
) -> bool {
    let Some(vm_status) = &vm.status else {
        return false;
    };
    status
        .target_pod_uid
        .as_deref()
        .is_some_and(|uid| !uid.is_empty())
        && vm_status.pod_name == status.target_pod
        && vm_status.pod_uid == status.target_pod_uid
        && current.uid() == status.target_pod_uid
        && vm_status.node_name == status.target_node
        && vm_status.state_volume_claim == status.target_state_volume_claim
        && vm_status
            .state_volume_uid
            .as_deref()
            .is_some_and(|uid| !uid.is_empty())
        && !current.annotations().contains_key(TARGET_ANNOTATION)
}

/// Run the VM's migrations. `Some` means the VM reconcile stops here: a
/// migration is in progress (or just changed state) and owns the VM's Pods.
pub async fn drive(
    ctx: &Context,
    vm: &VirtualMachine,
    vms: &Api<VirtualMachine>,
    pods: &Api<Pod>,
    source: &Pod,
    inputs: &Inputs<'_>,
) -> Result<Option<Action>> {
    ensure_guard(ctx, vm).await?;
    let api: Api<VirtualMachineMigration> =
        Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let mut migrations: Vec<_> = api
        .list(&ListParams::default())
        .await?
        .items
        .into_iter()
        .filter(|m| m.spec.vm_name == vm.name_any())
        .collect();
    let existing: Vec<String> = migrations.iter().map(|m| m.name_any()).collect();
    collect_abandoned(ctx, vm, pods, source, &existing).await?;
    migrations.sort_by_key(|m| {
        (
            m.metadata.creation_timestamp.clone().map(|t| t.0),
            m.name_any(),
        )
    });
    for migration in migrations.iter().filter(|m| terminal(phase(m))) {
        if !migration.status.as_ref().is_some_and(|s| s.cleaned)
            && !clean(ctx, vm, &api, pods, migration).await?
        {
            return Ok(Some(Action::requeue(Duration::from_secs(2))));
        }
    }
    let mut active = migrations.iter().filter(|m| !terminal(phase(m)));
    let Some(current) = active.next() else {
        return Ok(None);
    };
    for later in active.filter(|m| phase(m).is_empty() || phase(m) == "Pending") {
        fail(&api, later, "another migration of this VM is in progress").await?;
    }
    // Keep the large preparation/adoption subworkflow out of the parent
    // controller future's inline stack footprint.
    Box::pin(step(ctx, vm, vms, pods, source, inputs, &api, current)).await
}

#[allow(clippy::too_many_arguments)]
async fn step(
    ctx: &Context,
    vm: &VirtualMachine,
    vms: &Api<VirtualMachine>,
    pods: &Api<Pod>,
    source: &Pod,
    inputs: &Inputs<'_>,
    api: &Api<VirtualMachineMigration>,
    migration: &VirtualMachineMigration,
) -> Result<Option<Action>> {
    let name = migration.name_any();
    let suffix = suffix(migration)?;
    let vm_name = vm.name_any();
    let mut status = migration.status.clone().unwrap_or_default();
    if vm.metadata.deletion_timestamp.is_some() || !vm.spec.running {
        return fail(api, migration, "the VM is stopping").await;
    }
    if adoption_published(vm, &status, source) {
        child_identity(&source.metadata, vm)?;
        status.phase = "Succeeded".into();
        status.message = format!("The guest runs on {} now", source.name_any());
        status.completed_at = Some(now());
        set_status(api, migration, &status).await?;
        return Ok(Some(Action::requeue(Duration::from_secs(1))));
    }
    if status.phase.is_empty() || status.phase == "Pending" {
        if !ready(source) || source.metadata.deletion_timestamp.is_some() {
            if status.phase.is_empty() {
                status.phase = "Pending".into();
                status.message = "Waiting for the VM to be running and ready".into();
                set_status(api, migration, &status).await?;
            }
            return Ok(Some(Action::requeue(Duration::from_secs(5))));
        }
        if GuestSize::of(vm) != inputs.boot {
            return fail(api, migration, "the VM has hot-plugged vCPUs or memory beyond its boot size; restart it at the new size first").await;
        }
        // The Pod stays Ready while its guest is off; Redfish reports it.
        if vm.status.as_ref().and_then(|s| s.power_state.as_deref()) == Some("Off") {
            return fail_because(api, migration, POWERED_OFF, POWERED_OFF_MESSAGE).await;
        }
        let claims: Api<PersistentVolumeClaim> =
            Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
        for (role, claim) in shared_claims(vm) {
            if !claims.get_opt(claim).await?.is_some_and(|c| shareable(&c)) {
                return fail_because(api, migration, UNSHARED_CLAIM, &format!(
                    "the VM's {role} claim {claim} is not ReadWriteMany or ReadOnlyMany, so a target on another node cannot mount it while the source runs; live migration needs shareable {role} claims"
                )).await;
            }
        }
        let source_node = source
            .spec
            .as_ref()
            .and_then(|s| s.node_name.clone())
            .ok_or_else(|| Error::Invalid("the VM Pod is not scheduled".into()))?;
        let nodes: Api<Node> = Api::all(ctx.client.clone());
        let nodes = nodes.list(&ListParams::default()).await?.items;
        let kvm = vm.spec.accelerator == crate::api::Accelerator::Kvm;
        let target_node = match choose_node(
            &nodes,
            &source_node,
            migration.spec.target_node.as_deref(),
            kvm,
        ) {
            Ok(node) => node,
            Err(message) => return fail(api, migration, &message).await,
        };
        let current = Incarnation::of(vm);
        status = VirtualMachineMigrationStatus {
            phase: "TargetStarting".into(),
            reason: String::new(),
            message: format!("Starting the target on {target_node}"),
            source_pod: Some(source.name_any()),
            source_pod_uid: source.uid(),
            source_node: Some(source_node),
            target_pod: Some(target_pod_name(&vm_name, &suffix)),
            target_pod_uid: None,
            target_node: Some(target_node),
            target_state_volume_claim: Some(target_claim_name(&vm_name, &suffix)),
            previous_state_volume_claim: Some(current.state_claim),
            started_at: Some(now()),
            completed_at: None,
            cleaned: false,
        };
        set_status(api, migration, &status).await?;
        let mut vm_status = crate::controller::state(
            vm,
            Some(source),
            "Migrating",
            true,
            "Migrating",
            &format!("Live migration {name} is starting"),
        );
        vm_status.resize = vm.status.as_ref().and_then(|s| s.resize.clone());
        crate::controller::publish(vms, vm, vm_status).await?;
        return Ok(Some(Action::requeue(Duration::from_secs(1))));
    }
    let (Some(target), Some(target_node), Some(target_claim)) = (
        status.target_pod.clone(),
        status.target_node.clone(),
        status.target_state_volume_claim.clone(),
    ) else {
        return fail(api, migration, "the migration status lost its target").await;
    };
    if source.uid() != status.source_pod_uid {
        return fail(
            api,
            migration,
            "the source VM Pod was replaced during the migration",
        )
        .await;
    }
    let source_exit = manager_exit(source);
    let live_target = pods.get_opt(&target).await?;
    if let Some(target_pod) = &live_target {
        child_identity(&target_pod.metadata, vm)?;
        if target_pod.uid() == source.uid()
            || status
                .target_pod_uid
                .as_ref()
                .is_some_and(|uid| target_pod.metadata.uid.as_ref() != Some(uid))
            || (status.target_pod_uid.is_none()
                && target_pod.annotations().get(TARGET_ANNOTATION) != Some(&name))
        {
            return Err(Error::Ownership(format!(
                "Pod {target} is not the recorded migration target"
            )));
        }
        // A target without storage gates can finish before our first Pod
        // observation. Persist identity before either adoption or ungating,
        // so a lost completion write always has a durable UID receipt.
        if status.target_pod_uid.is_none() {
            status.target_pod_uid = target_pod.uid();
            set_status(api, migration, &status).await?;
            return Ok(Some(Action::requeue(Duration::from_secs(1))));
        }
    }
    // The source handed the guest over: the target owns it and its disks now.
    if handed_over(source) {
        let Some(target_pod) = live_target else {
            return Err(Error::Invalid(format!("the source of migration {name} handed over to {target}, which is gone; fence its disks before restarting the VM")));
        };
        return Box::pin(adopt(
            ctx,
            vm,
            vms,
            api,
            migration,
            status,
            &target_pod,
            inputs,
        ))
        .await;
    }
    if source_exit.is_some() {
        return fail(
            api,
            migration,
            "the source VM process exited before handing over",
        )
        .await;
    }
    if status
        .started_at
        .is_some_and(|start| now() > start + DEADLINE_SECS)
    {
        return fail(
            api,
            migration,
            "the migration did not finish within 10 minutes",
        )
        .await;
    }
    let source_ip = source
        .status
        .as_ref()
        .and_then(|s| s.pod_ip.clone())
        .ok_or_else(|| Error::Invalid("the VM Pod has no IP".into()))?;
    let policy = window(
        vm,
        &ctx.settings.namespace,
        &name,
        &source.name_any(),
        &target,
    )?;
    match policies(ctx).get_opt(&policy_name(&name)).await? {
        Some(actual) => {
            child_identity(&actual.metadata, vm)?;
            if actual.data["specs"] != policy["specs"] {
                return Err(Error::Ownership(
                    "existing migration window has another plan".into(),
                ));
            }
        }
        None => {
            policies(ctx)
                .create(&PostParams::default(), &serde_json::from_value(policy)?)
                .await?;
        }
    }
    let incarnation = Incarnation {
        pod: target.clone(),
        state_claim: target_claim.clone(),
        node: Some(target_node.clone()),
    };
    let desired = resources::render_incarnation(
        vm,
        &ctx.settings,
        inputs.provisioned,
        inputs.boot,
        inputs.nics,
        &incarnation,
    )?;
    let claims: Api<PersistentVolumeClaim> =
        Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    if let Some(actual) = claims.get_opt(&target_claim).await? {
        state_claim_identity(&actual, vm)?;
    } else {
        let current = claims
            .get_opt(
                status
                    .previous_state_volume_claim
                    .as_deref()
                    .unwrap_or(&vm.spec.state_volume_claim),
            )
            .await?
            .ok_or_else(|| Error::Invalid("the source state PVC is missing".into()))?;
        let spec = current.spec.clone().unwrap_or_default();
        let claim = json!({"apiVersion":"v1","kind":"PersistentVolumeClaim",
            "metadata":{"name":target_claim,"namespace":ctx.settings.namespace,
                "labels":{"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":STATE_COMPONENT,OWNER_UID:uid(vm)?},
                "ownerReferences":[owner(vm)?]},
            "spec":{"accessModes":["ReadWriteOncePod"],"volumeMode":"Filesystem",
                "storageClassName":spec.storage_class_name,"resources":{"requests":spec.resources.and_then(|r| r.requests).unwrap_or_default()}}});
        claims
            .create(&PostParams::default(), &serde_json::from_value(claim)?)
            .await?;
    }
    // Each NIC's anchor on the target node, as a standby holding the
    // source anchor's address (forks/cilium P1/P2), plumbed by the target
    // node's qrab-netd before the target VM Pod starts.
    if live_target.is_none() && !vm.spec.nics.is_empty() {
        let source_node = status.source_node.clone().unwrap_or_default();
        let mut pending = Vec::new();
        for nic in &vm.spec.nics {
            let ip = inputs
                .nics
                .iter()
                .find(|e| e.name == nic.name)
                .and_then(|e| match e.ip {
                    std::net::IpAddr::V4(ip) => Some(ip),
                    std::net::IpAddr::V6(_) => None,
                })
                .ok_or_else(|| Error::Invalid(format!("NIC {} has no address", nic.name)))?;
            let anchor = crate::nic::handoff_target(
                crate::nic::render_anchor(
                    vm,
                    &ctx.settings,
                    nic,
                    &crate::address::pool_name(ip),
                    &target_node,
                )?,
                &ctx.settings.namespace,
                &crate::nic::anchor_name(vm, &nic.name, &source_node),
                &name,
            );
            match pods.get_opt(&anchor.pod.name_any()).await? {
                None => {
                    pods.create(&PostParams::default(), &anchor.pod).await?;
                    pending.push(nic.name.clone());
                }
                Some(actual) if !resources::owned(&actual, vm) => {
                    return fail(
                        api,
                        migration,
                        &format!("Pod {} does not belong to this VM", actual.name_any()),
                    )
                    .await;
                }
                Some(actual) if crate::nic::endpoint(&actual, &nic.name).is_none() => {
                    pending.push(nic.name.clone())
                }
                Some(_) => {}
            }
        }
        if !pending.is_empty() {
            if status.message != "Waiting for the target NIC anchors" {
                status.message = "Waiting for the target NIC anchors".into();
                set_status(api, migration, &status).await?;
            }
            return Ok(Some(Action::requeue(Duration::from_secs(2))));
        }
    }
    let node_ip = if vm.spec.nics.is_empty() {
        None
    } else {
        let nodes: Api<Node> = Api::all(ctx.client.clone());
        nodes
            .get_opt(&target_node)
            .await?
            .and_then(|n| {
                n.status?
                    .addresses?
                    .into_iter()
                    .find(|a| a.type_ == "InternalIP")
            })
            .map(|a| a.address)
    };
    let configs: Api<ConfigMap> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let config = incoming_config(
        &desired,
        &migration.uid().unwrap_or_default(),
        &source_ip,
        node_ip.as_deref(),
    )?;
    match configs.get_opt(&config.name_any()).await? {
        Some(actual) => {
            child_identity(&actual.metadata, vm)?;
            if actual.data != config.data {
                return Err(Error::Ownership(
                    "existing migration ConfigMap has another plan".into(),
                ));
            }
        }
        None => {
            configs.create(&PostParams::default(), &config).await?;
        }
    }
    let Some(target_pod) = live_target else {
        let mut pod = desired.pod.clone();
        pod.annotations_mut()
            .insert(TARGET_ANNOTATION.into(), name.clone());
        pods.create(&PostParams::default(), &pod).await?;
        return Ok(Some(Action::requeue(Duration::from_secs(1))));
    };
    if !resources::owned(&target_pod, vm) {
        return fail(
            api,
            migration,
            &format!("Pod {target} does not belong to this VM"),
        )
        .await;
    }
    if target_pod
        .spec
        .as_ref()
        .and_then(|s| s.scheduling_gates.as_ref())
        .is_some_and(|g| g.iter().any(|g| g.name == storage_guard::OWNER_GATE))
    {
        let gates: Vec<_> = target_pod
            .spec
            .as_ref()
            .and_then(|s| s.scheduling_gates.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|g| g.name != storage_guard::OWNER_GATE)
            .collect();
        pods.patch(&target, &PatchParams::default(), &Patch::Merge(json!({"metadata":{"uid":target_pod.uid(),"resourceVersion":target_pod.resource_version()},"spec":{"schedulingGates":gates}}))).await?;
        return Ok(Some(Action::requeue(Duration::from_secs(1))));
    }
    if failed(&target_pod) || target_pod.metadata.deletion_timestamp.is_some() {
        let (reason, message) = target_failure(&target, &target_pod);
        return fail_because(api, migration, reason, &message).await;
    }
    let running = target_pod.status.as_ref().and_then(|s| s.phase.as_deref()) == Some("Running");
    if running && status.phase != "Migrating" {
        status.phase = "Migrating".into();
        status.message = format!(
            "Streaming the guest from {} to {target_node}",
            status.source_node.clone().unwrap_or_default()
        );
        set_status(api, migration, &status).await?;
    }
    Ok(Some(Action::requeue(Duration::from_secs(2))))
}

#[allow(clippy::too_many_arguments)]
async fn adopt(
    ctx: &Context,
    vm: &VirtualMachine,
    vms: &Api<VirtualMachine>,
    api: &Api<VirtualMachineMigration>,
    migration: &VirtualMachineMigration,
    mut status: VirtualMachineMigrationStatus,
    target: &Pod,
    inputs: &Inputs<'_>,
) -> Result<Option<Action>> {
    let (Some(target_node), Some(claim_name)) = (
        status.target_node.clone(),
        status.target_state_volume_claim.clone(),
    ) else {
        return fail(api, migration, "the migration status lost its target").await;
    };
    let incarnation = Incarnation {
        pod: target.name_any(),
        state_claim: claim_name.clone(),
        node: Some(target_node.clone()),
    };
    let desired = resources::render_incarnation(
        vm,
        &ctx.settings,
        inputs.provisioned,
        inputs.boot,
        inputs.nics,
        &incarnation,
    )?;
    // Without `incoming`, the VM's normal reconcile finds this config current.
    let configs: Api<ConfigMap> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let actual = configs.get(&desired.config.name_any()).await?;
    child_identity(&actual.metadata, vm)?;
    let claims: Api<PersistentVolumeClaim> =
        Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    let claim = claims
        .get_opt(&claim_name)
        .await?
        .ok_or_else(|| Error::Invalid(format!("the target state PVC {claim_name} is missing")))?;
    state_claim_identity(&claim, vm)?;
    let claim_uid = claim.uid().expect("checked claim UID");
    configs
        .patch(
            &desired.config.name_any(),
            &PatchParams::default(),
            &Patch::Merge(json!({"metadata":{"uid":actual.metadata.uid,"resourceVersion":actual.metadata.resource_version},"data":desired.config.data})),
        )
        .await?;
    let source = status.source_node.clone().unwrap_or_default();
    let mut vm_status: VirtualMachineStatus = crate::controller::state(
        vm,
        Some(target),
        "Running",
        ready(target),
        "Migrated",
        &format!(
            "Live-migrated from {source} to {target_node} by {}",
            migration.name_any()
        ),
    );
    vm_status.pod_name = Some(target.name_any());
    vm_status.pod_uid = target.uid();
    vm_status.node_name = Some(target_node.clone());
    vm_status.storage_released_pod_uid = None;
    vm_status.storage_recovery = None;
    vm_status.state_volume_claim = Some(claim_name);
    vm_status.state_volume_uid = Some(claim_uid);
    vm_status.placement_node = Some(target_node);
    // The target owns the guest now: it is no longer a disposable target.
    let pods: Api<Pod> = Api::namespaced(ctx.client.clone(), &ctx.settings.namespace);
    pods.patch(
        &target.name_any(),
        &PatchParams::default(),
        &Patch::Merge(
            json!({"metadata":{"uid":target.uid(),"resourceVersion":target.resource_version(),"annotations":{TARGET_ANNOTATION:null}}}),
        ),
    )
    .await?;
    crate::controller::publish(vms, vm, vm_status).await?;
    status.phase = "Succeeded".into();
    status.message = format!("The guest runs on {} now", target.name_any());
    status.completed_at = Some(now());
    set_status(api, migration, &status).await?;
    tracing::info!(migration=%migration.name_any(), vm=%vm.name_any(), "live migration succeeded");
    Ok(Some(Action::requeue(Duration::from_secs(1))))
}
