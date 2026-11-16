//! The launch form: validation, the rendered Kanidm groups, PVC and
//! VirtualMachine, and the step-by-step creation with a report.
use crate::images::BaseImage;
use crate::{
    cluster::{self, Cluster},
    kanidm::Kanidm,
};
use k8s_openapi::api::core::v1::PersistentVolumeClaim;
use qrab_operator::resources::CREATOR;
use serde_json::json;

pub const BOOT_DISK: &str = "/opt/qrab/demo/boot.img";
pub const STORAGE_CLASS: &str = "zfs-block";
pub const MAX_NAME: usize = 50;
pub const FORM_DISKS: usize = 4;
pub const MAX_DISK_GIB: u64 = 1024;
/// NIC rows on the form, named `eth0`, `eth1`, ...; the CRD allows 4.
pub const FORM_NICS: usize = 2;
/// The security group every VPN NIC joins: `contrib/kubernetes/proton/sg-vpn-egress.yaml`
/// admits only the tunnel's UDP to any address, because Proton servers change.
pub const VPN_EGRESS_GROUP: &str = "vpn-egress";
pub const MAX_SECURITY_GROUPS: usize = 8;
pub use qrab_operator::resources::USER_DATA_LIMIT;
/// A launch form carries up to 32 KiB of user-data, URL-encoded.
pub const LAUNCH_BODY_LIMIT: usize = 4 * USER_DATA_LIMIT;
/// Marks resources the dashboard created; only such VMs can be deleted here.
pub use crate::access::{MANAGED_BY, MANAGER};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormError {
    pub field: Option<String>,
    pub message: String,
}

impl FormError {
    pub fn at(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: Some(field.into()),
            message: message.into(),
        }
    }
}
impl From<String> for FormError {
    fn from(message: String) -> Self {
        Self {
            field: None,
            message,
        }
    }
}
impl From<&str> for FormError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disk {
    pub id: String,
    pub size_gib: u64,
    pub base_image: Option<(String, u32)>,
}

/// The chosen operating system: a copy-on-write clone of a catalog image
/// as the VM's `root` disk, booted first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Os {
    pub image: BaseImage,
    pub root_gib: u64,
}

/// The disk ID of an OS image clone.
pub const ROOT_DISK: &str = "root";

/// A guest NIC, backed by the NIC Pod `<vm>-nic-<name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nic {
    pub name: String,
    pub security_groups: Vec<String>,
    /// The VpnProfile whose Secret `vpn-<profile>` carries this NIC's tunnel.
    pub vpn_profile: Option<String>,
}

/// A VpnProfile as the launch form offers it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnChoice {
    pub name: String,
    /// `CH#12 · CH · Zurich`, or why there is no server yet.
    pub summary: String,
    /// `vm/nic` of a NIC that already uses the profile.
    pub used_by: Option<String>,
}

/// The form value of a NIC's network choice.
pub fn vpn_value(profile: &str) -> String {
    format!("vpn:{profile}")
}

/// The profile named by a `vpn-<profile>` NIC Secret.
pub fn vpn_profile_of(secret_name: &str) -> Option<&str> {
    secret_name.strip_prefix("vpn-").filter(|p| !p.is_empty())
}

/// The guest name of form NIC `i`, matching the seed's `ethN`.
pub fn nic_name(i: usize) -> String {
    format!("eth{i}")
}

/// `spec.cloudInit`. Its Debug output shows only the user-data size, so it
/// cannot reach a log line.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct CloudInit {
    pub hostname: Option<String>,
    pub user_data: Option<String>,
}

impl std::fmt::Debug for CloudInit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudInit")
            .field("hostname", &self.hostname)
            .field("user_data_bytes", &self.user_data.as_ref().map(String::len))
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub name: String,
    pub os: Option<Os>,
    pub cpus: u32,
    pub max_cpus: Option<u32>,
    pub memory_mib: u32,
    pub max_memory_mib: Option<u32>,
    pub kvm: bool,
    pub disks: Vec<Disk>,
    pub nics: Vec<Nic>,
    pub cloud_init: Option<CloudInit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Names {
    pub vm: String,
    pub pvc: String,
    pub admins: String,
    pub viewers: String,
}

/// The namespace-specific VM name limit, including generated access groups.
pub fn max_name_length(namespace: &str) -> usize {
    MAX_NAME.min(64usize.saturating_sub(format!("qrab_vm_{namespace}__viewers").len()))
}

/// The names one launch creates. Kanidm names are at most 64 characters.
///
/// ```
/// let n = qrab_dashboard::launch::names("qrab", "web").unwrap();
/// assert_eq!(n.pvc, "web-state");
/// assert_eq!(n.admins, "qrab_vm_qrab_web_admins");
/// assert_eq!(n.viewers, "qrab_vm_qrab_web_viewers");
/// ```
pub fn names(namespace: &str, name: &str) -> Result<Names, String> {
    if !crate::config::dns_label(name) || name.len() > MAX_NAME {
        return Err(format!(
            "Name must be a DNS label of at most {MAX_NAME} characters: lowercase letters, digits and '-', starting and ending with a letter or digit."
        ));
    }
    let viewers = format!("qrab_vm_{namespace}_{name}_viewers");
    if viewers.len() > 64 {
        return Err(format!(
            "Name is too long for its Kanidm group {viewers} (at most 64 characters; {} available for the name).",
            64usize.saturating_sub(viewers.len() - name.len())
        ));
    }
    Ok(Names {
        vm: name.into(),
        pvc: format!("{name}-state"),
        admins: format!("qrab_vm_{namespace}_{name}_admins"),
        viewers,
    })
}

fn disk_id(id: &str) -> bool {
    let b = id.as_bytes();
    (1..=20).contains(&b.len())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

fn image_name(name: &str) -> bool {
    let b = name.as_bytes();
    (1..=32).contains(&b.len())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && b[0] != b'-'
        && b[b.len() - 1] != b'-'
}

/// Looks up a submitted field.
pub fn field<'a>(fields: &'a [(String, String)], key: &str) -> &'a str {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.trim())
        .unwrap_or("")
}

