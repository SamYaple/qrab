use crate::{
    api::{Accelerator, CloudInit, Nic, VirtualMachine},
    nic::{mac as nic_mac, NicEndpoint, GUEST_GATEWAY, NIC_LIMIT, RELAY_PORT},
    provision::Attached,
    Error, Result,
};
use k8s_openapi::api::core::v1::{ConfigMap, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{api::DynamicObject, Resource, ResourceExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const HASH: &str = "qrab.yapplesauce.com/template-hash";
/// Guest size the Pod's QEMU booted with; resizes never change it.
pub const BOOT_SIZE: &str = "qrab.yapplesauce.com/boot-size";
/// Extended resource of the qrab KVM device plugin (CDI-injected /dev/kvm).
pub const KVM_RESOURCE: &str = "qrab.yapplesauce.com/kvm";
/// `kubectl exec`/`logs` without `-c` reach the VM owner, not a sidecar.
pub const DEFAULT_CONTAINER: &str = "kubectl.kubernetes.io/default-container";
const CPU_LIMIT_OVERHEAD_MILLIS: u32 = 1000;
// Sized before the BMC moved to a sidecar; the rollout measures QEMU RSS
// before these change.
const MEMORY_REQUEST_OVERHEAD_MIB: u32 = 256;
const MEMORY_LIMIT_OVERHEAD_MIB: u32 = 512;
const SIZE_DIR: &str = "/run/qrab-size";
/// Provisioned-disk capacities `{"<id>": bytes}`. Projected into SIZE_DIR as
/// `disk_capacity`; the manager grows the running guest's disks from it.
/// The Kanidm subject that created the VM, stamped by the `qrab-vm-creator`
/// admission policy and immutable afterwards. That person is always a viewer.
pub const CREATOR: &str = "qrab.yapplesauce.com/creator-subject";
pub const DISK_CAPACITY: &str = "qrab.yapplesauce.com/disk-capacity";

/// The capacity annotation value for verified exports.
pub fn disk_capacity(provisioned: &[Attached]) -> String {
    serde_json::to_string(
        &provisioned
            .iter()
            .map(|d| (d.id.clone(), d.capacity_bytes))
            .collect::<BTreeMap<_, _>>(),
    )
    .expect("capacity map serializes")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuestSize {
    pub cpus: u32,
    #[serde(rename = "memoryMiB")]
    pub memory_mib: u32,
}

impl GuestSize {
    pub fn of(vm: &VirtualMachine) -> Self {
        Self {
            cpus: vm.spec.cpus,
            memory_mib: vm.spec.memory_mib,
        }
    }

    fn fits(self, bound: Self) -> bool {
        self.cpus <= bound.cpus && self.memory_mib <= bound.memory_mib
    }

    /// Manager container resources: guest size plus a fixed, bounded overhead
    /// for QEMU, swtpm and the manager. Sidecars have their own resources.
    /// A KVM VM also requests one KVM device, whose count a resize never changes.
    pub fn container_resources(self, accelerator: Accelerator) -> Value {
        let mut resources = json!({
            "requests":{"cpu":self.cpus.to_string(),"memory":memory_quantity(self.memory_mib+MEMORY_REQUEST_OVERHEAD_MIB)},
            "limits":{"cpu":(self.cpus+CPU_LIMIT_OVERHEAD_MILLIS/1000).to_string(),"memory":memory_quantity(self.memory_mib+MEMORY_LIMIT_OVERHEAD_MIB)}
        });
        if accelerator == Accelerator::Kvm {
            resources["requests"][KVM_RESOURCE] = json!("1");
            resources["limits"][KVM_RESOURCE] = json!("1");
        }
        resources
    }

    /// Inverse of `container_resources` for the quantity forms it renders.
    pub fn from_limits(limits: &Value) -> Option<Self> {
        let cpus = limits.get("cpu")?.as_str()?.parse::<u32>().ok()?;
        let memory = limits.get("memory")?.as_str()?;
        let memory_mib = match (memory.strip_suffix("Gi"), memory.strip_suffix("Mi")) {
            (Some(gib), _) => gib.parse::<u32>().ok()?.checked_mul(1024)?,
            (_, Some(mib)) => mib.parse().ok()?,
            _ => return None,
        };
        Some(Self {
            cpus: cpus.checked_sub(CPU_LIMIT_OVERHEAD_MILLIS / 1000)?,
            memory_mib: memory_mib.checked_sub(MEMORY_LIMIT_OVERHEAD_MIB)?,
        })
    }

    /// Hot-plug ceilings fixed when QEMU boots at `self`. Linux treats a TCG
    /// guest as bare metal and refuses hot-added vCPUs, so only KVM VMs get
    /// vCPU headroom.
    pub fn maxima(self, vm: &VirtualMachine) -> Self {
        Self {
            cpus: if vm.spec.accelerator == Accelerator::Kvm {
                vm.spec
                    .max_cpus
                    .unwrap_or((self.cpus * 4).min(64))
                    .clamp(self.cpus, 256)
            } else {
                self.cpus
            },
            memory_mib: vm
                .spec
                .max_memory_mib
                .unwrap_or((self.memory_mib * 4).min(self.memory_mib + 65_536))
                .clamp(self.memory_mib, 1_048_576),
        }
    }
}

/// Boot size recorded on a Pod this operator created.
pub fn boot_size(pod: &Pod) -> Option<GuestSize> {
    serde_json::from_str(pod.annotations().get(BOOT_SIZE)?).ok()
}
/// Shared only with sidecars; the manager's QMP/VNC runtime volume is not.
pub const CONTROL_DIR: &str = "/run/qrab-control/manager";
pub const CONTROL_SOCKET: &str = "/run/qrab-control/manager/control.sock";

#[derive(Clone)]
pub struct Settings {
    pub namespace: String,
    pub vm_image: String,
    pub oidc_issuer: String,
    pub oidc_client_id: String,
    pub idp_ip: Option<String>,
    /// Issuer and audience (OAuth2 client ID) of Redfish bearer tokens.
    pub redfish_oidc_issuer: String,
    pub redfish_oidc_audience: String,
    /// ID tokens the dashboard forwards to every BMC's JSON API.
    pub bmc_bearer: Option<BearerSettings>,
    /// Stable per-NIC addresses; without it a NIC Pod takes an address from
    /// its node's block and changes it when it moves.
    pub nic_addresses: Option<crate::address::Addressing>,
}

/// The issuer and audience (OAuth2 client ID) of accepted bearer ID tokens.
#[derive(Clone)]
pub struct BearerSettings {
    pub issuer: String,
    pub audience: String,
}

pub fn pinned_image(image: &str) -> bool {
    image
        .strip_prefix("zot.yapplesauce.com/qrab/manager@sha256:")
        .is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
}

fn dns_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

pub fn origin(raw: &str) -> Result<String> {
    let url = url::Url::parse(raw).map_err(|e| Error::Invalid(e.to_string()))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Invalid("console.publicUrl must be an HTTPS origin without credentials, path, query or fragment".into()));
    }
    Ok(url.origin().ascii_serialization())
}

/// Validate event destinations before deriving either the sidecar allowlist or
/// the matching Cilium egress rule. URL paths remain application-level exact.
pub fn event_destination(raw: &str) -> Result<url::Url> {
    let url = url::Url::parse(raw)
        .map_err(|_| Error::Invalid("invalid Redfish event destination URL".into()))?;
    if raw.len() > 2048
        || raw.contains(',')
        || raw.chars().any(char::is_whitespace)
        || url.scheme() != "https"
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(Error::Invalid("Redfish event destinations must be exact HTTPS URLs on port 443 without credentials, query or fragment".into()));
    }
    match url.host().unwrap() {
        url::Host::Domain(host) if !dns_name(host) => {
            return Err(Error::Invalid(
                "invalid Redfish event destination DNS name".into(),
            ))
        }
        url::Host::Ipv4(ip)
            if ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_multicast()
                || ip.is_broadcast()
                || ip.is_link_local() =>
        {
            return Err(Error::Invalid(
                "invalid Redfish event destination IP address".into(),
            ))
        }
        url::Host::Ipv6(ip)
            if ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_multicast()
                || ip.is_unicast_link_local() =>
        {
            return Err(Error::Invalid(
                "invalid Redfish event destination IP address".into(),
            ))
        }
        _ => {}
    }
    Ok(url)
}

