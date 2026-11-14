use super::{IscsiDisk, Seed};
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
fn persistent_state() -> PathBuf {
    "/var/lib/qrab".into()
}
fn uefi_code() -> PathBuf {
    "/opt/qrab/firmware/OVMF_CODE_4M.fd".into()
}
fn uefi_vars() -> PathBuf {
    "/opt/qrab/firmware/OVMF_VARS_4M.fd".into()
}
fn uefi_boot_disk() -> PathBuf {
    "/opt/qrab/demo/boot.img".into()
}
fn enabled() -> bool {
    true
}
fn swtpm_binary() -> PathBuf {
    "/usr/bin/swtpm".into()
}
fn limits_dir() -> PathBuf {
    "/run/qrab-size".into()
}
fn cgroup_dir() -> PathBuf {
    "/sys/fs/cgroup".into()
}

/// virtio-mem plugs whole blocks; QEMU's x86 default and minimum is 2 MiB.
pub const HOTPLUG_BLOCK_MIB: u64 = 2;

/// Boot-time headroom for in-place growth. QEMU starts with `cpus` and
/// `memory_mib`; the supervisor grows the guest toward the container's
/// downward-API limits, never past what its cgroup already permits.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Hotplug {
    pub max_cpus: u32,
    pub max_memory_mib: u64,
    /// Downward-API files `cpu_limit_millis` and `memory_limit_mib` for this container.
    #[serde(default = "limits_dir")]
    pub limits_dir: PathBuf,
    #[serde(default = "cgroup_dir")]
    pub cgroup_dir: PathBuf,
    /// The container limit is the guest size plus these fixed overheads.
    pub cpu_overhead_millis: u64,
    pub memory_overhead_mib: u64,
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
pub struct Uefi {
    #[serde(default = "uefi_code")]
    pub code: PathBuf,
    #[serde(default = "uefi_vars")]
    pub vars_template: PathBuf,
    #[serde(default = "uefi_boot_disk")]
    pub boot_disk: PathBuf,
}

/// Most guest NICs one VM may attach.
pub const NIC_LIMIT: usize = 8;

/// A guest NIC whose frames travel over a QEMU `stream` netdev to a relay
/// (`qrab-manager nic`) in a separate network namespace. The guest traffic
/// never enters this VM's own namespace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Nic {
    /// DNS label; QEMU ids are `nic-<id>`.
    pub id: String,
    /// Unicast, locally administered guest MAC, e.g. `02:00:00:00:00:01`.
    pub mac: String,
    /// IP literal of the relay.
    pub host: String,
    pub port: u16,
}

impl Nic {
    /// The QEMU netdev and device id.
    pub fn qemu_id(&self) -> String {
        format!("nic-{}", self.id)
    }
}

fn dns_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

/// Parses `xx:xx:xx:xx:xx:xx` and requires a unicast, locally administered
/// address, so a guest MAC never collides with a vendor-assigned one.
pub fn parse_guest_mac(value: &str) -> Option<[u8; 6]> {
    let mut mac = [0u8; 6];
    let mut parts = value.split(':');
    for byte in &mut mac {
        let part = parts.next()?;
        if part.len() != 2 {
            return None;
        }
        *byte = u8::from_str_radix(part, 16).ok()?;
    }
    (parts.next().is_none() && mac[0] & 0b01 == 0 && mac[0] & 0b10 != 0).then_some(mac)
}

