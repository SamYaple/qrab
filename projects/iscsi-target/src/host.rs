use crate::{
    valid_quota, Backend, Image, Record, Refusal, Spec, HBA, OWNER, PARENT, PARENT_QUOTA, PORTAL,
};
use anyhow::{bail, ensure, Context, Result};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{symlink, FileTypeExt, MetadataExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const CONFIG: &str = "/sys/kernel/config/target";
const ZFS: &str = "/usr/bin/zfs";
const PROP: &str = "org.qrab:owner";
const VM_PROP: &str = "org.qrab:vm-uid";
const DISK_PROP: &str = "org.qrab:disk";
const ORIGIN_IMAGE_PROP: &str = "org.qrab:origin-image";
const ORIGIN_GENERATION_PROP: &str = "org.qrab:origin-generation";
const IMAGE_PROP: &str = "org.qrab:image";
const IMAGE_GENERATION_PROP: &str = "org.qrab:image-generation";
const STATE_PROP: &str = "org.qrab:state";
const IMAGE_HOLD: &str = "qrab-image";
const ZFS_BOUND: Duration = Duration::from_secs(30);
/// Bounded full copy of a fenced source into a new image generation.
const PUBLISH_BOUND: Duration = Duration::from_secs(1500);
const TARGET_CHILDREN: &[&str] = &["tpgt_1", "fabric_statistics", "param"];
const TARGET_PARAMS: &[&str] = &[
    "cmd_completion_affinity",
    "default_submit_type",
    "direct_submit_supported",
];

pub struct LinuxHost;
impl Default for LinuxHost {
    fn default() -> Self {
        Self::new()
    }
}
impl LinuxHost {
    pub fn new() -> Self {
        Self
    }
    pub fn bootstrap(&mut self) -> Result<()> {
        // Only fixed kernel modules; no target, ZFS or credential changes here.
        for module in ["target_core_iblock", "iscsi_target_mod"] {
            ensure!(
                Command::new("/usr/bin/modprobe")
                    .arg(module)
                    .env_clear()
                    .status()?
                    .success(),
                "required target module could not be loaded"
            );
        }
        self.kernel()
    }
    fn kernel(&self) -> Result<()> {
        ensure!(
            nix::sys::statfs::statfs(Path::new(CONFIG))?
                .filesystem_type()
                .0 as u64
                == 0x6265_6570,
            "target configfs is not mounted; use the reviewed installer"
        );
        ensure!(
            Path::new(CONFIG).join("core").is_dir(),
            "target kernel modules are not loaded"
        );
        Ok(())
    }
    fn command(args: &[&str]) -> Command {
        let mut command = Command::new(ZFS);
        command
            .args(args)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LC_ALL", "C")
            .stderr(Stdio::null());
        command
    }
    /// `zfs send SNAPSHOT | zfs receive -o ... TARGET` without a shell.
    fn send_receive(&self, snapshot: &str, target: &str, properties: &[String]) -> Result<()> {
        let mut send = Self::command(&["send", snapshot])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .context("start fixed ZFS send")?;
        let stream = send
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("ZFS send stream absent"))?;
        let mut args = vec!["receive"];
        for property in properties {
            args.extend(["-o", property.as_str()]);
        }
        args.push(target);
        let mut receive = match Self::command(&args)
            .stdin(Stdio::from(stream))
            .stdout(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(error) => {
                let _ = send.kill();
                let _ = send.wait();
                return Err(error).context("start fixed ZFS receive");
            }
        };
        let deadline = Instant::now() + PUBLISH_BOUND;
        let (mut sent, mut received) = (None, None);
        while sent.is_none() || received.is_none() {
            if sent.is_none() {
                sent = send.try_wait()?;
            }
            if received.is_none() {
                received = receive.try_wait()?;
            }
            if Instant::now() >= deadline {
                let _ = send.kill();
                let _ = receive.kill();
                let _ = send.wait();
                let _ = receive.wait();
                bail!("image copy exceeded its bound; publication stays incomplete");
            }
            thread::sleep(Duration::from_millis(50));
        }
        ensure!(
            sent.is_some_and(|s| s.success()) && received.is_some_and(|s| s.success()),
            "fixed ZFS image copy failed"
        );
        Ok(())
    }
    fn zfs(&self, args: &[&str]) -> Result<String> {
        let mut child = Self::command(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .context("start fixed ZFS operation")?;
        let deadline = Instant::now() + ZFS_BOUND;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("ZFS stdout absent"))?;
        let reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        });
        loop {
            if child.try_wait()?.is_some() {
                let status = child.wait()?;
                let output = reader
                    .join()
                    .map_err(|_| anyhow::anyhow!("ZFS output reader stopped"))??;
                ensure!(status.success(), "fixed ZFS operation failed");
                ensure!(
                    output.len() <= 4 * 1024 * 1024,
                    "ZFS inventory exceeds bound"
                );
                return String::from_utf8(output).context("ZFS output was not UTF-8");
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                bail!("ZFS operation exceeded its bound; ownership is retained for inspection");
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn exists(&self, name: &str) -> Result<bool> {
        Ok(self
            .zfs(&["list", "-H", "-o", "name"])?
            .lines()
            .any(|s| s == name))
    }
    fn snapshot_exists(&self, snapshot: &str) -> Result<bool> {
        let (dataset, _) = snapshot
            .split_once('@')
            .ok_or_else(|| anyhow::anyhow!("not a snapshot name"))?;
        Ok(self
            .zfs(&[
                "list", "-H", "-t", "snapshot", "-o", "name", "-d", "1", dataset,
            ])?
            .lines()
            .any(|s| s == snapshot))
    }
    fn held(&self, snapshot: &str, tag: &str) -> Result<bool> {
        Ok(self
            .zfs(&["holds", "-H", snapshot])?
            .lines()
            .any(|line| line.split('\t').nth(1) == Some(tag)))
    }
    fn ensure_hold(&self, snapshot: &str, tag: &str) -> Result<()> {
        if !self.held(snapshot, tag)? {
            self.zfs(&["hold", tag, snapshot])?;
        }
        ensure!(self.held(snapshot, tag)?, "snapshot hold is unverified");
        Ok(())
    }
    fn guid(&self, name: &str) -> Result<String> {
        self.properties(name, "guid")?
            .remove("guid")
            .filter(|g| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit()))
            .ok_or_else(|| anyhow::anyhow!("ZFS GUID absent"))
    }
    fn number(p: &BTreeMap<String, String>, key: &str) -> Option<u64> {
        p.get(key).and_then(|v| v.parse::<u64>().ok())
    }
    fn properties(&self, name: &str, props: &str) -> Result<BTreeMap<String, String>> {
        let output = self.zfs(&["get", "-H", "-p", "-o", "property,value", props, name])?;
        output
            .lines()
            .map(|line| {
                line.split_once('\t')
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .ok_or_else(|| anyhow::anyhow!("malformed ZFS properties"))
            })
            .collect()
    }
    fn object(spec: &Spec) -> PathBuf {
        Path::new(CONFIG).join("core").join(HBA).join(spec.object())
    }
    fn target(spec: &Spec) -> PathBuf {
        Path::new(CONFIG).join("iscsi").join(spec.target())
    }
    fn tpg(spec: &Spec) -> PathBuf {
        Self::target(spec).join("tpgt_1")
    }
    fn acl(spec: &Spec) -> PathBuf {
        Self::tpg(spec).join("acls").join(spec.initiator())
    }
    fn lun_link(spec: &Spec) -> PathBuf {
        Self::tpg(spec).join("lun/lun_0").join(spec.object())
    }
    fn map_link(spec: &Spec) -> PathBuf {
        Self::acl(spec).join("lun_0/lun_0")
    }
    fn mkdir(path: &Path) -> Result<()> {
        match fs::create_dir(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Self::directory(path)?;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
    fn directory(path: &Path) -> Result<()> {
        let meta = fs::symlink_metadata(path)?;
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink() && meta.uid() == 0,
            "foreign configfs directory"
        );
        Ok(())
    }
    fn write(path: &Path, value: &str) -> Result<()> {
        ensure!(
            fs::symlink_metadata(path)?.file_type().is_file(),
            "unexpected configfs attribute"
        );
        fs::write(path, value).context("write fixed target attribute")
    }
    fn read(path: &Path) -> Result<String> {
        Ok(fs::read_to_string(path)
            .context("read target attribute")?
            .trim()
            .to_owned())
    }
    fn check(path: &Path, value: &str) -> Result<()> {
        ensure!(
            Self::read(path)? == value,
            "target attribute differs from the owned configuration"
        );
        Ok(())
    }
    fn link(path: &Path, target: &Path) -> Result<()> {
        match fs::symlink_metadata(path) {
            Ok(m) => {
                ensure!(
                    m.file_type().is_symlink()
                        && fs::canonicalize(path)? == fs::canonicalize(target)?,
                    "foreign target symlink"
                );
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                symlink(target, path)?;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
    fn remove_link(path: &Path, target: &Path) -> Result<()> {
        match fs::symlink_metadata(path) {
            Ok(m) => {
                ensure!(
                    m.file_type().is_symlink()
                        && fs::canonicalize(path)? == fs::canonicalize(target)?,
                    "refusing foreign target link"
                );
                fs::remove_file(path)?;
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
    fn children(path: &Path) -> Result<Vec<String>> {
        fs::read_dir(path)?
            .map(|e| {
                e?.file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("non-UTF8 target object"))
            })
            .collect()
    }
    fn restricted_children(path: &Path, allowed: &[&str]) -> Result<()> {
        for name in Self::children(path)? {
            ensure!(
                allowed.contains(&name.as_str()),
                "foreign object '{}' under managed target {}",
                name,
                path.display()
            );
        }
        Ok(())
    }
    fn target_param(name: &str, value: &str) -> Result<()> {
        let number = value
            .parse::<i64>()
            .map_err(|_| anyhow::anyhow!("kernel target parameter is not numeric"))?;
        ensure!(
            match name {
                "cmd_completion_affinity" => (-1..=i32::MAX as i64).contains(&number),
                "default_submit_type" | "direct_submit_supported" => number == 0 || number == 1,
                _ => false,
            },
            "unknown or unsupported kernel target parameter"
        );
        Ok(())
    }
    fn target_params(path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        Self::directory(path)?;
        Self::restricted_children(path, TARGET_PARAMS)?;
        for name in TARGET_PARAMS {
            let attribute = path.join(name);
            let meta = fs::symlink_metadata(&attribute)?;
            ensure!(
                meta.is_file()
                    && !meta.file_type().is_symlink()
                    && meta.uid() == 0
                    && meta.mode() & 0o022 == 0,
                "unsafe kernel target parameter file"
            );
            Self::target_param(name, &Self::read(&attribute)?)?;
        }
        Ok(())
    }
    fn only_owned_link(path: &Path, name: &str, target: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_symlink() {
                ensure!(
                    entry.file_name() == std::ffi::OsStr::new(name)
                        && fs::canonicalize(entry.path())? == fs::canonicalize(target)?,
                    "foreign link under the owned LUN"
                );
            }
        }
        Ok(())
    }
    /// Pure ownership/geometry/origin check of one zvol's ZFS properties.
    /// `origin_guid` is the recorded sealed-snapshot GUID of a clone (looked
    /// up by the caller as `origin_actual`).
    fn check_volume_properties(
        spec: &Spec,
        p: &BTreeMap<String, String>,
        guid: Option<&str>,
        origin_guid: Option<&str>,
        origin_actual: Option<&str>,
    ) -> Result<String> {
        let unset = "-".to_string();
        ensure!(
            p.get("type").is_some_and(|v| v == "volume")
                && p.get(PROP).is_some_and(|v| v == OWNER)
                && p.get(VM_PROP) == Some(&spec.vm_uid)
                && p.get(DISK_PROP) == Some(spec.disk.as_ref().unwrap_or(&unset)),
            Refusal("foreign-dataset")
        );
        ensure!(
            Self::number(p, "volsize") == Some(spec.capacity_bytes)
                && Self::number(p, "volblocksize") == Some(16384),
            "zvol geometry drift"
        );
        match &spec.origin {
            None => ensure!(
                p.get("origin") == Some(&unset)
                    && p.get(ORIGIN_IMAGE_PROP) == Some(&unset)
                    && Self::number(p, "refreservation").is_some_and(|v| v >= spec.capacity_bytes),
                "zvol origin/reservation drift"
            ),
            Some(origin) => ensure!(
                p.get("origin") == Some(&origin.snapshot())
                    && p.get(ORIGIN_IMAGE_PROP) == Some(&origin.image)
                    && p.get(ORIGIN_GENERATION_PROP) == Some(&origin.generation.to_string())
                    && origin_guid.is_none_or(|g| origin_actual == Some(g)),
                Refusal("clone-origin-mismatch")
            ),
        }
        ensure!(
            p.get("sync").is_some_and(|v| v == "always"),
            "prototype zvol sync policy differs"
        );
        let actual = p
            .get("guid")
            .ok_or_else(|| anyhow::anyhow!("zvol GUID absent"))?;
        ensure!(
            guid.is_none_or(|g| g == actual),
            Refusal("dataset-replaced")
        );
        Ok(actual.clone())
    }
    fn volume_check(
        &self,
        spec: &Spec,
        guid: Option<&str>,
        origin_guid: Option<&str>,
    ) -> Result<String> {
        ensure!(
            self.exists(&spec.dataset())?,
            "owned zvol disappeared; do not recreate storage"
        );
        let p = self.properties(
            &spec.dataset(),
            "type,guid,volsize,volblocksize,refreservation,sync,origin,org.qrab:owner,org.qrab:vm-uid,org.qrab:disk,org.qrab:origin-image,org.qrab:origin-generation",
        )?;
        let origin_actual = match (&spec.origin, origin_guid) {
            (Some(origin), Some(_)) => Some(self.guid(&origin.snapshot())?),
            _ => None,
        };
        Self::check_volume_properties(spec, &p, guid, origin_guid, origin_actual.as_deref())
    }
    fn record_check(&self, r: &Record) -> Result<String> {
        self.volume_check(&r.spec, r.zvol_guid.as_deref(), r.origin_guid.as_deref())
    }
    /// Pure check of an image dataset's ZFS properties.
    fn check_image_properties(
        image: &Image,
        p: &BTreeMap<String, String>,
        guid: Option<&str>,
    ) -> Result<String> {
        ensure!(
            p.get("type").is_some_and(|v| v == "volume")
                && p.get(PROP).is_some_and(|v| v == OWNER)
                && p.get(IMAGE_PROP) == Some(&image.origin.image)
                && p.get(IMAGE_GENERATION_PROP) == Some(&image.origin.generation.to_string())
                && p.get("origin").is_some_and(|v| v == "-"),
            Refusal("foreign-dataset")
        );
        ensure!(
            p.get("readonly").is_some_and(|v| v == "on")
                && Self::number(p, "volsize") == Some(image.capacity_bytes)
                && Self::number(p, "volblocksize") == Some(16384),
            Refusal("image-replaced")
        );
        let actual = p
            .get("guid")
            .ok_or_else(|| anyhow::anyhow!("image GUID absent"))?;
        ensure!(guid.is_none_or(|g| g == actual), Refusal("image-replaced"));
        Ok(actual.clone())
    }
    fn image_check(&self, image: &Image, guid: Option<&str>) -> Result<String> {
        let dataset = image.origin.dataset();
        ensure!(self.exists(&dataset)?, Refusal("image-not-published"));
        let p = self.properties(
            &dataset,
            "type,guid,volsize,volblocksize,readonly,origin,org.qrab:owner,org.qrab:image,org.qrab:image-generation",
        )?;
        Self::check_image_properties(image, &p, guid)
    }
    /// Pure check of the dedicated parent's policy; returns its GUID and quota.
    fn check_parent_properties(
        p: &BTreeMap<String, String>,
        guid: Option<&str>,
    ) -> Result<(String, u64)> {
        let quota = Self::number(p, "quota").unwrap_or(0);
        ensure!(
            p.get("type").is_some_and(|v| v == "filesystem")
                && p.get("mountpoint").is_some_and(|v| v == "none")
                && p.get("canmount").is_some_and(|v| v == "off")
                && p.get(PROP).is_some_and(|v| v == OWNER),
            "dedicated parent policy differs; refusing adoption"
        );
        ensure!(
            valid_quota(quota),
            "dedicated parent quota is outside the reviewed 8GiB..4TiB whole-GiB range"
        );
        let actual = p
            .get("guid")
            .ok_or_else(|| anyhow::anyhow!("parent GUID absent"))?;
        ensure!(guid.is_none_or(|g| g == actual), "parent GUID changed");
        Ok((actual.clone(), quota))
    }
    fn parent_check(&self, guid: Option<&str>) -> Result<(String, u64)> {
        let p = self.properties(PARENT, "type,guid,mountpoint,canmount,quota,org.qrab:owner")?;
        Self::check_parent_properties(&p, guid)
    }
    fn volsize(&self, dataset: &str) -> Result<u64> {
        Self::number(&self.properties(dataset, "volsize")?, "volsize")
            .ok_or_else(|| anyhow::anyhow!("zvol size absent"))
    }
    fn check_partial(&self, r: &Record) -> Result<()> {
        self.kernel()?;
        self.record_check(r)?;
        let spec = &r.spec;
        let object = Self::object(spec);
        let target = Self::target(spec);
        let tpg = Self::tpg(spec);
        if object.exists() {
            Self::directory(&object)?;
            let info = Self::read(&object.join("info"))?;
            ensure!(
                info.split_whitespace()
                    .collect::<Vec<_>>()
                    .windows(3)
                    .any(|parts| parts[0] == "UDEV"
                        && parts[1] == "PATH:"
                        && parts[2] == spec.device().to_string_lossy()),
                "backstore points to another device"
            );
            if object.join("alua").exists() {
                Self::restricted_children(&object.join("alua"), &["default_tg_pt_gp"])?;
            }
        }
        if target.exists() {
            Self::directory(&target)?;
            Self::restricted_children(&target, TARGET_CHILDREN)?;
            Self::target_params(&target.join("param"))?;
        }
        if tpg.exists() {
            Self::directory(&tpg)?;
            Self::restricted_children(&tpg.join("acls"), &[&spec.initiator()])?;
            Self::restricted_children(&tpg.join("np"), &[PORTAL])?;
            Self::restricted_children(&tpg.join("lun"), &["lun_0"])?;
        }
        let acl = Self::acl(spec);
        if acl.exists() {
            for name in Self::children(&acl)? {
                ensure!(
                    [
                        "auth",
                        "attrib",
                        "param",
                        "fabric_statistics",
                        "info",
                        "cmdsn_depth",
                        "tag",
                        "lun_0"
                    ]
                    .contains(&name.as_str()),
                    "foreign mapped LUN/ACL attribute"
                );
            }
        }
        Self::only_owned_link(&tpg.join("lun/lun_0"), &spec.object(), &Self::object(spec))?;
        Self::only_owned_link(&acl.join("lun_0"), "lun_0", &tpg.join("lun/lun_0"))?;
        if Self::lun_link(spec).symlink_metadata().is_ok() {
            ensure!(
                fs::canonicalize(Self::lun_link(spec))? == fs::canonicalize(object)?,
                "foreign LUN backing device"
            );
        }
        if Self::map_link(spec).symlink_metadata().is_ok() {
            ensure!(
                fs::canonicalize(Self::map_link(spec))? == fs::canonicalize(tpg.join("lun/lun_0"))?,
                "foreign mapped LUN"
            );
        }
        Ok(())
    }
    fn flush(&self, spec: &Spec) -> Result<()> {
        let path = spec.device();
        ensure!(
            fs::metadata(&path)?.file_type().is_block_device(),
            "zvol device is not a block device"
        );
        OpenOptions::new()
            .read(true)
            .open(path)?
            .sync_all()
            .context("flush fenced zvol")
    }
}
impl Backend for LinuxHost {
    fn preflight_new_scope(&mut self) -> Result<()> {
        self.kernel()?;
        ensure!(
            !self.exists(PARENT)? && !Path::new(CONFIG).join("core").join(HBA).exists(),
            "dedicated ZFS parent/HBA already exists without ownership state"
        );
        Ok(())
    }
    fn ensure_pool(&mut self, guid: Option<&str>) -> Result<String> {
        self.kernel()?;
        if !self.exists(PARENT)? {
            ensure!(
                guid.is_none(),
                "owned parent disappeared; refusing recreation"
            );
            self.zfs(&[
                "create",
                "-o",
                "mountpoint=none",
                "-o",
                "canmount=off",
                "-o",
                "compression=lz4",
                "-o",
                &format!("quota={PARENT_QUOTA}"),
                "-o",
                &format!("{PROP}={OWNER}"),
                PARENT,
            ])?;
        }
        let (actual, _) = self.parent_check(guid)?;
        Self::mkdir(&Path::new(CONFIG).join("core").join(HBA))?;
        Self::mkdir(&Path::new(CONFIG).join("iscsi"))?;
        Ok(actual)
    }
    fn preflight_new_volume(&mut self, spec: &Spec) -> Result<()> {
        ensure!(
            !self.exists(&spec.dataset())?
                && !Self::object(spec).exists()
                && !Self::target(spec).exists(),
            "preexisting volume/target has no owned manifest; refusing adoption"
        );
        Ok(())
    }
    fn ensure_volume(&mut self, spec: &Spec, guid: Option<&str>) -> Result<String> {
        if !self.exists(&spec.dataset())? {
            ensure!(
                guid.is_none(),
                "owned zvol disappeared; refusing recreation"
            );
            ensure!(spec.origin.is_none(), "a clone is not a blank zvol");
            let size = spec.capacity_bytes.to_string();
            let owner = format!("{PROP}={OWNER}");
            let vm = format!("{VM_PROP}={}", spec.vm_uid);
            let disk = spec.disk.as_ref().map(|d| format!("{DISK_PROP}={d}"));
            let dataset = spec.dataset();
            let mut args = vec![
                "create",
                "-V",
                &size,
                "-b",
                "16384",
                "-o",
                "sync=always",
                "-o",
                &owner,
                "-o",
                &vm,
            ];
            if let Some(disk) = &disk {
                args.extend(["-o", disk.as_str()]);
            }
            args.push(&dataset);
            self.zfs(&args)?;
        }
        self.volume_check(spec, guid, None)
    }
    fn activate(&mut self, r: &Record) -> Result<()> {
        let spec = &r.spec;
        // Validate backstore ownership even if its target directory is absent.
        // No create/link/enable is permitted before this check.
        self.check_partial(r)?;
        // A partial retry cannot rewrite an active export around live sessions.
        if Self::target(spec).exists() {
            self.fence(r)?;
        }
        let object = Self::object(spec);
        if !object.exists() {
            Self::mkdir(&object)?;
            Self::write(
                &object.join("control"),
                &format!("udev_path={}", spec.device().display()),
            )?;
            Self::write(&object.join("enable"), "1")?;
        }
        Self::mkdir(&Self::target(spec))?;
        let tpg = Self::tpg(spec);
        Self::mkdir(&tpg)?;
        for (name, value) in [
            ("generate_node_acls", "0"),
            ("cache_dynamic_acls", "0"),
            ("demo_mode_write_protect", "1"),
            ("prod_mode_write_protect", "0"),
            ("authentication", "1"),
        ] {
            Self::write(&tpg.join("attrib").join(name), value)?;
        }
        Self::write(&tpg.join("param/AuthMethod"), "CHAP")?;
        Self::write(&tpg.join("param/MaxConnections"), "1")?;
        Self::mkdir(&tpg.join("np").join(PORTAL))?;
        Self::mkdir(&tpg.join("lun/lun_0"))?;
        Self::link(&Self::lun_link(spec), &object)?;
        let acl = Self::acl(spec);
        Self::mkdir(&acl)?;
        Self::write(&acl.join("auth/userid"), &spec.username())?;
        Self::write(&acl.join("auth/password"), &r.chap_password)?;
        Self::mkdir(&acl.join("lun_0"))?;
        Self::link(&Self::map_link(spec), &tpg.join("lun/lun_0"))?;
        Self::write(&acl.join("lun_0/write_protect"), "0")?;
        Self::write(&tpg.join("enable"), "1")?;
        Ok(())
    }
    fn restore_active(&mut self, r: &Record) -> Result<()> {
        self.record_check(r)?;
        if Self::target(&r.spec).exists() {
            return self.verify_active(r);
        }
        self.check_partial(r)?;
        self.activate(r)?;
        self.verify_active(r)
    }
    fn verify_active(&mut self, r: &Record) -> Result<()> {
        self.check_partial(r)?;
        let tpg = Self::tpg(&r.spec);
        Self::check(&tpg.join("enable"), "1")?;
        for (name, value) in [
            ("generate_node_acls", "0"),
            ("cache_dynamic_acls", "0"),
            ("prod_mode_write_protect", "0"),
            ("authentication", "1"),
        ] {
            Self::check(&tpg.join("attrib").join(name), value)?;
        }
        Self::check(&tpg.join("param/AuthMethod"), "CHAP")?;
        Self::check(&tpg.join("param/MaxConnections"), "1")?;
        Self::check(&Self::acl(&r.spec).join("auth/userid"), &r.spec.username())?;
        Self::check(&Self::acl(&r.spec).join("auth/password"), &r.chap_password)?;
        ensure!(
            Self::lun_link(&r.spec).is_symlink()
                && Self::map_link(&r.spec).is_symlink()
                && tpg.join("np").join(PORTAL).is_dir(),
            "owned export is incomplete"
        );
        Ok(())
    }
    fn verify_fenced(&mut self, r: &Record) -> Result<()> {
        self.check_partial(r)?;
        let tpg = Self::tpg(&r.spec);
        if tpg.exists() {
            Self::check(&tpg.join("enable"), "0")?;
            ensure!(
                !Self::acl(&r.spec).exists() && !Self::lun_link(&r.spec).exists(),
                "target still has credentials or a LUN export"
            );
        }
        let members = Self::object(&r.spec).join("alua/default_tg_pt_gp/members");
        if members.exists() {
            ensure!(
                Self::read(&members)?.is_empty(),
                "backstore is still exported through a target port"
            );
        }
        Ok(())
    }
    fn fence(&mut self, r: &Record) -> Result<()> {
        self.check_partial(r)?;
        let spec = &r.spec;
        let tpg = Self::tpg(spec);
        if tpg.exists() {
            if Self::read(&tpg.join("enable"))? != "0" {
                Self::write(&tpg.join("enable"), "0")?;
            }
            Self::check(&tpg.join("enable"), "0")?;
            let acl = Self::acl(spec);
            if acl.exists() {
                Self::remove_link(&Self::map_link(spec), &tpg.join("lun/lun_0"))?;
                if acl.join("lun_0").exists() {
                    fs::remove_dir(acl.join("lun_0"))?;
                }
                fs::remove_dir(&acl).context("wait for initiator session/ACL destruction")?;
            }
            // Unlink invokes core_tpg_remove_lun -> transport_clear_lun_ref,
            // which waits for every active LUN I/O reference, not a timer.
            Self::remove_link(&Self::lun_link(spec), &Self::object(spec))?;
            if tpg.join("lun/lun_0").exists() {
                fs::remove_dir(tpg.join("lun/lun_0"))?;
            }
        }
        self.flush(spec)?;
        self.verify_fenced(r)
    }
    fn parent_quota(&mut self) -> Result<u64> {
        Ok(self.parent_check(None)?.1)
    }
    fn retire_image(&mut self, image: &Image) -> Result<()> {
        self.verify_image(image)?;
        let sealed = image.origin.snapshot();
        let holds = self.zfs(&["holds", "-H", &sealed])?;
        ensure!(
            holds
                .lines()
                .all(|line| line.split('\t').nth(1) == Some(IMAGE_HOLD)),
            Refusal("image-in-use")
        );
        // Clones keep `origin` pointing here even without a hold.
        let origins = self.zfs(&["list", "-H", "-t", "volume", "-r", "-o", "origin", PARENT])?;
        ensure!(
            !origins.lines().any(|origin| origin == sealed),
            Refusal("image-in-use")
        );
        self.zfs(&[
            "set",
            &format!("{STATE_PROP}=retained"),
            &image.origin.dataset(),
        ])?;
        Ok(())
    }
    fn logical_bytes(&mut self) -> Result<u64> {
        self.zfs(&[
            "list", "-H", "-p", "-t", "volume", "-r", "-o", "volsize", PARENT,
        ])?
        .lines()
        .try_fold(0u64, |sum, line| {
            line.parse::<u64>()
                .ok()
                .and_then(|v| sum.checked_add(v))
                .ok_or_else(|| anyhow::anyhow!("malformed ZFS volume inventory"))
        })
    }
    fn ensure_clone(&mut self, spec: &Spec, image: &Image, guid: Option<&str>) -> Result<String> {
        self.verify_image(image)?;
        let origin = spec
            .origin
            .as_ref()
            .filter(|o| **o == image.origin)
            .ok_or_else(|| anyhow::anyhow!("clone origin differs from the image"))?;
        let snapshot = origin.snapshot();
        let tag = spec.clone_hold();
        let dataset = spec.dataset();
        if !self.exists(&dataset)? {
            ensure!(
                guid.is_none(),
                "owned clone disappeared; refusing recreation"
            );
            // Hold first: the sealed snapshot cannot vanish under the clone.
            self.ensure_hold(&snapshot, &tag)?;
            let properties = [
                format!("{PROP}={OWNER}"),
                format!("{VM_PROP}={}", spec.vm_uid),
                format!("{DISK_PROP}={}", spec.disk.as_deref().unwrap_or("-")),
                format!("{ORIGIN_IMAGE_PROP}={}", origin.image),
                format!("{ORIGIN_GENERATION_PROP}={}", origin.generation),
                "sync=always".into(),
                "readonly=off".into(),
            ];
            let mut args = vec!["clone"];
            for property in &properties {
                args.extend(["-o", property.as_str()]);
            }
            args.extend([snapshot.as_str(), dataset.as_str()]);
            if let Err(error) = self.zfs(&args) {
                if !self.exists(&dataset)? {
                    self.zfs(&["release", &tag, &snapshot])?;
                }
                return Err(error);
            }
        }
        self.ensure_hold(&snapshot, &tag)?;
        // Still provisioning: grow the fresh clone to the requested capacity.
        if guid.is_none() && self.volsize(&dataset)? < spec.capacity_bytes {
            self.zfs(&["set", &format!("volsize={}", spec.capacity_bytes), &dataset])?;
        }
        self.volume_check(spec, guid, image.snapshot_guid.as_deref())
    }
    fn verify_volume(&mut self, r: &Record) -> Result<()> {
        self.record_check(r).map(|_| ())
    }
    fn expand(&mut self, r: &Record, capacity_bytes: u64) -> Result<()> {
        let dataset = r.spec.dataset();
        let current = self.volsize(&dataset)?;
        ensure!(
            current == r.spec.capacity_bytes || current == capacity_bytes,
            "zvol geometry drift"
        );
        let mut observed = r.spec.clone();
        observed.capacity_bytes = current;
        self.volume_check(&observed, r.zvol_guid.as_deref(), r.origin_guid.as_deref())?;
        if current != capacity_bytes {
            self.zfs(&["set", &format!("volsize={capacity_bytes}"), &dataset])?;
        }
        if r.spec.origin.is_none() {
            // Blank disks stay thick so pool pressure never reaches the guest.
            self.zfs(&["set", "refreservation=auto", &dataset])?;
        }
        observed.capacity_bytes = capacity_bytes;
        self.volume_check(&observed, r.zvol_guid.as_deref(), r.origin_guid.as_deref())?;
        Ok(())
    }
    fn publish_image(&mut self, source: &Record, image: &mut Image) -> Result<()> {
        self.record_check(source)?;
        let staging = format!("{}@qrab-publish", source.spec.dataset());
        let dataset = image.origin.dataset();
        let sealed = image.origin.snapshot();
        let received = format!("{dataset}@qrab-publish");
        if !self.exists(&dataset)? {
            ensure!(
                image.dataset_guid.is_none(),
                "published image disappeared; refusing recreation"
            );
            // A leftover staging snapshot on our fenced source is ours to redo.
            if self.snapshot_exists(&staging)? {
                self.zfs(&["destroy", &staging])?;
            }
            self.zfs(&["snapshot", &staging])?;
            self.send_receive(
                &staging,
                &dataset,
                &[
                    format!("{PROP}={OWNER}"),
                    format!("{IMAGE_PROP}={}", image.origin.image),
                    format!("{IMAGE_GENERATION_PROP}={}", image.origin.generation),
                    "readonly=on".into(),
                    "refreservation=none".into(),
                    "sync=always".into(),
                ],
            )?;
        }
        let guid = self.image_check(image, image.dataset_guid.as_deref())?;
        if !self.snapshot_exists(&sealed)? {
            ensure!(
                self.snapshot_exists(&received)?,
                "received image has no publication snapshot"
            );
            self.zfs(&["rename", &received, &sealed])?;
        }
        self.ensure_hold(&sealed, IMAGE_HOLD)?;
        let snapshot_guid = self.guid(&sealed)?;
        ensure!(
            image
                .snapshot_guid
                .as_ref()
                .is_none_or(|g| *g == snapshot_guid),
            Refusal("image-replaced")
        );
        image.dataset_guid = Some(guid);
        image.snapshot_guid = Some(snapshot_guid);
        if self.snapshot_exists(&staging)? {
            self.zfs(&["destroy", &staging])?;
        }
        Ok(())
    }
    fn verify_image(&mut self, image: &Image) -> Result<()> {
        let guid = image
            .dataset_guid
            .as_deref()
            .ok_or(Refusal("image-not-published"))?;
        self.image_check(image, Some(guid))?;
        let sealed = image.origin.snapshot();
        ensure!(
            self.snapshot_exists(&sealed)?
                && image.snapshot_guid.as_deref() == Some(self.guid(&sealed)?.as_str())
                && self.held(&sealed, IMAGE_HOLD)?,
            Refusal("image-replaced")
        );
        Ok(())
    }
    fn retire(&mut self, r: &Record) -> Result<()> {
        self.check_partial(r)?;
        let spec = &r.spec;
        let tpg = Self::tpg(spec);
        if tpg.exists() {
            Self::check(&tpg.join("enable"), "0")?;
            ensure!(
                !Self::acl(spec).exists() && !Self::lun_link(spec).exists(),
                Refusal("fence-required")
            );
            let portal = tpg.join("np").join(PORTAL);
            if portal.exists() {
                fs::remove_dir(&portal)?;
            }
            fs::remove_dir(&tpg)?;
        }
        if Self::target(spec).exists() {
            fs::remove_dir(Self::target(spec))?;
        }
        let object = Self::object(spec);
        if object.exists() {
            let members = object.join("alua/default_tg_pt_gp/members");
            ensure!(
                !members.exists() || Self::read(&members)?.is_empty(),
                "backstore is still exported through a target port"
            );
            fs::remove_dir(&object).context("remove the fenced backstore")?;
        }
        self.zfs(&["set", &format!("{STATE_PROP}=retained"), &spec.dataset()])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Origin;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }
    const UID: &str = "e309f751-5dda-4271-a0f4-4454c9527a59";
    fn blank() -> (Spec, BTreeMap<String, String>) {
        let spec = Spec::disk(UID.into(), "root".into(), 64 << 20, None).unwrap();
        let p = props(&[
            ("type", "volume"),
            ("guid", "34"),
            ("volsize", "67108864"),
            ("volblocksize", "16384"),
            ("refreservation", "68000000"),
            ("sync", "always"),
            ("origin", "-"),
            (PROP, OWNER),
            (VM_PROP, UID),
            (DISK_PROP, "root"),
            (ORIGIN_IMAGE_PROP, "-"),
            (ORIGIN_GENERATION_PROP, "-"),
        ]);
        (spec, p)
    }
    fn refusal_of(result: Result<String>) -> Option<&'static str> {
        result.err().and_then(|e| crate::refusal(&e))
    }
    #[test]
    fn parent_quota_accepts_the_reviewed_whole_gib_range_only() {
        let parent = |quota: &str| {
            props(&[
                ("type", "filesystem"),
                ("guid", "12"),
                ("mountpoint", "none"),
                ("canmount", "off"),
                ("quota", quota),
                (PROP, OWNER),
            ])
        };
        for quota in [PARENT_QUOTA, 64 << 30, crate::MAX_PARENT_QUOTA] {
            let (_, accepted) =
                LinuxHost::check_parent_properties(&parent(&quota.to_string()), Some("12"))
                    .unwrap();
            assert_eq!(accepted, quota);
        }
        for quota in [
            "0",
            "none",
            &(PARENT_QUOTA - (1 << 30)).to_string(),
            &(crate::MAX_PARENT_QUOTA + (1 << 30)).to_string(),
            &(PARENT_QUOTA + 1).to_string(),
        ] {
            assert!(
                LinuxHost::check_parent_properties(&parent(quota), None).is_err(),
                "{quota}"
            );
        }
        let mut foreign = parent(&PARENT_QUOTA.to_string());
        foreign.insert(PROP.into(), "-".into());
        assert!(LinuxHost::check_parent_properties(&foreign, None).is_err());
        assert!(
            LinuxHost::check_parent_properties(&parent(&PARENT_QUOTA.to_string()), Some("13"))
                .is_err()
        );
    }
    #[test]
    fn blank_disk_properties_require_exact_ownership_and_thick_reservation() {
        let (spec, p) = blank();
        LinuxHost::check_volume_properties(&spec, &p, Some("34"), None, None).unwrap();
        for (key, value) in [(PROP, "someone-else"), (VM_PROP, "other"), (DISK_PROP, "-")] {
            let mut foreign = p.clone();
            foreign.insert(key.into(), value.into());
            assert_eq!(
                refusal_of(LinuxHost::check_volume_properties(
                    &spec, &foreign, None, None, None
                )),
                Some("foreign-dataset")
            );
        }
        let mut thin = p.clone();
        thin.insert("refreservation".into(), "0".into());
        assert!(LinuxHost::check_volume_properties(&spec, &thin, None, None, None).is_err());
        assert_eq!(
            refusal_of(LinuxHost::check_volume_properties(
                &spec,
                &p,
                Some("35"),
                None,
                None
            )),
            Some("dataset-replaced")
        );
        // A legacy (diskless) spec never matches a provisioned disk's zvol.
        let legacy = Spec::new(UID.into(), 64 << 20).unwrap();
        assert!(LinuxHost::check_volume_properties(&legacy, &p, None, None, None).is_err());
    }
    #[test]
    fn clone_properties_require_the_recorded_sealed_origin_on_every_read() {
        let origin = Origin::new("debian".into(), 3).unwrap();
        let spec = Spec::disk(UID.into(), "root".into(), 64 << 20, Some(origin.clone())).unwrap();
        let (_, mut p) = blank();
        p.insert("origin".into(), origin.snapshot());
        p.insert(ORIGIN_IMAGE_PROP.into(), "debian".into());
        p.insert(ORIGIN_GENERATION_PROP.into(), "3".into());
        p.insert("refreservation".into(), "0".into());
        LinuxHost::check_volume_properties(&spec, &p, None, Some("78"), Some("78")).unwrap();
        assert_eq!(
            refusal_of(LinuxHost::check_volume_properties(
                &spec,
                &p,
                None,
                Some("78"),
                Some("79")
            )),
            Some("clone-origin-mismatch")
        );
        let mut other = p.clone();
        other.insert(
            "origin".into(),
            "tank/qrab-iscsi/image-debian-g2@sealed".into(),
        );
        assert_eq!(
            refusal_of(LinuxHost::check_volume_properties(
                &spec, &other, None, None, None
            )),
            Some("clone-origin-mismatch")
        );
        // A blank spec refuses a zvol that is secretly a clone.
        let (blank_spec, _) = blank();
        assert!(LinuxHost::check_volume_properties(&blank_spec, &p, None, None, None).is_err());
    }
    #[test]
    fn image_properties_require_ownership_readonly_and_recorded_guid() {
        let image = Image {
            schema: 2,
            owner: OWNER.into(),
            origin: Origin::new("debian".into(), 3).unwrap(),
            capacity_bytes: 64 << 20,
            source_vm_uid: UID.into(),
            source_disk: "golden".into(),
            source_guid: "34".into(),
            phase: crate::disk::ImagePhase::Sealed,
            dataset_guid: Some("77".into()),
            snapshot_guid: Some("78".into()),
        };
        let p = props(&[
            ("type", "volume"),
            ("guid", "77"),
            ("volsize", "67108864"),
            ("volblocksize", "16384"),
            ("readonly", "on"),
            ("origin", "-"),
            (PROP, OWNER),
            (IMAGE_PROP, "debian"),
            (IMAGE_GENERATION_PROP, "3"),
        ]);
        LinuxHost::check_image_properties(&image, &p, Some("77")).unwrap();
        let mut writable = p.clone();
        writable.insert("readonly".into(), "off".into());
        assert_eq!(
            refusal_of(LinuxHost::check_image_properties(
                &image,
                &writable,
                Some("77")
            )),
            Some("image-replaced")
        );
        let mut foreign = p.clone();
        foreign.insert(PROP.into(), "-".into());
        assert_eq!(
            refusal_of(LinuxHost::check_image_properties(&image, &foreign, None)),
            Some("foreign-dataset")
        );
        assert_eq!(
            refusal_of(LinuxHost::check_image_properties(&image, &p, Some("70"))),
            Some("image-replaced")
        );
    }
    #[test]
    fn foreign_alias_under_a_managed_lun_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let lun = temp.path().join("lun");
        let object = temp.path().join("owned");
        let foreign = temp.path().join("foreign");
        fs::create_dir(&lun).unwrap();
        fs::create_dir(&object).unwrap();
        fs::create_dir(&foreign).unwrap();
        symlink(&object, lun.join("expected")).unwrap();
        LinuxHost::only_owned_link(&lun, "expected", &object).unwrap();
        symlink(&foreign, lun.join("unexpected")).unwrap();
        assert!(LinuxHost::only_owned_link(&lun, "expected", &object).is_err());
    }
    #[test]
    fn known_kernel_target_param_group_is_not_another_export() {
        let temp = tempfile::tempdir().unwrap();
        for name in TARGET_CHILDREN {
            fs::create_dir(temp.path().join(name)).unwrap();
        }
        LinuxHost::restricted_children(temp.path(), TARGET_CHILDREN).unwrap();
        fs::create_dir(temp.path().join("tpgt_2")).unwrap();
        let message = LinuxHost::restricted_children(temp.path(), TARGET_CHILDREN)
            .unwrap_err()
            .to_string();
        assert!(message.contains("tpgt_2") && message.contains(temp.path().to_str().unwrap()));
        LinuxHost::target_param("cmd_completion_affinity", "-1").unwrap();
        LinuxHost::target_param("default_submit_type", "1").unwrap();
        LinuxHost::target_param("direct_submit_supported", "1").unwrap();
        assert!(LinuxHost::target_param("default_submit_type", "3").is_err());
        assert!(LinuxHost::target_param("unknown", "1").is_err());
    }
}