pub fn redfish_policy_name(vm: &VirtualMachine) -> String {
    format!("{}-redfish-events", vm.name_any())
}

/// Additive VM-Pod egress for explicitly enabled event receivers. Cilium policy
/// is per Pod, while exact URL paths are enforced by the Redfish process.
pub fn redfish_policy(vm: &VirtualMachine, settings: &Settings) -> Result<Option<DynamicObject>> {
    validate(vm, settings)?;
    let redfish = &vm.spec.components.redfish;
    if !redfish.enabled || redfish.event_destinations.is_empty() {
        return Ok(None);
    }
    let uid = vm
        .uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    let owner = vm
        .controller_owner_ref(&())
        .ok_or_else(|| Error::Invalid("VM owner reference is unavailable".into()))?;
    let mut hosts = BTreeSet::new();
    let mut ips = BTreeSet::new();
    for destination in &redfish.event_destinations {
        match event_destination(destination)?.host().unwrap() {
            url::Host::Domain(host) => {
                hosts.insert(host.to_owned());
            }
            url::Host::Ipv4(ip) => {
                ips.insert(format!("{ip}/32"));
            }
            url::Host::Ipv6(ip) => {
                ips.insert(format!("{ip}/128"));
            }
        }
    }
    let https = json!([{"ports":[{"port":"443","protocol":"TCP"}]}]);
    let mut egress = Vec::new();
    if !hosts.is_empty() {
        // Existing VM policy permits kube-dns53. Wildcard interception keeps
        // that DNS behavior while populating Cilium's FQDN cache; exact names
        // in the separate HTTPS rule determine which destination hosts admit.
        egress.push(json!({"toEndpoints":[{"matchLabels":{"k8s:io.kubernetes.pod.namespace":"kube-system","k8s:k8s-app":"kube-dns"}}],
            "toPorts":[{"ports":[{"port":"53","protocol":"UDP"},{"port":"53","protocol":"TCP"}],"rules":{"dns":[{"matchPattern":"*"}]}}]}));
        egress.push(json!({"toFQDNs":hosts.iter().map(|host|json!({"matchName":host})).collect::<Vec<_>>(),"toPorts":https}));
    }
    if !ips.is_empty() {
        egress.push(json!({"toCIDR":ips,"toPorts":https}));
    }
    Ok(Some(serde_json::from_value(
        json!({"apiVersion":"cilium.io/v2","kind":"CiliumNetworkPolicy",
            "metadata":{"name":redfish_policy_name(vm),"namespace":settings.namespace,"ownerReferences":[owner],
                "labels":{"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"redfish-events","app.kubernetes.io/instance":vm.name_any(),crate::nic::OWNER_UID:uid}},
            "spec":{"description":"Explicit Redfish event destinations only","endpointSelector":{"matchLabels":{"app.kubernetes.io/name":"qrab","app.kubernetes.io/component":"vm",crate::nic::OWNER_UID:uid}},
                "enableDefaultDeny":{"ingress":false,"egress":false},"egress":egress}
        }),
    )?))
}

pub fn valid_iqn(value: &str) -> bool {
    if value.len() > 223
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-:".contains(&b))
    {
        return false;
    }
    let Some((date, rest)) = value.strip_prefix("iqn.").and_then(|v| v.split_once('.')) else {
        return false;
    };
    if date.len() != 7
        || date.as_bytes()[4] != b'-'
        || !date[..4].bytes().all(|b| b.is_ascii_digit())
        || !matches!(date[5..].parse::<u32>(), Ok(1..=12))
    {
        return false;
    }
    let (domain, unique) = rest
        .split_once(':')
        .map_or((rest, None), |(a, b)| (a, Some(b)));
    domain.contains('.') && dns_name(domain) && unique.is_none_or(|v| !v.is_empty())
}

/// Admin and viewer lists hold at most 128 distinct UUIDs and never overlap.
fn subjects(what: &str, admins: &[String], viewers: &[String]) -> Result<()> {
    let parsed = |list: &[String]| -> Option<BTreeSet<uuid::Uuid>> {
        (list.len() <= 128)
            .then(|| list.iter().map(|s| uuid::Uuid::parse_str(s).ok()).collect())
            .flatten()
    };
    match (parsed(admins), parsed(viewers)) {
        (Some(admins), Some(viewers)) if admins.is_disjoint(&viewers) => Ok(()),
        _ => Err(Error::Invalid(format!(
            "{what} must be at most 128 UUIDs each, with none in both the admin and viewer lists"
        ))),
    }
}

/// The creator subject recorded by admission, if any.
pub fn creator(vm: &VirtualMachine) -> Result<Option<String>> {
    vm.annotations()
        .get(CREATOR)
        .map(|value| {
            uuid::Uuid::parse_str(value)
                .map(|id| id.to_string())
                .map_err(|_| Error::Invalid(format!("annotation {CREATOR} must be a subject UUID")))
        })
        .transpose()
}

/// Validate one component's subject and group lists. Some grant must exist;
/// the creator counts as a viewer.
fn access(
    what: &str,
    people: (&[String], &[String]),
    groups: (&[String], &[String]),
    creator: Option<&str>,
) -> Result<()> {
    subjects(&format!("{what} subjects"), people.0, people.1)?;
    subjects(&format!("{what} groups"), groups.0, groups.1)?;
    if people.0.is_empty()
        && people.1.is_empty()
        && groups.0.is_empty()
        && groups.1.is_empty()
        && creator.is_none()
    {
        return Err(Error::Invalid(format!(
            "{what} needs admin or viewer subjects or groups"
        )));
    }
    Ok(())
}

/// The viewer subjects a sidecar receives: the listed viewers plus the
/// creator, unless the creator is already an admin.
fn viewers_with_creator(
    admins: &[String],
    viewers: &[String],
    creator: Option<&str>,
) -> Vec<String> {
    let mut all = viewers.to_vec();
    if let Some(creator) = creator {
        let listed = |list: &[String]| {
            list.iter()
                .any(|s| uuid::Uuid::parse_str(s).is_ok_and(|id| id.to_string() == creator))
        };
        if !listed(admins) && !listed(viewers) {
            all.push(creator.to_owned());
        }
    }
    all
}

const NO_BMC_ACCESS: &str = "console.publicUrl is required unless the operator sets QRAB_BMC_BEARER_ISSUER and QRAB_BMC_BEARER_AUDIENCE for dashboard access";

pub fn validate(vm: &VirtualMachine, settings: &Settings) -> Result<()> {
    if vm.name_any().len() > 50
        || !dns_name(&vm.name_any())
        || !dns_name(&vm.spec.state_volume_claim)
    {
        return Err(Error::Invalid(
            "invalid VM name (max 50) or stateVolumeClaim".into(),
        ));
    }
    if !(256..=1_048_576).contains(&vm.spec.memory_mib)
        || !(1..=256).contains(&vm.spec.cpus)
        || vm.spec.restart_nonce.len() > 128
    {
        return Err(Error::Invalid(
            "invalid memory, CPU count or restartNonce".into(),
        ));
    }
    if vm
        .spec
        .max_cpus
        .is_some_and(|max| !(vm.spec.cpus..=256).contains(&max))
        || vm
            .spec
            .max_memory_mib
            .is_some_and(|max| !(vm.spec.memory_mib..=1_048_576).contains(&max))
    {
        return Err(Error::Invalid(
            "maxCpus/maxMemoryMiB must not be below cpus/memoryMiB".into(),
        ));
    }
    if !pinned_image(vm.spec.image.as_deref().unwrap_or(&settings.vm_image)) {
        return Err(Error::Invalid(
            "image must pin an immutable qrab/manager digest".into(),
        ));
    }
    if vm.spec.boot_disk != "/opt/qrab/demo/boot.img" {
        return Err(Error::Invalid(
            "bootDisk must be the immutable image ESP /opt/qrab/demo/boot.img; mounted state and Secrets cannot be guest disks".into(),
        ));
    }
    let creator = creator(vm)?;
    match &vm.spec.console {
        Some(console) => {
            match &console.public_url {
                Some(url) => {
                    origin(url)?;
                }
                None if vm.spec.components.bmc.enabled && settings.bmc_bearer.is_none() => {
                    return Err(Error::Invalid(NO_BMC_ACCESS.into()))
                }
                None => {}
            }
            access(
                "console",
                (&console.allowed_subjects, &console.viewer_subjects),
                (&console.admin_groups, &console.viewer_groups),
                creator.as_deref(),
            )?;
        }
        None if vm.spec.components.bmc.enabled => {
            return Err(Error::Invalid(
                "console is required while components.bmc is enabled".into(),
            ))
        }
        None => {}
    }
    let redfish = &vm.spec.components.redfish;
    if !redfish.enabled
        && (!redfish.event_destinations.is_empty()
            || redfish.state_volume_claim.is_some()
            || redfish.telemetry_interval_seconds.is_some())
    {
        return Err(Error::Invalid(
            "Redfish event destinations, persistence or periodic telemetry require components.redfish.enabled".into(),
        ));
    }
    if redfish
        .telemetry_interval_seconds
        .is_some_and(|seconds| !(5..=3600).contains(&seconds))
    {
        return Err(Error::Invalid(
            "Redfish telemetryIntervalSeconds must be from 5 to 3600".into(),
        ));
    }
    if redfish.event_destinations.len() > 32 {
        return Err(Error::Invalid(
            "at most 32 Redfish event destinations are supported".into(),
        ));
    }
    let mut destinations = BTreeSet::new();
    for destination in &redfish.event_destinations {
        if !destinations.insert(event_destination(destination)?.to_string()) {
            return Err(Error::Invalid(
                "Redfish event destinations must be unique".into(),
            ));
        }
    }
    if redfish
        .state_volume_claim
        .as_ref()
        .is_some_and(|claim| !dns_name(claim) || claim == &vm.spec.state_volume_claim)
    {
        return Err(Error::Invalid(
            "components.redfish.stateVolumeClaim must name a separate valid PVC".into(),
        ));
    }
    let mut claims = BTreeSet::from([vm.spec.state_volume_claim.as_str()]);
    for claim in [
        redfish.state_volume_claim.as_ref(),
        vm.spec.media_volume_claim.as_ref(),
        vm.spec.firmware_catalog_volume_claim.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if !dns_name(claim) || !claims.insert(claim.as_str()) {
            return Err(Error::Invalid("identity, Redfish state, media and firmware PVC roles must use distinct valid claim names".into()));
        }
    }
    if redfish.enabled {
        access(
            "components.redfish",
            (&redfish.admin_subjects, &redfish.viewer_subjects),
            (&redfish.admin_groups, &redfish.viewer_groups),
            creator.as_deref(),
        )?;
    }
    if vm.uid().is_none() {
        return Err(Error::Invalid("VM UID is required".into()));
    }
    let mut ids = BTreeSet::new();
    let mut endpoints = BTreeSet::new();
    let mut boot_indexes = BTreeSet::new();
    if vm.spec.iscsi_disks.len() > 8 {
        return Err(Error::Invalid(
            "at most eight iSCSI disks are supported".into(),
        ));
    }
    if vm
        .spec
        .storage_node
        .as_ref()
        .is_some_and(|n| n.len() > 63 || !dns_name(n))
    {
        return Err(Error::Invalid(
            "storageNode must be a valid node hostname label".into(),
        ));
    }
    if !vm.spec.iscsi_disks.is_empty()
        && !vm.spec.storage_node.as_ref().is_some_and(|n| dns_name(n))
    {
        return Err(Error::Invalid("native iSCSI disks require an explicit storageNode; automatic cross-node failover is disabled".into()));
    }
    for disk in &vm.spec.iscsi_disks {
        if disk.id.is_empty()
            || disk.id.len() > 20
            || !disk
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || disk.id.starts_with('-')
            || disk.id.ends_with('-')
            || !ids.insert(&disk.id)
        {
            return Err(Error::Invalid(
                "disk ids must be unique lowercase identifiers of 1..20 characters".into(),
            ));
        }
        let portal = disk
            .portal
            .parse::<std::net::SocketAddr>()
            .map_err(|_| Error::Invalid("disk portal must be literal IP:port".into()))?;
        if portal.port() == 0
            || portal.ip().is_unspecified()
            || portal.ip().is_multicast()
            || matches!(portal.ip(),std::net::IpAddr::V4(ip) if ip.is_broadcast())
        {
            return Err(Error::Invalid("invalid storage endpoint".into()));
        }
        if !valid_iqn(&disk.target) || disk.lun > 16383 {
            return Err(Error::Invalid("invalid iSCSI target or LUN".into()));
        }
        if !endpoints.insert((portal, disk.target.as_str(), disk.lun)) {
            return Err(Error::Invalid(
                "the same LUN cannot be attached twice".into(),
            ));
        }
        if disk.username.is_empty()
            || disk.username.len() > 223
            || !disk.username.bytes().all(|b| b.is_ascii_graphic())
            || !dns_name(&disk.password_secret.name)
            || disk.password_secret.key.is_empty()
            || disk.password_secret.key.len() > 253
            || !disk
                .password_secret
                .key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err(Error::Invalid(
                "invalid CHAP account or password Secret reference".into(),
            ));
        }
        if disk
            .boot_index
            .is_some_and(|i| i == 1 || i > 65535 || !boot_indexes.insert(i))
        {
            return Err(Error::Invalid(
                "disk bootIndex must be unique; 1 is reserved for the demo ESP".into(),
            ));
        }
    }
    validate_nics(&vm.spec.nics)?;
    if vm.spec.iscsi_disks.len() + vm.spec.disks.len() > 8 {
        return Err(Error::Invalid(
            "at most eight iSCSI disks are supported".into(),
        ));
    }
    if !vm.spec.disks.is_empty() && !vm.spec.storage_node.as_ref().is_some_and(|n| dns_name(n)) {
        return Err(Error::Invalid("provisioned disks require an explicit storageNode; automatic cross-node failover is disabled".into()));
    }
    for disk in &vm.spec.disks {
        if !valid_name(&disk.id, 20) || !ids.insert(&disk.id) {
            return Err(Error::Invalid(
                "disk ids must be unique lowercase identifiers of 1..20 characters across iscsiDisks and disks".into(),
            ));
        }
        if !(16 << 20..=1 << 40).contains(&disk.size_bytes)
            || !disk.size_bytes.is_multiple_of(1 << 20)
        {
            return Err(Error::Invalid(
                "disk sizeBytes must be 16MiB..1TiB in whole MiB".into(),
            ));
        }
        if disk
            .base_image
            .as_ref()
            .is_some_and(|b| !valid_name(&b.name, 32) || !(1..=999_999).contains(&b.generation))
        {
            return Err(Error::Invalid(
                "baseImage needs a 1..32 character lowercase name and generation 1..999999".into(),
            ));
        }
        if disk
            .boot_index
            .is_some_and(|i| i == 1 || i > 65535 || !boot_indexes.insert(i))
        {
            return Err(Error::Invalid(
                "disk bootIndex must be unique; 1 is reserved for the demo ESP".into(),
            ));
        }
    }
    if let Some(cloud_init) = &vm.spec.cloud_init {
        validate_cloud_init(cloud_init)?;
    }
    Ok(())
}

fn validate_nics(nics: &[Nic]) -> Result<()> {
    if nics.len() > NIC_LIMIT {
        return Err(Error::Invalid(format!(
            "at most {NIC_LIMIT} NICs are supported"
        )));
    }
    let mut names = BTreeSet::new();
    for nic in nics {
        if !valid_name(&nic.name, 15) || !names.insert(&nic.name) {
            return Err(Error::Invalid(
                "NIC names must be unique lowercase DNS labels of 1..15 characters".into(),
            ));
        }
        let mut groups = BTreeSet::new();
        if nic.security_groups.len() > 8
            || !nic
                .security_groups
                .iter()
                .all(|g| valid_name(&g.0, 63) && groups.insert(&g.0))
        {
            return Err(Error::Invalid(format!(
                "NIC {} may name at most 8 distinct security groups, each a DNS label",
                nic.name
            )));
        }
    }
    Ok(())
}

/// Bytes of `spec.cloudInit.userData`; the manager bounds the whole seed at 64 KiB.
pub const USER_DATA_LIMIT: usize = 32 * 1024;

/// Errors never quote user-data: it may hold secrets and errors reach status.
fn validate_cloud_init(cloud_init: &CloudInit) -> Result<()> {
    if cloud_init
        .hostname
        .as_ref()
        .is_some_and(|h| !valid_name(h, 63))
    {
        return Err(Error::Invalid(
            "cloudInit.hostname must be a DNS label of 1..63 lowercase letters, digits or hyphens"
                .into(),
        ));
    }
    if cloud_init
        .user_data
        .as_ref()
        .is_some_and(|u| u.len() > USER_DATA_LIMIT)
    {
        return Err(Error::Invalid(format!(
            "cloudInit.userData must be at most {USER_DATA_LIMIT} bytes"
        )));
    }
    Ok(())
}

/// A YAML double-quoted scalar. JSON string syntax is valid YAML, so a value
/// such as `no` or `0123` stays a string.
fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serializes")
}

