//! Read-only VM configuration views. Sensitive fields are `Option`s that the
//! redacted view clears, so a viewer response cannot carry them by accident.
use super::{Accelerator, Config, DiskFormat, GuestSize, IscsiBus, Resize, ResizePhase};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEPLOYMENT_FIELD_LIMIT: usize = 512;

/// Where this VM instance runs, as reported by its pod environment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pod: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

impl Deployment {
    /// Reads `QRAB_NODE_NAME`, `QRAB_POD_NAME`, `QRAB_POD_NAMESPACE` and
    /// `QRAB_IMAGE`. Empty, oversized or non-printable values are ignored.
    pub fn from_env() -> Self {
        let read = |name| {
            std::env::var(name).ok().filter(|v: &String| {
                !v.is_empty()
                    && v.len() <= DEPLOYMENT_FIELD_LIMIT
                    && v.bytes().all(|b| b.is_ascii_graphic())
            })
        };
        Self {
            node: read("QRAB_NODE_NAME"),
            pod: read("QRAB_POD_NAME"),
            namespace: read("QRAB_POD_NAMESPACE"),
            image: read("QRAB_IMAGE"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootMode {
    Uefi,
    Kernel,
}

/// Admin-only locations of host/container files. Contents are never read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathView {
    pub qemu_binary: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initrd: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uefi_boot_disk: Option<PathBuf>,
    pub state_dir: PathBuf,
    pub runtime_dir: PathBuf,
}

/// Admin-only iSCSI binding. `password_file` is the Secret mount reference;
/// the secret itself is never read into a view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IscsiTargetView {
    pub portal: String,
    pub target: String,
    pub lun: u32,
    pub initiator_name: String,
    pub username: String,
    pub password_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiskView {
    File {
        format: DiskFormat,
        read_only: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<PathBuf>,
    },
    Iscsi {
        id: String,
        read_only: bool,
        bus: IscsiBus,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        boot_index: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<IscsiTargetView>,
    },
}

/// A guest NIC. The relay endpoint is an internal Pod address, admin-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NicView {
    pub id: String,
    pub mac: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

/// In-place growth ceilings and progress. Not sensitive: viewers see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotplugView {
    /// Boot-time maxima; growing past them needs a restart.
    pub max_cpus: u32,
    pub max_memory_mib: u64,
    /// Filled from the supervisor's last published status, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<ResizePhase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<GuestSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desired: Option<GuestSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// The effective VM configuration as shown to a BMC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigView {
    pub name: String,
    pub memory_mib: u64,
    pub cpus: u32,
    pub accelerator: Accelerator,
    pub boot: BootMode,
    pub console: bool,
    pub tpm: bool,
    /// True when the VM has guest NICs; without them QEMU runs `-nic none`.
    pub guest_network: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nics: Vec<NicView>,
    pub disks: Vec<DiskView>,
    pub startup_timeout_secs: u64,
    pub shutdown_grace_secs: u64,
    /// True when sensitive fields were removed.
    pub redacted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel_args: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<PathView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<Deployment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotplug: Option<HotplugView>,
}

impl ConfigView {
    pub fn new(config: &Config, deployment: &Deployment) -> Self {
        let mut disks: Vec<_> = config
            .disk
            .iter()
            .map(|disk| DiskView::File {
                format: disk.format,
                read_only: disk.read_only,
                path: Some(disk.path.clone()),
            })
            .collect();
        disks.extend(config.iscsi_disks.iter().map(|disk| DiskView::Iscsi {
            id: disk.id.clone(),
            read_only: disk.read_only,
            bus: disk.bus,
            boot_index: disk.boot_index,
            target: Some(IscsiTargetView {
                portal: disk.portal.clone(),
                target: disk.target.clone(),
                lun: disk.lun,
                initiator_name: disk.initiator_name.clone(),
                username: disk.username.clone(),
                password_file: disk.password_file.clone(),
            }),
        }));
        Self {
            name: config.name.clone(),
            memory_mib: config.memory_mib,
            cpus: config.cpus,
            accelerator: config.accelerator,
            boot: if config.uefi.is_some() {
                BootMode::Uefi
            } else {
                BootMode::Kernel
            },
            console: config.console,
            tpm: config.tpm.is_some(),
            guest_network: !config.nics.is_empty(),
            nics: config
                .nics
                .iter()
                .map(|nic| NicView {
                    id: nic.id.clone(),
                    mac: nic.mac.clone(),
                    endpoint: Some(
                        std::net::SocketAddr::new(
                            nic.host
                                .parse()
                                .unwrap_or(std::net::Ipv4Addr::UNSPECIFIED.into()),
                            nic.port,
                        )
                        .to_string(),
                    ),
                })
                .collect(),
            disks,
            startup_timeout_secs: config.startup_timeout_secs,
            shutdown_grace_secs: config.shutdown_grace_secs,
            redacted: false,
            state_id: Some(
                config
                    .state_id
                    .clone()
                    .unwrap_or_else(|| config.name.clone()),
            ),
            kernel_args: (!config.kernel_args.is_empty()).then(|| config.kernel_args.clone()),
            paths: Some(PathView {
                qemu_binary: config.qemu_binary.clone(),
                kernel: config.kernel.clone(),
                initrd: config.initrd.clone(),
                uefi_boot_disk: config.uefi.as_ref().map(|uefi| uefi.boot_disk.clone()),
                state_dir: config.state_dir.clone(),
                runtime_dir: config.runtime_dir.clone(),
            }),
            deployment: Some(deployment.clone()),
            hotplug: Resize::new(config).map(|resize| HotplugView {
                max_cpus: resize.maximum.cpus,
                max_memory_mib: resize.maximum.memory_mib,
                phase: None,
                current: None,
                desired: None,
                last_error: None,
            }),
        }
    }

    /// Add resize progress from a published status record.
    pub fn with_resize(mut self, resize: Option<&Resize>) -> Self {
        if let (Some(view), Some(resize)) = (self.hotplug.as_mut(), resize) {
            view.phase = Some(resize.phase);
            view.current = Some(resize.current);
            view.desired = resize.desired;
            view.last_error = resize.last_error.clone();
        }
        self
    }

    /// Remove storage endpoints, credentials references, identity, paths,
    /// node and image references. Shape, sizing and device kinds remain.
    pub fn redacted(&self) -> Self {
        Self {
            disks: self
                .disks
                .iter()
                .map(|disk| match disk {
                    DiskView::File {
                        format, read_only, ..
                    } => DiskView::File {
                        format: *format,
                        read_only: *read_only,
                        path: None,
                    },
                    DiskView::Iscsi {
                        id,
                        read_only,
                        bus,
                        boot_index,
                        ..
                    } => DiskView::Iscsi {
                        id: id.clone(),
                        read_only: *read_only,
                        bus: *bus,
                        boot_index: *boot_index,
                        target: None,
                    },
                })
                .collect(),
            nics: self
                .nics
                .iter()
                .map(|nic| NicView {
                    endpoint: None,
                    ..nic.clone()
                })
                .collect(),
            redacted: true,
            state_id: None,
            kernel_args: None,
            paths: None,
            deployment: None,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nics_show_id_and_mac_to_viewers_and_the_endpoint_to_admins() {
        let config: Config = serde_json::from_value(serde_json::json!({
            "kernel": "/unused",
            "nics": [{"id": "lan", "mac": "02:aa:bb:cc:dd:ee", "host": "fd00::7", "port": 7700}]
        }))
        .unwrap();
        let view = ConfigView::new(&config, &Deployment::default());
        assert!(view.guest_network);
        assert_eq!(view.nics[0].endpoint.as_deref(), Some("[fd00::7]:7700"));
        let viewer = view.redacted();
        assert!(viewer.guest_network);
        assert_eq!(
            viewer.nics,
            [NicView {
                id: "lan".into(),
                mac: "02:aa:bb:cc:dd:ee".into(),
                endpoint: None
            }]
        );
        let none: Config =
            serde_json::from_value(serde_json::json!({"kernel": "/unused"})).unwrap();
        let view = ConfigView::new(&none, &Deployment::default());
        assert!(!view.guest_network && view.nics.is_empty());
        assert!(serde_json::to_value(&view).unwrap().get("nics").is_none());
    }

    #[test]
    fn hotplug_ceilings_and_progress_survive_redaction() {
        let config: Config = serde_json::from_value(serde_json::json!({
            "kernel": "/unused", "cpus": 1, "memory_mib": 512,
            "hotplug": {"max_cpus": 4, "max_memory_mib": 2048,
                "cpu_overhead_millis": 1000, "memory_overhead_mib": 512}
        }))
        .unwrap();
        let view = ConfigView::new(&config, &Deployment::default());
        let hotplug = view.hotplug.clone().unwrap();
        assert_eq!((hotplug.max_cpus, hotplug.max_memory_mib), (4, 2048));
        assert_eq!(hotplug.phase, None);

        let mut resize = Resize::new(&config).unwrap();
        resize.phase = ResizePhase::Retrying;
        resize.last_error = Some("CPU hot-add refused".into());
        let viewer = view.with_resize(Some(&resize)).redacted();
        let hotplug = viewer.hotplug.unwrap();
        assert_eq!(hotplug.phase, Some(ResizePhase::Retrying));
        assert_eq!(hotplug.current, Some(resize.boot));
        assert_eq!(hotplug.last_error.as_deref(), Some("CPU hot-add refused"));

        let fixed: Config =
            serde_json::from_value(serde_json::json!({"kernel": "/unused"})).unwrap();
        assert!(ConfigView::new(&fixed, &Deployment::default())
            .hotplug
            .is_none());
    }
}