fn number<T: std::str::FromStr + PartialOrd + std::fmt::Display + Copy>(
    errors: &mut Vec<FormError>,
    field: &str,
    label: &str,
    raw: &str,
    min: T,
    max: T,
) -> Option<T> {
    match raw.parse::<T>() {
        Ok(v) if v >= min && v <= max => Some(v),
        _ => {
            errors.push(FormError::at(
                field,
                format!("{label} must be a whole number from {min} to {max}."),
            ));
            None
        }
    }
}

/// Comma-separated DNS labels, distinct, at most [`MAX_SECURITY_GROUPS`].
fn security_groups(errors: &mut Vec<FormError>, field: &str, nic: &str, raw: &str) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    for group in raw.split(',').map(str::trim).filter(|g| !g.is_empty()) {
        if !crate::config::dns_label(group) {
            errors.push(FormError::at(field, format!(
                "Security group {group} on {nic} must be a DNS label: 1-63 lowercase letters, digits or '-', starting and ending with a letter or digit."
            )));
        } else if groups.iter().any(|g| g == group) {
            errors.push(FormError::at(
                field,
                format!("Security group {group} on {nic} is listed twice."),
            ));
        } else {
            groups.push(group.to_owned());
        }
    }
    if groups.len() > MAX_SECURITY_GROUPS {
        errors.push(FormError::at(
            field,
            format!("{nic} may have at most {MAX_SECURITY_GROUPS} security groups."),
        ));
    }
    groups
}

/// The submitted user-data, untrimmed, with the browser's CRLF line breaks
/// as LF. Whitespace only means none.
pub fn user_data(fields: &[(String, String)]) -> Option<String> {
    fields
        .iter()
        .find(|(k, _)| k == "user_data")
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(_, v)| v.replace("\r\n", "\n"))
}

/// Validates the form against the CRD's limits. Every problem is reported.
/// Errors never quote user-data.
pub fn parse(
    fields: &[(String, String)],
    namespace: &str,
    storage_node: Option<&str>,
    catalog: &[BaseImage],
    vpn: &[VpnChoice],
) -> Result<Request, Vec<String>> {
    parse_form(fields, namespace, storage_node, catalog, vpn)
        .map_err(|errors| errors.into_iter().map(|error| error.message).collect())
}