/// NoCloud meta-data: the immutable VM UID as instance-id, so cloud-init runs
/// its per-instance modules once per VM, and the guest hostname. It is a JSON
/// object: cloud-init reads it as YAML, of which JSON is a subset, and
/// CirrOS's own NoCloud reader accepts only JSON. Values stay strings, even
/// a hostname such as `no` or `0123`.
///
/// ```
/// use qrab_operator::resources::seed_meta_data;
/// assert_eq!(
///     seed_meta_data("7c1d", "no"),
///     "{\"instance-id\":\"7c1d\",\"local-hostname\":\"no\"}\n"
/// );
/// ```
pub fn seed_meta_data(instance_id: &str, hostname: &str) -> String {
    format!(
        "{}\n",
        json!({"instance-id": instance_id, "local-hostname": hostname})
    )
}

/// The manager's `seed` config for a VM with `spec.cloudInit` or NICs. NICs
/// add static network-config; a VM with NICs gets a seed even without
/// `cloudInit`.
fn seed(vm: &VirtualMachine, uid: &str, nics: &[NicEndpoint]) -> Option<Value> {
    if vm.spec.cloud_init.is_none() && nics.is_empty() {
        return None;
    }
    let cloud_init = vm.spec.cloud_init.clone().unwrap_or_default();
    let hostname = cloud_init.hostname.clone().unwrap_or_else(|| vm.name_any());
    let mut seed = json!({"meta_data": seed_meta_data(uid, &hostname)});
    if let Some(user_data) = &cloud_init.user_data {
        seed["user_data"] = json!(user_data);
    }
    if !nics.is_empty() {
        seed["network_config"] = json!(seed_network_config(&nic_addresses(uid, nics)));
    }
    Some(seed)
}

