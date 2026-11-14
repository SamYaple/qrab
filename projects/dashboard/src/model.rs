//! Plain view data derived from VirtualMachine, Pod and metrics objects.
use crate::{access::Role, cluster::Usage};
use k8s_openapi::api::core::v1::{Container, Pod};
use qrab_operator::api::{ResizeStatus, VirtualMachine};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub namespace: String,
    pub name: String,
    pub phase: String,
    pub ready: bool,
    pub node: String,
    pub cpus: u32,
    pub max_cpus: Option<u32>,
    pub memory_mib: u32,
    pub max_memory_mib: Option<u32>,
    pub disks: usize,
    pub disk_bytes: u64,
    /// The first NIC's IP: its NIC Pod's IP.
    pub ip: Option<String>,
    pub usage: Option<Usage>,
    pub role: Role,
}

impl Row {
    pub fn path(&self) -> String {
        format!("/vms/{}/{}", self.namespace, self.name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiskRow {
    pub id: String,
    pub size_bytes: u64,
    pub capacity_bytes: Option<u64>,
    pub phase: String,
    pub generation: Option<u64>,
    pub base_image: Option<String>,
}

/// A `spec.nics` entry joined with its `status.nics` entry by name.
#[derive(Debug, Clone, PartialEq)]
pub struct NicRow {
    pub name: String,
    pub mac: Option<String>,
    pub ip: Option<String>,
    pub security_groups: Vec<String>,
    pub pod_name: Option<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerRow {
    pub name: String,
    pub kind: &'static str,
    pub requests: String,
    pub limits: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Detail {
    pub row: Row,
    pub uid: String,
    pub accelerator: &'static str,
    pub boot: String,
    /// "CirrOS 0.6.3 (cirros g1) clone on root", or "demo ESP".
    pub boots_from: String,
    pub running: bool,
    pub storage_node: String,
    pub state_pvc: String,
    pub reason: String,
    pub message: String,
    pub disks: Vec<DiskRow>,
    pub iscsi_disks: Vec<String>,
    pub resize: Option<ResizeStatus>,
    pub nics: Vec<NicRow>,
    /// The seed in one line; never the user-data itself.
    pub cloud_init: String,
    pub bmc_enabled: bool,
    pub redfish_enabled: bool,
    pub pod_name: Option<String>,
    pub pod_ip: Option<String>,
    pub containers: Vec<ContainerRow>,
}

pub fn row(vm: &VirtualMachine, role: Role, usage: &HashMap<String, Usage>) -> Row {
    let status = vm.status.clone().unwrap_or_default();
    let resize = status.resize.as_ref();
    let cpus = resize.and_then(|r| r.applied_cpus).unwrap_or(vm.spec.cpus);
    let memory_mib = resize
        .and_then(|r| r.applied_memory_mib)
        .unwrap_or(vm.spec.memory_mib);
    let max_cpus = resize
        .map(|r| r.max_cpus)
        .filter(|m| *m > 0)
        .or(vm.spec.max_cpus)
        .filter(|m| *m > cpus);
    let max_memory_mib = resize
        .map(|r| r.max_memory_mib)
        .filter(|m| *m > 0)
        .or(vm.spec.max_memory_mib)
        .filter(|m| *m > memory_mib);
    Row {
        namespace: vm.metadata.namespace.clone().unwrap_or_default(),
        name: vm.metadata.name.clone().unwrap_or_default(),
        phase: if status.phase.is_empty() {
            "Pending".into()
        } else {
            status.phase.clone()
        },
        ready: status.ready,
        node: status.node_name.clone().unwrap_or_default(),
        cpus,
        max_cpus,
        memory_mib,
        max_memory_mib,
        disks: vm.spec.disks.len() + vm.spec.iscsi_disks.len(),
        disk_bytes: status.disks.iter().map(|d| d.capacity_bytes).sum(),
        ip: nics(vm).into_iter().next().and_then(|n| n.ip),
        usage: status
            .pod_name
            .as_ref()
            .and_then(|pod| usage.get(pod))
            .copied(),
        role,
    }
}

fn resources(
    map: Option<
        &std::collections::BTreeMap<
            String,
            k8s_openapi::apimachinery::pkg::api::resource::Quantity,
        >,
    >,
) -> String {
    let Some(map) = map.filter(|m| !m.is_empty()) else {
        return "—".into();
    };
    map.iter()
        .map(|(k, v)| format!("{k} {}", v.0))
        .collect::<Vec<_>>()
        .join(", ")
}

fn container(c: &Container, kind: &'static str) -> ContainerRow {
    let r = c.resources.as_ref();
    ContainerRow {
        name: c.name.clone(),
        kind,
        requests: resources(r.and_then(|r| r.requests.as_ref())),
        limits: resources(r.and_then(|r| r.limits.as_ref())),
    }
}

pub fn detail(
    vm: &VirtualMachine,
    role: Role,
    usage: &HashMap<String, Usage>,
    pod: Option<&Pod>,
    pod_ip: Option<std::net::IpAddr>,
    catalog: &[crate::images::BaseImage],
) -> Detail {
    let status = vm.status.clone().unwrap_or_default();
    let disks = vm
        .spec
        .disks
        .iter()
        .map(|d| {
            let observed = status.disks.iter().find(|s| s.id == d.id);
            DiskRow {
                id: d.id.clone(),
                size_bytes: d.size_bytes,
                capacity_bytes: observed.map(|s| s.capacity_bytes),
                phase: observed.map_or_else(|| "Pending".into(), |s| s.phase.clone()),
                generation: observed.map(|s| s.generation),
                base_image: d
                    .base_image
                    .as_ref()
                    .map(|b| format!("{} (generation {})", b.name, b.generation)),
            }
        })
        .collect();
    let mut containers = Vec::new();
    if let Some(spec) = pod.and_then(|p| p.spec.as_ref()) {
        for c in spec.init_containers.iter().flatten() {
            containers.push(container(c, "sidecar"));
        }
        for c in &spec.containers {
            containers.push(container(c, "container"));
        }
    }
    Detail {
        row: row(vm, role, usage),
        uid: vm.metadata.uid.clone().unwrap_or_default(),
        accelerator: vm.spec.accelerator.as_str(),
        boot: format!("UEFI from {}", vm.spec.boot_disk),
        boots_from: boots_from(vm, catalog),
        running: vm.spec.running,
        storage_node: vm.spec.storage_node.clone().unwrap_or_default(),
        state_pvc: vm.spec.state_volume_claim.clone(),
        reason: status.reason.clone(),
        message: status.message.clone(),
        disks,
        iscsi_disks: vm.spec.iscsi_disks.iter().map(|d| d.id.clone()).collect(),
        resize: status.resize.clone(),
        nics: nics(vm),
        cloud_init: cloud_init(vm),
        bmc_enabled: vm.spec.components.bmc.enabled,
        redfish_enabled: vm.spec.components.redfish_enabled(),
        pod_name: status.pod_name.clone(),
        pod_ip: pod_ip.map(|ip| ip.to_string()),
        containers,
    }
}

/// The VM's NICs in spec order, with what `status.nics` reports for each.
pub fn nics(vm: &VirtualMachine) -> Vec<NicRow> {
    let observed = vm
        .status
        .as_ref()
        .map(|s| s.nics.as_slice())
        .unwrap_or_default();
    vm.spec
        .nics
        .iter()
        .map(|nic| {
            let status = observed.iter().find(|s| s.name == nic.name);
            NicRow {
                name: nic.name.clone(),
                mac: status.map(|s| s.mac.clone()).filter(|m| !m.is_empty()),
                ip: status
                    .and_then(|s| s.ip.clone())
                    .filter(|ip| !ip.is_empty()),
                security_groups: nic.security_groups.iter().map(|g| g.0.clone()).collect(),
                pod_name: status.map(|s| s.pod_name.clone()).filter(|p| !p.is_empty()),
                ready: status.is_some_and(|s| s.ready),
            }
        })
        .collect()
}

/// "hostname web, user-data 120 bytes". Only the size of user-data is shown.
///
/// ```
/// use qrab_dashboard::model::cloud_init;
/// let vm: qrab_operator::api::VirtualMachine = serde_json::from_value(serde_json::json!({
///     "apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "VirtualMachine",
///     "metadata": {"name": "web"},
///     "spec": {"stateVolumeClaim": "web-state", "cloudInit": {"userData": "#cloud-config\n"}}
/// })).unwrap();
/// assert_eq!(cloud_init(&vm), "hostname web, user-data 14 bytes");
/// ```
pub fn cloud_init(vm: &VirtualMachine) -> String {
    let name = vm.metadata.name.clone().unwrap_or_default();
    match &vm.spec.cloud_init {
        Some(c) => format!(
            "hostname {}, {}",
            c.hostname.as_deref().unwrap_or(&name),
            match &c.user_data {
                Some(u) => format!("user-data {} bytes", u.len()),
                None => "no user-data".into(),
            }
        ),
        None if !vm.spec.nics.is_empty() => {
            format!("hostname {name}, no user-data (seed for the NICs only)")
        }
        None => "none".into(),
    }
}

/// The VM's first boot source: a base-image clone with boot index 0, named
/// by its catalog title when the catalog still lists it, else the demo ESP.
pub fn boots_from(vm: &VirtualMachine, catalog: &[crate::images::BaseImage]) -> String {
    let Some((disk, image)) = vm
        .spec
        .disks
        .iter()
        .find_map(|d| (d.boot_index == Some(0)).then_some((d, d.base_image.as_ref()?)))
    else {
        return "demo ESP".into();
    };
    let label = format!("{} g{}", image.name, image.generation);
    match catalog
        .iter()
        .find(|c| c.name == image.name && c.generation == image.generation)
    {
        Some(c) => format!("{} ({label}) clone on {}", c.title, disk.id),
        None => format!("{label} clone on {}", disk.id),
    }
}

/// Bytes as GiB with one decimal.
pub fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / 1024f64.powi(3))
}

pub fn usage_text(usage: Option<Usage>) -> (String, String) {
    match usage {
        Some(u) => (
            format!("{:.0}m", u.cpu_cores * 1000.0),
            format!("{:.0} MiB", u.memory_bytes / 1024f64.powi(2)),
        ),
        None => ("—".into(), "—".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::tests::vm;

    #[test]
    fn rows_show_current_size_ceilings_disks_and_usage() {
        let mut machine = vm(
            "web",
            serde_json::json!({"cpus": 2, "maxCpus": 8, "memoryMiB": 1024,
                "storageNode": "kube03",
                "disks": [{"id": "a", "sizeBytes": 1073741824u64}, {"id": "b", "sizeBytes": 2147483648u64}]}),
            None,
        );
        machine.status = Some(
            serde_json::from_value(serde_json::json!({
                "observedGeneration": 1, "phase": "Running", "ready": true, "reason": "", "message": "",
                "podName": "web", "nodeName": "kube03",
                "disks": [{"id": "a", "capacityBytes": 1073741824u64, "generation": 1, "phase": "Active"}],
                "resize": {"phase": "Applied", "message": "", "cpus": 3, "memoryMiB": 1024,
                    "appliedCpus": 3, "appliedMemoryMiB": 1024, "bootCpus": 2, "bootMemoryMiB": 1024,
                    "maxCpus": 8, "maxMemoryMiB": 1024}
            }))
            .unwrap(),
        );
        let usage = HashMap::from([(
            "web".to_string(),
            Usage {
                cpu_cores: 0.5,
                memory_bytes: 600.0 * 1048576.0,
            },
        )]);
        let r = row(&machine, Role::Viewer, &usage);
        assert_eq!((r.cpus, r.max_cpus), (3, Some(8)));
        assert_eq!((r.memory_mib, r.max_memory_mib), (1024, None));
        assert_eq!((r.disks, r.disk_bytes), (2, 1073741824));
        assert_eq!(usage_text(r.usage), ("500m".into(), "600 MiB".into()));
        assert_eq!(r.path(), "/vms/qrab/web");
        let d = detail(&machine, Role::Viewer, &usage, None, None, &[]);
        assert_eq!(d.boots_from, "demo ESP");
        assert_eq!(d.disks[0].phase, "Active");
        assert_eq!(d.disks[1].phase, "Pending");
        assert_eq!(d.storage_node, "kube03");
        assert_eq!(gib(1073741824), "1.0 GiB");
    }

    #[test]
    fn nics_join_spec_and_status_and_cloud_init_shows_only_sizes() {
        let mut machine = vm(
            "web",
            serde_json::json!({"nics": [{"name": "eth0", "securityGroups": ["web", "ssh"]},
                {"name": "eth1"}]}),
            None,
        );
        assert_eq!(row(&machine, Role::Viewer, &HashMap::new()).ip, None);
        assert_eq!(
            cloud_init(&machine),
            "hostname web, no user-data (seed for the NICs only)"
        );
        machine.status = Some(
            serde_json::from_value(serde_json::json!({
                "observedGeneration": 1, "phase": "Pending", "ready": false, "reason": "NicPending", "message": "",
                "nics": [{"name": "eth1", "mac": "02:00:00:00:00:02", "podName": "web-nic-eth1", "ip": null, "ready": false},
                    {"name": "eth0", "mac": "02:00:00:00:00:01", "podName": "web-nic-eth0", "ip": "10.42.1.7", "ready": true}]
            }))
            .unwrap(),
        );
        assert_eq!(
            row(&machine, Role::Viewer, &HashMap::new()).ip.as_deref(),
            Some("10.42.1.7")
        );
        let nics = nics(&machine);
        assert_eq!(
            nics[0],
            NicRow {
                name: "eth0".into(),
                mac: Some("02:00:00:00:00:01".into()),
                ip: Some("10.42.1.7".into()),
                security_groups: vec!["web".into(), "ssh".into()],
                pod_name: Some("web-nic-eth0".into()),
                ready: true,
            }
        );
        assert_eq!((nics[1].ip.as_deref(), nics[1].ready), (None, false));
        machine.spec.cloud_init = Some(qrab_operator::api::CloudInit {
            hostname: Some("db".into()),
            user_data: None,
        });
        assert_eq!(cloud_init(&machine), "hostname db, no user-data");
        machine.spec.nics.clear();
        machine.spec.cloud_init = None;
        assert_eq!(cloud_init(&machine), "none");
    }

    #[test]
    fn the_boot_source_names_the_cloned_image() {
        let clone = |boot: serde_json::Value| {
            let mut root = serde_json::json!({"id": "root", "sizeBytes": 1073741824u64,
                "baseImage": {"name": "cirros", "generation": 1}});
            if !boot.is_null() {
                root["bootIndex"] = boot;
            }
            vm(
                "web",
                serde_json::json!({"storageNode": "kube03", "disks": [
                {"id": "data", "sizeBytes": 1073741824u64}, root]}),
                None,
            )
        };
        let catalog = [crate::images::tests::cirros()];
        assert_eq!(
            boots_from(&clone(0.into()), &catalog),
            "CirrOS 0.6.3 (cirros g1) clone on root"
        );
        assert_eq!(boots_from(&clone(0.into()), &[]), "cirros g1 clone on root");
        assert_eq!(
            boots_from(&clone(serde_json::Value::Null), &catalog),
            "demo ESP"
        );
        assert_eq!(boots_from(&clone(2.into()), &catalog), "demo ESP");
    }
}
