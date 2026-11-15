//! Stable guest NIC addresses (`QRAB_NIC_ADDRESS_RANGE`; required for guest
//! NICs).
//!
//! Each NIC claims one address from the range as a cluster-scoped Cilium
//! multi-pool `CiliumPodIPPool` holding just that /32. The pool is named after
//! the address, so creating it is an atomic claim. The NIC's anchor Pod is pinned to
//! the pool, so it keeps the address on whichever node it runs. One
//! `FRRConfiguration` per NIC, selecting the node that holds the address,
//! announces the /32 to that node's BGP neighbors; frr-k8s merges it with the
//! node's base configuration (labelled [`ROUTE_BASE_LABEL`]), whose ASN and
//! neighbors it copies.
use crate::{api::VirtualMachine, nic, Error, Result};
use k8s_openapi::api::core::v1::{Node, Pod};
use kube::{
    api::{
        ApiResource, DeleteParams, DynamicObject, GroupVersionKind, ListParams, Patch, PatchParams,
        PostParams, Preconditions,
    },
    Api, Client, Resource, ResourceExt,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    net::Ipv4Addr,
    str::FromStr,
};

/// On the VM while it may own address pools, and on each pool while in use.
pub const FINALIZER: &str = "qrab.yapplesauce.com/nic-address";
pub const COMPONENT: &str = "vm-nic-address";
/// Marks the FRRConfiguration whose ASN and neighbors per-NIC routes copy.
pub const ROUTE_BASE_LABEL: &str = "qrab.yapplesauce.com/nic-route-base";
pub const POOL_ANNOTATION: &str = "ipam.cilium.io/ip-pool";
pub const REQUIRE_POOL_MATCH: &str = "ipam.cilium.io/require-pool-match";
/// Cilium multi-pool IPAM gives endpoints from a pool with this annotation set
/// to "true" the no-SNAT flag, so a guest's traffic keeps its own address.
pub const SKIP_MASQUERADE: &str = "ipam.cilium.io/skip-masquerade";
const NAME_PREFIX: &str = "qrab-nic-";
const VM_ANNOTATION: &str = "qrab.yapplesauce.com/vm";

/// An aligned IPv4 range NIC addresses come from.
///
/// ```
/// use qrab_operator::address::AddressRange;
/// let range: AddressRange = "10.244.128.0/17".parse().unwrap();
/// assert!(range.contains("10.244.200.9".parse().unwrap()));
/// assert!(!range.contains("10.244.4.9".parse().unwrap()));
/// assert!("10.244.128.1/17".parse::<AddressRange>().is_err());
/// assert_eq!(range.to_string(), "10.244.128.0/17");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressRange {
    network: Ipv4Addr,
    prefix: u8,
}

impl FromStr for AddressRange {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, String> {
        let (addr, prefix) = s
            .split_once('/')
            .ok_or_else(|| format!("{s}: expected <address>/<prefix length>"))?;
        let network: Ipv4Addr = addr.parse().map_err(|e| format!("{s}: {e}"))?;
        let prefix: u8 = prefix.parse().map_err(|e| format!("{s}: {e}"))?;
        if !(8..=30).contains(&prefix) {
            return Err(format!("{s}: the prefix length must be 8 to 30"));
        }
        if u32::from(network) & !mask(prefix) != 0 {
            return Err(format!("{s}: not the network address of the range"));
        }
        Ok(Self { network, prefix })
    }
}

impl fmt::Display for AddressRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix)
    }
}

fn mask(prefix: u8) -> u32 {
    u32::MAX << (32 - u32::from(prefix))
}

impl AddressRange {
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        u32::from(ip) & mask(self.prefix) == u32::from(self.network)
    }

    /// Addresses not in `taken`, lowest first. The range's first and last
    /// addresses are never handed out.
    ///
    /// ```
    /// use qrab_operator::address::AddressRange;
    /// let range: AddressRange = "10.9.0.0/30".parse().unwrap();
    /// let taken = ["10.9.0.1".parse().unwrap()].into();
    /// let free: Vec<_> = range.free(&taken).map(|a| a.to_string()).collect();
    /// assert_eq!(free, ["10.9.0.2"]);
    /// ```
    pub fn free<'a>(&self, taken: &'a BTreeSet<Ipv4Addr>) -> impl Iterator<Item = Ipv4Addr> + 'a {
        let first = u32::from(self.network);
        let last = first | !mask(self.prefix);
        (first + 1..last)
            .map(Ipv4Addr::from)
            .filter(move |a| !taken.contains(a))
    }
}

