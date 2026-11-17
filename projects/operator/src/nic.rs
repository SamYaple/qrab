//! Guest NICs. Each NIC is its own restricted anchor Pod on the VM Pod's
//! node; qrab-netd builds a kernel tap with vhost-net in its network namespace
//! and hands the descriptors to the VM's manager. Guest traffic has the
//! anchor's network namespace, Cilium identity and BGP-advertised /32, and
//! never enters the VM Pod that holds the BMC and the manager's control
//! socket. Security groups are anchor Pod labels that users'
//! CiliumNetworkPolicies select.
use crate::{
    api::{Nic, VirtualMachine},
    resources::{Settings, HASH},
    Error, Result,
};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{ApiResource, DynamicObject, GroupVersionKind},
    Resource, ResourceExt,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, net::IpAddr};

pub const NIC_LIMIT: usize = 4;
/// The guest's IPv4 gateway and DNS server on every NIC: qrab-netd answers
/// ARP for it on the tap, routes through it and redirects its port 53 to the
/// anchor's resolver.
pub const GUEST_GATEWAY: std::net::Ipv4Addr = qrab_netd::GUEST_GATEWAY;
pub const COMPONENT: &str = "vm-nic";
pub const NIC_LABEL: &str = "qrab.yapplesauce.com/nic";
pub const OWNER_UID: &str = "qrab.yapplesauce.com/owner-uid";
/// `sg.qrab.yapplesauce.com/<group>: ""` marks a NIC Pod's security groups.
pub const SECURITY_GROUP_PREFIX: &str = "sg.qrab.yapplesauce.com/";

/// A guest NIC and its address: the NIC's stable pool address, which the
/// guest configures statically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NicEndpoint {
    pub name: String,
    pub ip: IpAddr,
}

pub fn pod_name(vm: &VirtualMachine, nic: &str) -> String {
    format!("{}-nic-{nic}", vm.name_any())
}

/// The anchor of `nic` on `node`. The node is part of the name, so a live
/// migration's target anchor already has the name the VM's own reconcile
/// renders once the VM Pod lives on that node.
pub fn anchor_name(vm: &VirtualMachine, nic: &str, node: &str) -> String {
    format!("{}-{node}", pod_name(vm, nic))
}

/// On a migration's target anchor: the migration that owns it while it is
/// active (the VM's reconcile leaves it alone until then).
pub const MIGRATION_ANNOTATION: &str = "qrab.yapplesauce.com/migration";
/// On an anchor no longer on the VM Pod's node: when it may be deleted (Unix
/// seconds). The delay lets the route announcement converge on the new node
/// while the old one still forwards stragglers.
pub const RETIRE_ANNOTATION: &str = "qrab.yapplesauce.com/retire-after";
/// Seconds between retiring an anchor and deleting it.
pub const RETIRE_DELAY_SECS: i64 = 20;
/// forks/cilium P1: the anchor takes `<namespace>/<pod>`'s IP as a standby
/// endpoint, without IPAM.
pub const HANDOFF_FROM: &str = "ipam.cilium.io/handoff-from";

/// A live migration's target anchor: the normal anchor for the target node,
/// plus (outside the template hash, so it matches the normal template after
/// the move) the handoff source and the owning migration.
pub fn handoff_target(
    mut anchor: DesiredAnchor,
    namespace: &str,
    source: &str,
    migration: &str,
) -> DesiredAnchor {
    let annotations = anchor
        .pod
        .metadata
        .annotations
        .get_or_insert_with(Default::default);
    annotations.insert(HANDOFF_FROM.into(), format!("{namespace}/{source}"));
    annotations.insert(MIGRATION_ANNOTATION.into(), migration.into());
    anchor
}

/// The relay CiliumNetworkPolicy the removed stream datapath created per
/// NIC; the operator deletes it from VMs created before the change.
pub fn legacy_relay_policy_name(vm: &VirtualMachine, nic: &str) -> String {
    format!("{}-relay", pod_name(vm, nic))
}