/// Each NIC's static address: the NIC Pod IP as a host address, reached
/// through passt's fixed gateway, which also forwards DNS. passt's DHCP
/// offers the same address for guests that ignore the seed.
pub fn nic_addresses(uid: &str, nics: &[NicEndpoint]) -> Vec<NicAddress> {
    nics.iter()
        .map(|nic| NicAddress {
            mac: nic_mac(uid, &nic.name),
            address_cidr: match nic.ip {
                std::net::IpAddr::V4(ip) => format!("{ip}/32"),
                std::net::IpAddr::V6(ip) => format!("{ip}/128"),
            },
            gateway: nic.ip.is_ipv4().then(|| GUEST_GATEWAY.to_string()),
            nameservers: if nic.ip.is_ipv4() {
                vec![GUEST_GATEWAY.to_string()]
            } else {
                Vec::new()
            },
        })
        .collect()
}

/// One guest NIC's static address for NoCloud network-config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NicAddress {
    /// Guest MAC, matched to name the interface.
    pub mac: String,
    /// Address with prefix length, e.g. `10.0.0.5/32`.
    pub address_cidr: String,
    pub gateway: Option<String>,
    pub nameservers: Vec<String>,
}

/// cloud-init network-config v2 for static addresses: NIC `i` is matched by
/// MAC and named `eth{i}`. A host address (/32 or /128) reaches its gateway
/// `on-link`, since the gateway lies outside the interface's own prefix.
///
/// ```
/// use qrab_operator::resources::{seed_network_config, NicAddress};
/// let yaml = seed_network_config(&[NicAddress {
///     mac: "52:54:00:12:34:56".into(),
///     address_cidr: "10.0.0.5/32".into(),
///     gateway: Some("10.0.0.1".into()),
///     nameservers: vec!["10.0.0.53".into()],
/// }]);
/// assert!(yaml.starts_with("version: 2\nethernets:\n  eth0:\n"));
/// assert!(yaml.contains("        on-link: true\n"));
/// ```
pub fn seed_network_config(nics: &[NicAddress]) -> String {
    if nics.is_empty() {
        return "version: 2\nethernets: {}\n".into();
    }
    let mut yaml = String::from("version: 2\nethernets:\n");
    for (index, nic) in nics.iter().enumerate() {
        let name = format!("eth{index}");
        yaml += &format!(
            "  {name}:\n    match:\n      macaddress: {}\n    set-name: {}\n    addresses:\n      - {}\n",
            yaml_string(&nic.mac.to_ascii_lowercase()),
            yaml_string(&name),
            yaml_string(&nic.address_cidr)
        );
        if let Some(gateway) = &nic.gateway {
            let v6 = gateway.contains(':');
            let host = nic
                .address_cidr
                .rsplit_once('/')
                .is_some_and(|(_, prefix)| prefix == if v6 { "128" } else { "32" });
            yaml += &format!(
                "    routes:\n      - to: {}\n        via: {}\n",
                yaml_string(if v6 { "::/0" } else { "0.0.0.0/0" }),
                yaml_string(gateway)
            );
            if host {
                yaml += "        on-link: true\n";
            }
        }
        if !nic.nameservers.is_empty() {
            yaml += "    nameservers:\n      addresses:\n";
            for server in &nic.nameservers {
                yaml += &format!("        - {}\n", yaml_string(server));
            }
        }
    }
    yaml
}

