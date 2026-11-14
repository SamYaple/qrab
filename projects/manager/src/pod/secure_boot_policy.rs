//! Explicit next-cold-boot Secure Boot setting. Never edit a live varstore.
use super::Config;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const POLICY: &str = "secure-boot-settings.json";
const MAX_VARS: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecureBootSettings {
    pub supported: bool,
    /// Accepted next-cold-boot policy, or observed flag if no policy was staged.
    pub enabled: Option<bool>,
    pub current_setting: Option<bool>,
    pub pending_cold_boot: bool,
    pub generation: u64,
    pub applied_generation: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema: u32,
    state_id: String,
    name: String,
    generation: u64,
    enabled: bool,
    applied_generation: Option<u64>,
}
fn invalid(message: &str) -> Error {
    Error::Config(message.into())
}
fn capable(config: &Config) -> bool {
    config.uefi.as_ref().is_some_and(|u| u.secure_boot_capable)
}
fn read(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.len() > maximum as u64
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(invalid(
            "Secure Boot state must be a private bounded regular file",
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(invalid("Secure Boot state exceeds its bound"));
    }
    Ok(bytes)
}
fn directory(config: &Config) -> Result<PathBuf> {
    let directory = config.state_dir.join("identity");
    let metadata = fs::symlink_metadata(&directory)?;
    if !metadata.file_type().is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        return Err(invalid(
            "Secure Boot settings require a private VM identity directory",
        ));
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&read(&directory.join("manifest.json"), 64 * 1024)?)?;
    if manifest["version"] != 1
        || manifest["state_id"] != config.state_id.as_deref().unwrap_or(&config.name)
        || manifest["name"] != config.name
        || manifest["uefi"] != true
    {
        return Err(invalid(
            "Secure Boot settings do not match this VM identity",
        ));
    }
    Ok(directory)
}
fn load(config: &Config) -> Result<Option<Policy>> {
    let path = config.state_dir.join("identity").join(POLICY);
    let bytes = match read(&path, 4096) {
        Ok(bytes) => bytes,
        Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    directory(config)?;
    let policy: Policy = serde_json::from_slice(&bytes)?;
    if policy.schema != 1
        || policy.state_id != config.state_id.as_deref().unwrap_or(&config.name)
        || policy.name != config.name
        || policy.generation == 0
        || policy
            .applied_generation
            .is_some_and(|g| g > policy.generation)
    {
        return Err(invalid("Secure Boot policy identity/generation is invalid"));
    }
    Ok(Some(policy))
}
fn replace(directory: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let mut candidate = tempfile::NamedTempFile::new_in(directory)?;
    candidate
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    candidate.write_all(bytes)?;
    candidate.as_file().sync_all()?;
    candidate
        .persist(directory.join(name))
        .map_err(|e| Error::Io(e.error))?;
    File::open(directory)?.sync_all()?;
    Ok(())
}
fn save(directory: &Path, policy: &Policy) -> Result<()> {
    replace(directory, POLICY, &serde_json::to_vec(policy)?)
}

pub(super) fn settings(config: &Config) -> Result<SecureBootSettings> {
    let policy = load(config)?;
    if !capable(config) {
        if policy.as_ref().is_some_and(|p| p.enabled) {
            return Err(invalid(
                "An enabled Secure Boot policy requires secure-capable firmware",
            ));
        }
        return Ok(Default::default());
    }
    let current = match read(&config.state_dir.join("identity/OVMF_VARS.fd"), MAX_VARS) {
        Ok(bytes) => super::secure_boot_store::inspect(&bytes)?.enabled,
        Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    Ok(match policy {
        Some(policy) => SecureBootSettings {
            supported: true,
            enabled: Some(policy.enabled),
            current_setting: current,
            pending_cold_boot: policy.applied_generation != Some(policy.generation)
                || current != Some(policy.enabled),
            generation: policy.generation,
            applied_generation: policy.applied_generation,
        },
        None => SecureBootSettings {
            supported: true,
            enabled: current,
            current_setting: current,
            ..Default::default()
        },
    })
}
/// Called with a retained live supervisor lease. This writes only request metadata.
pub(super) fn stage(config: &Config, enabled: bool) -> Result<SecureBootSettings> {
    if !capable(config) {
        return Err(invalid(
            "Secure Boot management requires explicit secure-capable firmware",
        ));
    }
    let directory = directory(config)?;
    let current = read(&directory.join("OVMF_VARS.fd"), MAX_VARS)?;
    let info = super::secure_boot_store::inspect(&current)?;
    if enabled && !(info.has_pk && info.has_db) {
        return Err(invalid(
            "Enabling Secure Boot requires enrolled PK and allowed signature database",
        ));
    }
    let previous = load(config)?;
    let generation = previous
        .as_ref()
        .map_or(Some(1), |p| p.generation.checked_add(1))
        .ok_or_else(|| invalid("Secure Boot policy generation is exhausted"))?;
    let policy = Policy {
        schema: 1,
        state_id: config.state_id.as_deref().unwrap_or(&config.name).into(),
        name: config.name.clone(),
        generation,
        enabled,
        applied_generation: previous.and_then(|p| p.applied_generation),
    };
    save(&directory, &policy)?;
    Ok(SecureBootSettings {
        supported: true,
        enabled: Some(enabled),
        current_setting: info.enabled,
        pending_cold_boot: true,
        generation,
        applied_generation: policy.applied_generation,
    })
}
/// Startup-only under exclusive state lease, before any QEMU child exists.
pub(super) fn apply(config: &Config) -> Result<()> {
    let Some(mut policy) = load(config)? else {
        return Ok(());
    };
    if !capable(config) {
        return Err(invalid(
            "Retained Secure Boot settings require secure-capable firmware",
        ));
    }
    let directory = directory(config)?;
    let current = read(&directory.join("OVMF_VARS.fd"), MAX_VARS)?;
    let candidate = super::secure_boot_store::set_enabled(&current, policy.enabled)?;
    if candidate != current {
        // One prior store is retained, including its original keys/boot variables.
        replace(&directory, "OVMF_VARS.before-secure-boot.fd", &current)?;
        replace(&directory, "OVMF_VARS.fd", &candidate)?;
    }
    policy.applied_generation = Some(policy.generation);
    save(&directory, &policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrong_vm_metadata_and_unprotected_files_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let identity = dir.path().join("identity");
        fs::create_dir(&identity).unwrap();
        fs::set_permissions(&identity, fs::Permissions::from_mode(0o700)).unwrap();
        let config:Config=serde_json::from_value(serde_json::json!({"name":"vm","state_id":"uid","kernel":"/unused","state_dir":dir.path()})).unwrap();
        let policy = Policy {
            schema: 1,
            state_id: "other".into(),
            name: "vm".into(),
            generation: 1,
            enabled: true,
            applied_generation: None,
        };
        save(&identity, &policy).unwrap();
        assert!(settings(&config).is_err());
        fs::set_permissions(identity.join(POLICY), fs::Permissions::from_mode(0o666)).unwrap();
        assert!(load(&config).is_err());
        assert!(stage(&config, true).is_err());
    }
    #[test]
    fn defaults_do_not_read_firmware_or_enable_anything() {
        let config: Config = serde_json::from_value(
            serde_json::json!({"kernel":"/unused","state_dir":"/unused/qrab-secure-policy"}),
        )
        .unwrap();
        assert_eq!(settings(&config).unwrap(), SecureBootSettings::default());
        assert!(apply(&config).is_ok());
    }
    #[test]
    fn stage_is_metadata_only_and_cold_apply_backs_up_and_preserves_identity() {
        let dir = tempfile::tempdir().unwrap();
        let identity = dir.path().join("identity");
        fs::create_dir(&identity).unwrap();
        fs::set_permissions(&identity, fs::Permissions::from_mode(0o700)).unwrap();
        let config:Config=serde_json::from_value(serde_json::json!({"name":"vm","state_id":"uid","state_dir":dir.path(),"uefi":{"secure_boot_capable":true}})).unwrap();
        replace(
            &identity,
            "manifest.json",
            br#"{"version":1,"name":"vm","state_id":"uid","uefi":true}"#,
        )
        .unwrap();
        let original = super::super::secure_boot_store::test_store(true);
        replace(&identity, "OVMF_VARS.fd", &original).unwrap();
        replace(&identity, "tpm-marker", b"retained-key").unwrap();
        let requested = stage(&config, false).unwrap();
        assert!(requested.pending_cold_boot);
        assert_eq!(fs::read(identity.join("OVMF_VARS.fd")).unwrap(), original);
        assert_eq!(settings(&config).unwrap().current_setting, Some(true));
        apply(&config).unwrap();
        let applied = settings(&config).unwrap();
        assert!(!applied.pending_cold_boot);
        assert_eq!(applied.current_setting, Some(false));
        assert_eq!(
            fs::read(identity.join("OVMF_VARS.before-secure-boot.fd")).unwrap(),
            original
        );
        assert_eq!(
            fs::read(identity.join("tpm-marker")).unwrap(),
            b"retained-key"
        );
        let disabled = fs::read(identity.join("OVMF_VARS.fd")).unwrap();
        assert!(stage(&config, true).is_err());
        assert_eq!(fs::read(identity.join("OVMF_VARS.fd")).unwrap(), disabled);
        apply(&config).unwrap();
        assert_eq!(
            fs::read(identity.join("OVMF_VARS.before-secure-boot.fd")).unwrap(),
            original
        );
        let mut another = config;
        another.state_id = Some("other".into());
        assert!(apply(&another).is_err());
        assert_eq!(fs::read(identity.join("OVMF_VARS.fd")).unwrap(), disabled);
    }
}
