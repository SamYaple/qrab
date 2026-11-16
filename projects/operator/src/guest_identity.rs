//! Guest workload identity registration (docs/spire-vm.md, "Guest workload
//! identity"): the SPIRE `ClusterStaticEntry` objects of a VM's
//! `spec.guestIdentity`.
//!
//! One node alias entry maps every SPIRE agent whose `tpm_devid` DevID
//! certificate names this VM (subject CN `qrab-vm-<uid>`, issued by the qrab
//! DevID CA) to `spiffe://yapplesauce.com/qrab/guest-node/<uid>`, and one
//! entry per workload hangs under that alias. `<uid>` is the VM's UID, the
//! one its manager Pod's `vm/<uid>` SVID carries. Entries are cluster-scoped,
//! so a finalizer on the VM ([`FINALIZER`]) deletes them, and labels tie each
//! to its VM. contrib/kubernetes/operator/guest-identity.yaml bounds what the
//! operator may write.
use crate::{
    api::{GuestIdentity, VirtualMachine},
    nic, Error, Result,
};
use kube::{
    api::{
        ApiResource, DeleteParams, DynamicObject, GroupVersionKind, ListParams, Patch, PatchParams,
        PostParams, Preconditions,
    },
    Api, Client, Resource, ResourceExt,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// On the VM while it may own ClusterStaticEntries.
pub const FINALIZER: &str = "qrab.yapplesauce.com/guest-identity";
pub const COMPONENT: &str = "guest-identity";
/// Every entry the operator writes is named `qrab-guest-*`.
pub const NAME_PREFIX: &str = "qrab-guest-";
/// The namespace of the VM an entry belongs to.
pub const VM_NAMESPACE_LABEL: &str = "qrab.yapplesauce.com/vm-namespace";
/// The default issuer CN of the qrab DevID CA (`QRAB_GUEST_DEVID_CA_CN`).
pub const DEFAULT_DEVID_CA_CN: &str = "qrab-devid-ca-1";
/// The spire-controller-manager class that reconciles the entries; it ignores
/// classless ones.
pub const CLASS_NAME: &str = "kubernetes-tpm";
pub const SERVER_ID: &str = "spiffe://yapplesauce.com/spire/server";
pub const WORKLOAD_X509_SVID_TTL: &str = "10m";
pub const MAX_WORKLOADS: usize = 16;
pub const MAX_SELECTORS: usize = 8;

pub fn entry_resource() -> ApiResource {
    ApiResource::from_gvk_with_plural(
        &GroupVersionKind::gvk("spire.spiffe.io", "v1alpha1", "ClusterStaticEntry"),
        "clusterstaticentries",
    )
}

/// The guest's node alias SPIFFE ID.
///
/// ```
/// use qrab_operator::guest_identity::{node_alias_id, workload_id};
/// let uid = "0b7c4e6a-1d2e-4f30-8a4b-5c6d7e8f9a0b";
/// assert_eq!(node_alias_id(uid), format!("spiffe://yapplesauce.com/qrab/guest-node/{uid}"));
/// assert_eq!(workload_id(uid, "web"), format!("spiffe://yapplesauce.com/qrab/guest/{uid}/web"));
/// ```
pub fn node_alias_id(uid: &str) -> String {
    format!("spiffe://yapplesauce.com/qrab/guest-node/{uid}")
}

pub fn workload_id(uid: &str, workload: &str) -> String {
    format!("spiffe://yapplesauce.com/qrab/guest/{uid}/{workload}")
}

pub fn node_alias_name(uid: &str) -> String {
    format!("{NAME_PREFIX}node-{uid}")
}

pub fn workload_entry_name(uid: &str, workload: &str) -> String {
    format!("{NAME_PREFIX}{uid}-{workload}")
}

/// A workload name: a DNS label of at most 40 characters.
///
/// ```
/// use qrab_operator::guest_identity::valid_workload_name;
/// assert!(valid_workload_name("web-1"));
/// assert!(!valid_workload_name("Web") && !valid_workload_name("web-") && !valid_workload_name(""));
/// assert!(!valid_workload_name(&"a".repeat(41)));
/// ```
pub fn valid_workload_name(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !name.starts_with('-')
        && !name.ends_with('-')
}

/// A selector of the guest agent's `unix` or `systemd` workload attestor:
/// `<attestor>:<type>:<value>`, with no whitespace.
///
/// ```
/// use qrab_operator::guest_identity::valid_selector;
/// assert!(valid_selector("unix:uid:1000") && valid_selector("systemd:id:app.service"));
/// assert!(!valid_selector("k8s:ns:qrab") && !valid_selector("unix:uid:") && !valid_selector("unix:path:/a b"));
/// ```
pub fn valid_selector(selector: &str) -> bool {
    let Some(rest) = selector
        .strip_prefix("unix:")
        .or_else(|| selector.strip_prefix("systemd:"))
    else {
        return false;
    };
    let Some((kind, value)) = rest.split_once(':') else {
        return false;
    };
    (1..=32).contains(&kind.len())
        && kind.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        && (1..=200).contains(&value.len())
        && value.bytes().all(|b| b.is_ascii_graphic())
}

/// The default qrab-tpm-issuer origin (`QRAB_TPM_ISSUER_URL`).
pub const DEFAULT_TPM_ISSUER_URL: &str = "https://qrab-tpm-issuer.qrab-tpm-issuer.svc:8443";
/// The default SPIFFE ID VM managers require of it (`QRAB_TPM_ISSUER_ID`).
pub const DEFAULT_TPM_ISSUER_ID: &str =
    "spiffe://yapplesauce.com/kubernetes/qrab-tpm-issuer/qrab-tpm-issuer";

/// Where VM managers get their vTPM's EK and DevID certificates: rendered
/// into vm.json (`guest_identity`) for VMs with `spec.guestIdentity`.
///
/// ```
/// use qrab_operator::guest_identity::{TpmIssuer, DEFAULT_TPM_ISSUER_URL};
/// let env = |name: &str| (name == "QRAB_TPM_ISSUER_URL").then(|| "https://issuer:9443".to_string());
/// let issuer = TpmIssuer::from_env(env).unwrap();
/// assert_eq!(issuer.url, "https://issuer:9443");
/// assert_eq!(TpmIssuer::from_env(|_| None).unwrap(), TpmIssuer::default());
/// assert_eq!(TpmIssuer::default().url, DEFAULT_TPM_ISSUER_URL);
/// assert!(TpmIssuer::from_env(|_| Some("http://issuer".into())).is_err());
/// assert!(TpmIssuer::from_env(|n| (n == "QRAB_TPM_ISSUER_ID").then(|| "issuer".into())).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpmIssuer {
    pub url: String,
    pub id: String,
}