fn validate_nics(nics: &[Nic]) -> Result<()> {
    if nics.len() > NIC_LIMIT {
        return Err(Error::Config(format!(
            "at most {NIC_LIMIT} guest NICs are supported"
        )));
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut macs = std::collections::BTreeSet::new();
    for nic in nics {
        if !dns_label(&nic.id) || !ids.insert(nic.id.as_str()) {
            return Err(Error::Config("NIC ids must be unique DNS labels".into()));
        }
        if !parse_guest_mac(&nic.mac).is_some_and(|mac| macs.insert(mac)) {
            return Err(Error::Config(format!(
                "NIC {} needs a unique unicast, locally administered MAC",
                nic.id
            )));
        }
        let host: std::net::IpAddr = nic
            .host
            .parse()
            .map_err(|_| Error::Config(format!("NIC {} host must be an IP literal", nic.id)))?;
        if host.is_unspecified()
            || host.is_multicast()
            || matches!(host, std::net::IpAddr::V4(ip) if ip.is_broadcast())
            || nic.port == 0
        {
            return Err(Error::Config(format!(
                "NIC {} needs a unicast relay address and a non-zero port",
                nic.id
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tpm {
    #[serde(default = "swtpm_binary")]
    pub swtpm_binary: PathBuf,
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
    #[serde(default)]
    pub kernel: Option<PathBuf>,
    #[serde(default)]
    pub initrd: Option<PathBuf>,
    #[serde(default)]
    pub kernel_args: String,
    /// Enable a graphical console on a private Unix socket in the session directory.
    #[serde(default)]
    pub console: bool,
    /// Serve the typed control API on `{runtime_dir}/control/control.sock`
    /// for a sidecar sharing the runtime volume. See `pod::socket`.
    #[serde(default)]
    pub control_socket: bool,
    /// Attach the `org.qemu.guest_agent.0` virtio-serial port to a private
    /// Unix socket in the session directory. Harmless if no agent runs.
    #[serde(default = "enabled")]
    pub guest_agent: bool,
    /// Directory for the control socket, created by the manager. Defaults to
    /// `{runtime_dir}/control`; a separate volume shares only the socket.
    #[serde(default)]
    pub control_dir: Option<PathBuf>,
    /// In-memory status for the supervisor's control channel; never serialized.
    #[serde(skip)]
    pub(crate) status_sink: super::state::StatusSink,
    #[serde(default)]
    pub disk: Option<Disk>,
    #[serde(default)]
    pub iscsi_disks: Vec<IscsiDisk>,
    #[serde(default)]
    pub firmware_dir: Option<PathBuf>,
    #[serde(default)]
    pub uefi: Option<Uefi>,
    #[serde(default)]
    pub tpm: Option<Tpm>,
    #[serde(default = "persistent_state")]
    pub state_dir: PathBuf,
    /// Immutable VM identity (the Kubernetes CR UID in operator deployments).
    /// Standalone configurations default to the VM name.
    #[serde(default)]
    pub state_id: Option<String>,
    #[serde(default = "runtime")]
    pub runtime_dir: PathBuf,
    #[serde(default = "startup")]
    pub startup_timeout_secs: u64,
    #[serde(default = "grace")]
    pub shutdown_grace_secs: u64,
    #[serde(default)]
    pub hotplug: Option<Hotplug>,
    /// Guest NICs. Empty means QEMU runs with `-nic none`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nics: Vec<Nic>,
    /// cloud-init NoCloud files, attached as a read-only `cidata` disk.
    #[serde(default)]
    pub seed: Option<Seed>,
    /// Accept live migrations of this VM to another manager.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration: Option<super::migration::Outgoing>,
    /// Start as the target of a live migration instead of booting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incoming: Option<super::migration::Incoming>,
    /// Exact machine type to run, set from a migration offer; never serialized.
    #[serde(skip)]
    pub(crate) machine: Option<String>,
}

impl Config {
    /// Upper vCPU bound fixed at boot; equal to `cpus` without hot-plug.
    pub fn max_cpus(&self) -> u32 {
        self.hotplug
            .as_ref()
            .map_or(self.cpus, |h| h.max_cpus.max(self.cpus))
    }

    /// Size of the virtio-mem region above boot memory, in whole blocks.
    pub fn hotplug_memory_mib(&self) -> u64 {
        self.hotplug.as_ref().map_or(0, |h| {
            h.max_memory_mib
                .saturating_sub(self.memory_mib)
                .div_ceil(HOTPLUG_BLOCK_MIB)
                * HOTPLUG_BLOCK_MIB
        })
    }

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
        if let Some(hotplug) = &self.hotplug {
            if !(self.cpus..=256).contains(&hotplug.max_cpus)
                || !(self.memory_mib..=1_048_576).contains(&hotplug.max_memory_mib)
                || hotplug.cpu_overhead_millis > 64_000
                || hotplug.memory_overhead_mib > 65_536
                || !hotplug.limits_dir.is_absolute()
                || !hotplug.cgroup_dir.is_absolute()
            {
                return Err(Error::Config(
                    "hotplug maxima must cover the boot size within 256 vCPUs/1 TiB, with bounded overheads and absolute source directories".into(),
                ));
            }
        }
        if let Some(uefi) = &self.uefi {
            if self.kernel.is_some() || self.initrd.is_some() || !self.kernel_args.is_empty() {
                return Err(Error::Config(
                    "UEFI boot uses its EFI boot disk, not a direct kernel/initrd/command line"
                        .into(),
                ));
            }
            for path in [&uefi.code, &uefi.vars_template, &uefi.boot_disk] {
                required_file(path)?;
            }
            if std::fs::metadata(&uefi.vars_template)?.len() < 64 {
                return Err(Error::Config("UEFI variable template is empty".into()));
            }
        } else if self.kernel.is_none() {
            return Err(Error::Config(
                "select UEFI boot or supply a direct kernel".into(),
            ));
        }
        for path in std::iter::once(&self.qemu_binary)
            .chain(self.kernel.iter())
            .chain(self.initrd.iter())
        {
            required_file(path)?;
        }
        if !self.runtime_dir.is_absolute()
            || self
                .control_dir
                .as_ref()
                .is_some_and(|dir| !dir.is_absolute())
        {
            return Err(Error::Config(
                "runtime_dir and control_dir must be absolute".into(),
            ));
        }
        if self.uefi.is_some() || self.tpm.is_some() || !self.iscsi_disks.is_empty() {
            if !self.state_dir.is_absolute() || self.state_dir == self.runtime_dir {
                return Err(Error::Config(
                    "state_dir must be an absolute persistent directory separate from runtime_dir"
                        .into(),
                ));
            }
            let id = self.state_id.as_deref().unwrap_or(&self.name);
            if id.is_empty()
                || id.len() > 128
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            {
                return Err(Error::Config(
                    "state_id must be 1..128 ASCII letters, digits, dots, hyphens or underscores"
                        .into(),
                ));
            }
        }
        if let Some(tpm) = &self.tpm {
            required_file(&tpm.swtpm_binary)?;
            for path in [&self.state_dir, &self.runtime_dir] {
                if path.to_str().is_none_or(|path| path.contains(',')) {
                    return Err(Error::Config(
                        "swtpm state/runtime paths must be UTF-8 without commas".into(),
                    ));
                }
            }
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
        super::iscsi::validate(&self.iscsi_disks)?;
        validate_nics(&self.nics)?;
        if let Some(seed) = &self.seed {
            seed.validate()?;
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
        let hotplug_memory = self.hotplug_memory_mib();
        let memory = if hotplug_memory == 0 {
            self.memory_mib.to_string()
        } else {
            format!(
                "size={}M,maxmem={}M",
                self.memory_mib,
                self.memory_mib + hotplug_memory
            )
        };
        let smp = if self.max_cpus() > self.cpus {
            format!("cpus={},maxcpus={}", self.cpus, self.max_cpus())
        } else {
            self.cpus.to_string()
        };
        let mut machine = self.machine.clone().unwrap_or_else(|| "q35".into());
        if self.uefi.is_some() {
            machine.push_str(",pflash0=pflash-code,pflash1=pflash-vars");
        }
        let mut args: Vec<String> = [
            "-name",
            &self.name,
            "-machine",
            &machine,
            "-m",
            &memory,
            "-smp",
            &smp,
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
            "-S",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        if self.nics.is_empty() {
            args.extend(["-nic".into(), "none".into()]);
        }
        for nic in &self.nics {
            // QEMU reconnects on its own after the relay or its Pod restarts.
            // An empty romfile keeps iPXE out of the boot order.
            args.extend([
                "-netdev".into(),
                serde_json::json!({"type":"stream","id":nic.qemu_id(),"server":false,
                    "addr":{"type":"inet","host":nic.host,"port":nic.port.to_string()},
                    "reconnect-ms":1000})
                .to_string(),
                "-device".into(),
                format!(
                    "virtio-net-pci,id={0},netdev={0},mac={1},romfile=",
                    nic.qemu_id(),
                    nic.mac
                ),
            ]);
        }
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
        ]);
        if self.guest_agent {
            args.extend([
                "-device".into(),
                "virtio-serial-pci,id=qga-serial".into(),
                "-chardev".into(),
                format!(
                    "socket,id=qga0,path={},server=on,wait=off",
                    socket
                        .with_file_name("qga.sock")
                        .to_string_lossy()
                        .replace(',', ",,")
                ),
                "-device".into(),
                "virtserialport,bus=qga-serial.0,chardev=qga0,name=org.qemu.guest_agent.0".into(),
            ]);
        }
        if let Some(uefi) = &self.uefi {
            let vars = self.state_dir.join("identity/OVMF_VARS.fd");
            for (name, path, read_only) in [
                ("pflash-code", &uefi.code, true),
                ("pflash-vars", &vars, false),
            ] {
                args.extend([
                    "-blockdev".into(), serde_json::json!({"driver":"file", "filename":path, "node-name":format!("{name}-file"), "read-only":read_only}).to_string(),
                    "-blockdev".into(), serde_json::json!({"driver":"raw", "file":format!("{name}-file"), "node-name":name, "read-only":read_only}).to_string(),
                ]);
            }
            args.extend([
                "-blockdev".into(), serde_json::json!({"driver":"file", "filename":uefi.boot_disk, "node-name":"boot-file", "read-only":true}).to_string(),
                "-blockdev".into(), serde_json::json!({"driver":"raw", "file":"boot-file", "node-name":"boot", "read-only":true}).to_string(),
                "-device".into(), "virtio-blk-pci,drive=boot,bootindex=1".into(),
            ]);
        } else if let Some(kernel) = &self.kernel {
            args.extend(["-kernel".into(), kernel.to_string_lossy().into_owned()]);
        }
        if self.tpm.is_some() {
            args.extend([
                "-chardev".into(),
                format!(
                    "socket,id=tpm,path={}",
                    socket
                        .with_file_name("swtpm.sock")
                        .to_string_lossy()
                        .replace(',', ",,")
                ),
                "-tpmdev".into(),
                "emulator,id=tpm0,chardev=tpm".into(),
                "-device".into(),
                "tpm-tis,tpmdev=tpm0".into(),
            ]);
        }
        if let Some(initrd) = &self.initrd {
            args.extend(["-initrd".into(), initrd.to_string_lossy().into_owned()]);
        }
        if !self.kernel_args.is_empty() {
            args.extend(["-append".into(), self.kernel_args.clone()]);
        }
        if self.console {
            args.extend([
                "-device".into(),
                "VGA,vgamem_mb=16".into(),
                "-device".into(),
                "qemu-xhci,id=usb".into(),
                "-device".into(),
                "usb-tablet,bus=usb.0".into(),
                "-vnc".into(),
                format!(
                    "unix:{},share=force-shared",
                    socket
                        .with_file_name("vnc.sock")
                        .to_string_lossy()
                        .replace(',', ",,")
                ),
            ]);
        }
        if let Some(path) = &self.firmware_dir {
            args.extend(["-L".into(), path.to_string_lossy().into_owned()]);
        }
        if let Some(disk) = &self.disk {
            args.extend(["-blockdev".into(), serde_json::json!({"driver":"file", "filename":disk.path, "node-name":"disk-file", "read-only":disk.read_only}).to_string(),
                "-blockdev".into(), serde_json::json!({"driver":disk.format, "file":"disk-file", "node-name":"disk", "read-only":disk.read_only}).to_string(),
                "-device".into(), "virtio-blk-pci,drive=disk".into()]);
        }
        if hotplug_memory > 0 {
            // Unplugged blocks are reserved address space only; the guest
            // touches memory after the supervisor raises requested-size.
            args.extend([
                "-object".into(),
                serde_json::json!({"qom-type":"memory-backend-ram", "id":"hotplug-memory", "size":hotplug_memory << 20}).to_string(),
                "-device".into(),
                format!("virtio-mem-pci,id={},memdev=hotplug-memory,requested-size=0,block-size={HOTPLUG_BLOCK_MIB}M", super::hotplug::MEMORY_DEVICE),
            ]);
        }
        super::iscsi::arguments(&self.iscsi_disks, &mut args);
        if self.seed.is_some() {
            super::seed::arguments(socket, &mut args);
        }
        if self.incoming.is_some() {
            args.extend(["-incoming".into(), "defer".into()]);
        }
        args
    }
}

fn required_file(path: &Path) -> Result<()> {
    if !path.is_absolute() || !path.is_file() {
        return Err(Error::Config(format!(
            "required absolute file is missing: {}",
            path.display()
        )));
    }
    Ok(())
}