/// The same validation with stable field targets for the browser form.
pub fn parse_form(
    fields: &[(String, String)],
    namespace: &str,
    storage_node: Option<&str>,
    catalog: &[BaseImage],
    vpn: &[VpnChoice],
) -> Result<Request, Vec<FormError>> {
    let mut errors = Vec::new();
    let name = field(fields, "name").to_owned();
    if let Err(e) = names(namespace, &name) {
        errors.push(FormError::at("name", e));
    }
    // The choice must still be in the catalog at submit time.
    let os = match field(fields, "os") {
        "" | "none" => None,
        key => match catalog.iter().find(|image| image.key() == key) {
            None => {
                errors.push(FormError::at("operating-system", format!(
                    "The operating system image {key} is not in the base-image catalog (it may have been removed). Reload the form and choose again."
                )));
                None
            }
            Some(image) => {
                let min = image.min_root_gib();
                let root = match field(fields, "root_size_gib") {
                    "" => Some(min),
                    raw => number(
                        &mut errors,
                        "root_size_gib",
                        "Root disk size (GiB)",
                        raw,
                        min,
                        MAX_DISK_GIB,
                    ),
                };
                if storage_node.is_none() {
                    errors.push("Operating system images need a storage node; DASHBOARD_STORAGE_NODE is not configured.".into());
                }
                root.map(|root_gib| Os {
                    image: image.clone(),
                    root_gib,
                })
            }
        },
    };
    let cpus = number(
        &mut errors,
        "cpus",
        "vCPUs",
        field(fields, "cpus"),
        1u32,
        256,
    );
    let max_cpus = match field(fields, "max_cpus") {
        "" => None,
        raw => number(&mut errors, "max_cpus", "Maximum vCPUs", raw, 1u32, 256),
    };
    let memory_mib = number(
        &mut errors,
        "memory_mib",
        "Memory",
        field(fields, "memory_mib"),
        256u32,
        1_048_576,
    );
    let max_memory_mib = match field(fields, "max_memory_mib") {
        "" => None,
        raw => number(
            &mut errors,
            "max_memory_mib",
            "Maximum memory",
            raw,
            256u32,
            1_048_576,
        ),
    };
    if let (Some(c), Some(m)) = (cpus, max_cpus)
        && m < c
    {
        errors.push(FormError::at(
            "max_cpus",
            "Maximum vCPUs must be at least vCPUs.",
        ));
    }
    if let (Some(c), Some(m)) = (memory_mib, max_memory_mib)
        && m < c
    {
        errors.push(FormError::at(
            "max_memory_mib",
            "Maximum memory must be at least the memory size.",
        ));
    }
    let kvm = match field(fields, "accelerator") {
        "tcg" | "" => false,
        "kvm" => true,
        _ => {
            errors.push(FormError::at(
                "accelerator",
                "Accelerator must be kvm or tcg.",
            ));
            false
        }
    };
    let mut disks: Vec<Disk> = Vec::new();
    for i in 0..FORM_DISKS {
        let key = |suffix: &str| format!("disk{i}_{suffix}");
        let id = field(fields, &key("id"));
        let size = field(fields, &key("size_gib"));
        let base = field(fields, &key("base_image"));
        let generation = field(fields, &key("base_generation"));
        if id.is_empty() {
            if !(size.is_empty() && base.is_empty() && generation.is_empty()) {
                errors.push(FormError::at(
                    key("id"),
                    format!("Disk {} needs an ID.", i + 1),
                ));
            }
            continue;
        }
        let label = format!("Disk {}", i + 1);
        if id == ROOT_DISK && !matches!(field(fields, "os"), "" | "none") {
            errors.push(FormError::at(
                key("id"),
                format!("{label} cannot be named {ROOT_DISK}: that is the operating system disk."),
            ));
        }
        if !disk_id(id) {
            errors.push(FormError::at(key("id"), format!("{label} ID must be 1-20 lowercase letters, digits or '-', starting and ending with a letter or digit.")));
        } else if disks.iter().any(|d| d.id == id) {
            errors.push(FormError::at(
                key("id"),
                format!("{label} ID {id} is used twice."),
            ));
        }
        let size_gib = number(
            &mut errors,
            &key("size_gib"),
            &format!("{label} size (GiB)"),
            size,
            1u64,
            MAX_DISK_GIB,
        );
        let base_image = match (base, generation) {
            ("", "") => None,
            (name, raw) if image_name(name) => number(
                &mut errors,
                &key("base_generation"),
                &format!("{label} base image generation"),
                raw,
                1u32,
                999_999,
            )
            .map(|g| (name.to_owned(), g)),
            _ => {
                errors.push(FormError::at(
                    key("base_image"),
                    format!(
                        "{label} base image name must be 1-32 lowercase letters, digits or '-'."
                    ),
                ));
                None
            }
        };
        if let Some(size_gib) = size_gib {
            disks.push(Disk {
                id: id.into(),
                size_gib,
                base_image,
            });
        }
    }
    if !disks.is_empty() && storage_node.is_none() {
        errors.push(
            "Data disks need a storage node; DASHBOARD_STORAGE_NODE is not configured.".into(),
        );
    }
    let mut nics: Vec<Nic> = Vec::new();
    for i in 0..FORM_NICS {
        let name = nic_name(i);
        let groups_field = format!("nic{i}_security_groups");
        let network_field = format!("nic{i}_network");
        let raw = field(fields, &groups_field);
        let mut security_groups = security_groups(&mut errors, &groups_field, &name, raw);
        let vpn_profile = match field(fields, &network_field) {
            "" | "direct" => None,
            value => match value
                .strip_prefix("vpn:")
                .and_then(|p| vpn.iter().find(|c| c.name == p))
            {
                None => {
                    errors.push(FormError::at(&network_field, format!(
                        "{name}: the VPN profile {} does not exist (it may have been deleted). Reload the form and choose again.",
                        value.strip_prefix("vpn:").unwrap_or(value)
                    )));
                    None
                }
                Some(choice) => {
                    if let Some(used_by) = &choice.used_by {
                        errors.push(FormError::at(&network_field, format!(
                            "{name}: VPN profile {} is already used by {used_by}; a profile is one Proton connection for one NIC. Create another profile on the VPN page.",
                            choice.name
                        )));
                    }
                    Some(choice.name.clone())
                }
            },
        };
        if field(fields, &format!("nic{i}_enabled")) != "on" {
            if !raw.is_empty() || vpn_profile.is_some() {
                errors.push(FormError::at(groups_field, format!(
                    "Security groups and VPN profiles apply to a network interface: attach {name} or clear its settings."
                )));
            }
            continue;
        }
        if let Some(profile) = &vpn_profile {
            if nics.iter().any(|n| n.vpn_profile.as_ref() == Some(profile)) {
                errors.push(FormError::at(&network_field, format!(
                    "{name}: VPN profile {profile} is already chosen for another interface of this VM."
                )));
            }
            if !security_groups.iter().any(|g| g == VPN_EGRESS_GROUP) {
                if security_groups.len() >= MAX_SECURITY_GROUPS {
                    errors.push(FormError::at(groups_field, format!(
                        "{name} may have at most {} other security groups: a VPN interface also joins {VPN_EGRESS_GROUP}.",
                        MAX_SECURITY_GROUPS - 1
                    )));
                }
                security_groups.push(VPN_EGRESS_GROUP.to_owned());
            }
        }
        nics.push(Nic {
            name,
            security_groups,
            vpn_profile,
        });
    }
    let hostname = match field(fields, "hostname") {
        "" => None,
        h if crate::config::dns_label(h) => Some(h.to_owned()),
        _ => {
            errors.push(FormError::at("hostname", "Hostname must be a DNS label: 1-63 lowercase letters, digits or '-', starting and ending with a letter or digit."));
            None
        }
    };
    let user_data = user_data(fields);
    if let Some(u) = &user_data
        && u.len() > USER_DATA_LIMIT
    {
        errors.push(FormError::at(
            "user_data",
            format!(
                "User-data is {} bytes; at most {USER_DATA_LIMIT} bytes (32 KiB) are allowed.",
                u.len()
            ),
        ));
    }
    // Cloud images expect a seed, so an OS launch always gets one.
    let cloud_init =
        (hostname.is_some() || user_data.is_some() || os.is_some()).then_some(CloudInit {
            hostname,
            user_data,
        });
    match (errors.is_empty(), cpus, memory_mib) {
        (true, Some(cpus), Some(memory_mib)) => Ok(Request {
            name,
            os,
            cpus,
            max_cpus,
            memory_mib,
            max_memory_mib,
            kvm,
            disks,
            nics,
            cloud_init,
        }),
        _ => Err(errors),
    }
}

pub fn render_pvc(names: &Names, namespace: &str) -> PersistentVolumeClaim {
    serde_json::from_value(json!({
        "apiVersion": "v1",
        "kind": "PersistentVolumeClaim",
        "metadata": {
            "name": names.pvc,
            "namespace": namespace,
            "labels": {MANAGED_BY: MANAGER},
            "annotations": {"qrab.yapplesauce.com/vm": names.vm},
        },
        "spec": {
            "storageClassName": STORAGE_CLASS,
            "accessModes": ["ReadWriteOncePod"],
            "volumeMode": "Filesystem",
            "resources": {"requests": {"storage": "1Gi"}},
        }
    }))
    .expect("static PVC shape")
}

pub struct Groups<'a> {
    pub admins: &'a str,
    pub viewers: &'a str,
}