impl Default for TpmIssuer {
    fn default() -> Self {
        Self {
            url: DEFAULT_TPM_ISSUER_URL.into(),
            id: DEFAULT_TPM_ISSUER_ID.into(),
        }
    }
}

impl TpmIssuer {
    pub fn from_env(env: impl Fn(&str) -> Option<String>) -> std::result::Result<Self, String> {
        let url = env("QRAB_TPM_ISSUER_URL").unwrap_or_else(|| DEFAULT_TPM_ISSUER_URL.into());
        let id = env("QRAB_TPM_ISSUER_ID").unwrap_or_else(|| DEFAULT_TPM_ISSUER_ID.into());
        let authority = url.strip_prefix("https://").unwrap_or_default();
        if authority.is_empty() || authority.contains(['/', '?', '#', '@']) {
            return Err("QRAB_TPM_ISSUER_URL must be https://host[:port] without a path".into());
        }
        qrab_spiffe::id(&id).map_err(|_| "QRAB_TPM_ISSUER_ID must be a SPIFFE ID".to_string())?;
        Ok(Self { url, id })
    }

    /// The manager's `guest_identity` config block.
    pub fn manager_config(&self) -> serde_json::Value {
        json!({"enabled":true,"issuer_url":self.url,"issuer_id":self.id})
    }
}

/// An X.509 CN fit for a `tpm_devid:issuer:cn:` selector.
///
/// ```
/// use qrab_operator::guest_identity::{valid_devid_ca_cn, DEFAULT_DEVID_CA_CN};
/// assert!(valid_devid_ca_cn(DEFAULT_DEVID_CA_CN) && valid_devid_ca_cn("qrab DevID CA 2"));
/// assert!(!valid_devid_ca_cn("") && !valid_devid_ca_cn(" x") && !valid_devid_ca_cn("a:b"));
/// ```
pub fn valid_devid_ca_cn(cn: &str) -> bool {
    (1..=64).contains(&cn.len())
        && cn
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" ._-".contains(&b))
        && cn.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
        && !cn.ends_with(' ')
}