/// What the operator needs to hand out stable NIC addresses.
#[derive(Clone, Debug)]
pub struct Addressing {
    pub range: AddressRange,
    /// Where frr-k8s reads FRRConfigurations.
    pub frr_namespace: String,
}

pub const DEFAULT_FRR_NAMESPACE: &str = "frr-k8s-system";

/// The pool (and route) name of an address.
///
/// ```
/// use qrab_operator::address::{pool_name, pool_address};
/// let ip = "10.244.128.7".parse().unwrap();
/// assert_eq!(pool_name(ip), "qrab-nic-10-244-128-7");
/// assert_eq!(pool_address("qrab-nic-10-244-128-7"), Some(ip));
/// assert_eq!(pool_address("default"), None);
/// ```
pub fn pool_name(ip: Ipv4Addr) -> String {
    format!("{NAME_PREFIX}{}", ip.to_string().replace('.', "-"))
}

pub fn pool_address(name: &str) -> Option<Ipv4Addr> {
    let ip: Ipv4Addr = name
        .strip_prefix(NAME_PREFIX)?
        .replace('-', ".")
        .parse()
        .ok()?;
    (pool_name(ip) == name).then_some(ip)
}

fn resource(group: &str, version: &str, kind: &str, plural: &str) -> ApiResource {
    ApiResource::from_gvk_with_plural(&GroupVersionKind::gvk(group, version, kind), plural)
}

pub fn pool_resource() -> ApiResource {
    resource(
        "cilium.io",
        "v2alpha1",
        "CiliumPodIPPool",
        "ciliumpodippools",
    )
}

pub fn route_resource() -> ApiResource {
    resource(
        "frrk8s.metallb.io",
        "v1beta1",
        "FRRConfiguration",
        "frrconfigurations",
    )
}

pub fn cilium_node_resource() -> ApiResource {
    resource("cilium.io", "v2", "CiliumNode", "ciliumnodes")
}

fn labels(uid: &str, nic: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("app.kubernetes.io/name".into(), "qrab".into()),
        ("app.kubernetes.io/component".into(), COMPONENT.into()),
        (nic::OWNER_UID.into(), uid.into()),
        (nic::NIC_LABEL.into(), nic.into()),
    ])
}

/// The one-address pool that claims `ip` for NIC `nic` of `vm`.
pub fn render_pool(vm: &VirtualMachine, nic: &str, ip: Ipv4Addr) -> Result<DynamicObject> {
    let uid = vm
        .uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    let vm_ref = format!("{}/{}", vm.namespace().unwrap_or_default(), vm.name_any());
    Ok(serde_json::from_value(json!({
        "apiVersion":"cilium.io/v2alpha1","kind":"CiliumPodIPPool",
        "metadata":{"name":pool_name(ip),"labels":labels(&uid,nic),
            "annotations":{VM_ANNOTATION:vm_ref,SKIP_MASQUERADE:"true"},"finalizers":[FINALIZER]},
        "spec":{"ipv4":{"cidrs":[format!("{ip}/32")],"maskSize":32}}
    }))?)
}

fn node_selector(base: &DynamicObject) -> Option<&serde_json::Map<String, Value>> {
    let selector = base.data["spec"]["nodeSelector"].as_object()?;
    if selector.keys().any(|k| k != "matchLabels") {
        return None;
    }
    selector["matchLabels"]
        .as_object()
        .filter(|m| !m.is_empty())
}

/// Whether a base FRRConfiguration's `matchLabels` select `node`
/// (`matchExpressions` are not supported).
pub fn selects(base: &DynamicObject, node: &Node) -> bool {
    node_selector(base).is_some_and(|labels| {
        labels.iter().all(|(k, v)| {
            v.as_str()
                .is_some_and(|v| node.labels().get(k).map(String::as_str) == Some(v))
        })
    })
}