/// A stable, locally administered unicast MAC for a VM UID and NIC name:
/// `02` followed by five bytes of SHA-256.
///
/// ```
/// let mac = qrab_operator::nic::mac("6b9f0c44-0000-4000-8000-000000000001", "lan");
/// assert_eq!(mac, qrab_operator::nic::mac("6b9f0c44-0000-4000-8000-000000000001", "lan"));
/// assert!(mac.starts_with("02:") && mac.len() == 17);
/// assert_ne!(mac, qrab_operator::nic::mac("6b9f0c44-0000-4000-8000-000000000001", "wan"));
/// ```
pub fn mac(uid: &str, nic: &str) -> String {
    let digest = Sha256::digest(format!("qrab-nic\0{uid}\0{nic}"));
    std::iter::once(0x02)
        .chain(digest[..5].iter().copied())
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

fn uid(vm: &VirtualMachine) -> Result<String> {
    vm.uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))
}

/// Labels that identify a NIC Pod. Security groups are separate: they change
/// in place.
pub fn identity_labels(vm: &VirtualMachine, nic: &str) -> Result<BTreeMap<String, String>> {
    Ok(BTreeMap::from([
        ("app.kubernetes.io/name".into(), "qrab".into()),
        ("app.kubernetes.io/component".into(), COMPONENT.into()),
        ("app.kubernetes.io/instance".into(), vm.name_any()),
        (OWNER_UID.into(), uid(vm)?),
        (NIC_LABEL.into(), nic.into()),
    ]))
}

pub fn security_group_labels(nic: &Nic) -> BTreeMap<String, String> {
    nic.security_groups
        .iter()
        .map(|g| (format!("{SECURITY_GROUP_PREFIX}{}", g.0), String::new()))
        .collect()
}

/// The `sg.` labels to set and remove on a live NIC Pod, as a merge patch,
/// or `None` when they already match.
pub fn security_group_patch(actual: &Pod, nic: &Nic) -> Option<Value> {
    let want = security_group_labels(nic);
    let have: BTreeMap<_, _> = actual
        .labels()
        .iter()
        .filter(|(k, _)| k.starts_with(SECURITY_GROUP_PREFIX))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if have == want {
        return None;
    }
    let mut labels = serde_json::Map::new();
    for key in have.keys().filter(|k| !want.contains_key(*k)) {
        labels.insert(key.clone(), Value::Null);
    }
    for (key, value) in &want {
        labels.insert(key.clone(), json!(value));
    }
    Some(
        json!({"metadata":{"uid":actual.uid(),"resourceVersion":actual.resource_version(),"labels":labels}}),
    )
}

pub fn policy_resource() -> ApiResource {
    ApiResource::from_gvk_with_plural(
        &GroupVersionKind::gvk("cilium.io", "v2", "CiliumNetworkPolicy"),
        "ciliumnetworkpolicies",
    )
}

fn owner_json(vm: &VirtualMachine) -> Result<Value> {
    let mut owner = vm
        .controller_owner_ref(&())
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    owner.block_owner_deletion = Some(false);
    Ok(serde_json::to_value(owner)?)
}

/// An anchor is current when its template hash matches; security-group
/// labels are reconciled in place.
pub fn anchor_matches(actual: &Pod, desired: &DesiredAnchor) -> bool {
    template_matches(actual, &desired.pod, &desired.hash)
}

/// Whether a live NIC Pod still matches a rendered template and its hash.
pub fn template_matches(actual: &Pod, desired: &Pod, hash: &str) -> bool {
    actual.annotations().get(HASH).map(String::as_str) == Some(hash)
        && desired
            .labels()
            .iter()
            .filter(|(k, _)| !k.starts_with(SECURITY_GROUP_PREFIX))
            .all(|(k, v)| actual.labels().get(k) == Some(v))
}

/// A NIC anchor (docs/networking.md, "Tap datapath").
pub struct DesiredAnchor {
    pub pod: Pod,
    pub hash: String,
}