fn valid_name(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

/// Operator-generated CHAP secrets are 32 lowercase hex characters.
pub fn valid_chap(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub struct Desired {
    pub config: ConfigMap,
    pub pod: Pod,
    pub hash: String,
    pub boot: GuestSize,
    pub maximum: GuestSize,
}

fn memory_quantity(mib: u32) -> String {
    if mib.is_multiple_of(1024) {
        format!("{}Gi", mib / 1024)
    } else {
        format!("{mib}Mi")
    }
}

fn subject_list(subjects: &[String]) -> String {
    subjects.join(",")
}

/// Native sidecars (restartable init containers) start before the manager,
/// keep serving while it stops the VM, and do not hold the Pod open after it
/// exits. They mount the control socket; only Redfish may additionally mount
/// its separately configured subscription-state PVC.
fn sidecars(
    vm: &VirtualMachine,
    settings: &Settings,
    image: &str,
    security: &Value,
) -> Result<Vec<Value>> {
    let sidecar = |name: &str, command: &str, port: (&str, u16), mut env: Value| {
        env.as_array_mut()
            .unwrap()
            .push(json!({"name":"QRAB_RESOURCE_SCOPE","value":"container"}));
        json!({"name":name,"image":image,"imagePullPolicy":"Always","restartPolicy":"Always",
            "command":[command],"args":["--socket",CONTROL_SOCKET],
            "ports":[{"name":port.0,"containerPort":port.1}],"env":env,
            "resources":{"requests":{"cpu":"20m","memory":"32Mi"},"limits":{"cpu":"500m","memory":"128Mi"}},
            "securityContext":security,"terminationMessagePolicy":"FallbackToLogsOnError",
            "volumeMounts":[{"name":"control","mountPath":"/run/qrab-control"}],
            // No startup probe: it would delay the manager, which starts after sidecars.
            "readinessProbe":{"httpGet":{"path":"/readyz","port":port.0},"periodSeconds":2,"failureThreshold":3,"timeoutSeconds":2},
            "livenessProbe":{"httpGet":{"path":"/healthz","port":port.0},"periodSeconds":10,"failureThreshold":3,"timeoutSeconds":2}})
    };
    let creator = creator(vm)?;
    let mut containers = Vec::new();
    if vm.spec.components.bmc.enabled {
        let console = vm.spec.console.as_ref().ok_or_else(|| {
            Error::Invalid("console is required while components.bmc is enabled".into())
        })?;
        let mut env = Vec::new();
        if let Some(url) = &console.public_url {
            env.extend([
                json!({"name":"BMC_PUBLIC_URL","value":origin(url)?}),
                json!({"name":"BMC_OIDC_ISSUER","value":settings.oidc_issuer}),
                json!({"name":"BMC_OIDC_CLIENT_ID","value":settings.oidc_client_id}),
            ]);
        }
        match &settings.bmc_bearer {
            Some(bearer) => env.extend([
                json!({"name":"BMC_BEARER_ISSUER","value":bearer.issuer}),
                json!({"name":"BMC_BEARER_AUDIENCE","value":bearer.audience}),
            ]),
            None if env.is_empty() => return Err(Error::Invalid(NO_BMC_ACCESS.into())),
            None => {}
        }
        let viewers = viewers_with_creator(
            &console.allowed_subjects,
            &console.viewer_subjects,
            creator.as_deref(),
        );
        for (name, list) in [
            ("BMC_ALLOWED_SUBJECTS", &console.allowed_subjects),
            ("BMC_VIEWER_SUBJECTS", &viewers),
            ("BMC_ADMIN_GROUPS", &console.admin_groups),
            ("BMC_VIEWER_GROUPS", &console.viewer_groups),
        ] {
            if !list.is_empty() {
                env.push(json!({"name":name,"value":subject_list(list)}));
            }
        }
        containers.push(sidecar(
            "bmc",
            "qrab-bmc",
            ("http", 8080),
            Value::Array(env),
        ));
    }
    let redfish = &vm.spec.components.redfish;
    if redfish.enabled {
        let mut env = vec![
            json!({"name":"REDFISH_OIDC_ISSUER","value":settings.redfish_oidc_issuer}),
            json!({"name":"REDFISH_OIDC_AUDIENCE","value":settings.redfish_oidc_audience}),
        ];
        let viewers = viewers_with_creator(
            &redfish.admin_subjects,
            &redfish.viewer_subjects,
            creator.as_deref(),
        );
        for (name, list) in [
            ("REDFISH_ADMIN_SUBJECTS", &redfish.admin_subjects),
            ("REDFISH_VIEWER_SUBJECTS", &viewers),
            ("REDFISH_ADMIN_GROUPS", &redfish.admin_groups),
            ("REDFISH_VIEWER_GROUPS", &redfish.viewer_groups),
        ] {
            if !list.is_empty() {
                env.push(json!({"name":name,"value":subject_list(list)}));
            }
        }
        if let Some(seconds) = redfish.telemetry_interval_seconds {
            env.push(json!({"name":"REDFISH_TELEMETRY_INTERVAL_SECS","value":seconds.to_string()}));
        }
        if !redfish.event_destinations.is_empty() {
            let destinations = redfish
                .event_destinations
                .iter()
                .map(|d| event_destination(d).map(|url| url.to_string()))
                .collect::<Result<Vec<_>>>()?;
            env.push(json!({"name":"REDFISH_EVENT_DESTINATIONS","value":destinations.join(",")}));
        }
        if redfish.state_volume_claim.is_some() {
            env.push(json!({"name":"REDFISH_SUBSCRIPTIONS_FILE","value":format!("/var/lib/qrab-redfish/subscriptions-{}.json",vm.uid().unwrap_or_default())}));
        }
        let mut container = sidecar(
            "redfish",
            "qrab-redfish",
            ("redfish", 8081),
            Value::Array(env),
        );
        if redfish.state_volume_claim.is_some() {
            container["volumeMounts"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name":"redfish-state","mountPath":"/var/lib/qrab-redfish"}));
        }
        containers.push(container);
    }
    Ok(containers)
}

/// Render a fresh instance that boots at the spec size.
pub fn render(vm: &VirtualMachine, settings: &Settings) -> Result<Desired> {
    render_with(vm, settings, &[])
}

/// Render with the verified exports of every `spec.disks` entry, in order.
pub fn render_with(
    vm: &VirtualMachine,
    settings: &Settings,
    provisioned: &[Attached],
) -> Result<Desired> {
    render_booted(vm, settings, provisioned, GuestSize::of(vm))
}

/// Render the instance that booted at `boot`. Its config and template hash
/// stay fixed while the container resources follow the spec size.
pub fn render_booted(
    vm: &VirtualMachine,
    settings: &Settings,
    provisioned: &[Attached],
    boot: GuestSize,
) -> Result<Desired> {
    render_attached(vm, settings, provisioned, boot, &[])
}

/// The manager's live-migration listener port (qrab-manager `migration.port`).
pub const MIGRATION_PORT: u16 = 7800;
/// Names the exact VM Pod incarnation, so a migration's source and target can
/// be told apart by policy.
pub const POD_LABEL: &str = "qrab.yapplesauce.com/pod";
/// Where the csi.spiffe.io driver serves the Workload API in qrab Pods.
pub const WORKLOAD_SOCKET_DIR: &str = "/run/spire";

/// The Workload API volume and its read-only mount for a qrab container.
pub fn workload_socket() -> (Value, Value) {
    (
        json!({"name":"workload-socket","csi":{"driver":"csi.spiffe.io","readOnly":true}}),
        json!({"name":"workload-socket","mountPath":WORKLOAD_SOCKET_DIR,"readOnly":true}),
    )
}

/// The environment pointing qrab-spiffe at [`workload_socket`].
pub fn workload_env() -> Value {
    json!({"name":"QRAB_SPIFFE_ENDPOINT_SOCKET","value":format!("unix:{WORKLOAD_SOCKET_DIR}/agent.sock")})
}

/// Which Pod, ConfigMap, state PVC and node a VM process uses. Normally the
/// Pod is named after the VM; a live migration's target has its own name,
/// state PVC and node, and VM status keeps them once it takes over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incarnation {
    pub pod: String,
    pub state_claim: String,
    pub node: Option<String>,
}

impl Incarnation {
    pub fn of(vm: &VirtualMachine) -> Self {
        let status = vm.status.as_ref();
        Self {
            pod: status
                .and_then(|s| s.pod_name.clone())
                .unwrap_or_else(|| vm.name_any()),
            state_claim: status
                .and_then(|s| s.state_volume_claim.clone())
                .unwrap_or_else(|| vm.spec.state_volume_claim.clone()),
            node: status
                .and_then(|s| s.placement_node.clone())
                .or_else(|| vm.spec.storage_node.clone()),
        }
    }

    pub fn config_name(&self) -> String {
        format!("{}-config", self.pod)
    }
}

/// As `render_booted`, with the relay endpoint of every `spec.nics` entry,
/// in order. A NIC Pod's IP is part of the VM's config: a replaced NIC Pod
/// with a new IP replaces the VM Pod.
pub fn render_attached(
    vm: &VirtualMachine,
    settings: &Settings,
    provisioned: &[Attached],
    boot: GuestSize,
    nics: &[NicEndpoint],
) -> Result<Desired> {
    render_incarnation(vm, settings, provisioned, boot, nics, &Incarnation::of(vm))
}

/// As `render_attached`, for an explicit incarnation.
pub fn render_incarnation(
    vm: &VirtualMachine,
    settings: &Settings,
    provisioned: &[Attached],
    boot: GuestSize,
    nics: &[NicEndpoint],
    incarnation: &Incarnation,
) -> Result<Desired> {
    validate(vm, settings)?;
    if vm.spec.nics.len() != nics.len()
        || vm
            .spec
            .nics
            .iter()
            .zip(nics)
            .any(|(spec, nic)| spec.name != nic.name)
    {
        return Err(Error::Invalid("the NIC Pods are not ready yet".into()));
    }
    if vm.spec.disks.len() != provisioned.len()
        || vm
            .spec
            .disks
            .iter()
            .zip(provisioned)
            .any(|(d, a)| d.id != a.id)
    {
        return Err(Error::Invalid(
            "provisioned disks are not exported yet".into(),
        ));
    }
    let maximum = boot.maxima(vm);
    let name = vm.name_any();
    let config_name = incarnation.config_name();
    let uid = vm.uid().expect("validated UID");
    let mut owner = vm.controller_owner_ref(&()).expect("VM has a UID");
    // Background GC needs no delete/finalizer authority on the parent CR.
    owner.block_owner_deletion = Some(false);
    let labels = json!({"app.kubernetes.io/name":"qrab", "app.kubernetes.io/component":"vm", "app.kubernetes.io/instance":name, "qrab.yapplesauce.com/owner-uid":uid, POD_LABEL:incarnation.pod});
    let mut config = json!({"restart_on_exit":true,"name":name,"memory_mib":boot.memory_mib,"cpus":boot.cpus,"accelerator":vm.spec.accelerator.as_str(),"console":true,"uefi":{"boot_disk":vm.spec.boot_disk},"tpm":{},"state_dir":"/var/lib/qrab","state_id":uid,"runtime_dir":"/run/qrab","control_socket":true,"control_dir":CONTROL_DIR,"startup_timeout_secs":30,"shutdown_grace_secs":5,
        "migration":{"port":MIGRATION_PORT},

        "hotplug":{"max_cpus":maximum.cpus,"max_memory_mib":maximum.memory_mib,"limits_dir":SIZE_DIR,"cpu_overhead_millis":CPU_LIMIT_OVERHEAD_MILLIS,"memory_overhead_mib":MEMORY_LIMIT_OVERHEAD_MIB}});
    if vm.spec.secure_boot_capable {
        config["uefi"]["secure_boot_capable"] = json!(true);
        config["uefi"]["code"] = json!("/opt/qrab/firmware/OVMF_CODE_4M.secboot.fd");
    }
    let initiator = format!("iqn.2026-11.com.yapplesauce:qrab.vm.{uid}");
    let mut disks: Vec<_> = vm.spec.iscsi_disks.iter().map(|d|json!({"id":d.id,"portal":d.portal,"target":d.target,"lun":d.lun,"initiator_name":initiator,"username":d.username,"password_file":format!("/run/qrab-auth/{}/password",d.id),"read_only":d.read_only,"boot_index":d.boot_index,"bus":"virtio"})).collect();
    disks.extend(provisioned.iter().map(|d|json!({"id":d.id,"portal":d.portal,"target":d.target,"lun":0,"initiator_name":initiator,"username":d.username,"password_file":format!("/run/qrab-auth/{}/password",d.id),"read_only":d.read_only,"boot_index":d.boot_index,"bus":"virtio"})));
    config["iscsi_disks"] = json!(disks);
    if !nics.is_empty() {
        config["nics"] = json!(nics
            .iter()
            .map(|n| json!({"id":n.name,"mac":nic_mac(&uid, &n.name),"host":n.ip.to_string(),"port":RELAY_PORT}))
            .collect::<Vec<_>>());
    }
    // Absent without spec.cloudInit or NICs, so existing template hashes are unchanged.
    if let Some(seed) = seed(vm, &uid, nics) {
        config["seed"] = seed;
    }
    let config_data = serde_json::to_string_pretty(&config)?;
    let config_map: ConfigMap = serde_json::from_value(
        json!({"apiVersion":"v1","kind":"ConfigMap","metadata":{"name":config_name,"namespace":settings.namespace,"labels":labels,"ownerReferences":[owner]},"data":{"vm.json":config_data}}),
    )?;
    let image = vm.spec.image.as_deref().unwrap_or(&settings.vm_image);
    let security = json!({"allowPrivilegeEscalation":false,"privileged":false,"readOnlyRootFilesystem":true,"capabilities":{"drop":["ALL"]}});
    let probe = json!(["qrab-manager", "live", "--config", "/etc/qrab/vm.json"]);
    let sidecars = sidecars(vm, settings, image, &security)?;
    let (socket_volume, socket_mount) = workload_socket();
    let mut pod = json!({"apiVersion":"v1","kind":"Pod","metadata":{"name":incarnation.pod,"namespace":settings.namespace,"labels":labels,"ownerReferences":[owner]},"spec":{
        "serviceAccountName":"qrab-manager","automountServiceAccountToken":false,"enableServiceLinks":false,
        "hostNetwork":false,"hostPID":false,"hostIPC":false,
        "restartPolicy":"Never","terminationGracePeriodSeconds":20,
        "securityContext":{"runAsNonRoot":true,"runAsUser":10001,"runAsGroup":10001,"fsGroup":10001,"fsGroupChangePolicy":"OnRootMismatch","seccompProfile":{"type":"RuntimeDefault"}},
        "tolerations":[{"key":"node-role.kubernetes.io/control-plane","operator":"Exists","effect":"NoSchedule"}],
        "containers":[{"name":"manager","image":image,"imagePullPolicy":"Always","command":["qrab-manager"],"args":["run","--config","/etc/qrab/vm.json"],
            "env":[{"name":"QRAB_RESOURCE_SCOPE","value":"container"},{"name":"QRAB_NODE_NAME","valueFrom":{"fieldRef":{"fieldPath":"spec.nodeName"}}},{"name":"QRAB_POD_NAME","valueFrom":{"fieldRef":{"fieldPath":"metadata.name"}}},{"name":"QRAB_POD_NAMESPACE","valueFrom":{"fieldRef":{"fieldPath":"metadata.namespace"}}},{"name":"QRAB_IMAGE","value":image},workload_env()],

            "resources":GuestSize::of(vm).container_resources(vm.spec.accelerator),
            "resizePolicy":[{"resourceName":"cpu","restartPolicy":"NotRequired"},{"resourceName":"memory","restartPolicy":"NotRequired"}],
            "securityContext":security,"terminationMessagePolicy":"FallbackToLogsOnError",
            "volumeMounts":[{"name":"config","mountPath":"/etc/qrab","readOnly":true},{"name":"state","mountPath":"/var/lib/qrab"},{"name":"runtime","mountPath":"/run/qrab"},{"name":"tmp","mountPath":"/tmp"},{"name":"control","mountPath":"/run/qrab-control"},socket_mount],
            // Live semantics: a paused VM stays reachable so it can be resumed.
            "startupProbe":{"exec":{"command":probe},"periodSeconds":1,"failureThreshold":90,"timeoutSeconds":2},
            "readinessProbe":{"exec":{"command":probe},"periodSeconds":2,"failureThreshold":3,"timeoutSeconds":2},
            "livenessProbe":{"exec":{"command":probe},"periodSeconds":5,"failureThreshold":3,"timeoutSeconds":2}}],
        "volumes":[{"name":"config","configMap":{"name":config_name}},{"name":"state","persistentVolumeClaim":{"claimName":incarnation.state_claim}},{"name":"runtime","emptyDir":{"medium":"Memory","sizeLimit":"64Mi"}},{"name":"tmp","emptyDir":{"medium":"Memory","sizeLimit":"64Mi"}},{"name":"control","emptyDir":{"medium":"Memory","sizeLimit":"1Mi"}},socket_volume]
    }});
    if !sidecars.is_empty() {
        pod["spec"]["initContainers"] = Value::Array(sidecars);
    }
    if let Some(claim) = vm
        .spec
        .components
        .redfish
        .state_volume_claim
        .as_ref()
        .filter(|_| vm.spec.components.redfish.enabled)
    {
        pod["spec"]["volumes"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"redfish-state","persistentVolumeClaim":{"claimName":claim}}));
    }
    for (claim, volume, mount_path, variable) in [
        (
            vm.spec.media_volume_claim.as_ref(),
            "media-catalog",
            "/var/lib/qrab-media",
            "QRAB_MEDIA_ROOT",
        ),
        (
            vm.spec.firmware_catalog_volume_claim.as_ref(),
            "firmware-catalog",
            "/var/lib/qrab-firmware",
            "QRAB_FIRMWARE_ROOT",
        ),
    ] {
        if let Some(claim) = claim {
            pod["spec"]["volumes"].as_array_mut().unwrap().push(
                json!({"name":volume,"persistentVolumeClaim":{"claimName":claim,"readOnly":true}}),
            );
            pod["spec"]["containers"][0]["volumeMounts"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name":volume,"mountPath":mount_path,"readOnly":true}));
            pod["spec"]["containers"][0]["env"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name":variable,"value":mount_path}));
        }
    }
    if let Some(node) = &incarnation.node {
        pod["spec"]["nodeSelector"] = json!({"kubernetes.io/hostname":node});
    }
    if vm.spec.accelerator == Accelerator::Kvm {
        // Pinned so a mutated live Pod reads as drift; see check_kvm_user_namespace.
        pod["spec"]["hostUsers"] = json!(true);
    }
    check_kvm_user_namespace(&pod["spec"])?;
    if !vm.spec.iscsi_disks.is_empty()
        || !vm.spec.disks.is_empty()
        || vm
            .metadata
            .finalizers
            .iter()
            .flatten()
            .any(|f| f == "qrab.yapplesauce.com/iscsi-state")
    {
        pod["metadata"]["finalizers"] = json!(["qrab.yapplesauce.com/iscsi-drain"]);
        pod["spec"]["schedulingGates"] = json!([{"name":"qrab.yapplesauce.com/iscsi-owner"}]);
    }
    for disk in &vm.spec.iscsi_disks {
        let volume = format!("iscsi-auth-{}", disk.id);
        pod["spec"]["volumes"].as_array_mut().expect("volumes").push(json!({"name":volume,"secret":{"secretName":disk.password_secret.name,"defaultMode":288,"items":[{"key":disk.password_secret.key,"path":"password","mode":288}]}}));
        pod["spec"]["containers"][0]["volumeMounts"].as_array_mut().expect("mounts").push(json!({"name":volume,"mountPath":format!("/run/qrab-auth/{}",disk.id),"readOnly":true}));
    }
    for disk in provisioned {
        let volume = format!("iscsi-auth-{}", disk.id);
        pod["spec"]["volumes"].as_array_mut().expect("volumes").push(json!({"name":volume,"secret":{"secretName":disk.secret_name,"defaultMode":288,"items":[{"key":crate::provision::SECRET_KEY,"path":"password","mode":288}]}}));
        pod["spec"]["containers"][0]["volumeMounts"].as_array_mut().expect("mounts").push(json!({"name":volume,"mountPath":format!("/run/qrab-auth/{}",disk.id),"readOnly":true}));
    }
    // The kubelet refreshes these limit files on resize, before the cgroup;
    // the manager waits for the cgroup itself before growing the guest.
    pod["spec"]["volumes"].as_array_mut().expect("volumes").push(json!({"name":"size","downwardAPI":{"defaultMode":292,"items":[
        {"path":"cpu_limit_millis","resourceFieldRef":{"containerName":"manager","resource":"limits.cpu","divisor":"1m"}},
        {"path":"memory_limit_mib","resourceFieldRef":{"containerName":"manager","resource":"limits.memory","divisor":"1Mi"}}]}}));
    // Online disk growth: the operator grows the LUN, then this annotation;
    // the kubelet refreshes the file and the manager calls block_resize.
    if !provisioned.is_empty() {
        pod["spec"]["volumes"].as_array_mut().expect("volumes").last_mut().expect("size volume")["downwardAPI"]["items"]
            .as_array_mut()
            .expect("items")
            .push(json!({"path":"disk_capacity","fieldRef":{"fieldPath":format!("metadata.annotations['{DISK_CAPACITY}']")}}));
    }
    pod["spec"]["containers"][0]["volumeMounts"]
        .as_array_mut()
        .expect("mounts")
        .push(json!({"name":"size","mountPath":SIZE_DIR,"readOnly":true}));
    if let Some(ip) = &settings.idp_ip {
        let mut hosts = BTreeSet::new();
        let bmc = vm.spec.components.bmc.enabled;
        let browser = bmc
            && vm
                .spec
                .console
                .as_ref()
                .is_some_and(|c| c.public_url.is_some());
        let bearer = settings.bmc_bearer.as_ref().filter(|_| bmc);
        for (enabled, issuer) in [
            (browser, &settings.oidc_issuer),
            (
                bearer.is_some(),
                bearer.map_or(&settings.oidc_issuer, |b| &b.issuer),
            ),
            (
                vm.spec.components.redfish.enabled,
                &settings.redfish_oidc_issuer,
            ),
        ] {
            if enabled {
                let issuer = url::Url::parse(issuer).map_err(|e| Error::Invalid(e.to_string()))?;
                hosts.insert(
                    issuer
                        .host_str()
                        .ok_or_else(|| Error::Invalid("OIDC issuer has no host".into()))?
                        .to_owned(),
                );
            }
        }
        if !hosts.is_empty() {
            pod["spec"]["hostAliases"] = json!([{"ip":ip,"hostnames":hosts}]);
        }
    }
    pod["metadata"]["annotations"] = json!({DEFAULT_CONTAINER: "manager"});
    // In-place resizes change only container resources, never the template.
    let mut template = pod.clone();
    template["spec"]["containers"][0]
        .as_object_mut()
        .expect("container")
        .remove("resources");
    let mut identity =
        json!({"pod":template,"config":config_data,"restartNonce":vm.spec.restart_nonce});
    // A rotated credential needs a fresh QEMU: libiscsi does not reload
    // secrets. Capacity grows online through DISK_CAPACITY instead. Absent for
    // legacy VMs, so their template hash is unchanged.
    if !provisioned.is_empty() {
        identity["provisioned"] = json!(provisioned
            .iter()
            .map(|d| json!({"id":d.id,"generation":d.generation}))
            .collect::<Vec<_>>());
    }
    let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&identity)?));
    let annotations = pod["metadata"]["annotations"]
        .as_object_mut()
        .expect("annotations");
    annotations.insert(HASH.into(), json!(hash));
    annotations.insert(BOOT_SIZE.into(), json!(serde_json::to_string(&boot)?));
    if !provisioned.is_empty() {
        pod["metadata"]["annotations"][DISK_CAPACITY] = json!(disk_capacity(provisioned));
    }
    Ok(Desired {
        config: config_map,
        pod: serde_json::from_value(pod)?,
        hash,
        boot,
        maximum,
    })
}