/// The FRRConfiguration announcing `pool`'s address from the nodes `base`
/// selects: the base's single router ASN (and VRF), its neighbors with every
/// receive filter dropped and the /32 as the only prefix they are sent.
pub fn render_route(
    base: &DynamicObject,
    pool: &DynamicObject,
    namespace: &str,
) -> Result<DynamicObject> {
    let base_name = base.name_any();
    let ip = pool_address(&pool.name_any())
        .ok_or_else(|| Error::Invalid(format!("{} is not a NIC address pool", pool.name_any())))?;
    let selector = node_selector(base).ok_or_else(|| {
        Error::Invalid(format!(
            "base FRRConfiguration {base_name} must select its node by matchLabels only"
        ))
    })?;
    let router = match base.data["spec"]["bgp"]["routers"].as_array() {
        Some(routers) if routers.len() == 1 => &routers[0],
        _ => {
            return Err(Error::Invalid(format!(
                "base FRRConfiguration {base_name} must declare exactly one router"
            )))
        }
    };
    let prefix = format!("{ip}/32");
    let neighbors: Vec<Value> = router["neighbors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| {
            let mut n = n.as_object()?.clone();
            n.remove("toReceive");
            n.insert(
                "toAdvertise".into(),
                json!({"allowed":{"mode":"filtered","prefixes":[prefix]}}),
            );
            Some(Value::Object(n))
        })
        .collect();
    let mut out = json!({"asn":router["asn"],"prefixes":[prefix],"neighbors":neighbors});
    if let Some(vrf) = router.get("vrf") {
        out["vrf"] = vrf.clone();
    }
    Ok(serde_json::from_value(json!({
        "apiVersion":"frrk8s.metallb.io/v1beta1","kind":"FRRConfiguration",
        "metadata":{"name":pool.name_any(),"namespace":namespace,"labels":pool.labels()},
        "spec":{"nodeSelector":{"matchLabels":selector},"bgp":{"routers":[out]}}
    }))?)
}

fn pool_entries<'a>(cilium_node: &'a DynamicObject, key: &str) -> &'a [Value] {
    cilium_node.data["spec"]["ipam"]["pools"][key]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn holds(cilium_node: &DynamicObject, pool: &str) -> bool {
    ["requested", "allocated"].iter().any(|k| {
        pool_entries(cilium_node, k)
            .iter()
            .any(|e| e["pool"] == pool)
    })
}

fn node_ready(node: &Node) -> bool {
    node.status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .is_some_and(|c| c.iter().any(|c| c.type_ == "Ready" && c.status == "True"))
}

/// CiliumNodes that still hold `pool` although their Node is gone or not
/// Ready and no Pod of this NIC is bound to them, so the address cannot
/// follow the NIC elsewhere. A Pod object still bound to the dead node keeps
/// the claim: only its (force) deletion declares that process gone.
pub fn stranded<'a>(
    pool: &str,
    cilium_nodes: &'a [DynamicObject],
    nodes: &[Node],
    bound: &BTreeSet<String>,
) -> Vec<&'a DynamicObject> {
    cilium_nodes
        .iter()
        .filter(|cn| holds(cn, pool) && !bound.contains(&cn.name_any()))
        .filter(|cn| {
            nodes
                .iter()
                .find(|n| n.name_any() == cn.name_any())
                .is_none_or(|n| !node_ready(n))
        })
        .collect()
}

/// A merge patch removing `pool` from a CiliumNode's requested and allocated
/// pools, valid only against the resourceVersion it was computed from.
pub fn fence_patch(cilium_node: &DynamicObject, pool: &str) -> Value {
    let mut pools = serde_json::Map::new();
    for key in ["requested", "allocated"] {
        if cilium_node.data["spec"]["ipam"]["pools"].get(key).is_some() {
            let kept: Vec<_> = pool_entries(cilium_node, key)
                .iter()
                .filter(|e| e["pool"] != pool)
                .cloned()
                .collect();
            pools.insert(key.into(), Value::Array(kept));
        }
    }
    json!({"metadata":{"resourceVersion":cilium_node.resource_version()},"spec":{"ipam":{"pools":pools}}})
}