pub fn validate(identity: &GuestIdentity) -> Result<()> {
    if identity.workloads.len() > MAX_WORKLOADS {
        return Err(Error::Invalid(format!(
            "guestIdentity may declare at most {MAX_WORKLOADS} workloads"
        )));
    }
    let mut names = BTreeSet::new();
    for workload in &identity.workloads {
        if !valid_workload_name(&workload.name) || !names.insert(&workload.name) {
            return Err(Error::Invalid(
                "guestIdentity workload names must be unique lowercase DNS labels of 1..40 characters"
                    .into(),
            ));
        }
        let mut selectors = BTreeSet::new();
        if workload.selectors.is_empty()
            || workload.selectors.len() > MAX_SELECTORS
            || !workload
                .selectors
                .iter()
                .all(|s| valid_selector(&s.0) && selectors.insert(&s.0))
        {
            return Err(Error::Invalid(format!(
                "guestIdentity workload {} needs 1..{MAX_SELECTORS} distinct unix:<type>:<value> or systemd:<type>:<value> selectors",
                workload.name
            )));
        }
    }
    Ok(())
}

/// The VM's UID, which must be a canonical (lowercase, hyphenated) UUID to
/// appear in the SPIFFE IDs.
fn uid(vm: &VirtualMachine) -> Result<String> {
    let uid = vm
        .uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    if uuid::Uuid::parse_str(&uid).map(|u| u.to_string()) != Ok(uid.clone()) {
        return Err(Error::Invalid(format!(
            "guestIdentity needs a canonical UUID VM UID, not {uid}"
        )));
    }
    Ok(uid)
}

fn labels(vm: &VirtualMachine, uid: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("app.kubernetes.io/name".into(), "qrab".into()),
        ("app.kubernetes.io/component".into(), COMPONENT.into()),
        ("app.kubernetes.io/instance".into(), vm.name_any()),
        (nic::OWNER_UID.into(), uid.into()),
        (
            VM_NAMESPACE_LABEL.into(),
            vm.namespace().unwrap_or_default(),
        ),
    ])
}

/// The ClusterStaticEntries `vm` should have: none without
/// `spec.guestIdentity`, else its node alias, then one per workload by name.
pub fn render(vm: &VirtualMachine, devid_ca_cn: &str) -> Result<Vec<DynamicObject>> {
    let Some(identity) = &vm.spec.guest_identity else {
        return Ok(Vec::new());
    };
    validate(identity)?;
    if !valid_devid_ca_cn(devid_ca_cn) {
        return Err(Error::Invalid(format!(
            "invalid DevID CA CN {devid_ca_cn:?}"
        )));
    }
    let uid = uid(vm)?;
    let labels = labels(vm, &uid);
    let alias = node_alias_id(&uid);
    let entry = |name: String, spec: serde_json::Value| -> Result<DynamicObject> {
        Ok(serde_json::from_value(json!({
            "apiVersion":"spire.spiffe.io/v1alpha1","kind":"ClusterStaticEntry",
            "metadata":{"name":name,"labels":labels},
            "spec":spec
        }))?)
    };
    let mut entries = vec![entry(
        node_alias_name(&uid),
        json!({"spiffeID":alias,"parentID":SERVER_ID,
            "selectors":[format!("tpm_devid:subject:cn:qrab-vm-{uid}"),format!("tpm_devid:issuer:cn:{devid_ca_cn}")],
            "className":CLASS_NAME}),
    )?];
    for workload in &identity.workloads {
        let selectors: Vec<&str> = workload.selectors.iter().map(|s| s.0.as_str()).collect();
        entries.push(entry(
            workload_entry_name(&uid, &workload.name),
            json!({"spiffeID":workload_id(&uid, &workload.name),"parentID":alias,
                "selectors":selectors,"x509SVIDTTL":WORKLOAD_X509_SVID_TTL,"className":CLASS_NAME}),
        )?);
    }
    entries[1..].sort_by_key(ResourceExt::name_any);
    Ok(entries)
}

