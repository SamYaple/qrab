//! The dashboard's finalizer: a deleted VM's Kanidm access groups are deleted
//! before the VM goes, so its name can be reused and no stale group lingers.
//!
//! Every [`INTERVAL`] the dashboard adds [`FINALIZER`] to each VM it launched
//! that lacks it (older launches included). For a VM being deleted that carries
//! it, the dashboard deletes each group in the VM's console admin and viewer
//! lists that it [`owned`], then removes the finalizer. The update policy
//! (`qrab-operator dashboard-update-policy`) lets the dashboard change no other
//! finalizer.
use crate::{
    access::{MANAGED_BY, MANAGER},
    cluster::Cluster,
    kanidm::{Group, Kanidm, group_name},
};
use anyhow::{Context, Result};
use qrab_operator::api::VirtualMachine;
use std::{sync::Arc, time::Duration};

pub use qrab_operator::api::DASHBOARD_FINALIZER as FINALIZER;
pub const INTERVAL: Duration = Duration::from_secs(15);

/// Whether `group` is the `role` access group this dashboard created for VM
/// `namespace/vm`. Its name must be a launch-ID name (`qrab_vm_<32 hex>_<role>`)
/// or this VM's own pre-launch-ID name (`qrab_vm_<ns>_<vm>_<role>`), and its
/// description, which only Kanidm administrators can change, must name this VM
/// and role as the dashboard wrote it. Editing a VM's console lists therefore
/// cannot get a platform group or another VM's group deleted.
///
/// ```
/// use qrab_dashboard::{cleanup::owned, kanidm::Group};
/// let g = |name: &str, description: &str| Group {
///     uuid: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
///     name: name.into(),
///     description: description.into(),
/// };
/// let id = "0123456789abcdef0123456789abcdef";
/// let admins = format!("qrab_vm_{id}_admins");
/// let web = "qrab VM qrab/web admins, launched by alice through qrab-dashboard";
/// assert!(owned(&g(&admins, web), "qrab", "web", "admins"));
/// assert!(owned(
///     &g("qrab_vm_qrab_web_viewers", "qrab VM qrab/web viewers, launched by alice through qrab-dashboard"),
///     "qrab", "web", "viewers"
/// ));
/// // Another VM's group, a platform group, the wrong role, a lookalike name:
/// assert!(!owned(&g(&admins, "qrab VM qrab/db admins, launched by alice"), "qrab", "web", "admins"));
/// assert!(!owned(&g("qrab_operators", web), "qrab", "web", "admins"));
/// assert!(!owned(&g(&admins, web), "qrab", "web", "viewers"));
/// assert!(!owned(&g("qrab_vm_qrab_db_admins", web), "qrab", "web", "admins"));
/// assert!(!owned(&g(&format!("qrab_vm_{}_admins", id.to_uppercase()), web), "qrab", "web", "admins"));
/// ```
pub fn owned(group: &Group, namespace: &str, vm: &str, role: &str) -> bool {
    let generated = group
        .name
        .strip_prefix("qrab_vm_")
        .and_then(|rest| rest.strip_suffix(&format!("_{role}")))
        .is_some_and(|id| {
            id.len() == 32
                && id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
    let legacy = group.name == format!("qrab_vm_{namespace}_{vm}_{role}");
    (generated || legacy)
        && group
            .description
            .starts_with(&format!("qrab VM {namespace}/{vm} {role}, "))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A live VM this dashboard launched, without the finalizer.
    AddFinalizer,
    /// A VM being deleted that still carries the finalizer.
    Finalize,
    Nothing,
}

pub fn step(vm: &VirtualMachine) -> Step {
    let has = vm
        .metadata
        .finalizers
        .as_ref()
        .is_some_and(|f| f.iter().any(|f| f == FINALIZER));
    let managed = vm
        .metadata
        .labels
        .as_ref()
        .and_then(|l| l.get(MANAGED_BY))
        .is_some_and(|v| v == MANAGER);
    match (vm.metadata.deletion_timestamp.is_some(), has) {
        (true, true) => Step::Finalize,
        (false, false) if managed => Step::AddFinalizer,
        _ => Step::Nothing,
    }
}

/// One pass over the namespace's VMs. A VM whose groups cannot be checked
/// (Kanidm unavailable) keeps its finalizer and is retried on the next pass.
pub async fn reconcile(cluster: &dyn Cluster, kanidm: &Kanidm, namespace: &str) -> Result<()> {
    for vm in cluster.list_vms().await? {
        if let Err(error) = reconcile_vm(cluster, kanidm, namespace, &vm).await {
            tracing::warn!(vm=?vm.metadata.name, error=%format!("{error:#}"), "access-group finalizer: retrying later");
        }
    }
    Ok(())
}

async fn reconcile_vm(
    cluster: &dyn Cluster,
    kanidm: &Kanidm,
    namespace: &str,
    vm: &VirtualMachine,
) -> Result<()> {
    let step = step(vm);
    if step == Step::Nothing {
        return Ok(());
    }
    let name = vm.metadata.name.as_deref().context("VM without a name")?;
    let uid = vm.metadata.uid.as_deref().context("VM without a UID")?;
    let version = vm
        .metadata
        .resource_version
        .as_deref()
        .context("VM without a resourceVersion")?;
    let mut finalizers = vm.metadata.finalizers.clone().unwrap_or_default();
    if step == Step::AddFinalizer {
        finalizers.push(FINALIZER.into());
        cluster
            .set_vm_finalizers(name, uid, version, &finalizers)
            .await?;
        tracing::info!(vm=%name, "added the access-group finalizer");
        return Ok(());
    }
    if let Some(console) = &vm.spec.console {
        for (role, list) in [
            ("admins", &console.admin_groups),
            ("viewers", &console.viewer_groups),
        ] {
            for entry in list {
                // Anything else cannot name a group the dashboard created.
                if crate::config::uuid(entry).is_none() && !group_name(entry) {
                    continue;
                }
                match kanidm.group(entry).await? {
                    Some(group) if owned(&group, namespace, name, role) => {
                        kanidm.delete_group(&group.uuid).await?;
                        tracing::info!(vm=%name, group=%group.name, uuid=%group.uuid, "deleted the deleted VM's access group");
                    }
                    _ => {}
                }
            }
        }
    }
    finalizers.retain(|f| f != FINALIZER);
    cluster
        .set_vm_finalizers(name, uid, version, &finalizers)
        .await?;
    tracing::info!(vm=%name, "removed the access-group finalizer");
    Ok(())
}

/// Runs [`reconcile`] every [`INTERVAL`] until the process exits.
pub async fn run(cluster: Arc<dyn Cluster>, kanidm: Kanidm, namespace: String) {
    loop {
        if let Err(error) = reconcile(cluster.as_ref(), &kanidm, &namespace).await {
            tracing::warn!(error=%format!("{error:#}"), "access-group finalizer: listing VMs failed");
        }
        tokio::time::sleep(INTERVAL).await;
    }
}