/// A merge patch adding `pool`, which holds `ip`, to a CiliumNode's requested
/// and allocated pools, or `None` when the node lists it in both. A live
/// migration's target anchor took the address through a handoff (forks/cilium
/// P1), so no IPAM on its node holds it; this moves the pool there while the
/// source node still lists it (the fork lets a one-address pool sit on two
/// nodes during a handoff), and the target agent adopts its handoff address
/// into the pool. Valid only against the resourceVersion it was computed from.
pub fn adopt_patch(cilium_node: &DynamicObject, pool: &str, ip: Ipv4Addr) -> Option<Value> {
    let entries = [
        ("requested", json!({"pool":pool,"needed":{"ipv4-addrs":1}})),
        (
            "allocated",
            json!({"pool":pool,"cidrs":[format!("{ip}/32")]}),
        ),
    ];
    let mut pools = serde_json::Map::new();
    for (key, entry) in entries {
        let current = pool_entries(cilium_node, key);
        if !current.iter().any(|e| e["pool"] == pool) {
            let mut grown = current.to_vec();
            grown.push(entry);
            pools.insert(key.into(), Value::Array(grown));
        }
    }
    (!pools.is_empty()).then(|| {
        json!({"metadata":{"resourceVersion":cilium_node.resource_version()},"spec":{"ipam":{"pools":pools}}})
    })
}

/// The live address pools and routes of NICs.
pub struct Addresses {
    pools: Api<DynamicObject>,
    routes: Api<DynamicObject>,
    client: Client,
    all: Vec<DynamicObject>,
}