/// The NIC's anchor Pod: a restricted Pod that only holds the NIC's Cilium
/// endpoint, /32 (from its one-address `pool`) and security groups, pinned
/// to `node`, the VM Pod's node. qrab-netd builds the tap in its network
/// namespace and sets the readiness gate. It has no ports and no probes (its
/// IP is the guest's, so a network probe would reach the guest); its labels
/// carry no per-Pod value, so a second anchor of the same NIC shares its
/// Cilium identity.
pub fn render_anchor(
    vm: &VirtualMachine,
    settings: &Settings,
    nic: &Nic,
    pool: &str,
    node: &str,
) -> Result<DesiredAnchor> {
    let owner = owner_json(vm)?;
    let image = vm.spec.image.as_deref().unwrap_or(&settings.vm_image);
    let mut labels = identity_labels(vm, &nic.name)?;
    let security = json!({"allowPrivilegeEscalation":false,"privileged":false,"readOnlyRootFilesystem":true,"capabilities":{"drop":["ALL"]}});
    let mut annotations = serde_json::Map::from_iter([
        (crate::address::POOL_ANNOTATION.to_owned(), json!(pool)),
        (crate::address::REQUIRE_POOL_MATCH.to_owned(), json!("true")),
        (
            qrab_netd::DATAPATH_ANNOTATION.to_owned(),
            json!(qrab_netd::DATAPATH_TAP),
        ),
    ]);
    if nic.vpn.is_some() {
        annotations.insert(
            qrab_netd::vpn::VPN_ANNOTATION.to_owned(),
            json!(qrab_netd::vpn::VPN_WIREGUARD),
        );
    }
    let mut pod = json!({"apiVersion":"v1","kind":"Pod","metadata":{"name":anchor_name(vm,&nic.name,node),"namespace":settings.namespace,"labels":labels,"annotations":annotations,"ownerReferences":[owner]},"spec":{
        "serviceAccountName":"qrab-manager","automountServiceAccountToken":false,"enableServiceLinks":false,
        "hostNetwork":false,"hostPID":false,"hostIPC":false,
        "restartPolicy":"Always","terminationGracePeriodSeconds":2,
        "securityContext":{"runAsNonRoot":true,"runAsUser":10001,"runAsGroup":10001,"seccompProfile":{"type":"RuntimeDefault"}},
        "tolerations":[{"key":"node-role.kubernetes.io/control-plane","operator":"Exists","effect":"NoSchedule"}],
        "affinity":{"nodeAffinity":{"requiredDuringSchedulingIgnoredDuringExecution":{"nodeSelectorTerms":[{"matchFields":[{"key":"metadata.name","operator":"In","values":[node]}]}]}}},
        "readinessGates":[{"conditionType":qrab_netd::PLUMBED_CONDITION}],
        "containers":[{"name":qrab_netd::ANCHOR_CONTAINER,"image":image,"imagePullPolicy":"Always","command":["qrab-manager"],"args":["anchor"],
            "resources":{"requests":{"cpu":"5m","memory":"16Mi"},"limits":{"cpu":"100m","memory":"32Mi"}},
            "securityContext":security,"terminationMessagePolicy":"FallbackToLogsOnError"}]
    }});
    // The tunnel's config, readable only by root: qrab-netd reads it through
    // the anchor's mount namespace; the anchor's own process cannot.
    if let Some(vpn) = &nic.vpn {
        pod["spec"]["volumes"] = json!([{"name":"vpn","secret":{"secretName":vpn.secret_name,
            "items":[{"key":qrab_netd::vpn::VPN_FILE,"path":qrab_netd::vpn::VPN_FILE}],"defaultMode":0o400}}]);
        pod["spec"]["containers"][0]["volumeMounts"] =
            json!([{"name":"vpn","mountPath":qrab_netd::vpn::VPN_MOUNT,"readOnly":true}]);
    }
    let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&pod)?));
    annotations.insert(HASH.into(), json!(hash));
    pod["metadata"]["annotations"] = Value::Object(annotations);
    labels.extend(security_group_labels(nic));
    pod["metadata"]["labels"] = json!(labels);
    Ok(DesiredAnchor {
        pod: serde_json::from_value(pod)?,
        hash,
    })
}

/// The address of a Running, Ready NIC Pod.
pub fn endpoint(pod: &Pod, nic: &str) -> Option<NicEndpoint> {
    let status = pod.status.as_ref()?;
    let ready = status
        .conditions
        .iter()
        .flatten()
        .any(|c| c.type_ == "Ready" && c.status == "True");
    (ready && pod.metadata.deletion_timestamp.is_none()).then_some(())?;
    Some(NicEndpoint {
        name: nic.into(),
        ip: status.pod_ip.as_deref()?.parse().ok()?,
    })
}

/// Whether a live CiliumNetworkPolicy carries the desired labels and rules.
pub fn policy_matches(actual: &DynamicObject, desired: &DynamicObject) -> bool {
    crate::resources::contains(&actual.data, &desired.data)
        && desired
            .labels()
            .iter()
            .all(|(k, v)| actual.labels().get(k) == Some(v))
}