/// Refuses a Pod spec that requests the KVM device inside a user namespace.
/// The runtime creates the CDI `/dev/kvm` node with mode 0666 only in the host
/// user namespace; with `hostUsers: false`, runc bind-mounts the host's 0660
/// `root:kvm` node instead and QEMU (UID 10001) cannot open it.
///
/// ```
/// use qrab_operator::resources::{check_kvm_user_namespace, KVM_RESOURCE};
/// let mut spec = serde_json::json!({"containers":[{"resources":{"limits":{KVM_RESOURCE:"1"}}}]});
/// assert!(check_kvm_user_namespace(&spec).is_ok());
/// spec["hostUsers"] = true.into();
/// assert!(check_kvm_user_namespace(&spec).is_ok());
/// spec["hostUsers"] = false.into();
/// assert!(check_kvm_user_namespace(&spec).is_err());
/// ```
pub fn check_kvm_user_namespace(spec: &Value) -> Result<()> {
    let wants_kvm = spec["containers"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|c| c["resources"]["limits"].get(KVM_RESOURCE).is_some());
    if wants_kvm && spec.get("hostUsers") == Some(&Value::Bool(false)) {
        return Err(Error::Invalid(
            "a KVM VM Pod cannot use a user namespace (hostUsers: false): /dev/kvm would not be openable".into(),
        ));
    }
    Ok(())
}