/// The VirtualMachine. The console has groups only, no publicUrl: the
/// dashboard is the browser entry point and the BMC accepts bearers.
pub fn render_vm(
    request: &Request,
    names: &Names,
    namespace: &str,
    creator: &str,
    groups: &Groups<'_>,
    storage_node: Option<&str>,
) -> serde_json::Value {
    let mut spec = json!({
        "stateVolumeClaim": names.pvc,
        "memoryMiB": request.memory_mib,
        "cpus": request.cpus,
        "running": true,
        "bootDisk": BOOT_DISK,
        "console": {"adminGroups": [groups.admins], "viewerGroups": [groups.viewers]},
    });
    if let Some(max) = request.max_cpus {
        spec["maxCpus"] = max.into();
    }
    if let Some(max) = request.max_memory_mib {
        spec["maxMemoryMiB"] = max.into();
    }
    if request.kvm {
        spec["accelerator"] = "kvm".into();
    }
    let mut disks: Vec<serde_json::Value> = Vec::new();
    // OVMF boots the clone first; the demo ESP (boot index 1) is the fallback.
    if let Some(os) = &request.os {
        disks.push(json!({
            "id": ROOT_DISK,
            "sizeBytes": os.root_gib * 1024 * 1024 * 1024,
            "baseImage": {"name": os.image.name, "generation": os.image.generation},
            "bootIndex": 0,
        }));
    }
    disks.extend(request.disks.iter().map(|d| {
        let mut disk = json!({"id": d.id, "sizeBytes": d.size_gib * 1024 * 1024 * 1024});
        if let Some((name, generation)) = &d.base_image {
            disk["baseImage"] = json!({"name": name, "generation": generation});
        }
        disk
    }));
    if !disks.is_empty() {
        spec["storageNode"] = storage_node.into();
        spec["disks"] = disks.into();
    }
    if !request.nics.is_empty() {
        spec["nics"] = request
            .nics
            .iter()
            .map(|nic| {
                let mut value = json!({"name": nic.name});
                if !nic.security_groups.is_empty() {
                    value["securityGroups"] = json!(nic.security_groups);
                }
                if let Some(profile) = &nic.vpn_profile {
                    value["vpn"] = json!({"secretName": format!("vpn-{profile}")});
                }
                value
            })
            .collect();
    }
    if let Some(cloud_init) = &request.cloud_init {
        let mut value = json!({});
        if let Some(hostname) = &cloud_init.hostname {
            value["hostname"] = hostname.as_str().into();
        }
        if let Some(user_data) = &cloud_init.user_data {
            value["userData"] = user_data.as_str().into();
        }
        spec["cloudInit"] = value;
    }
    json!({
        "apiVersion": "qrab.yapplesauce.com/v1alpha1",
        "kind": "VirtualMachine",
        "metadata": {
            "name": names.vm,
            "namespace": namespace,
            "labels": {MANAGED_BY: "qrab-dashboard"},
            "annotations": {CREATOR: creator},
        },
        "spec": spec,
    })
}

/// What one launch did, for the result page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub created: Vec<String>,
    pub failed: Option<(String, String)>,
    pub rolled_back: Vec<String>,
    pub left: Vec<String>,
}

pub struct Launcher<'a> {
    pub cluster: &'a dyn Cluster,
    pub kanidm: &'a Kanidm,
    pub namespace: &'a str,
    pub vm_users_group: &'a str,
    pub storage_node: Option<&'a str>,
}

/// A PVC is reusable only if this dashboard created it and it never bound,
/// so it cannot hold another VM's TPM identity.
pub fn reusable_pvc(pvc: &PersistentVolumeClaim) -> bool {
    pvc.metadata
        .labels
        .as_ref()
        .and_then(|l| l.get(MANAGED_BY))
        .is_some_and(|v| v == MANAGER)
        && pvc.metadata.deletion_timestamp.is_none()
        && pvc.status.as_ref().and_then(|s| s.phase.as_deref()) == Some("Pending")
}

