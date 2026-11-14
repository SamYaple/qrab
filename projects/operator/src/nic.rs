//! Guest NICs. Each NIC is its own Pod running `qrab-manager nic` (passt plus
//! a one-connection relay on port 7700), so guest traffic has its own network
//! namespace, Cilium identity and BGP-advertised Pod IP, and never enters the
//! VM Pod that holds the BMC and the manager's control socket. Security groups
//! are NIC Pod labels that users' CiliumNetworkPolicies select.
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

/// The relay port QEMU connects to in every NIC Pod.
pub const RELAY_PORT: u16 = 7700;
pub const NIC_LIMIT: usize = 4;
/// The guest's IPv4 gateway and DNS server on every NIC: passt answers ARP
/// for it, routes through it and forwards its port 53 to the NIC Pod's
/// resolver (`qrab-manager nic` passes `-g` and `--dns-forward`).
pub const GUEST_GATEWAY: std::net::Ipv4Addr = std::net::Ipv4Addr::new(169, 254, 1, 1);
pub const COMPONENT: &str = "vm-nic";
pub const NIC_LABEL: &str = "qrab.yapplesauce.com/nic";
pub const OWNER_UID: &str = "qrab.yapplesauce.com/owner-uid";
/// `sg.qrab.yapplesauce.com/<group>: ""` marks a NIC Pod's security groups.
pub const SECURITY_GROUP_PREFIX: &str = "sg.qrab.yapplesauce.com/";
const SOCKET: &str = "/run/nic/passt.sock";

/// A ready NIC Pod: the guest NIC `name` reaches it at `ip`:RELAY_PORT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NicEndpoint {
    pub name: String,
    pub ip: IpAddr,
}

pub fn pod_name(vm: &VirtualMachine, nic: &str) -> String {
    format!("{}-nic-{nic}", vm.name_any())
}

pub fn policy_name(vm: &VirtualMachine, nic: &str) -> String {
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

pub struct DesiredNic {
    pub pod: Pod,
    pub hash: String,
    pub policy: DynamicObject,
}

pub fn policy_resource() -> ApiResource {
    ApiResource::from_gvk_with_plural(
        &GroupVersionKind::gvk("cilium.io", "v2", "CiliumNetworkPolicy"),
        "ciliumnetworkpolicies",
    )
}

/// The NIC Pod and its relay policy, addressed from its node's block.
pub fn render(vm: &VirtualMachine, settings: &Settings, nic: &Nic) -> Result<DesiredNic> {
    render_in_pool(vm, settings, nic, None)
}

/// The NIC Pod and its relay policy; with `pool`, the Pod takes its address
/// from that one-address CiliumPodIPPool (see [`crate::address`]).
pub fn render_in_pool(
    vm: &VirtualMachine,
    settings: &Settings,
    nic: &Nic,
    pool: Option<&str>,
) -> Result<DesiredNic> {
    let uid = uid(vm)?;
    let owner = owner_json(vm)?;
    let image = vm.spec.image.as_deref().unwrap_or(&settings.vm_image);
    let mut labels = identity_labels(vm, &nic.name)?;
    let vm_pod =
        json!({"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm",OWNER_UID:uid});
    let security = json!({"allowPrivilegeEscalation":false,"privileged":false,"readOnlyRootFilesystem":true,"capabilities":{"drop":["ALL"]}});
    let probe = json!(["qrab-manager", "nic-ready", "--socket", SOCKET]);
    let (socket_volume, socket_mount) = crate::resources::workload_socket();
    let mut pod = json!({"apiVersion":"v1","kind":"Pod","metadata":{"name":pod_name(vm,&nic.name),"namespace":settings.namespace,"labels":labels,"ownerReferences":[owner]},"spec":{
        "serviceAccountName":"qrab-manager","automountServiceAccountToken":false,"enableServiceLinks":false,
        "hostNetwork":false,"hostPID":false,"hostIPC":false,
        "restartPolicy":"Always","terminationGracePeriodSeconds":5,
        "securityContext":{"runAsNonRoot":true,"runAsUser":10001,"runAsGroup":10001,"seccompProfile":{"type":"RuntimeDefault"}},
        "tolerations":[{"key":"node-role.kubernetes.io/control-plane","operator":"Exists","effect":"NoSchedule"}],
        "affinity":{"podAffinity":{"preferredDuringSchedulingIgnoredDuringExecution":[{"weight":100,"podAffinityTerm":{"labelSelector":{"matchLabels":vm_pod},"topologyKey":"kubernetes.io/hostname"}}]}},
        // passt binds one socket per forwarded TCP and UDP port; their kernel
        // memory (about 260 MiB) is charged to this container.
        "containers":[{"name":"nic","image":image,"imagePullPolicy":"Always","command":["qrab-manager"],
            "args":["nic","--listen",format!("0.0.0.0:{RELAY_PORT}"),"--socket",SOCKET,"--guest-mac",mac(&uid,&nic.name)],
            "ports":[{"name":"relay","containerPort":RELAY_PORT,"protocol":"TCP"}],
            "resources":{"requests":{"cpu":"50m","memory":"320Mi"},"limits":{"cpu":"1","memory":"512Mi"}},
            "securityContext":security,"terminationMessagePolicy":"FallbackToLogsOnError",
            "env":[crate::resources::workload_env()],
            "volumeMounts":[{"name":"nic","mountPath":"/run/nic"},socket_mount],
            // Never a TCP or socket probe: passt and the relay serve one client.
            "readinessProbe":{"exec":{"command":probe},"periodSeconds":2,"failureThreshold":2,"timeoutSeconds":2},
            "livenessProbe":{"exec":{"command":probe},"initialDelaySeconds":10,"periodSeconds":10,"failureThreshold":3,"timeoutSeconds":2}}],
        "volumes":[{"name":"nic","emptyDir":{"medium":"Memory","sizeLimit":"1Mi"}},socket_volume]
    }});
    if let Some(node) = &vm.spec.storage_node {
        pod["spec"]["affinity"]["nodeAffinity"] = json!({"preferredDuringSchedulingIgnoredDuringExecution":[{"weight":50,"preference":{"matchExpressions":[{"key":"kubernetes.io/hostname","operator":"In","values":[node]}]}}]});
    }
    let mut annotations = serde_json::Map::new();
    if let Some(pool) = pool {
        annotations.insert(crate::address::POOL_ANNOTATION.into(), json!(pool));
        annotations.insert(crate::address::REQUIRE_POOL_MATCH.into(), json!("true"));
        pod["metadata"]["annotations"] = Value::Object(annotations.clone());
    }
    let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&pod)?));
    annotations.insert(HASH.into(), json!(hash));
    pod["metadata"]["annotations"] = Value::Object(annotations);
    labels.extend(security_group_labels(nic));
    pod["metadata"]["labels"] = json!(labels);
    let policy = serde_json::from_value(relay_policy(vm, settings, &nic.name, owner)?)?;
    Ok(DesiredNic {
        pod: serde_json::from_value(pod)?,
        hash,
        policy,
    })
}