fn by_uid(object: &DynamicObject) -> DeleteParams {
    DeleteParams {
        preconditions: Some(Preconditions {
            uid: object.uid(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

impl Addresses {
    pub async fn load(client: &Client, frr_namespace: &str) -> Result<Self> {
        let pools = Api::all_with(client.clone(), &pool_resource());
        let all = pools
            .list(
                &ListParams::default().labels(&format!("app.kubernetes.io/component={COMPONENT}")),
            )
            .await?
            .items;
        Ok(Self {
            routes: Api::namespaced_with(client.clone(), frr_namespace, &route_resource()),
            pools,
            client: client.clone(),
            all,
        })
    }

    /// Pools of the VM with UID `uid`.
    pub fn owned_by<'a>(&'a self, uid: &'a str) -> impl Iterator<Item = &'a DynamicObject> + 'a {
        self.all
            .iter()
            .filter(move |p| p.labels().get(nic::OWNER_UID).map(String::as_str) == Some(uid))
    }

    pub fn of<'a>(&'a self, uid: &'a str, nic: &str) -> Option<&'a DynamicObject> {
        self.owned_by(uid)
            .find(|p| p.labels().get(nic::NIC_LABEL).map(String::as_str) == Some(nic))
    }

    /// Claim the lowest free address for NIC `nic`.
    pub async fn claim(
        &mut self,
        range: &AddressRange,
        vm: &VirtualMachine,
        nic: &str,
    ) -> Result<DynamicObject> {
        let taken: BTreeSet<Ipv4Addr> = self
            .all
            .iter()
            .filter_map(|p| pool_address(&p.name_any()))
            .collect();
        for ip in range.free(&taken).take(64) {
            match self
                .pools
                .create(&PostParams::default(), &render_pool(vm, nic, ip)?)
                .await
            {
                Ok(pool) => {
                    tracing::info!(vm=%vm.name_any(), nic, %ip, "claimed NIC address");
                    self.all.push(pool.clone());
                    return Ok(pool);
                }
                // Claimed meanwhile, possibly by a pool without our labels.
                Err(kube::Error::Api(status)) if status.code == 409 => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(Error::Invalid(format!(
            "no free NIC address in {range} after 64 attempts"
        )))
    }

    /// Announce the address from the node of the Pod that holds it, or
    /// withdraw it while none does.
    pub async fn route(&self, pool: &DynamicObject, pod: Option<&Pod>) -> Result<()> {
        let name = pool.name_any();
        let ip = pool_address(&name).map(|ip| ip.to_string());
        let node = pod
            .filter(|p| p.status.as_ref().and_then(|s| s.pod_ip.as_ref()) == ip.as_ref())
            .and_then(|p| p.spec.as_ref()?.node_name.clone());
        let actual = self.routes.get_opt(&name).await?;
        let Some(node) = node else {
            if let Some(actual) = actual {
                self.routes.delete(&name, &by_uid(&actual)).await?;
            }
            return Ok(());
        };
        let node: Node = Api::<Node>::all(self.client.clone()).get(&node).await?;
        let bases = self
            .routes
            .list(&ListParams::default().labels(&format!("{ROUTE_BASE_LABEL}=true")))
            .await?;
        let base = bases
            .items
            .iter()
            .find(|b| selects(b, &node))
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "no FRRConfiguration labelled {ROUTE_BASE_LABEL}=true selects node {}",
                    node.name_any()
                ))
            })?;
        let desired = render_route(base, pool, self.routes.namespace().unwrap_or_default())?;
        if actual.is_some_and(|a| {
            crate::resources::contains(&a.data, &desired.data)
                && desired
                    .labels()
                    .iter()
                    .all(|(k, v)| a.labels().get(k) == Some(v))
        }) {
            return Ok(());
        }
        self.routes
            .patch(
                &name,
                &PatchParams::apply("qrab-operator").force(),
                &Patch::Apply(&desired),
            )
            .await?;
        Ok(())
    }

    /// Free `pool` from CiliumNodes of dead nodes that run no Pod of it.
    pub async fn fence(&self, pool: &DynamicObject, bound: &BTreeSet<String>) -> Result<()> {
        let cilium_nodes: Api<DynamicObject> =
            Api::all_with(self.client.clone(), &cilium_node_resource());
        let held = cilium_nodes.list(&ListParams::default()).await?.items;
        if !held.iter().any(|cn| holds(cn, &pool.name_any())) {
            return Ok(());
        }
        let nodes = Api::<Node>::all(self.client.clone())
            .list(&ListParams::default())
            .await?
            .items;
        for cn in stranded(&pool.name_any(), &held, &nodes, bound) {
            tracing::warn!(pool=%pool.name_any(), node=%cn.name_any(), "fencing a NIC address off a node that is gone or not Ready");
            cilium_nodes
                .patch(
                    &cn.name_any(),
                    &PatchParams::default(),
                    &Patch::Merge(fence_patch(cn, &pool.name_any())),
                )
                .await?;
        }
        Ok(())
    }

    /// Move `pool` to `node`'s CiliumNode (see [`adopt_patch`]).
    pub async fn adopt(&self, pool: &DynamicObject, node: &str, ip: Ipv4Addr) -> Result<()> {
        let cilium_nodes: Api<DynamicObject> =
            Api::all_with(self.client.clone(), &cilium_node_resource());
        let cn = cilium_nodes.get(node).await?;
        if let Some(patch) = adopt_patch(&cn, &pool.name_any(), ip) {
            tracing::info!(pool=%pool.name_any(), %node, "moving a handed-off NIC address's pool to its new node");
            cilium_nodes
                .patch(node, &PatchParams::default(), &Patch::Merge(patch))
                .await?;
        }
        Ok(())
    }

    /// Withdraw the route, then let the pool go. Only once no Pod holds it.
    pub async fn release(&self, pool: &DynamicObject) -> Result<()> {
        let name = pool.name_any();
        if let Some(route) = self.routes.get_opt(&name).await? {
            self.routes.delete(&name, &by_uid(&route)).await?;
        }
        if pool.finalizers().iter().any(|f| f == FINALIZER) {
            let kept: Vec<_> = pool
                .finalizers()
                .iter()
                .filter(|f| *f != FINALIZER)
                .collect();
            self.pools.patch(&name, &PatchParams::default(), &Patch::Merge(json!({"metadata":{"uid":pool.uid(),"resourceVersion":pool.resource_version(),"finalizers":kept}}))).await?;
        }
        if pool.metadata.deletion_timestamp.is_none() {
            match self.pools.delete(&name, &by_uid(pool)).await {
                Ok(_) => {}
                Err(kube::Error::Api(status)) if status.code == 404 => {}
                Err(error) => return Err(error.into()),
            }
        }
        tracing::info!(pool=%name, "released NIC address");
        Ok(())
    }

    pub fn all(&self) -> &[DynamicObject] {
        &self.all
    }
}

pub fn has_finalizer<K: Resource>(resource: &K) -> bool {
    resource
        .meta()
        .finalizers
        .as_ref()
        .is_some_and(|f| f.iter().any(|f| f == FINALIZER))
}

pub async fn vm_finalizer(vms: &Api<VirtualMachine>, vm: &VirtualMachine, add: bool) -> Result<()> {
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