/// Whether `entry` is one the operator wrote for the VM with `uid` in
/// `namespace`.
pub fn owned(entry: &DynamicObject, uid: &str, namespace: &str) -> bool {
    let labels = entry.labels();
    entry.name_any().starts_with(NAME_PREFIX)
        && labels
            .get("app.kubernetes.io/component")
            .map(String::as_str)
            == Some(COMPONENT)
        && labels.get(nic::OWNER_UID).map(String::as_str) == Some(uid)
        && labels.get(VM_NAMESPACE_LABEL).map(String::as_str) == Some(namespace)
}

pub fn has_finalizer<K: Resource>(resource: &K) -> bool {
    resource
        .meta()
        .finalizers
        .as_ref()
        .is_some_and(|f| f.iter().any(|f| f == FINALIZER))
}

async fn set_finalizer(vms: &Api<VirtualMachine>, vm: &VirtualMachine, add: bool) -> Result<()> {
    let mut finalizers: Vec<String> = vm
        .finalizers()
        .iter()
        .filter(|f| *f != FINALIZER)
        .cloned()
        .collect();
    if add {
        finalizers.push(FINALIZER.into());
    }
    vms.patch(&vm.name_any(), &PatchParams::default(), &Patch::Merge(json!({"metadata":{"uid":vm.uid(),"resourceVersion":vm.resource_version(),"finalizers":finalizers}}))).await?;
    Ok(())
}

fn is_status(error: &kube::Error, code: u16) -> bool {
    matches!(error, kube::Error::Api(status) if status.code == code)
}

/// The live entries of VMs in `namespace`, optionally only those of `uid`.
async fn list(
    entries: &Api<DynamicObject>,
    namespace: &str,
    uid: Option<&str>,
) -> Result<Vec<DynamicObject>> {
    let mut selector =
        format!("app.kubernetes.io/component={COMPONENT},{VM_NAMESPACE_LABEL}={namespace}");
    if let Some(uid) = uid {
        selector.push_str(&format!(",{}={uid}", nic::OWNER_UID));
    }
    Ok(entries
        .list(&ListParams::default().labels(&selector))
        .await?
        .items
        .into_iter()
        .filter(|e| {
            e.name_any().starts_with(NAME_PREFIX) && uid.is_none_or(|uid| owned(e, uid, namespace))
        })
        .collect())
}

async fn delete(entries: &Api<DynamicObject>, entry: &DynamicObject) -> Result<()> {
    let params = DeleteParams {
        preconditions: Some(Preconditions {
            uid: entry.uid(),
            ..Default::default()
        }),
        ..Default::default()
    };
    match entries.delete(&entry.name_any(), &params).await {
        Ok(_) => {}
        Err(error) if is_status(&error, 404) => {}
        Err(error) => return Err(error.into()),
    }
    tracing::info!(entry=%entry.name_any(), "deleted guest identity entry");
    Ok(())
}