pub fn owned<K: Resource<DynamicType = ()>>(resource: &K, vm: &VirtualMachine) -> bool {
    owned_meta(resource.meta(), vm)
}

/// `owned` for objects of any type, such as CiliumNetworkPolicies.
pub fn owned_meta(meta: &ObjectMeta, vm: &VirtualMachine) -> bool {
    meta.owner_references.as_ref().is_some_and(|owners| {
        owners.iter().any(|o| {
            o.controller == Some(true)
                && Some(o.uid.clone()) == vm.uid()
                && o.kind == "VirtualMachine"
                && o.name == vm.name_any()
        })
    })
}

/// Compare only declared fields: the API server adds defaults to stored Pods.
/// Arrays retain their declared membership, except injected default tolerations.
pub(crate) fn contains(actual: &Value, desired: &Value) -> bool {
    match (actual, desired) {
        (Value::Object(a), Value::Object(d)) => d.iter().all(|(key, value)| {
            if key == "tolerations" {
                return value.as_array().is_some_and(|want| {
                    a.get(key).and_then(Value::as_array).is_some_and(|have| {
                        want.iter().all(|v| have.iter().any(|h| contains(h, v)))
                    })
                });
            }
            a.get(key).is_some_and(|v| contains(v, value))
        }),
        (Value::Array(a), Value::Array(d)) => {
            a.len() == d.len() && a.iter().zip(d).all(|(a, d)| contains(a, d))
        }
        _ => actual == desired,
    }
}

