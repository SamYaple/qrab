use crate::{Backend, Record, Spec, HBA, OWNER, PARENT, PARENT_QUOTA, PORTAL};
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
    fn zfs(&self, args: &[&str]) -> Result<String> {
        let mut child = Command::new(ZFS)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("start fixed ZFS operation")?;
        let deadline = Instant::now() + Duration::from_secs(30);
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
                "foreign object under the managed target"
            );
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
    fn volume_check(&self, spec: &Spec, guid: Option<&str>) -> Result<String> {
        ensure!(
            self.exists(&spec.dataset())?,
            "owned zvol disappeared; do not recreate storage"
        );
        let p = self.properties(
            &spec.dataset(),
            "type,guid,volsize,volblocksize,refreservation,sync,org.qrab:owner,org.qrab:vm-uid",
        )?;
        ensure!(
            p.get("type").is_some_and(|v| v == "volume")
                && p.get(PROP).is_some_and(|v| v == OWNER)
                && p.get(VM_PROP) == Some(&spec.vm_uid),
            "foreign zvol ownership"
        );
        ensure!(
            p.get("volsize").and_then(|v| v.parse::<u64>().ok()) == Some(spec.capacity_bytes)
                && p.get("volblocksize").and_then(|v| v.parse::<u64>().ok()) == Some(16384)
                && p.get("refreservation")
                    .and_then(|v| v.parse::<u64>().ok())
                    .is_some_and(|v| v >= spec.capacity_bytes),
            "zvol geometry/reservation drift"
        );
        ensure!(
            p.get("sync").is_some_and(|v| v == "always"),
            "prototype zvol sync policy differs"
        );
        let actual = p
            .get("guid")
            .ok_or_else(|| anyhow::anyhow!("zvol GUID absent"))?;
        ensure!(
            guid.is_none_or(|g| g == actual),
            "zvol was replaced; refusing adoption"
        );
        Ok(actual.clone())
    }
    fn check_partial(&self, r: &Record) -> Result<()> {
        self.kernel()?;
        self.volume_check(&r.spec, r.zvol_guid.as_deref())?;
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
            Self::restricted_children(&target, &["tpgt_1", "fabric_statistics"])?;
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
        let p = self.properties(PARENT, "type,guid,mountpoint,canmount,quota,org.qrab:owner")?;
        ensure!(
            p.get("type").is_some_and(|v| v == "filesystem")
                && p.get("mountpoint").is_some_and(|v| v == "none")
                && p.get("canmount").is_some_and(|v| v == "off")
                && p.get("quota").and_then(|v| v.parse::<u64>().ok()) == Some(PARENT_QUOTA)
                && p.get(PROP).is_some_and(|v| v == OWNER),
            "dedicated parent policy differs; refusing adoption"
        );
        let actual = p
            .get("guid")
            .ok_or_else(|| anyhow::anyhow!("parent GUID absent"))?;
        ensure!(guid.is_none_or(|g| g == actual), "parent GUID changed");
        Self::mkdir(&Path::new(CONFIG).join("core").join(HBA))?;
        Self::mkdir(&Path::new(CONFIG).join("iscsi"))?;
        Ok(actual.clone())
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
            self.zfs(&[
                "create",
                "-V",
                &spec.capacity_bytes.to_string(),
                "-b",
                "16384",
                "-o",
                "sync=always",
                "-o",
                &format!("{PROP}={OWNER}"),
                "-o",
                &format!("{VM_PROP}={}", spec.vm_uid),
                &spec.dataset(),
            ])?;
        }
        self.volume_check(spec, guid)
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
        self.volume_check(&r.spec, r.zvol_guid.as_deref())?;
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