/// Bring `vm`'s ClusterStaticEntries to [`render`]'s: record the finalizer
/// before the first entry, create or update the desired entries, delete the
/// VM's others (removed or renamed workloads), and drop the finalizer once
/// none is desired. Returns true when it changed the VM, which must then be
/// reconciled afresh.
pub async fn reconcile(
    vm: &VirtualMachine,
    client: &Client,
    devid_ca_cn: &str,
    vms: &Api<VirtualMachine>,
) -> Result<bool> {
    if vm.spec.guest_identity.is_none() && !has_finalizer(vm) {
        return Ok(false);
    }
    let desired = render(vm, devid_ca_cn)?;
    if !desired.is_empty() && !has_finalizer(vm) {
        set_finalizer(vms, vm, true).await?;
        return Ok(true);
    }
    let uid = vm
        .uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    let namespace = vm.namespace().unwrap_or_default();
    let entries: Api<DynamicObject> = Api::all_with(client.clone(), &entry_resource());
    let actual = match list(&entries, &namespace, Some(&uid)).await {
        Ok(actual) => actual,
        // Without the CRD no entry can exist.
        Err(Error::Kubernetes(e)) if desired.is_empty() && is_status(&e, 404) => Vec::new(),
        Err(Error::Kubernetes(e)) if is_status(&e, 404) || is_status(&e, 403) => {
            return Err(Error::Invalid(
                "guestIdentity needs the spire.spiffe.io ClusterStaticEntry CRD and the operator's \
                 contrib/kubernetes/operator/guest-identity.yaml RBAC"
                    .into(),
            ))
        }
        Err(error) => return Err(error),
    };
    for want in &desired {
        let name = want.name_any();
        match actual.iter().find(|a| a.name_any() == name) {
            None => match entries.create(&PostParams::default(), want).await {
                Ok(_) => {
                    tracing::info!(vm=%vm.name_any(), entry=%name, "created guest identity entry")
                }
                Err(error) if is_status(&error, 409) => {
                    return Err(Error::Ownership(format!(
                        "ClusterStaticEntry {name} belongs to another owner"
                    )))
                }
                Err(error) => return Err(error.into()),
            },
            Some(have)
                if crate::resources::contains(&have.data, &want.data)
                    && want
                        .labels()
                        .iter()
                        .all(|(k, v)| have.labels().get(k) == Some(v)) => {}
            Some(have) => {
                // Bind the write to the exact object whose ownership we checked.
                let mut want = want.clone();
                want.metadata.uid = have.metadata.uid.clone();
                want.metadata.resource_version = have.metadata.resource_version.clone();
                entries
                    .patch(
                        &name,
                        &PatchParams::apply("qrab-operator").force(),
                        &Patch::Apply(&want),
                    )
                    .await?;
                tracing::info!(vm=%vm.name_any(), entry=%name, "updated guest identity entry");
            }
        }
    }
    let wanted: BTreeSet<String> = desired.iter().map(ResourceExt::name_any).collect();
    for stale in actual.iter().filter(|a| !wanted.contains(&a.name_any())) {
        delete(&entries, stale).await?;
    }
    if desired.is_empty() && has_finalizer(vm) {
        set_finalizer(vms, vm, false).await?;
        return Ok(true);
    }
    Ok(false)
}

/// On VM deletion: delete its entries and drop the finalizer. Returns true
/// when it changed the VM. Without the ClusterStaticEntry CRD no entry can
/// exist, so the finalizer goes.
pub async fn finalize(
    vm: &VirtualMachine,
    client: &Client,
    vms: &Api<VirtualMachine>,
) -> Result<bool> {
    if !has_finalizer(vm) {
        return Ok(false);
    }
    let uid = vm
        .uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    let entries: Api<DynamicObject> = Api::all_with(client.clone(), &entry_resource());
    match list(&entries, &vm.namespace().unwrap_or_default(), Some(&uid)).await {
        Ok(owned) => {
            for entry in &owned {
                delete(&entries, entry).await?;
            }
        }
        Err(Error::Kubernetes(error)) if is_status(&error, 404) => {}
        Err(error) => return Err(error),
    }
    set_finalizer(vms, vm, false).await?;
    Ok(true)
}

/// Delete entries of `namespace`'s VMs that are gone (deleted past their
/// finalizer). Entries are listed before the VMs, so a VM created meanwhile
/// cannot lose its new entries.
pub async fn sweep(client: &Client, namespace: &str) -> Result<()> {
    let entries: Api<DynamicObject> = Api::all_with(client.clone(), &entry_resource());
    let all = match list(&entries, namespace, None).await {
        Ok(all) => all,
        Err(Error::Kubernetes(error)) if is_status(&error, 404) => return Ok(()),
        Err(error) => return Err(error),
    };
    if all.is_empty() {
        return Ok(());
    }
    let vms: Api<VirtualMachine> = Api::namespaced(client.clone(), namespace);
    let live: BTreeSet<String> = vms
        .list(&ListParams::default())
        .await?
        .items
        .iter()
        .filter_map(ResourceExt::uid)
        .collect();
    for entry in &all {
        let Some(uid) = entry.labels().get(nic::OWNER_UID) else {
            continue;
        };
        if !live.contains(uid) && owned(entry, uid, namespace) {
            tracing::warn!(entry=%entry.name_any(), %uid, "deleting the guest identity entry of a VM that is gone");
            delete(&entries, entry).await?;
        }
    }
    Ok(())
}