pub fn pod_matches(actual: &Pod, desired: &Desired) -> bool {
    let Some(spec) = &actual.spec else {
        return false;
    };
    let desired_init = desired
        .pod
        .spec
        .as_ref()
        .and_then(|s| s.init_containers.as_ref())
        .map_or(0, Vec::len);
    // Declared sidecars are compared field by field below; nothing else may run.
    if spec.init_containers.as_ref().map_or(0, Vec::len) != desired_init
        || spec
            .ephemeral_containers
            .as_ref()
            .is_some_and(|v| !v.is_empty())
        || spec
            .image_pull_secrets
            .as_ref()
            .is_some_and(|v| !v.is_empty())
    {
        return false;
    }
    for container in spec
        .containers
        .iter()
        .chain(spec.init_containers.iter().flatten())
    {
        if container.env_from.as_ref().is_some_and(|v| !v.is_empty())
            || container
                .volume_devices
                .as_ref()
                .is_some_and(|v| !v.is_empty())
        {
            return false;
        }
        for mount in container.volume_mounts.iter().flatten() {
            if mount.sub_path.as_ref().is_some_and(|v| !v.is_empty())
                || mount.sub_path_expr.as_ref().is_some_and(|v| !v.is_empty())
                || (mount.name != "config"
                    && mount.name != "size"
                    && mount.name != "media-catalog"
                    && mount.name != "firmware-catalog"
                    && mount.name != "workload-socket"
                    && !mount.name.starts_with("iscsi-auth-")
                    && mount.read_only.unwrap_or(false))
                || mount
                    .mount_propagation
                    .as_ref()
                    .is_some_and(|v| v != "None")
            {
                return false;
            }
        }
    }
    if spec.volumes.iter().flatten().any(|v| {
        v.name != "media-catalog"
            && v.name != "firmware-catalog"
            && v.persistent_volume_claim
                .as_ref()
                .is_some_and(|v| v.read_only.unwrap_or(false))
    }) {
        return false;
    }
    let mut actual_spec = serde_json::to_value(&actual.spec).expect("Pod serializes");
    let mut desired_spec = serde_json::to_value(&desired.pod.spec).expect("Pod serializes");
    for spec in [&mut actual_spec, &mut desired_spec] {
        // Container resources are reconciled in place by `plan_resize`.
        for container in spec
            .get_mut("containers")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if let Some(container) = container.as_object_mut() {
                container.remove("resources");
            }
        }
        if let Some(gates) = spec
            .get_mut("schedulingGates")
            .and_then(Value::as_array_mut)
        {
            gates.retain(|gate| {
                gate.get("name").and_then(Value::as_str) != Some("qrab.yapplesauce.com/iscsi-owner")
            });
            if gates.is_empty() {
                spec.as_object_mut()
                    .expect("spec object")
                    .remove("schedulingGates");
            }
        }
    }
    // These Go PodSpec bool fields use omitempty; absent means false. Do not
    // apply this normalization to pointer booleans such as token mounting.
    for field in ["hostNetwork", "hostPID", "hostIPC"] {
        if actual_spec.get(field).is_none_or(Value::is_null) {
            actual_spec[field] = Value::Bool(false);
        }
    }
    actual.annotations().get(HASH) == Some(&desired.hash)
        && desired
            .pod
            .labels()
            .iter()
            .all(|(k, v)| actual.labels().get(k) == Some(v))
        && contains(&actual_spec, &desired_spec)
}

pub fn data(config: &ConfigMap) -> BTreeMap<String, String> {
    config.data.clone().unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResizePlan {
    /// The Pod already requests the spec size.
    Steady,
    /// Strategic-merge patch for the Pod's `resize` subresource.
    Grow(Value),
    /// The change cannot be applied to the running guest.
    Restart(&'static str),
}

fn manager_container(pod: &Value) -> Option<&Value> {
    pod["spec"]["containers"]
        .as_array()?
        .iter()
        .find(|c| c["name"] == "manager")
}

/// Decide how a converged Pod reaches the spec size. Growth within the boot
/// maxima is applied in place; a running guest is never shrunk in place.
pub fn plan_resize(vm: &VirtualMachine, pod: &Pod, desired: &Desired) -> ResizePlan {
    let pod_value = serde_json::to_value(pod).expect("Pod serializes");
    let want = GuestSize::of(vm);
    let Some(have) = manager_container(&pod_value)
        .and_then(|c| GuestSize::from_limits(&c["resources"]["limits"]))
    else {
        return ResizePlan::Restart("the VM Pod has unrecognized container resources");
    };
    if want == have {
        return ResizePlan::Steady;
    }
    if pod_value["status"]["phase"] != "Running" {
        return ResizePlan::Restart("the VM Pod has not started; recreating it at the new size");
    }
    if !have.fits(want) {
        return ResizePlan::Restart("a running guest cannot shrink in place");
    }
    if !want.fits(desired.maximum) {
        return ResizePlan::Restart("the new size exceeds the boot-time hot-plug maximum");
    }
    ResizePlan::Grow(json!({
        "metadata":{"uid":pod.uid()},
        "spec":{"containers":[{"name":"manager","resources":want.container_resources(vm.spec.accelerator)}]}
    }))
}

fn condition<'a>(pod: &'a Value, kind: &str) -> Option<&'a Value> {
    pod["status"]["conditions"]
        .as_array()?
        .iter()
        .find(|c| c["type"] == kind && c["status"] == "True")
}

/// Kubelet-side resize state. `rejected` is an API-server refusal of the
/// resize patch, which Kubernetes 1.37 returns synchronously for requests
/// beyond the node's allocatable capacity.
pub fn resize_status(
    pod: &Pod,
    desired: &Desired,
    rejected: Option<String>,
) -> crate::api::ResizeStatus {
    let pod_value = serde_json::to_value(pod).expect("Pod serializes");
    let requested = manager_container(&pod_value)
        .and_then(|c| GuestSize::from_limits(&c["resources"]["limits"]));
    let applied = pod_value["status"]["containerStatuses"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["name"] == "manager")
        .and_then(|c| GuestSize::from_limits(&c["resources"]["limits"]));
    let text = |c: &Value, key: &str| c[key].as_str().unwrap_or_default().to_owned();
    let observed = pod_value["status"]["observedGeneration"]
        .as_i64()
        .unwrap_or(0)
        >= pod.metadata.generation.unwrap_or(0);
    let (phase, message) = if let Some(message) = rejected {
        ("Infeasible".to_owned(), message)
    } else if let Some(c) = condition(&pod_value, "PodResizePending") {
        (text(c, "reason"), text(c, "message"))
    } else if let Some(c) = condition(&pod_value, "PodResizeInProgress") {
        let phase = if c["reason"] == "Error" {
            "Error"
        } else {
            "InProgress"
        };
        (phase.to_owned(), text(c, "message"))
    } else if observed && requested.is_some() && applied == requested {
        ("Applied".to_owned(), String::new())
    } else {
        (
            "Proposed".to_owned(),
            "waiting for the kubelet to act on the resize".to_owned(),
        )
    };
    let requested = requested.unwrap_or(desired.boot);
    crate::api::ResizeStatus {
        phase,
        message,
        cpus: requested.cpus,
        memory_mib: requested.memory_mib,
        applied_cpus: applied.map(|s| s.cpus),
        applied_memory_mib: applied.map(|s| s.memory_mib),
        boot_cpus: desired.boot.cpus,
        boot_memory_mib: desired.boot.memory_mib,
        max_cpus: desired.maximum.cpus,
        max_memory_mib: desired.maximum.memory_mib,
    }
}
