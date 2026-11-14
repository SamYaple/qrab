use super::{iscsi::Binding, Config};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    state_id: String,
    name: String,
    uefi: bool,
    tpm: bool,
    tpm_initialized: bool,
    /// Retain removed IDs so they cannot later be repurposed for another LUN.
    #[serde(default)]
    iscsi_disks: BTreeMap<String, Binding>,
}

/// The PVC mount root may be group-writable through fsGroup. All identity
/// material lives in an owner-only child, protected by a separate state lock.
pub(super) struct PersistentState {
    directory: PathBuf,
    manifest: Manifest,
    _lock: StateLease,
}

pub(super) type StateLease = Arc<nix::fcntl::Flock<File>>;

fn private_directory(path: &Path) -> Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if !fs::symlink_metadata(path)?.file_type().is_dir() {
                return Err(Error::Config(format!(
                    "identity path is not a directory: {}",
                    path.display()
                )));
            }
        }
        Err(error) => return Err(error.into()),
    }
    // Kubelet fsGroup ownership changes can add group bits on a remount.
    // Narrow them again before opening retained identity files.
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn private_file(path: &Path) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(Error::Config(format!(
            "identity file is not a regular file: {}",
            path.display()
        )));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(metadata)
}

fn write_manifest(directory: &Path, manifest: &Manifest) -> Result<()> {
    let mut candidate = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(&mut candidate, manifest)?;
    candidate.write_all(b"\n")?;
    candidate.as_file().sync_all()?;
    candidate
        .persist(directory.join("manifest.json"))
        .map_err(|e| Error::Io(e.error))?;
    File::open(directory)?.sync_all()?;
    Ok(())
}

impl PersistentState {
    pub(super) fn prepare(config: &Config) -> Result<Option<Self>> {
        if config.uefi.is_none() && config.tpm.is_none() && config.iscsi_disks.is_empty() {
            return Ok(None);
        }
        fs::create_dir_all(&config.state_dir)?;
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(config.state_dir.join("owner.lock"))?;
        lock_file.set_permissions(fs::Permissions::from_mode(0o600))?;
        let lock = nix::fcntl::Flock::lock(lock_file, nix::fcntl::FlockArg::LockExclusiveNonblock)
            .map_err(|_| Error::Config("another manager owns this persistent VM state".into()))?;
        let directory = config.state_dir.join("identity");
        if config.incoming.is_some() && fs::symlink_metadata(&directory).is_ok() {
            // The guest's TPM and UEFI state arrive in the migration stream; a
            // retained identity here would be a second, divergent copy.
            return Err(Error::Config(
                "a migration target needs empty VM state; refusing to overwrite retained identity"
                    .into(),
            ));
        }
        let expected = Manifest {
            version: 1,
            state_id: config.state_id.as_deref().unwrap_or(&config.name).into(),
            name: config.name.clone(),
            uefi: config.uefi.is_some(),
            tpm: config.tpm.is_some(),
            tpm_initialized: false,
            iscsi_disks: config
                .iscsi_disks
                .iter()
                .map(|disk| (disk.id.clone(), disk.binding()))
                .collect(),
        };
        let mut manifest = match fs::symlink_metadata(&directory) {
            Ok(_) => {
                private_directory(&directory)?;
                let path = directory.join("manifest.json");
                private_file(&path).map_err(|e| Error::Config(format!("persistent identity manifest is missing or invalid; refusing to reset: {e}")))?;
                let manifest: Manifest = serde_json::from_slice(&fs::read(path)?).map_err(|e| {
                    Error::Config(format!(
                        "persistent identity manifest is invalid; refusing to reset: {e}"
                    ))
                })?;
                if manifest.version != expected.version
                    || manifest.state_id != expected.state_id
                    || manifest.name != expected.name
                    || manifest.uefi != expected.uefi
                    || manifest.tpm != expected.tpm
                {
                    return Err(Error::Config("persistent VM state belongs to a different state_id/name or boot/TPM configuration; deliberate identity migration is required".into()));
                }
                manifest
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let candidate = tempfile::Builder::new()
                    .prefix(".identity-init-")
                    .permissions(fs::Permissions::from_mode(0o700))
                    .tempdir_in(&config.state_dir)?;
                if let Some(uefi) = &config.uefi {
                    let path = candidate.path().join("OVMF_VARS.fd");
                    fs::copy(&uefi.vars_template, &path)?;
                    private_file(&path)?;
                    File::open(path)?.sync_all()?;
                }
                if config.tpm.is_some() {
                    private_directory(&candidate.path().join("tpm"))?;
                }
                write_manifest(candidate.path(), &expected)?;
                fs::rename(candidate.path(), &directory)?;
                File::open(&config.state_dir)?.sync_all()?;
                expected
            }
            Err(error) => return Err(error.into()),
        };
        if let Some(uefi) = &config.uefi {
            let vars = directory.join("OVMF_VARS.fd");
            let metadata = private_file(&vars)?;
            if metadata.len() != fs::metadata(&uefi.vars_template)?.len() {
                return Err(Error::Config(
                    "retained UEFI variables have the wrong size; refusing to reset".into(),
                ));
            }
            // Firmware-volume headers are immutable; variable updates occur
            // after this header. Reject obvious corruption instead of reseeding.
            let mut current = [0; 64];
            let mut template = [0; 64];
            File::open(&vars)?.read_exact(&mut current)?;
            File::open(&uefi.vars_template)?.read_exact(&mut template)?;
            if current != template {
                return Err(Error::Config(
                    "retained UEFI variable header is corrupt; refusing to reset".into(),
                ));
            }
        }
        if config.tpm.is_some() {
            let tpm = directory.join("tpm");
            private_directory(&tpm)?;
            for entry in fs::read_dir(&tpm)? {
                private_file(&entry?.path())?;
            }
            let permanent = tpm.join("tpm2-00.permall");
            match fs::symlink_metadata(&permanent) {
                Ok(metadata) if metadata.file_type().is_file() && metadata.len()>0 => {},
                Err(error) if error.kind() == std::io::ErrorKind::NotFound && !manifest.tpm_initialized => {},
                _ => return Err(Error::Config("retained TPM permanent state is missing or invalid; refusing to create a new identity".into())),
            }
        }
        let mut added = false;
        for disk in &config.iscsi_disks {
            let binding = disk.binding();
            match manifest.iscsi_disks.get(&disk.id) {
                Some(previous) if previous!=&binding => return Err(Error::Config(format!("iSCSI disk {} is bound to a different portal/target/LUN/initiator; deliberate disk identity migration is required",disk.id))),
                Some(_) => {},
                None => {manifest.iscsi_disks.insert(disk.id.clone(),binding);added=true;},
            }
        }
        if added {
            write_manifest(&directory, &manifest)?;
        }
        Ok(Some(Self {
            directory,
            manifest,
            _lock: Arc::new(lock),
        }))
    }

