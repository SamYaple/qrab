use crate::{Image, Origin, Record, Spec, OWNER, ROOT};
use anyhow::{ensure, Context, Result};
use nix::{
    fcntl::{Flock, FlockArg, OFlag},
    unistd::{chown, Gid, Uid},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

pub struct Store {
    root: PathBuf,
    owner: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pool {
    schema: u32,
    owner: String,
    guid: Option<String>,
}

fn handoff_directory(path: &Path, owner: u32) -> Result<()> {
    match fs::DirBuilder::new().mode(0o755).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    // Check and chmod the same directory inode. Creation mode alone is masked
    // by the installer's umask077; file copies remain recipient-owned0600.
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags((OFlag::O_NOFOLLOW | OFlag::O_DIRECTORY).bits())
        .open(path)?;
    let meta = directory.metadata()?;
    ensure!(
        meta.is_dir() && meta.uid() == owner && meta.mode() & 0o022 == 0,
        "unsafe handoff directory"
    );
    directory.set_permissions(fs::Permissions::from_mode(0o755))?;
    directory.sync_all()?;
    Ok(())
}
impl Store {
    pub fn system() -> Self {
        Self {
            root: ROOT.into(),
            owner: 0,
        }
    }
    #[cfg(test)]
    pub(crate) fn test(root: PathBuf) -> Self {
        Self {
            root,
            owner: nix::unistd::getuid().as_raw(),
        }
    }
    pub fn lock(&self) -> Result<Flock<File>> {
        if self.owner == 0 {
            for path in ["/", "/var", "/var/lib"] {
                let m = fs::symlink_metadata(path)?;
                ensure!(
                    m.is_dir()
                        && !m.file_type().is_symlink()
                        && m.uid() == 0
                        && m.mode() & 0o022 == 0,
                    "unsafe state ancestor"
                );
            }
        }
        self.dir(&self.root)?;
        let lock = self.root.join("owner.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(OFlag::O_NOFOLLOW.bits())
            .open(&lock)?;
        self.private_file(&lock)?;
        // Bounded wait: concurrent daemon requests serialize instead of failing,
        // within the daemon's wrapper budget.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut file = file;
        loop {
            match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
                Ok(lock) => return Ok(lock),
                Err((returned, _)) if std::time::Instant::now() < deadline => {
                    file = returned;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(_) => {
                    return Err(anyhow::Error::new(crate::Refusal("busy"))
                        .context("another target administrator is active"))
                }
            }
        }
    }
    fn dir(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            fs::DirBuilder::new().mode(0o700).create(path)?;
        }
        let m = fs::symlink_metadata(path)?;
        ensure!(
            m.is_dir()
                && !m.file_type().is_symlink()
                && m.uid() == self.owner
                && m.mode() & 0o077 == 0,
            "unsafe target state directory"
        );
        Ok(())
    }
    fn private_file(&self, path: &Path) -> Result<()> {
        let m = fs::symlink_metadata(path)?;
        ensure!(
            m.is_file()
                && !m.file_type().is_symlink()
                && m.uid() == self.owner
                && m.nlink() == 1
                && m.mode() & 0o077 == 0
                && m.len() <= 16384,
            "unsafe target state file"
        );
        Ok(())
    }
    fn load<T: serde::de::DeserializeOwned>(&self, path: &Path) -> Result<Option<T>> {
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            other => {
                other?;
            }
        }
        self.private_file(path)?;
        let bytes = fs::read(path)?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| anyhow::anyhow!("target state is malformed"))
    }
    fn save<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        match fs::symlink_metadata(path) {
            Ok(_) => self.private_file(path)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let mut temp = tempfile::Builder::new()
            .prefix(".update-")
            .permissions(fs::Permissions::from_mode(0o600))
            .tempfile_in(&self.root)?;
        serde_json::to_writer_pretty(&mut temp, value)?;
        temp.write_all(b"\n")?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| e.error)?;
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }
    pub(crate) fn manifest_path(&self, key: &str) -> PathBuf {
        self.root.join(format!("{key}.json"))
    }
    fn read_key(&self, key: &str, uid: &str, disk: Option<&str>) -> Result<Option<Record>> {
        let record: Option<Record> = self.load(&self.manifest_path(key))?;
        if let Some(r) = &record {
            r.validate()?;
            ensure!(
                r.spec.vm_uid == uid && r.spec.disk.as_deref() == disk,
                "manifest identity differs from its path"
            );
        }
        Ok(record)
    }
    /// The legacy single-disk demo record.
    pub fn read(&self, uid: &str) -> Result<Option<Record>> {
        Spec::new(uid.into(), 16 * 1024 * 1024)?.validate()?;
        self.read_key(uid, uid, None)
    }
    pub fn read_disk(&self, uid: &str, disk: &str) -> Result<Option<Record>> {
        let spec = Spec::disk(uid.into(), disk.into(), 16 * 1024 * 1024, None)?;
        self.read_key(&spec.key(), uid, Some(disk))
    }
    pub fn write(&self, record: &Record) -> Result<()> {
        record.validate()?;
        self.save(&self.manifest_path(&record.spec.key()), record)
    }
    pub fn read_image(&self, origin: &Origin) -> Result<Option<Image>> {
        origin.validate()?;
        let image: Option<Image> = self.load(&self.manifest_path(&origin.key()))?;
        if let Some(image) = &image {
            image.validate()?;
            ensure!(
                &image.origin == origin,
                "image manifest identity differs from its path"
            );
        }
        Ok(image)
    }
    pub fn write_image(&self, image: &Image) -> Result<()> {
        image.validate()?;
        self.save(&self.manifest_path(&image.origin.key()), image)
    }
    /// Removes the journal of an unsealed import after its dataset was
    /// destroyed. Published or sealed journals are never removed.
    pub(crate) fn remove_unsealed_import(&self, origin: &Origin) -> Result<()> {
        let Some(image) = self.read_image(origin)? else {
            return Ok(());
        };
        ensure!(
            image.import.is_some() && image.phase == crate::disk::ImagePhase::Publishing,
            "only an unsealed import journal may be removed"
        );
        fs::remove_file(self.manifest_path(&origin.key()))?;
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }
    /// Removes a retired disk's journal once the reaper destroyed its zvol.
    pub(crate) fn remove_retired(&self, record: &Record) -> Result<()> {
        let (uid, disk) = (&record.spec.vm_uid, record.spec.disk.as_deref());
        let disk = disk.ok_or_else(|| anyhow::anyhow!("not a VM disk journal"))?;
        let Some(current) = self.read_disk(uid, disk)? else {
            return Ok(());
        };
        ensure!(
            current.phase == crate::Phase::Retired,
            "only a retired disk journal may be removed"
        );
        fs::remove_file(self.manifest_path(&current.spec.key()))?;
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }
    /// The unsealed stage of one image generation, `stage-<name>-g<gen>.json`.
    pub fn read_stage(&self, origin: &Origin) -> Result<Option<Record>> {
        origin.validate()?;
        let record: Option<Record> = self.load(&self.manifest_path(&origin.stage_key()))?;
        if let Some(r) = &record {
            r.validate()?;
            ensure!(
                r.spec.stage.as_ref() == Some(origin),
                "stage manifest identity differs from its path"
            );
        }
        Ok(record)
    }
    /// Removes a stage journal after its zvol was destroyed or sealed.
    pub(crate) fn remove_stage(&self, origin: &Origin) -> Result<()> {
        if self.read_stage(origin)?.is_none() {
            return Ok(());
        }
        fs::remove_file(self.manifest_path(&origin.stage_key()))?;
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }
    pub fn images(&self) -> Result<Vec<Image>> {
        let mut result = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some(key) = name
                .strip_suffix(".json")
                .and_then(|n| n.strip_prefix("image-"))
            else {
                continue;
            };
            let (image, generation) = key
                .rsplit_once("-g")
                .ok_or_else(|| anyhow::anyhow!("unknown image manifest name"))?;
            let origin = Origin::new(image.into(), generation.parse()?)?;
            ensure!(
                origin.key() == name.trim_end_matches(".json"),
                "unknown image manifest name"
            );
            if let Some(image) = self.read_image(&origin)? {
                result.push(image);
            }
        }
        Ok(result)
    }
    pub fn pool_intent_exists(&self) -> Result<bool> {
        Ok(self.load::<Pool>(&self.root.join("pool.json"))?.is_some())
    }
    pub fn pool_guid(&self) -> Result<Option<String>> {
        let pool: Option<Pool> = self.load(&self.root.join("pool.json"))?;
        if let Some(pool) = pool {
            ensure!(
                pool.schema == 1 && pool.owner == OWNER,
                "unknown pool owner"
            );
            Ok(pool.guid)
        } else {
            Ok(None)
        }
    }
    pub fn write_pool(&self, guid: Option<&str>) -> Result<()> {
        self.save(
            &self.root.join("pool.json"),
            &Pool {
                schema: 1,
                owner: OWNER.into(),
                guid: guid.map(str::to_owned),
            },
        )
    }
    /// Disk and legacy journals. Stages are excluded: boot restore never
    /// re-enables a stage export, and no stage is a clone descendant.
    pub fn records(&self) -> Result<Vec<Record>> {
        let mut result = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if name == "pool.json"
                || !name.ends_with(".json")
                || name.starts_with("image-")
                || name.starts_with("stage-")
            {
                continue;
            }
            // Unknown journal names fail closed in the UID/disk validation.
            let key = name.trim_end_matches(".json");
            let record = match key.split_once('.') {
                None => self.read(key)?,
                Some((uid, disk)) => self.read_disk(uid, disk)?,
            };
            if let Some(r) = record {
                result.push(r);
            }
        }
        Ok(result)
    }
    /// Root-owned parent, recipient-owned0600 copy; never an authoritative input.
    pub fn handoff(&self, record: &Record, recipient: u32) -> Result<PathBuf> {
        ensure!(
            recipient >= 1000,
            "credential recipient must be an explicit ordinary user UID"
        );
        let directory = PathBuf::from("/var/lib/qrab-iscsi-handoff");
        handoff_directory(&directory, 0)?;
        let path = directory.join(format!("{}.{}.json", record.spec.vm_uid, record.generation));
        #[derive(Serialize)]
        struct Handoff<'a> {
            schema: u32,
            vm_uid: &'a str,
            portal: &'a str,
            target_iqn: String,
            initiator_iqn: String,
            lun: u32,
            chap_username: String,
            chap_password: &'a str,
            generation: u64,
        }
        let value = Handoff {
            schema: 1,
            vm_uid: &record.spec.vm_uid,
            portal: crate::PORTAL,
            target_iqn: record.spec.target(),
            initiator_iqn: record.spec.initiator(),
            lun: 0,
            chap_username: record.spec.username(),
            chap_password: &record.chap_password,
            generation: record.generation,
        };
        let bytes = serde_json::to_vec_pretty(&value)?;
        if path.exists() {
            let m = fs::symlink_metadata(&path)?;
            ensure!(
                m.is_file()
                    && !m.file_type().is_symlink()
                    && m.uid() == recipient
                    && m.mode() & 0o077 == 0
                    && m.nlink() == 1,
                "unsafe existing handoff file"
            );
            ensure!(
                fs::read(&path)? == bytes,
                "handoff was modified; inspect rather than overwrite it"
            );
            return Ok(path);
        }
        let mut temp = tempfile::Builder::new()
            .prefix(".handoff-")
            .permissions(fs::Permissions::from_mode(0o600))
            .tempfile_in(&directory)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        chown(
            temp.path(),
            Some(Uid::from_raw(recipient)),
            Some(Gid::from_raw(recipient)),
        )
        .context("set explicit credential recipient")?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(&path).map_err(|e| e.error)?;
        File::open(&directory)?.sync_all()?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restrictive_creation_mode_is_normalized_without_widening_credentials() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("handoff");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let credential = directory.join("copy.json");
        fs::write(&credential, b"private fixture").unwrap();
        fs::set_permissions(&credential, fs::Permissions::from_mode(0o600)).unwrap();
        handoff_directory(&directory, nix::unistd::getuid().as_raw()).unwrap();
        assert_eq!(fs::metadata(&directory).unwrap().mode() & 0o777, 0o755);
        assert_eq!(fs::metadata(&credential).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(&credential).unwrap().uid(),
            nix::unistd::getuid().as_raw()
        );
    }
    #[test]
    fn unsafe_directory_is_not_chmodded_and_symlinks_are_not_followed() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("handoff");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o770)).unwrap();
        assert!(handoff_directory(&directory, nix::unistd::getuid().as_raw()).is_err());
        assert_eq!(fs::metadata(&directory).unwrap().mode() & 0o777, 0o770);
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(handoff_directory(&directory, nix::unistd::getuid().as_raw() + 1).is_err());
        let link = temp.path().join("symlink");
        std::os::unix::fs::symlink(&directory, &link).unwrap();
        assert!(handoff_directory(&link, nix::unistd::getuid().as_raw()).is_err());
        assert_eq!(fs::metadata(&directory).unwrap().mode() & 0o777, 0o700);
    }
}
