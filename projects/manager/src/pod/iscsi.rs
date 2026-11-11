use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::OpenOptions,
    io::Read,
    net::{IpAddr, SocketAddr},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IscsiBus {
    #[default]
    Virtio,
    Scsi,
}

/// A native QEMU/libiscsi block backend. Credentials are file references only;
/// there is intentionally no inline password or credential-bearing URI field.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IscsiDisk {
    pub id: String,
    pub portal: String,
    pub target: String,
    #[serde(default)]
    pub lun: u32,
    pub initiator_name: String,
    pub username: String,
    pub password_file: PathBuf,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub bus: IscsiBus,
    #[serde(default)]
    pub boot_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Binding {
    portal: String,
    target: String,
    lun: u32,
    initiator_name: String,
}

impl IscsiDisk {
    pub(super) fn binding(&self) -> Binding {
        Binding {
            portal: self.portal.clone(),
            target: self.target.clone(),
            lun: self.lun,
            initiator_name: self.initiator_name.clone(),
        }
    }
}

fn valid_iqn(value: &str) -> bool {
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
        .map_or((rest, None), |(d, u)| (d, Some(u)));
    let labels: Vec<_> = domain.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
        })
        && unique.is_none_or(|value| !value.is_empty())
}

pub(super) fn validate(disks: &[IscsiDisk]) -> Result<()> {
    if disks.len() > 16 {
        return Err(Error::Config(
            "at most 16 native iSCSI disks are supported".into(),
        ));
    }
    let mut ids = HashSet::new();
    let mut luns = HashSet::new();
    let mut boot_indexes = HashSet::new();
    let mut initiator = None;
    for disk in disks {
        let id = &disk.id;
        if id.is_empty()
            || id.len() > 20
            || !id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !id.as_bytes()[0].is_ascii_alphanumeric()
            || !id.as_bytes()[id.len() - 1].is_ascii_alphanumeric()
            || !ids.insert(id)
        {
            return Err(Error::Config("native iSCSI disk IDs must be unique 1..20 lowercase letters/digits/hyphens with alphanumeric ends".into()));
        }
        let portal: SocketAddr = disk.portal.parse().map_err(|_| {
            Error::Config(format!(
                "iSCSI disk {id}: portal must be a literal IP:port, not a URL"
            ))
        })?;
        if portal.port() == 0
            || portal.ip().is_unspecified()
            || portal.ip().is_multicast()
            || matches!(portal.ip(),IpAddr::V4(ip) if ip.is_broadcast())
            || !valid_iqn(&disk.target)
            || !valid_iqn(&disk.initiator_name)
            || disk.lun > i32::MAX as u32
        {
            return Err(Error::Config(format!(
                "iSCSI disk {id}: invalid portal, target/initiator IQN or LUN"
            )));
        }
        if !luns.insert((portal, &disk.target, disk.lun)) {
            return Err(Error::Config(
                "the same iSCSI target/LUN must not be attached twice".into(),
            ));
        }
        if initiator.is_some_and(|name| name != &disk.initiator_name) {
            return Err(Error::Config(
                "all iSCSI disks of a VM must use the same stable initiator_name".into(),
            ));
        }
        initiator = Some(&disk.initiator_name);
        if disk.username.is_empty()
            || disk.username.len() > 255
            || !disk.username.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Error::Config(format!(
                "iSCSI disk {id}: a nonempty printable CHAP username is required"
            )));
        }
        if let Some(index) = disk.boot_index {
            if index == 1 || index > i32::MAX as u32 || !boot_indexes.insert(index) {
                return Err(Error::Config("native iSCSI boot indexes must be unique nonnegative int32 values other than reserved ESP index1".into()));
            }
        }
        // Kubernetes projected Secret keys are symlinks. Follow the key, but
        // require a readable bounded regular file and never include its data in
        // errors, Config, command arguments or status records.
        if !disk.password_file.is_absolute() {
            return Err(Error::Config(format!(
                "iSCSI disk {id}: password_file must be absolute"
            )));
        }
        let invalid = || {
            Error::Config(format!(
                "iSCSI disk {id}: CHAP password file is missing, unreadable or invalid"
            ))
        };
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NONBLOCK)
            .open(&disk.password_file)
            .map_err(|_| invalid())?;
        let metadata = file.metadata().map_err(|_| invalid())?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 4096 {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        file.take(4097)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid())?;
        if bytes.is_empty()
            || bytes.len() > 4096
            || bytes.iter().any(u8::is_ascii_control)
            || std::str::from_utf8(&bytes).is_err()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(super) fn arguments(disks: &[IscsiDisk], args: &mut Vec<String>) {
    if disks.iter().any(|disk| disk.bus == IscsiBus::Scsi) {
        args.extend([
            "-device".into(),
            "virtio-scsi-pci,id=iscsi-controller".into(),
        ]);
    }
    for disk in disks {
        let secret = format!("iscsi-{}-chap", disk.id);
        let backend = format!("iscsi-{}-lun", disk.id);
        let node = format!("iscsi-{}", disk.id);
        args.extend([
            "-object".into(),serde_json::json!({"qom-type":"secret","id":secret,"file":disk.password_file,"format":"raw"}).to_string(),
            "-blockdev".into(),serde_json::json!({"driver":"iscsi","transport":"tcp","portal":disk.portal,"target":disk.target,"lun":disk.lun,"user":disk.username,"password-secret":secret,"initiator-name":disk.initiator_name,"node-name":backend,"read-only":disk.read_only,"timeout":30,"cache":{"no-flush":false}}).to_string(),
            "-blockdev".into(),serde_json::json!({"driver":"raw","file":backend,"node-name":node,"read-only":disk.read_only,"cache":{"no-flush":false}}).to_string(),
        ]);
        let device = match disk.bus {
            IscsiBus::Virtio => format!(
                "virtio-blk-pci,drive={node},id=disk-{},serial={}",
                disk.id, disk.id
            ),
            IscsiBus::Scsi => format!(
                "scsi-hd,bus=iscsi-controller.0,drive={node},id=disk-{},serial={}",
                disk.id, disk.id
            ),
        };
        args.extend([
            "-device".into(),
            match disk.boot_index {
                Some(index) => format!("{device},bootindex={index}"),
                None => device,
            },
        ]);
    }
}