    pub(super) fn tpm_directory(&self) -> PathBuf {
        self.directory.join("tpm")
    }

    pub(super) fn lease(&self) -> StateLease {
        self._lock.clone()
    }

    /// Every iSCSI binding this state has recorded, including removed IDs.
    pub(super) fn bindings(&self) -> BTreeMap<String, Binding> {
        self.manifest.iscsi_disks.clone()
    }

    /// Adopt a migration source's retained bindings; a conflicting binding
    /// for the same ID is refused.
    pub(super) fn retain(&mut self, bindings: &BTreeMap<String, Binding>) -> Result<()> {
        let mut added = false;
        for (id, binding) in bindings {
            match self.manifest.iscsi_disks.get(id) {
                Some(current) if current != binding => {
                    return Err(Error::Config(format!(
                        "iSCSI disk {id} is bound differently on the migration source"
                    )))
                }
                Some(_) => {}
                None => {
                    self.manifest
                        .iscsi_disks
                        .insert(id.clone(), binding.clone());
                    added = true;
                }
            }
        }
        if added {
            write_manifest(&self.directory, &self.manifest)?;
        }
        Ok(())
    }

    pub(super) fn mark_tpm_initialized(&mut self) -> Result<()> {
        if self.manifest.tpm_initialized {
            return Ok(());
        }
        let path = self.tpm_directory().join("tpm2-00.permall");
        if private_file(&path)?.len() == 0 {
            return Err(Error::Config(
                "swtpm did not create permanent identity state".into(),
            ));
        }
        self.manifest.tpm_initialized = true;
        write_manifest(&self.directory, &self.manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Config) {
        let dir = tempfile::tempdir().unwrap();
        for name in ["code", "vars", "boot"] {
            fs::write(dir.path().join(name), vec![0x42; 128]).unwrap();
        }
        let config = serde_json::from_value(serde_json::json!({
            "name":"vm", "qemu_binary":"/bin/true", "state_id":"cr-uid-one", "state_dir":dir.path().join("state"),
            "uefi":{"code":dir.path().join("code"),"vars_template":dir.path().join("vars"),"boot_disk":dir.path().join("boot")}
        })).unwrap();
        (dir, config)
    }

    #[test]
    fn nvram_is_retained_and_state_owner_is_exclusive_across_runtime_directories() {
        let (_directory, mut config) = fixture();
        let state = PersistentState::prepare(&config).unwrap().unwrap();
        config.runtime_dir = "/different/runtime".into();
        assert!(PersistentState::prepare(&config)
            .err()
            .unwrap()
            .to_string()
            .contains("another manager"));
        let vars = state.directory.join("OVMF_VARS.fd");
        let mut contents = fs::read(&vars).unwrap();
        contents[100] = 0x43;
        fs::write(&vars, &contents).unwrap();
        // Simulate kubelet adding group permissions while a retained PVC remounts.
        fs::set_permissions(&state.directory, fs::Permissions::from_mode(0o770)).unwrap();
        fs::set_permissions(&vars, fs::Permissions::from_mode(0o660)).unwrap();
        drop(state);
        let state = PersistentState::prepare(&config).unwrap().unwrap();
        assert_eq!(fs::read(&vars).unwrap(), contents);
        assert_eq!(
            fs::metadata(&state.directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&vars).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn a_new_cr_cannot_adopt_an_existing_tpm_or_nvram_identity() {
        let (_directory, mut config) = fixture();
        let state = PersistentState::prepare(&config).unwrap().unwrap();
        let vars = fs::read(state.directory.join("OVMF_VARS.fd")).unwrap();
        drop(state);
        config.state_id = Some("cr-uid-two".into());
        assert!(PersistentState::prepare(&config)
            .err()
            .unwrap()
            .to_string()
            .contains("different state_id"));
        assert_eq!(
            fs::read(config.state_dir.join("identity/OVMF_VARS.fd")).unwrap(),
            vars
        );
    }

    #[test]
    fn corrupt_nvram_and_missing_initialized_tpm_state_are_never_reset() {
        let (_directory, mut config) = fixture();
        let state = PersistentState::prepare(&config).unwrap().unwrap();
        let vars = state.directory.join("OVMF_VARS.fd");
        drop(state);
        fs::write(&vars, [1, 2, 3]).unwrap();
        assert!(PersistentState::prepare(&config)
            .err()
            .unwrap()
            .to_string()
            .contains("wrong size"));
        assert_eq!(fs::read(&vars).unwrap(), [1, 2, 3]);

        let (_directory2, config2) = fixture();
        config = config2;
        config.tpm = Some(super::super::Tpm {
            swtpm_binary: "/bin/true".into(),
        });
        let mut state = PersistentState::prepare(&config).unwrap().unwrap();
        let permanent = state.tpm_directory().join("tpm2-00.permall");
        fs::write(&permanent, b"retained seed placeholder").unwrap();
        state.mark_tpm_initialized().unwrap();
        drop(state);
        fs::remove_file(&permanent).unwrap();
        assert!(PersistentState::prepare(&config)
            .err()
            .unwrap()
            .to_string()
            .contains("refusing to create a new identity"));
        assert!(!permanent.exists());
    }

    #[test]
    fn malformed_manifests_and_symlinked_identity_files_are_refused() {
        let (_directory, config) = fixture();
        let state = PersistentState::prepare(&config).unwrap().unwrap();
        let manifest = state.directory.join("manifest.json");
        drop(state);
        fs::write(&manifest, b"not JSON").unwrap();
        assert!(PersistentState::prepare(&config).is_err());
        assert_eq!(fs::read(&manifest).unwrap(), b"not JSON");

        let (directory, config) = fixture();
        let state = PersistentState::prepare(&config).unwrap().unwrap();
        let vars = state.directory.join("OVMF_VARS.fd");
        drop(state);
        fs::remove_file(&vars).unwrap();
        std::os::unix::fs::symlink(directory.path().join("vars"), &vars).unwrap();
        assert!(PersistentState::prepare(&config)
            .err()
            .unwrap()
            .to_string()
            .contains("not a regular file"));
    }

    #[test]
    fn iscsi_ids_bind_endpoints_but_allow_credential_and_presentation_changes() {
        let (_directory, mut config) = fixture();
        let state = PersistentState::prepare(&config).unwrap().unwrap();
        let manifest = state.directory.join("manifest.json");
        drop(state);
        // The previous release's manifest had no native-disk field.
        let mut previous: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        previous.as_object_mut().unwrap().remove("iscsi_disks");
        fs::write(&manifest, serde_json::to_vec(&previous).unwrap()).unwrap();
        let disk:super::super::IscsiDisk=serde_json::from_value(serde_json::json!({
            "id":"data","portal":"127.0.0.1:3260","target":"iqn.2026-11.com.example:disk","lun":1,"initiator_name":"iqn.2026-11.com.example:vm.test","username":"first","password_file":"/first/secret"
        })).unwrap();
        config.iscsi_disks.push(disk.clone());
        drop(PersistentState::prepare(&config).unwrap());
        config.iscsi_disks[0].username = "rotated".into();
        config.iscsi_disks[0].password_file = "/rotated/secret".into();
        config.iscsi_disks[0].read_only = true;
        config.iscsi_disks[0].bus = super::super::IscsiBus::Scsi;
        config.iscsi_disks[0].boot_index = Some(2);
        drop(PersistentState::prepare(&config).unwrap());
        let mut added = disk.clone();
        added.id = "second".into();
        added.lun = 2;
        config.iscsi_disks.push(added);
        drop(PersistentState::prepare(&config).unwrap());
        config.iscsi_disks.clear();
        drop(PersistentState::prepare(&config).unwrap());
        let mut repurposed = disk;
        repurposed.target = "iqn.2026-11.com.example:different".into();
        config.iscsi_disks.push(repurposed);
        assert!(PersistentState::prepare(&config)
            .err()
            .unwrap()
            .to_string()
            .contains("different portal/target/LUN/initiator"));
    }
}
