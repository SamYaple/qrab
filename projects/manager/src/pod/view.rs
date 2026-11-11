//! Read-only VM configuration views. Sensitive fields are `Option`s that the
//! redacted view clears, so a viewer response cannot carry them by accident.
use super::{Accelerator, Config, DiskFormat, IscsiBus};
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
    /// Always false: QEMU starts with `-nic none`.
    pub guest_network: bool,
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
            guest_network: false,
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
        }
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
            redacted: true,
            state_id: None,
            kernel_args: None,
            paths: None,
            deployment: None,
            ..self.clone()
        }
    }
}