impl Launcher<'_> {
    /// Groups, then PVC, then VM. Nothing is created when a preflight check
    /// fails; groups created here are deleted again if a later step fails.
    pub async fn run(&self, request: &Request, subject: &str, username: &str) -> Report {
        let mut report = Report::default();
        let names = match names(self.namespace, &request.name) {
            Ok(n) => n,
            Err(e) => {
                report.failed = Some(("Validate".into(), e));
                return report;
            }
        };
        if let Err((step, error)) = self.preflight(&names).await {
            report.failed = Some((step, error));
            return report;
        }
        let mut groups: Vec<(String, Option<String>)> = Vec::new();
        let result = self
            .groups(&names, subject, username, &mut groups, &mut report)
            .await;
        let (admins, viewers) = match result {
            Ok(ids) => ids,
            Err(failure) => {
                report.failed = Some(failure);
                self.rollback(&groups, &mut report).await;
                return report;
            }
        };
        let pvc_name = format!("PersistentVolumeClaim {}/{}", self.namespace, names.pvc);
        // Checked again: the claim may have changed since the preflight.
        match self.cluster.get_pvc(&names.pvc).await {
            Ok(Some(pvc)) if reusable_pvc(&pvc) => report
                .created
                .push(format!("{pvc_name} (reused, never bound)")),
            Ok(Some(_)) => {
                report.failed = Some((
                    "Reuse the state PVC".into(),
                    format!(
                        "{pvc_name} appeared or bound during the launch and may hold another VM's identity."
                    ),
                ));
                self.rollback(&groups, &mut report).await;
                return report;
            }
            Ok(None) => match self
                .cluster
                .create_pvc(&render_pvc(&names, self.namespace))
                .await
            {
                Ok(()) => report.created.push(pvc_name.clone()),
                Err(e) => {
                    report.failed = Some(("Create the state PVC".into(), cluster::describe(&e)));
                    self.rollback(&groups, &mut report).await;
                    return report;
                }
            },
            Err(e) => {
                report.failed = Some(("Read the state PVC".into(), cluster::describe(&e)));
                self.rollback(&groups, &mut report).await;
                return report;
            }
        }
        let vm = render_vm(
            request,
            &names,
            self.namespace,
            subject,
            &Groups {
                admins: &admins,
                viewers: &viewers,
            },
            self.storage_node,
        );
        match self.cluster.create_vm(&vm).await {
            Ok(()) => report
                .created
                .push(format!("VirtualMachine {}/{}", self.namespace, names.vm)),
            Err(e) => {
                report.failed = Some(("Create the VirtualMachine".into(), cluster::describe(&e)));
                self.rollback(&groups, &mut report).await;
                report.left.push(format!(
                    "{pvc_name}: kept (the dashboard cannot delete PVCs); a retry with the same name reuses it while it is unbound"
                ));
            }
        }
        report
    }

    async fn preflight(&self, names: &Names) -> Result<(), (String, String)> {
        let step = || "Check that the name is free".to_owned();
        match self.cluster.get_vm(&names.vm).await {
            Ok(None) => {}
            Ok(Some(_)) => {
                return Err((
                    step(),
                    format!("VirtualMachine {} already exists.", names.vm),
                ));
            }
            Err(e) => return Err((step(), cluster::describe(&e))),
        }
        match self.cluster.get_pvc(&names.pvc).await {
            Ok(None) => {}
            Ok(Some(pvc)) if reusable_pvc(&pvc) => {}
            Ok(Some(_)) => {
                return Err((
                    step(),
                    format!(
                        "PersistentVolumeClaim {} already exists and may hold another VM's identity.",
                        names.pvc
                    ),
                ));
            }
            Err(e) => return Err((step(), cluster::describe(&e))),
        }
        for group in [&names.admins, &names.viewers] {
            match self.kanidm.group_uuid(group).await {
                Ok(None) => {}
                Ok(Some(_)) => {
                    return Err((
                        step(),
                        format!(
                            "Kanidm group {group} already exists; it is never adopted, because its members would gain access."
                        ),
                    ));
                }
                Err(e) => return Err((step(), format!("Kanidm: {e}"))),
            }
        }
        Ok(())
    }

    async fn groups(
        &self,
        names: &Names,
        subject: &str,
        username: &str,
        created: &mut Vec<(String, Option<String>)>,
        report: &mut Report,
    ) -> Result<(String, String), (String, String)> {
        let vm = format!("{}/{}", self.namespace, names.vm);
        let mut ids = Vec::new();
        for (group, role) in [(&names.admins, "admins"), (&names.viewers, "viewers")] {
            let description =
                format!("qrab VM {vm} {role}, launched by {username} through qrab-dashboard");
            self.kanidm
                .create_group(group, &description)
                .await
                .map_err(|e| (format!("Create Kanidm group {group}"), e.to_string()))?;
            // From here the group exists: it is rolled back by name if its
            // UUID cannot be read.
            created.push((group.clone(), None));
            let mut lookup = self.kanidm.group_uuid(group).await;
            if !matches!(lookup, Ok(Some(_))) {
                lookup = self.kanidm.group_uuid(group).await;
            }
            let id = match lookup {
                Ok(Some(id)) => id,
                Ok(None) => {
                    return Err((
                        format!("Read Kanidm group {group}"),
                        "the group was created but cannot be found".into(),
                    ));
                }
                Err(e) => return Err((format!("Read Kanidm group {group}"), e.to_string())),
            };
            report.created.push(format!("Kanidm group {group} ({id})"));
            if let Some(last) = created.last_mut() {
                last.1 = Some(id.clone());
            }
            ids.push(id);
        }
        for ((group, _), id) in created.iter().zip(&ids) {
            self.kanidm
                .add_member(self.vm_users_group, id)
                .await
                .map_err(|e| (format!("Nest {group} in qrab_vm_users"), e.to_string()))?;
        }
        self.kanidm
            .add_member(&ids[0], subject)
            .await
            .map_err(|e| (format!("Add you to {}", names.admins), e.to_string()))?;
        Ok((ids[0].clone(), ids[1].clone()))
    }

    /// Deletes the groups this launch created, by UUID or else by name.
    async fn rollback(&self, groups: &[(String, Option<String>)], report: &mut Report) {
        for (name, id) in groups {
            match self
                .kanidm
                .delete_group(id.as_deref().unwrap_or(name))
                .await
            {
                Ok(()) => report
                    .rolled_back
                    .push(format!("Kanidm group {name} deleted")),
                Err(e) => report
                    .left
                    .push(format!("Kanidm group {name}: rollback failed: {e}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn form(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn form_errors_target_the_fields_without_changing_validation_messages() {
        let fields = form(&[
            ("name", "web"),
            ("cpus", "4"),
            ("max_cpus", "2"),
            ("memory_mib", "512"),
            ("disk0_id", "data"),
            ("disk0_size_gib", "oops"),
        ]);
        let errors = parse_form(&fields, "qrab", Some("kube03"), &[], &[]).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.field.as_deref() == Some("max_cpus"))
        );
        assert!(
            errors
                .iter()
                .any(|e| e.field.as_deref() == Some("disk0_size_gib"))
        );
        assert_eq!(
            parse(&fields, "qrab", Some("kube03"), &[], &[]).unwrap_err(),
            errors.into_iter().map(|e| e.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn names_follow_the_kanidm_convention_and_limits() {
        let n = names("qrab", "web-1").unwrap();
        assert_eq!(
            n,
            Names {
                vm: "web-1".into(),
                pvc: "web-1-state".into(),
                admins: "qrab_vm_qrab_web-1_admins".into(),
                viewers: "qrab_vm_qrab_web-1_viewers".into(),
            }
        );
        // 8 + 4 + 1 + name + 8 <= 64 → name <= 43 in namespace "qrab".
        assert_eq!(max_name_length("qrab"), 43);
        assert_eq!(max_name_length("q"), 46);
        assert_eq!(max_name_length(&"a".repeat(63)), 0);
        assert!(names("qrab", &"a".repeat(43)).is_ok());
        assert!(names("qrab", &"a".repeat(44)).unwrap_err().contains("64"));
        assert!(names("q", &"a".repeat(46)).is_ok());
        assert!(names("q", &"a".repeat(47)).is_err());
        assert!(
            names("q", &"a".repeat(51))
                .unwrap_err()
                .contains("at most 50")
        );
        for bad in ["", "Web", "web_1", "-web", "web-", "a.b", "web/x"] {
            assert!(names("qrab", bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_form_is_validated_against_crd_limits() {
        let ok = parse(
            &form(&[
                ("name", "web"),
                ("cpus", "2"),
                ("max_cpus", "4"),
                ("memory_mib", "1024"),
                ("max_memory_mib", ""),
                ("accelerator", "kvm"),
                ("redfish", "on"),
                ("disk0_id", "data"),
                ("disk0_size_gib", "10"),
                ("disk1_id", "root"),
                ("disk1_size_gib", "20"),
                ("disk1_base_image", "debian-13"),
                ("disk1_base_generation", "3"),
                ("disk2_id", ""),
                ("disk2_size_gib", ""),
            ]),
            "qrab",
            Some("kube03"),
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(ok.cpus, 2);
        assert_eq!(ok.max_cpus, Some(4));
        assert_eq!(ok.max_memory_mib, None);
        assert!(ok.kvm);
        assert_eq!(ok.disks[1].base_image, Some(("debian-13".into(), 3)));
        for (pairs, needle) in [
            (
                vec![("name", "web"), ("cpus", "0"), ("memory_mib", "512")],
                "vCPUs",
            ),
            (
                vec![
                    ("name", "web"),
                    ("cpus", "4"),
                    ("max_cpus", "2"),
                    ("memory_mib", "512"),
                ],
                "at least vCPUs",
            ),
            (
                vec![("name", "web"), ("cpus", "1"), ("memory_mib", "128")],
                "Memory",
            ),
            (
                vec![
                    ("name", "web"),
                    ("cpus", "1"),
                    ("memory_mib", "512"),
                    ("max_memory_mib", "256"),
                ],
                "at least the memory",
            ),
            (
                vec![
                    ("name", "web"),
                    ("cpus", "1"),
                    ("memory_mib", "512"),
                    ("accelerator", "hvf"),
                ],
                "Accelerator",
            ),
            (
                vec![
                    ("name", "web"),
                    ("cpus", "1"),
                    ("memory_mib", "512"),
                    ("disk0_size_gib", "1"),
                ],
                "needs an ID",
            ),
            (
                vec![
                    ("name", "web"),
                    ("cpus", "1"),
                    ("memory_mib", "512"),
                    ("disk0_id", "a"),
                    ("disk0_size_gib", "2000"),
                ],
                "size",
            ),
            (
                vec![
                    ("name", "web"),
                    ("cpus", "1"),
                    ("memory_mib", "512"),
                    ("disk0_id", "a"),
                    ("disk0_size_gib", "1"),
                    ("disk1_id", "a"),
                    ("disk1_size_gib", "1"),
                ],
                "used twice",
            ),
            (
                vec![
                    ("name", "web"),
                    ("cpus", "1"),
                    ("memory_mib", "512"),
                    ("disk0_id", "a"),
                    ("disk0_size_gib", "1"),
                    ("disk0_base_image", "x"),
                ],
                "generation",
            ),
            (
                vec![("name", "Web"), ("cpus", "1"), ("memory_mib", "512")],
                "DNS label",
            ),
        ] {
            let errors = parse(&form(&pairs), "qrab", Some("kube03"), &[], &[]).unwrap_err();
            assert!(
                errors.iter().any(|e| e.contains(needle)),
                "{needle}: {errors:?}"
            );
        }
        let errors = parse(
            &form(&[
                ("name", "web"),
                ("cpus", "1"),
                ("memory_mib", "512"),
                ("disk0_id", "a"),
                ("disk0_size_gib", "1"),
            ]),
            "qrab",
            None,
            &[],
            &[],
        )
        .unwrap_err();
        assert!(errors[0].contains("storage node"));
    }

    #[test]
    fn rendered_resources_match_the_launch() {
        let request = parse(
            &form(&[
                ("name", "web"),
                ("cpus", "2"),
                ("memory_mib", "1024"),
                ("max_memory_mib", "4096"),
                ("accelerator", "tcg"),
                ("redfish", "on"),
                ("disk0_id", "data"),
                ("disk0_size_gib", "2"),
                ("disk0_base_image", "debian"),
                ("disk0_base_generation", "1"),
            ]),
            "qrab",
            Some("kube03"),
            &[],
            &[],
        )
        .unwrap();
        let n = names("qrab", "web").unwrap();
        let vm = render_vm(
            &request,
            &n,
            "qrab",
            "sub-1",
            &Groups {
                admins: "ga",
                viewers: "gv",
            },
            Some("kube03"),
        );
        assert_eq!(
            vm,
            json!({
                "apiVersion": "qrab.yapplesauce.com/v1alpha1",
                "kind": "VirtualMachine",
                "metadata": {"name": "web", "namespace": "qrab",
                    "labels": {"app.kubernetes.io/managed-by": "qrab-dashboard"},
                    "annotations": {"qrab.yapplesauce.com/creator-subject": "sub-1"}},
                "spec": {
                    "stateVolumeClaim": "web-state", "memoryMiB": 1024, "maxMemoryMiB": 4096, "cpus": 2,
                    "running": true, "bootDisk": "/opt/qrab/demo/boot.img",
                    "console": {"adminGroups": ["ga"], "viewerGroups": ["gv"]},
                    "storageNode": "kube03",
                    "disks": [{"id": "data", "sizeBytes": 2147483648u64, "baseImage": {"name": "debian", "generation": 1}}]
                }
            })
        );
        // The form offers no Redfish options: the BMC's Redfish sidecar serves the dashboard.
        let plain = Request {
            disks: vec![],
            kvm: true,
            ..request
        };
        let vm = render_vm(
            &plain,
            &n,
            "qrab",
            "sub-1",
            &Groups {
                admins: "ga",
                viewers: "gv",
            },
            Some("kube03"),
        );
        assert_eq!(vm["spec"]["accelerator"], "kvm");
        for absent in ["components", "storageNode", "disks", "maxCpus"] {
            assert!(vm["spec"].get(absent).is_none(), "{absent}");
        }
        assert!(vm["spec"]["console"].get("publicUrl").is_none());
        let typed: qrab_operator::api::VirtualMachine = serde_json::from_value(vm).unwrap();
        assert!(typed.spec.console.unwrap().public_url.is_none());
        let pvc = serde_json::to_value(render_pvc(&n, "qrab")).unwrap();
        assert_eq!(pvc["metadata"]["name"], "web-state");
        assert_eq!(pvc["spec"]["storageClassName"], "zfs-block");
        assert_eq!(pvc["spec"]["accessModes"], json!(["ReadWriteOncePod"]));
        assert_eq!(pvc["spec"]["volumeMode"], "Filesystem");
        assert_eq!(pvc["spec"]["resources"]["requests"]["storage"], "1Gi");
    }

    fn os_form(extra: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut pairs = vec![("name", "web"), ("cpus", "1"), ("memory_mib", "512")];
        pairs.extend_from_slice(extra);
        form(&pairs)
    }

    #[test]
    fn an_os_choice_must_be_in_the_catalog_and_fit_its_root_disk() {
        use crate::images::tests::cirros;
        let catalog = [cirros()];
        let parsed = |extra: &[(&str, &str)], node: Option<&str>| {
            parse(&os_form(extra), "qrab", node, &catalog, &[])
        };
        // None keeps the demo-ESP launch.
        for none in [&[][..], &[("os", "none")][..], &[("os", "")][..]] {
            assert_eq!(parsed(none, Some("kube03")).unwrap().os, None);
        }
        let os = parsed(&[("os", "cirros:1")], Some("kube03"))
            .unwrap()
            .os
            .unwrap();
        assert_eq!((os.image.key(), os.root_gib), ("cirros:1".into(), 1));
        let os = parsed(
            &[("os", "cirros:1"), ("root_size_gib", "8")],
            Some("kube03"),
        )
        .unwrap()
        .os
        .unwrap();
        assert_eq!(os.root_gib, 8);
        for (extra, needle) in [
            (&[("os", "cirros:2")][..], "not in the base-image catalog"),
            (&[("os", "../etc")][..], "not in the base-image catalog"),
            (
                &[("os", "cirros:1"), ("root_size_gib", "0")][..],
                "Root disk size",
            ),
            (
                &[("os", "cirros:1"), ("root_size_gib", "1025")][..],
                "Root disk size",
            ),
            (
                &[
                    ("os", "cirros:1"),
                    ("disk0_id", "root"),
                    ("disk0_size_gib", "1"),
                ][..],
                "cannot be named root",
            ),
        ] {
            let errors = parsed(extra, Some("kube03")).unwrap_err();
            assert!(
                errors.iter().any(|e| e.contains(needle)),
                "{needle}: {errors:?}"
            );
        }
        // The minimum is the image size rounded up to whole GiB.
        let mut big = cirros();
        big.size_bytes = 5 * 1024 * 1024 * 1024 + 1;
        let errors = parse(
            &os_form(&[("os", "cirros:1"), ("root_size_gib", "5")]),
            "qrab",
            Some("kube03"),
            &[big.clone()],
            &[],
        )
        .unwrap_err();
        assert!(errors[0].contains("from 6 to 1024"), "{errors:?}");
        let os = parse(
            &os_form(&[("os", "cirros:1")]),
            "qrab",
            Some("kube03"),
            &[big],
            &[],
        )
        .unwrap()
        .os
        .unwrap();
        assert_eq!(os.root_gib, 6);
        // Without an OS, a data disk may be called root.
        assert!(
            parsed(
                &[("disk0_id", "root"), ("disk0_size_gib", "1")],
                Some("kube03")
            )
            .is_ok()
        );
        let errors = parsed(&[("os", "cirros:1")], None).unwrap_err();
        assert!(errors[0].contains("storage node"), "{errors:?}");
    }

    #[test]
    fn an_os_launch_clones_the_image_as_the_first_boot_disk() {
        use crate::images::tests::cirros;
        let request = parse(
            &os_form(&[
                ("os", "cirros:1"),
                ("root_size_gib", "2"),
                ("disk0_id", "data"),
                ("disk0_size_gib", "3"),
            ]),
            "qrab",
            Some("kube03"),
            &[cirros()],
            &[],
        )
        .unwrap();
        let n = names("qrab", "web").unwrap();
        let vm = render_vm(
            &request,
            &n,
            "qrab",
            "sub-1",
            &Groups {
                admins: "ga",
                viewers: "gv",
            },
            Some("kube03"),
        );
        assert_eq!(vm["spec"]["storageNode"], "kube03");
        assert_eq!(
            vm["spec"]["disks"],
            json!([
                {"id": "root", "sizeBytes": 2147483648u64, "baseImage": {"name": "cirros", "generation": 1}, "bootIndex": 0},
                {"id": "data", "sizeBytes": 3221225472u64}
            ])
        );
        assert_eq!(vm["spec"]["bootDisk"], BOOT_DISK);
        assert_eq!(vm["metadata"]["labels"][MANAGED_BY], MANAGER);
        assert_eq!(vm["metadata"]["annotations"][CREATOR], "sub-1");
        assert_eq!(
            vm["spec"]["console"],
            json!({"adminGroups": ["ga"], "viewerGroups": ["gv"]})
        );
        let typed: qrab_operator::api::VirtualMachine = serde_json::from_value(vm).unwrap();
        assert_eq!(typed.spec.disks[0].boot_index, Some(0));
    }

    fn rendered(extra: &[(&str, &str)], catalog: &[BaseImage]) -> serde_json::Value {
        let request = parse(&os_form(extra), "qrab", Some("kube03"), catalog, &[]).unwrap();
        render_vm(
            &request,
            &names("qrab", "web").unwrap(),
            "qrab",
            "sub-1",
            &Groups {
                admins: "ga",
                viewers: "gv",
            },
            Some("kube03"),
        )
    }

    #[test]
    fn network_and_cloud_init_render_only_when_requested() {
        use crate::images::tests::cirros;
        // Neither: no nics, no cloudInit.
        let vm = rendered(&[], &[]);
        assert!(vm["spec"].get("nics").is_none());
        assert!(vm["spec"].get("cloudInit").is_none());
        // eth0 with groups, hostname and user-data (CRLF from the browser).
        let vm = rendered(
            &[
                ("nic0_enabled", "on"),
                ("nic0_security_groups", " web, ssh ,,"),
                ("hostname", "web-1"),
                ("user_data", "#cloud-config\r\npackages: [htop]\r\n"),
            ],
            &[],
        );
        assert_eq!(
            vm["spec"]["nics"],
            json!([{"name": "eth0", "securityGroups": ["web", "ssh"]}])
        );
        assert_eq!(
            vm["spec"]["cloudInit"],
            json!({"hostname": "web-1", "userData": "#cloud-config\npackages: [htop]\n"})
        );
        let typed: qrab_operator::api::VirtualMachine = serde_json::from_value(vm).unwrap();
        assert_eq!(typed.spec.nics[0].security_groups.len(), 2);
        // A NIC without groups, and no cloud-init without an OS.
        let vm = rendered(&[("nic0_enabled", "on"), ("user_data", "  \r\n")], &[]);
        assert_eq!(vm["spec"]["nics"], json!([{"name": "eth0"}]));
        assert!(vm["spec"].get("cloudInit").is_none());
        // An OS image always gets a seed; the hostname defaults to the VM name.
        let vm = rendered(&[("os", "cirros:1")], &[cirros()]);
        assert_eq!(vm["spec"]["cloudInit"], json!({}));
        assert!(vm["spec"].get("nics").is_none());
        let typed: qrab_operator::api::VirtualMachine = serde_json::from_value(vm).unwrap();
        assert_eq!(typed.spec.cloud_init, Some(Default::default()));
    }

    #[test]
    fn a_vpn_nic_references_its_profile_secret_and_joins_the_egress_group() {
        let vpn = [
            VpnChoice {
                name: "ch-p2p".into(),
                summary: "CH#12".into(),
                used_by: None,
            },
            VpnChoice {
                name: "us".into(),
                summary: "US-GA#442".into(),
                used_by: Some("web/eth0".into()),
            },
        ];
        let request = parse(
            &os_form(&[
                ("nic0_enabled", "on"),
                ("nic0_network", "vpn:ch-p2p"),
                ("nic0_security_groups", "ssh"),
                ("nic1_enabled", "on"),
                ("nic1_network", "direct"),
            ]),
            "qrab",
            Some("kube03"),
            &[],
            &vpn,
        )
        .unwrap();
        assert_eq!(request.nics.len(), 2);
        let vm = render_vm(
            &request,
            &names("qrab", "web").unwrap(),
            "qrab",
            "subject",
            &Groups {
                admins: "a",
                viewers: "v",
            },
            Some("kube03"),
        );
        assert_eq!(
            vm["spec"]["nics"],
            json!([
                {"name": "eth0", "securityGroups": ["ssh", "vpn-egress"], "vpn": {"secretName": "vpn-ch-p2p"}},
                {"name": "eth1"},
            ])
        );
        let typed: qrab_operator::api::VirtualMachine = serde_json::from_value(vm).unwrap();
        assert!(typed.spec.nics[0].vpn.is_some());
        // vpn-egress given explicitly is not repeated.
        let request = parse(
            &os_form(&[
                ("nic0_enabled", "on"),
                ("nic0_network", "vpn:ch-p2p"),
                ("nic0_security_groups", "vpn-egress"),
            ]),
            "qrab",
            Some("kube03"),
            &[],
            &vpn,
        )
        .unwrap();
        assert_eq!(request.nics[0].security_groups, ["vpn-egress"]);
        for (extra, field, needle) in [
            (
                vec![("nic0_enabled", "on"), ("nic0_network", "vpn:gone")],
                "nic0_network",
                "does not exist",
            ),
            (
                vec![("nic0_enabled", "on"), ("nic0_network", "vpn:us")],
                "nic0_network",
                "already used by web/eth0",
            ),
            (
                vec![
                    ("nic0_enabled", "on"),
                    ("nic0_network", "vpn:ch-p2p"),
                    ("nic1_enabled", "on"),
                    ("nic1_network", "vpn:ch-p2p"),
                ],
                "nic1_network",
                "another interface of this VM",
            ),
            (
                vec![
                    ("nic0_enabled", "on"),
                    ("nic0_network", "vpn:ch-p2p"),
                    ("nic0_security_groups", "a,b,c,d,e,f,g,h"),
                ],
                "nic0_security_groups",
                "at most 7 other",
            ),
            (
                vec![("nic1_network", "vpn:ch-p2p")],
                "nic1_security_groups",
                "attach eth1 or clear",
            ),
        ] {
            let errors =
                parse_form(&os_form(&extra), "qrab", Some("kube03"), &[], &vpn).unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|e| e.field.as_deref() == Some(field) && e.message.contains(needle)),
                "{needle}: {errors:?}"
            );
        }
        assert_eq!(vpn_profile_of("vpn-ch-p2p"), Some("ch-p2p"));
        assert_eq!(vpn_profile_of("vpn-"), None);
        assert_eq!(vpn_profile_of("other"), None);
    }

    #[test]
    fn network_and_cloud_init_are_validated_without_quoting_user_data() {
        let secret = "#cloud-config\npassword: hunter2-secret\n";
        let too_big = format!("{secret}{}", "x".repeat(USER_DATA_LIMIT));
        let nine = "a,b,c,d,e,f,g,h,i";
        let long_host = "a".repeat(64);
        for (extra, needle) in [
            (
                vec![("nic0_enabled", "on"), ("nic0_security_groups", "Web")],
                "Security group Web on eth0 must be a DNS label",
            ),
            (
                vec![("nic0_enabled", "on"), ("nic0_security_groups", "web,web")],
                "listed twice",
            ),
            (
                vec![("nic0_enabled", "on"), ("nic0_security_groups", nine)],
                "at most 8 security groups",
            ),
            (
                vec![("nic0_security_groups", "web")],
                "attach eth0 or clear",
            ),
            (vec![("hostname", "web_1")], "Hostname must be a DNS label"),
            (vec![("hostname", long_host.as_str())], "Hostname"),
            (vec![("user_data", too_big.as_str())], "at most 32768 bytes"),
        ] {
            let errors = parse(&os_form(&extra), "qrab", Some("kube03"), &[], &[]).unwrap_err();
            assert!(
                errors.iter().any(|e| e.contains(needle)),
                "{needle}: {errors:?}"
            );
            assert!(!errors.iter().any(|e| e.contains("hunter2")), "{errors:?}");
        }
        // Eight groups and exactly 32 KiB are accepted.
        let groups = "a,b,c,d,e,f,g,h";
        let limit = "x".repeat(USER_DATA_LIMIT);
        let request = parse(
            &os_form(&[
                ("nic0_enabled", "on"),
                ("nic0_security_groups", groups),
                ("user_data", &limit),
            ]),
            "qrab",
            Some("kube03"),
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(request.nics[0].security_groups.len(), 8);
        // Debug output, which a log line could carry, shows only the size.
        let request = parse(
            &os_form(&[("user_data", secret)]),
            "qrab",
            Some("kube03"),
            &[],
            &[],
        )
        .unwrap();
        let debug = format!("{request:?}");
        assert!(!debug.contains("hunter2") && debug.contains("user_data_bytes: Some(39)"));
    }

    #[test]
    fn only_unbound_dashboard_pvcs_are_reused() {
        let pvc = |labelled: bool, phase: &str| -> PersistentVolumeClaim {
            let mut v =
                serde_json::to_value(render_pvc(&names("qrab", "web").unwrap(), "qrab")).unwrap();
            if !labelled {
                v["metadata"]["labels"] = json!({});
            }
            v["status"] = json!({"phase": phase});
            serde_json::from_value(v).unwrap()
        };
        assert!(reusable_pvc(&pvc(true, "Pending")));
        assert!(!reusable_pvc(&pvc(true, "Bound")));
        assert!(!reusable_pvc(&pvc(false, "Pending")));
    }
}
