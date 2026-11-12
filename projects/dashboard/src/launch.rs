//! The launch form: validation, the rendered Kanidm groups, PVC and
//! VirtualMachine, and the step-by-step creation with a report.
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
/// Marks resources the dashboard created; only such VMs can be deleted here.
pub use crate::access::{MANAGED_BY, MANAGER};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disk {
    pub id: String,
    pub size_gib: u64,
    pub base_image: Option<(String, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub name: String,
    pub cpus: u32,
    pub max_cpus: Option<u32>,
    pub memory_mib: u32,
    pub max_memory_mib: Option<u32>,
    pub kvm: bool,
    pub disks: Vec<Disk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Names {
    pub vm: String,
    pub pvc: String,
    pub admins: String,
    pub viewers: String,
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
    errors: &mut Vec<String>,
    label: &str,
    raw: &str,
    min: T,
    max: T,
) -> Option<T> {
    match raw.parse::<T>() {
        Ok(v) if v >= min && v <= max => Some(v),
        _ => {
            errors.push(format!(
                "{label} must be a whole number from {min} to {max}."
            ));
            None
        }
    }
}

/// Validates the form against the CRD's limits. Every problem is reported.
pub fn parse(
    fields: &[(String, String)],
    namespace: &str,
    storage_node: Option<&str>,
) -> Result<Request, Vec<String>> {
    let mut errors = Vec::new();
    let name = field(fields, "name").to_owned();
    if let Err(e) = names(namespace, &name) {
        errors.push(e);
    }
    let cpus = number(&mut errors, "vCPUs", field(fields, "cpus"), 1u32, 256);
    let max_cpus = match field(fields, "max_cpus") {
        "" => None,
        raw => number(&mut errors, "Maximum vCPUs", raw, 1u32, 256),
    };
    let memory_mib = number(
        &mut errors,
        "Memory",
        field(fields, "memory_mib"),
        256u32,
        1_048_576,
    );
    let max_memory_mib = match field(fields, "max_memory_mib") {
        "" => None,
        raw => number(&mut errors, "Maximum memory", raw, 256u32, 1_048_576),
    };
    if let (Some(c), Some(m)) = (cpus, max_cpus)
        && m < c
    {
        errors.push("Maximum vCPUs must be at least vCPUs.".into());
    }
    if let (Some(c), Some(m)) = (memory_mib, max_memory_mib)
        && m < c
    {
        errors.push("Maximum memory must be at least the memory size.".into());
    }
    let kvm = match field(fields, "accelerator") {
        "tcg" | "" => false,
        "kvm" => true,
        _ => {
            errors.push("Accelerator must be kvm or tcg.".into());
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
                errors.push(format!("Disk {} needs an ID.", i + 1));
            }
            continue;
        }
        let label = format!("Disk {}", i + 1);
        if !disk_id(id) {
            errors.push(format!("{label} ID must be 1-20 lowercase letters, digits or '-', starting and ending with a letter or digit."));
        } else if disks.iter().any(|d| d.id == id) {
            errors.push(format!("{label} ID {id} is used twice."));
        }
        let size_gib = number(
            &mut errors,
            &format!("{label} size (GiB)"),
            size,
            1u64,
            MAX_DISK_GIB,
        );
        let base_image = match (base, generation) {
            ("", "") => None,
            (name, raw) if image_name(name) => number(
                &mut errors,
                &format!("{label} base image generation"),
                raw,
                1u32,
                999_999,
            )
            .map(|g| (name.to_owned(), g)),
            _ => {
                errors.push(format!(
                    "{label} base image name must be 1-32 lowercase letters, digits or '-'."
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
    match (errors.is_empty(), cpus, memory_mib) {
        (true, Some(cpus), Some(memory_mib)) => Ok(Request {
            name,
            cpus,
            max_cpus,
            memory_mib,
            max_memory_mib,
            kvm,
            disks,
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
    if !request.disks.is_empty() {
        spec["storageNode"] = storage_node.into();
        spec["disks"] = request
            .disks
            .iter()
            .map(|d| {
                let mut disk = json!({"id": d.id, "sizeBytes": d.size_gib * 1024 * 1024 * 1024});
                if let Some((name, generation)) = &d.base_image {
                    disk["baseImage"] = json!({"name": name, "generation": generation});
                }
                disk
            })
            .collect();
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
            let errors = parse(&form(&pairs), "qrab", Some("kube03")).unwrap_err();
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
        // The form offers no Redfish: nothing admits port 8081 for these VMs.
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