fn owner_json(vm: &VirtualMachine) -> Result<Value> {
    let mut owner = vm
        .controller_owner_ref(&())
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    owner.block_owner_deletion = Some(false);
    Ok(serde_json::to_value(owner)?)
}

/// Port 7700 of the NIC Pod admits its VM Pod only: an allow rule, and an
/// `ingressDeny` for every other endpoint and entity, which no
/// security-group allow can override. A second rule lets the VM Pod reach it
/// even under an egress-restricting policy. Neither rule enables default
/// deny, so without security-group policies the NIC Pod stays default-allow.
fn relay_policy(
    vm: &VirtualMachine,
    settings: &Settings,
    nic: &str,
    owner: Value,
) -> Result<Value> {
    let uid = uid(vm)?;
    let labels = identity_labels(vm, nic)?;
    let nic_pod = json!({"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":COMPONENT,OWNER_UID:uid,NIC_LABEL:nic});
    let vm_pod =
        json!({"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm",OWNER_UID:uid});
    let relay = json!([{"ports":[{"port":RELAY_PORT.to_string(),"protocol":"TCP"}]}]);
    let additive = json!({"ingress":false,"egress":false});
    Ok(
        json!({"apiVersion":"cilium.io/v2","kind":"CiliumNetworkPolicy",
        "metadata":{"name":policy_name(vm,nic),"namespace":settings.namespace,"labels":labels,"ownerReferences":[owner]},
        "specs":[
            {"description":format!("Relay port of NIC {nic}: only VM {} reaches it",vm.name_any()),
             "endpointSelector":{"matchLabels":nic_pod},
             "enableDefaultDeny":additive,
             "ingress":[{"fromEndpoints":[{"matchLabels":vm_pod}],"toPorts":relay}],
             "ingressDeny":[
                {"fromEntities":["world","host","remote-node","kube-apiserver","ingress","health","init","unmanaged"],"toPorts":relay},
                // Any other Pod: another namespace, not a VM Pod, or another VM.
                {"fromEndpoints":[
                    {"matchExpressions":[{"key":"k8s:io.kubernetes.pod.namespace","operator":"NotIn","values":[settings.namespace]}]},
                    {"matchExpressions":[{"key":"app.kubernetes.io/component","operator":"NotIn","values":["vm"]}]},
                    {"matchExpressions":[{"key":OWNER_UID,"operator":"NotIn","values":[uid]}]}],
                 "toPorts":relay}]},
            {"description":format!("VM {} reaches the relay of its NIC {nic}",vm.name_any()),
             "endpointSelector":{"matchLabels":vm_pod},
             "enableDefaultDeny":additive,
             "egress":[{"toEndpoints":[{"matchLabels":nic_pod}],"toPorts":relay}]}
        ]}),
    )
}

/// A NIC Pod is current when its template hash matches; security-group
/// labels are reconciled in place.
pub fn pod_matches(actual: &Pod, desired: &DesiredNic) -> bool {
    actual.annotations().get(HASH) == Some(&desired.hash)
        && desired
            .pod
            .labels()
            .iter()
            .filter(|(k, _)| !k.starts_with(SECURITY_GROUP_PREFIX))
            .all(|(k, v)| actual.labels().get(k) == Some(v))
}

/// The relay endpoint of a Running, Ready NIC Pod.
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
