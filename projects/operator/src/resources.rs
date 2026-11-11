use crate::{api::VirtualMachine, provision::Attached, Error, Result};
use k8s_openapi::api::core::v1::{ConfigMap, Pod};
use kube::{Resource, ResourceExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const HASH: &str = "qrab.yapplesauce.com/template-hash";
/// Guest size the Pod's QEMU booted with; resizes never change it.
pub const BOOT_SIZE: &str = "qrab.yapplesauce.com/boot-size";
/// Rendered accelerator. Linux treats TCG guests as bare metal and refuses
/// hot-added vCPUs, so only KVM VMs get vCPU headroom.
const ACCELERATOR: &str = "tcg";
const CPU_LIMIT_OVERHEAD_MILLIS: u32 = 1000;
const MEMORY_REQUEST_OVERHEAD_MIB: u32 = 256;
const MEMORY_LIMIT_OVERHEAD_MIB: u32 = 512;
const SIZE_DIR: &str = "/run/qrab-size";

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

    /// Container resources: guest size plus a fixed, bounded QEMU/BMC overhead.
    pub fn container_resources(self) -> Value {
        json!({
            "requests":{"cpu":self.cpus.to_string(),"memory":memory_quantity(self.memory_mib+MEMORY_REQUEST_OVERHEAD_MIB)},
            "limits":{"cpu":(self.cpus+CPU_LIMIT_OVERHEAD_MILLIS/1000).to_string(),"memory":memory_quantity(self.memory_mib+MEMORY_LIMIT_OVERHEAD_MIB)}
        })
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

    /// Hot-plug ceilings fixed when QEMU boots at `self`.
    pub fn maxima(self, vm: &VirtualMachine) -> Self {
        Self {
            cpus: if ACCELERATOR == "kvm" {
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

#[derive(Clone)]
pub struct Settings {
    pub namespace: String,
    pub vm_image: String,
    pub oidc_issuer: String,
    pub oidc_client_id: String,
    pub idp_ip: Option<String>,
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
    origin(&vm.spec.console.public_url)?;
    if vm.spec.console.allowed_subjects.is_empty()
        || vm.spec.console.allowed_subjects.len() > 128
        || vm
            .spec
            .console
            .allowed_subjects
            .iter()
            .any(|s| uuid::Uuid::parse_str(s).is_err())
    {
        return Err(Error::Invalid(
            "console.allowedSubjects must contain operator UUIDs".into(),
        ));
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
    Ok(())
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
    validate(vm, settings)?;
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
    let config_name = format!("{name}-config");
    let uid = vm.uid().expect("validated UID");
    let mut owner = vm.controller_owner_ref(&()).expect("VM has a UID");
    // Background GC needs no delete/finalizer authority on the parent CR.
    owner.block_owner_deletion = Some(false);
    let labels = json!({"app.kubernetes.io/name":"qrab", "app.kubernetes.io/component":"vm", "app.kubernetes.io/instance":name, "qrab.yapplesauce.com/owner-uid":uid});
    let mut config = json!({"name":name,"memory_mib":boot.memory_mib,"cpus":boot.cpus,"accelerator":ACCELERATOR,"console":true,"uefi":{"boot_disk":vm.spec.boot_disk},"tpm":{},"state_dir":"/var/lib/qrab","state_id":uid,"runtime_dir":"/run/qrab","startup_timeout_secs":30,"shutdown_grace_secs":5,
        "hotplug":{"max_cpus":maximum.cpus,"max_memory_mib":maximum.memory_mib,"limits_dir":SIZE_DIR,"cpu_overhead_millis":CPU_LIMIT_OVERHEAD_MILLIS,"memory_overhead_mib":MEMORY_LIMIT_OVERHEAD_MIB}});
    let initiator = format!("iqn.2026-11.com.yapplesauce:qrab.vm.{uid}");
    let mut disks: Vec<_> = vm.spec.iscsi_disks.iter().map(|d|json!({"id":d.id,"portal":d.portal,"target":d.target,"lun":d.lun,"initiator_name":initiator,"username":d.username,"password_file":format!("/run/qrab-auth/{}/password",d.id),"read_only":d.read_only,"boot_index":d.boot_index,"bus":"virtio"})).collect();
    disks.extend(provisioned.iter().map(|d|json!({"id":d.id,"portal":d.portal,"target":d.target,"lun":0,"initiator_name":initiator,"username":d.username,"password_file":format!("/run/qrab-auth/{}/password",d.id),"read_only":d.read_only,"boot_index":d.boot_index,"bus":"virtio"})));
    config["iscsi_disks"] = json!(disks);
    let config_data = serde_json::to_string_pretty(&config)?;
    let config_map: ConfigMap = serde_json::from_value(
        json!({"apiVersion":"v1","kind":"ConfigMap","metadata":{"name":config_name,"namespace":settings.namespace,"labels":labels,"ownerReferences":[owner]},"data":{"vm.json":config_data}}),
    )?;
    let mut pod = json!({"apiVersion":"v1","kind":"Pod","metadata":{"name":name,"namespace":settings.namespace,"labels":labels,"ownerReferences":[owner]},"spec":{
        "serviceAccountName":"qrab-manager","automountServiceAccountToken":false,"enableServiceLinks":false,
        "hostNetwork":false,"hostPID":false,"hostIPC":false,
        "restartPolicy":"Never","terminationGracePeriodSeconds":20,
        "securityContext":{"runAsNonRoot":true,"runAsUser":10001,"runAsGroup":10001,"fsGroup":10001,"fsGroupChangePolicy":"OnRootMismatch","seccompProfile":{"type":"RuntimeDefault"}},
        "tolerations":[{"key":"node-role.kubernetes.io/control-plane","operator":"Exists","effect":"NoSchedule"}],
        "containers":[{"name":"manager","image":vm.spec.image.as_deref().unwrap_or(&settings.vm_image),"imagePullPolicy":"Always","command":["qrab-bmc"],"args":["--config","/etc/qrab/vm.json"],
            "ports":[{"name":"http","containerPort":8080}],
            "env":[{"name":"BMC_PUBLIC_URL","value":origin(&vm.spec.console.public_url)?},{"name":"BMC_OIDC_ISSUER","value":settings.oidc_issuer},{"name":"BMC_OIDC_CLIENT_ID","value":settings.oidc_client_id},{"name":"BMC_ALLOWED_SUBJECTS","value":vm.spec.console.allowed_subjects.join(",")}],
            "resources":GuestSize::of(vm).container_resources(),
            "resizePolicy":[{"resourceName":"cpu","restartPolicy":"NotRequired"},{"resourceName":"memory","restartPolicy":"NotRequired"}],
            "securityContext":{"allowPrivilegeEscalation":false,"privileged":false,"readOnlyRootFilesystem":true,"capabilities":{"drop":["ALL"]}},"terminationMessagePolicy":"FallbackToLogsOnError",
            "volumeMounts":[{"name":"config","mountPath":"/etc/qrab","readOnly":true},{"name":"state","mountPath":"/var/lib/qrab"},{"name":"runtime","mountPath":"/run/qrab"},{"name":"tmp","mountPath":"/tmp"}],
            "startupProbe":{"httpGet":{"path":"/readyz","port":"http"},"periodSeconds":1,"failureThreshold":90,"timeoutSeconds":2},
            "readinessProbe":{"httpGet":{"path":"/readyz","port":"http"},"periodSeconds":2,"failureThreshold":3,"timeoutSeconds":2},
            "livenessProbe":{"httpGet":{"path":"/readyz","port":"http"},"periodSeconds":5,"failureThreshold":3,"timeoutSeconds":2}}],
        "volumes":[{"name":"config","configMap":{"name":config_name}},{"name":"state","persistentVolumeClaim":{"claimName":vm.spec.state_volume_claim}},{"name":"runtime","emptyDir":{"medium":"Memory","sizeLimit":"64Mi"}},{"name":"tmp","emptyDir":{"medium":"Memory","sizeLimit":"64Mi"}}]
    }});
    if let Some(node) = &vm.spec.storage_node {
        pod["spec"]["nodeSelector"] = json!({"kubernetes.io/hostname":node});
    }
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
    pod["spec"]["containers"][0]["volumeMounts"]
        .as_array_mut()
        .expect("mounts")
        .push(json!({"name":"size","mountPath":SIZE_DIR,"readOnly":true}));
    if let Some(ip) = &settings.idp_ip {
        let issuer =
            url::Url::parse(&settings.oidc_issuer).map_err(|e| Error::Invalid(e.to_string()))?;
        pod["spec"]["hostAliases"] = json!([{"ip":ip,"hostnames":[issuer.host_str().ok_or_else(||Error::Invalid("OIDC issuer has no host".into()))?]}]);
    }
    // In-place resizes change only container resources, never the template.
    let mut template = pod.clone();
    template["spec"]["containers"][0]
        .as_object_mut()
        .expect("container")
        .remove("resources");
    let mut identity =
        json!({"pod":template,"config":config_data,"restartNonce":vm.spec.restart_nonce});
    // A rotated credential or grown LUN needs a fresh QEMU: libiscsi neither
    // reloads secrets nor sees capacity growth. Absent for legacy VMs, so
    // their template hash is unchanged.
    if !provisioned.is_empty() {
        identity["provisioned"] = json!(provisioned
            .iter()
            .map(|d| json!({"id":d.id,"generation":d.generation,"capacityBytes":d.capacity_bytes}))
            .collect::<Vec<_>>());
    }
    let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&identity)?));
    pod["metadata"]["annotations"] = json!({HASH:hash, BOOT_SIZE:serde_json::to_string(&boot)?});
    Ok(Desired {
        config: config_map,
        pod: serde_json::from_value(pod)?,
        hash,
        boot,
        maximum,
    })
}

pub fn owned<K: Resource<DynamicType = ()>>(resource: &K, vm: &VirtualMachine) -> bool {
    resource
        .meta()
        .owner_references
        .as_ref()
        .is_some_and(|owners| {
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
fn contains(actual: &Value, desired: &Value) -> bool {
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
    if spec.init_containers.as_ref().is_some_and(|v| !v.is_empty())
        || spec
            .image_pull_secrets
            .as_ref()
            .is_some_and(|v| !v.is_empty())
    {
        return false;
    }
    for container in &spec.containers {
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
        v.persistent_volume_claim
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
        "spec":{"containers":[{"name":"manager","resources":want.container_resources()}]}
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
