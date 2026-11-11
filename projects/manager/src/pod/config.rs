use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
};

fn binary() -> PathBuf {
    "/opt/qemu/bin/qemu-system-x86_64".into()
}
fn name() -> String {
    "vm".into()
}
fn memory() -> u64 {
    256
}
fn cpus() -> u32 {
    1
}
fn runtime() -> PathBuf {
    "/run/qrab".into()
}
fn startup() -> u64 {
    30
}
fn grace() -> u64 {
    5
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Accelerator {
    #[default]
    Tcg,
    Kvm,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiskFormat {
    #[default]
    Raw,
    Qcow2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Disk {
    pub path: PathBuf,
    #[serde(default)]
    pub format: DiskFormat,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "name")]
    pub name: String,
    #[serde(default = "binary")]
    pub qemu_binary: PathBuf,
    #[serde(default = "memory")]
    pub memory_mib: u64,
    #[serde(default = "cpus")]
    pub cpus: u32,
    #[serde(default)]
    pub accelerator: Accelerator,
    pub kernel: PathBuf,
    #[serde(default)]
    pub initrd: Option<PathBuf>,
    #[serde(default)]
    pub kernel_args: String,
    #[serde(default)]
    pub disk: Option<Disk>,
    #[serde(default)]
    pub firmware_dir: Option<PathBuf>,
    #[serde(default = "runtime")]
    pub runtime_dir: PathBuf,
    #[serde(default = "startup")]
    pub startup_timeout_secs: u64,
    #[serde(default = "grace")]
    pub shutdown_grace_secs: u64,
}

impl Config {
    pub fn read(path: &Path) -> Result<Self> {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty()
            || self.name.len() > 63
            || !self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        {
            return Err(Error::Config(
                "name must be 1..63 ASCII letters, digits, dots, hyphens or underscores".into(),
            ));
        }
        if !(64..=1_048_576).contains(&self.memory_mib)
            || !(1..=256).contains(&self.cpus)
            || !(1..=300).contains(&self.startup_timeout_secs)
            || self.shutdown_grace_secs > 120
        {
            return Err(Error::Config(
                "invalid memory, CPU count or lifecycle timeout".into(),
            ));
        }
        for path in [&self.qemu_binary, &self.kernel]
            .into_iter()
            .chain(self.initrd.iter())
        {
            if !path.is_absolute() || !path.is_file() {
                return Err(Error::Config(format!(
                    "required absolute file is missing: {}",
                    path.display()
                )));
            }
        }
        if !self.runtime_dir.is_absolute() {
            return Err(Error::Config("runtime_dir must be absolute".into()));
        }
        if let Some(path) = &self.firmware_dir {
            if !path.is_absolute() || !path.is_dir() {
                return Err(Error::Config(
                    "firmware_dir must be an existing absolute directory".into(),
                ));
            }
        }
        if let Some(disk) = &self.disk {
            let kind = std::fs::metadata(&disk.path)?.file_type();
            if !disk.path.is_absolute() || !(kind.is_file() || kind.is_block_device()) {
                return Err(Error::Config(
                    "disk must name an absolute regular file or block device".into(),
                ));
            }
        }
        if self.accelerator == Accelerator::Kvm {
            OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/kvm")
                .map_err(|e| Error::Config(format!("KVM requires access to /dev/kvm: {e}")))?;
        }
        Ok(())
    }

    /// JSON blockdev options keep filenames with commas or spaces unambiguous.
    pub fn arguments(&self, socket: &Path) -> Vec<String> {
        let mut args: Vec<String> = [
            "-name",
            &self.name,
            "-machine",
            "q35",
            "-m",
            &self.memory_mib.to_string(),
            "-smp",
            &self.cpus.to_string(),
            "-accel",
            match self.accelerator {
                Accelerator::Tcg => "tcg,thread=multi",
                Accelerator::Kvm => "kvm",
            },
            "-cpu",
            match self.accelerator {
                Accelerator::Tcg => "max",
                Accelerator::Kvm => "host",
            },
            "-nodefaults",
            "-display",
            "none",
            "-nic",
            "none",
            "-S",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        args.extend([
            "-chardev".into(),
            "stdio,id=serial,signal=off".into(),
            "-serial".into(),
            "chardev:serial".into(),
            "-chardev".into(),
            format!(
                "socket,id=qmp,path={},server=on,wait=off",
                socket.to_string_lossy().replace(',', ",,")
            ),
            "-object".into(),
            serde_json::json!({"qom-type":"monitor-qmp", "id":"monitor", "chardev":"qmp"})
                .to_string(),
            "-kernel".into(),
            self.kernel.to_string_lossy().into_owned(),
        ]);
        if let Some(initrd) = &self.initrd {
            args.extend(["-initrd".into(), initrd.to_string_lossy().into_owned()]);
        }
        if !self.kernel_args.is_empty() {
            args.extend(["-append".into(), self.kernel_args.clone()]);
        }
        if let Some(path) = &self.firmware_dir {
            args.extend(["-L".into(), path.to_string_lossy().into_owned()]);
        }
        if let Some(disk) = &self.disk {
            args.extend(["-blockdev".into(), serde_json::json!({"driver":"file", "filename":disk.path, "node-name":"disk-file", "read-only":disk.read_only}).to_string(),
                "-blockdev".into(), serde_json::json!({"driver":disk.format, "file":"disk-file", "node-name":"disk", "read-only":disk.read_only}).to_string(),
                "-device".into(), "virtio-blk-pci,drive=disk".into()]);
        }
        args
    }
}
