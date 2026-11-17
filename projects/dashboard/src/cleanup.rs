//! The dashboard's finalizer: a deleted VM's Kanidm access groups are deleted
//! before the VM goes, so its name can be reused and no stale group lingers.
//!
//! Every [`INTERVAL`] the dashboard adds [`FINALIZER`] to each VM it launched
//! that lacks it (older launches included). For a VM being deleted that carries
//! it, the dashboard deletes its recorded launch-owned groups after checking
//! exact provenance, then removes the finalizer. Mutable console grants do not
//! determine cleanup authority. Pre-launch-ID VMs use only their exact legacy
//! names. The update policy
//! (`qrab-operator dashboard-update-policy`) lets the dashboard change no other
//! finalizer.
use crate::{
    access::{MANAGED_BY, MANAGER},
    cluster::Cluster,
    kanidm::{Group, Kanidm},
    launch::{self, ADMINS_GROUP, LAUNCH_ID, VIEWERS_GROUP},
};
use anyhow::{Context, Result, ensure};
use kube::ResourceExt;
use qrab_operator::api::VirtualMachine;
use std::{sync::Arc, time::Duration};

pub use qrab_operator::api::DASHBOARD_FINALIZER as FINALIZER;
pub const INTERVAL: Duration = Duration::from_secs(15);

/// Whether `group` is the `role` access group this dashboard created for VM
/// `namespace/vm`. Its name must match the exact recorded launch ID, or, when
/// there is no launch ID, the VM's exact pre-launch-ID name. Its
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
/// assert!(owned(&g(&admins, web), "qrab", "web", "admins", Some(id)));
/// assert!(owned(
///     &g("qrab_vm_qrab_web_viewers", "qrab VM qrab/web viewers, launched by alice through qrab-dashboard"),
///     "qrab", "web", "viewers", None
/// ));
/// // Another VM's group, a platform group, the wrong role, a lookalike name:
/// assert!(!owned(&g(&admins, "qrab VM qrab/db admins, launched by alice"), "qrab", "web", "admins", Some(id)));
/// assert!(!owned(&g("qrab_operators", web), "qrab", "web", "admins", Some(id)));
/// assert!(!owned(&g(&admins, web), "qrab", "web", "viewers", Some(id)));
/// assert!(!owned(&g("qrab_vm_qrab_db_admins", web), "qrab", "web", "admins", None));
/// assert!(!owned(&g(&format!("qrab_vm_{}_admins", id.to_uppercase()), web), "qrab", "web", "admins", Some(id)));
/// // Same VM name, different launch: neither modern nor legacy cleanup owns it.
/// assert!(!owned(&g(&admins, web), "qrab", "web", "admins", Some("ffffffffffffffffffffffffffffffff")));
/// assert!(!owned(&g(&admins, web), "qrab", "web", "admins", None));
/// ```
pub fn owned(
    group: &Group,
    namespace: &str,
    vm: &str,
    role: &str,
    launch_id: Option<&str>,
) -> bool {
    if !matches!(role, "admins" | "viewers") {
        return false;
    }
    let expected = match launch_id {
        Some(id) => match launch::names(vm, id) {
            Ok(names) if role == "admins" => names.admins,
            Ok(names) => names.viewers,
            Err(_) => return false,
        },
        None => format!("qrab_vm_{namespace}_{vm}_{role}"),
    };
    group.name == expected
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
    let launch_id = vm.annotations().get(LAUNCH_ID).map(String::as_str);
    if let Some(id) = launch_id {
        launch::names(name, id).map_err(anyhow::Error::msg)?;
    } else {
        ensure!(
            !vm.annotations().contains_key(ADMINS_GROUP)
                && !vm.annotations().contains_key(VIEWERS_GROUP),
            "incomplete launch provenance; refusing legacy group cleanup"
        );
    }
    // Resolve and validate both references before any deletion. Incomplete
    // metadata keeps the finalizer; it never falls back to mutable grants.
    let references = ["admins", "viewers"]
        .into_iter()
        .zip([ADMINS_GROUP, VIEWERS_GROUP])
        .map(|(role, key)| {
            let reference = match launch_id {
                Some(_) => crate::config::uuid(
                    vm.annotations()
                        .get(key)
                        .context("missing recorded group UUID")?,
                )
                .context("invalid recorded group UUID")?,
                None => format!("qrab_vm_{namespace}_{name}_{role}"),
            };
            Ok((role, reference))
        })
        .collect::<Result<Vec<_>>>()?;
    for (role, reference) in references {
        if let Some(group) = kanidm.group(&reference).await? {
            ensure!(
                owned(&group, namespace, name, role, launch_id)
                    && (launch_id.is_none() || group.uuid == reference),
                "recorded access group does not match this VM launch"
            );
            kanidm.delete_group(&group.uuid).await?;
            tracing::info!(vm=%name, group=%group.name, uuid=%group.uuid, "deleted the deleted VM's access group");
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
