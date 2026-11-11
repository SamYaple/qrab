//! Fixed-scope root administration for qrab's dedicated ZFS/LIO targets.
#![deny(unsafe_code)]

mod host;
mod store;

use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::PathBuf};

pub use host::LinuxHost;
pub use store::Store;

pub const PARENT: &str = "tank/qrab-iscsi";
pub const ROOT: &str = "/var/lib/qrab-iscsi-target";
pub const PORTAL: &str = "10.203.0.1:3260";
pub const HBA: &str = "iblock_47";
pub const OWNER: &str = "qrab-iscsi-target-v1";
pub const MAX_VOLUME: u64 = 1024 * 1024 * 1024;
pub const PARENT_QUOTA: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub vm_uid: String,
    pub capacity_bytes: u64,
}
impl Spec {
    pub fn new(vm_uid: String, capacity_bytes: u64) -> Result<Self> {
        let spec = Self {
            vm_uid,
            capacity_bytes,
        };
        spec.validate()?;
        Ok(spec)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.vm_uid.len() == 36,
            "VM UID must be a canonical lowercase UUID"
        );
        for (i, byte) in self.vm_uid.bytes().enumerate() {
            ensure!(
                if [8, 13, 18, 23].contains(&i) {
                    byte == b'-'
                } else {
                    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
                },
                "VM UID must be a canonical lowercase UUID"
            );
        }
        ensure!(
            (16 * 1024 * 1024..=MAX_VOLUME).contains(&self.capacity_bytes)
                && self.capacity_bytes.is_multiple_of(1024 * 1024),
            "volume size must be 16..1024 whole MiB"
        );
        Ok(())
    }
    pub fn dataset(&self) -> String {
        format!("{PARENT}/vm-{}", self.vm_uid)
    }
    pub fn device(&self) -> PathBuf {
        PathBuf::from("/dev/zvol").join(self.dataset())
    }
    pub fn target(&self) -> String {
        format!("iqn.2026-11.com.yapplesauce:qrab.disk.{}", self.vm_uid)
    }
    pub fn initiator(&self) -> String {
        format!("iqn.2026-11.com.yapplesauce:qrab.vm.{}", self.vm_uid)
    }
    pub fn username(&self) -> String {
        format!("qrab-{}", self.vm_uid.replace('-', ""))
    }
    pub fn object(&self) -> String {
        format!("qrab-{}", self.vm_uid)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Preparing,
    Active,
    Fencing,
    Fenced,
}

// No Debug implementation: this is the authoritative root-only secret record.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema: u32,
    pub owner: String,
    pub spec: Spec,
    pub generation: u64,
    pub phase: Phase,
    pub zvol_guid: Option<String>,
    pub chap_password: String,
}
impl Record {
    fn validate(&self) -> Result<()> {
        self.spec.validate()?;
        ensure!(
            self.schema == 1 && self.owner == OWNER && self.generation > 0,
            "unknown target manifest owner/version"
        );
        ensure!(
            self.chap_password.len() == 32
                && self
                    .chap_password
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "target secret state is malformed"
        );
        ensure!(
            self.zvol_guid
                .as_ref()
                .is_none_or(|g| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit())),
            "target GUID state is malformed"
        );
        ensure!(
            self.phase != Phase::Active || self.zvol_guid.is_some(),
            "active target has no recorded zvol GUID"
        );
        Ok(())
    }
    pub fn report(&self) -> Report {
        Report::for_spec(&self.spec, Some(self.phase), self.generation)
    }
}

#[derive(Serialize)]
pub struct Report {
    pub schema: u32,
    pub vm_uid: String,
    pub capacity_bytes: u64,
    pub dataset: String,
    pub target_iqn: String,
    pub initiator_iqn: String,
    pub portal: String,
    pub lun: u32,
    pub chap_username: String,
    pub generation: u64,
    pub phase: Option<Phase>,
    pub parent_quota_bytes: u64,
    pub root_state: String,
    pub dedicated_hba: String,
}
impl Report {
    pub fn for_spec(spec: &Spec, phase: Option<Phase>, generation: u64) -> Self {
        Self {
            schema: 1,
            vm_uid: spec.vm_uid.clone(),
            capacity_bytes: spec.capacity_bytes,
            dataset: spec.dataset(),
            target_iqn: spec.target(),
            initiator_iqn: spec.initiator(),
            portal: PORTAL.into(),
            lun: 0,
            chap_username: spec.username(),
            generation,
            phase,
            parent_quota_bytes: PARENT_QUOTA,
            root_state: ROOT.into(),
            dedicated_hba: HBA.into(),
        }
    }
}

/// Target operations complete only when the host has observed the actual state.
/// In particular, fence is not a sleep/lease: it must remove kernel references.
pub trait Backend {
    fn preflight_new_scope(&mut self) -> Result<()>;
    fn ensure_pool(&mut self, guid: Option<&str>) -> Result<String>;
    fn preflight_new_volume(&mut self, spec: &Spec) -> Result<()>;
    fn ensure_volume(&mut self, spec: &Spec, guid: Option<&str>) -> Result<String>;
    fn activate(&mut self, record: &Record) -> Result<()>;
    fn verify_active(&mut self, record: &Record) -> Result<()>;
    fn verify_fenced(&mut self, record: &Record) -> Result<()>;
    fn fence(&mut self, record: &Record) -> Result<()>;
    fn restore_active(&mut self, record: &Record) -> Result<()>;
}

fn password() -> Result<String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub fn apply<B: Backend>(store: &Store, host: &mut B, spec: Spec) -> Result<Report> {
    spec.validate()?;
    let _lock = store.lock()?;
    let existing = store.read(&spec.vm_uid)?;
    if let Some(record) = &existing {
        ensure!(record.spec==spec,"existing VM target has another capacity/identity; adoption and resize are not supported");
        ensure!(
            record.phase != Phase::Fencing,
            "an interrupted fence must finish before recovery/apply"
        );
    }
    let pool = store.pool_guid()?;
    if pool.is_none() && !store.pool_intent_exists()? {
        host.preflight_new_scope()?;
        store.write_pool(None)?;
    }
    let guid = host.ensure_pool(pool.as_deref())?;
    store.write_pool(Some(&guid))?;
    let mut record = match existing {
        Some(record) => {
            ensure!(record.spec == spec, "existing VM target has another capacity/identity; adoption and resize are not supported");
            record
        }
        None => {
            host.preflight_new_volume(&spec)?;
            let record = Record {
                schema: 1,
                owner: OWNER.into(),
                spec,
                generation: 1,
                phase: Phase::Preparing,
                zvol_guid: None,
                chap_password: password()?,
            };
            store.write(&record)?;
            record
        }
    };
    match record.phase {
        Phase::Fencing => bail!("an interrupted fence must finish before recovery/apply"),
        Phase::Active => {
            let current = host.ensure_volume(&record.spec, record.zvol_guid.as_deref())?;
            ensure!(
                record.zvol_guid.as_deref() == Some(current.as_str()),
                "existing volume GUID changed"
            );
            host.verify_active(&record)?;
            return Ok(record.report());
        }
        Phase::Fenced => {
            host.verify_fenced(&record)?;
            record.generation = record
                .generation
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("credential generation exhausted"))?;
            record.chap_password = password()?;
            record.phase = Phase::Preparing;
            store.write(&record)?;
        }
        Phase::Preparing => {}
    }
    let guid = host.ensure_volume(&record.spec, record.zvol_guid.as_deref())?;
    record.zvol_guid = Some(guid);
    store.write(&record)?;
    host.activate(&record)?;
    host.verify_active(&record)?;
    record.phase = Phase::Active;
    store.write(&record)?;
    Ok(record.report())
}

pub fn fence<B: Backend>(store: &Store, host: &mut B, vm_uid: &str) -> Result<Report> {
    let _lock = store.lock()?;
    let mut record = store
        .read(vm_uid)?
        .ok_or_else(|| anyhow::anyhow!("no owned target manifest exists"))?;
    if record.phase == Phase::Fenced {
        host.verify_fenced(&record)?;
        return Ok(record.report());
    }
    record.phase = Phase::Fencing;
    store.write(&record)?;
    host.fence(&record)?;
    host.verify_fenced(&record)?;
    record.phase = Phase::Fenced;
    store.write(&record)?;
    Ok(record.report())
}

/// Boot restoration preserves active credentials. Every other phase stays off.
pub fn restore<B: Backend>(store: &Store, host: &mut B) -> Result<Vec<Report>> {
    let _lock = store.lock()?;
    let records = store.records()?;
    if records.is_empty() {
        return Ok(Vec::new());
    }
    let pool = store
        .pool_guid()?
        .ok_or_else(|| anyhow::anyhow!("owned parent GUID absent"))?;
    host.ensure_pool(Some(&pool))?;
    let mut reports = Vec::new();
    for mut record in records {
        if record.phase == Phase::Active {
            host.restore_active(&record)?;
        } else {
            record.phase = Phase::Fencing;
            store.write(&record)?;
            host.fence(&record)?;
            host.verify_fenced(&record)?;
            record.phase = Phase::Fenced;
            store.write(&record)?;
        }
        reports.push(record.report());
    }
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    const UID: &str = "e309f751-5dda-4271-a0f4-4454c9527a59";
    #[derive(Default)]
    struct Fake {
        active: bool,
        fail_fence: bool,
        foreign: bool,
        secret: String,
        calls: Vec<&'static str>,
        pool_calls: u32,
    }
    impl Backend for Fake {
        fn preflight_new_scope(&mut self) -> Result<()> {
            Ok(())
        }
        fn ensure_pool(&mut self, _: Option<&str>) -> Result<String> {
            self.pool_calls += 1;
            Ok("12".into())
        }
        fn preflight_new_volume(&mut self, _: &Spec) -> Result<()> {
            ensure!(!self.foreign, "foreign target");
            Ok(())
        }
        fn ensure_volume(&mut self, _: &Spec, _: Option<&str>) -> Result<String> {
            Ok("34".into())
        }
        fn activate(&mut self, r: &Record) -> Result<()> {
            self.calls.push("activate");
            self.active = true;
            self.secret = r.chap_password.clone();
            Ok(())
        }
        fn verify_active(&mut self, r: &Record) -> Result<()> {
            ensure!(self.active && self.secret == r.chap_password, "not active");
            Ok(())
        }
        fn verify_fenced(&mut self, _: &Record) -> Result<()> {
            ensure!(!self.active, "not drained");
            Ok(())
        }
        fn fence(&mut self, _: &Record) -> Result<()> {
            self.calls.push("fence");
            ensure!(!self.fail_fence, "drain failed");
            self.active = false;
            Ok(())
        }
        fn restore_active(&mut self, r: &Record) -> Result<()> {
            self.activate(r)
        }
    }
    fn fixture() -> (tempfile::TempDir, Store, Spec) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::test(dir.path().join("state"));
        let spec = Spec::new(UID.into(), 128 * 1024 * 1024).unwrap();
        (dir, store, spec)
    }
    #[test]
    fn validation_keeps_identifiers_and_capacity_in_scope() {
        for uid in [
            "../foreign",
            "E309f751-5dda-4271-a0f4-4454c9527a59",
            "e309f751-5dda-4271-a0f4-4454c9527a5/",
        ] {
            assert!(Spec::new(uid.into(), 128 * 1024 * 1024).is_err());
        }
        for size in [0, MAX_VOLUME + 1, 17 * 1024 * 1024 + 1] {
            assert!(Spec::new(UID.into(), size).is_err());
        }
    }
    #[test]
    fn fence_is_required_and_recovery_rotates_revoked_credentials() {
        let (_dir, store, spec) = fixture();
        let mut host = Fake::default();
        apply(&store, &mut host, spec.clone()).unwrap();
        let old = host.secret.clone();
        apply(&store, &mut host, spec.clone()).unwrap();
        assert_eq!(host.secret, old);
        fence(&store, &mut host, UID).unwrap();
        assert!(!host.active);
        apply(&store, &mut host, spec).unwrap();
        assert_ne!(host.secret, old);
        assert_eq!(store.read(UID).unwrap().unwrap().generation, 2);
    }
    #[test]
    fn failed_drain_never_records_a_fence_or_permits_reactivation() {
        let (_dir, store, spec) = fixture();
        let mut host = Fake::default();
        apply(&store, &mut host, spec.clone()).unwrap();
        host.fail_fence = true;
        assert!(fence(&store, &mut host, UID).is_err());
        assert_eq!(store.read(UID).unwrap().unwrap().phase, Phase::Fencing);
        assert!(apply(&store, &mut host, spec).is_err());
        assert_eq!(host.calls, vec!["activate", "fence"]);
    }
    #[test]
    fn foreign_resources_and_mutated_identity_are_never_adopted() {
        let (_dir, store, spec) = fixture();
        let mut host = Fake {
            foreign: true,
            ..Default::default()
        };
        assert!(apply(&store, &mut host, spec.clone()).is_err());
        assert!(store.read(UID).unwrap().is_none());
        assert!(host.calls.is_empty());
        host.foreign = false;
        apply(&store, &mut host, spec).unwrap();
        assert!(apply(
            &store,
            &mut host,
            Spec::new(UID.into(), 256 * 1024 * 1024).unwrap()
        )
        .is_err());
    }
    #[test]
    fn public_reports_exclude_password_and_private_state_stays_private() {
        let (_dir, store, spec) = fixture();
        let mut host = Fake::default();
        let report = apply(&store, &mut host, spec).unwrap();
        let public = serde_json::to_string(&report).unwrap();
        assert!(!public.contains(&host.secret));
        assert!(!public.contains("chap_password"));
        let meta = std::fs::metadata(store.manifest_path(UID)).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(meta.uid(), nix::unistd::getuid().as_raw());
    }
    #[test]
    fn boot_restore_does_not_rotate_active_credentials_or_resume_fenced_targets() {
        let (_dir, store, spec) = fixture();
        let mut host = Fake::default();
        apply(&store, &mut host, spec).unwrap();
        let secret = host.secret.clone();
        host.active = false;
        restore(&store, &mut host).unwrap();
        assert!(host.active);
        assert_eq!(host.secret, secret);
        assert_eq!(store.read(UID).unwrap().unwrap().generation, 1);
        fence(&store, &mut host, UID).unwrap();
        restore(&store, &mut host).unwrap();
        assert!(!host.active);
        assert_eq!(store.read(UID).unwrap().unwrap().phase, Phase::Fenced);
    }
    #[test]
    fn unsafe_manifest_permissions_symlinks_and_changed_guid_fail_closed() {
        let (_dir, store, spec) = fixture();
        let mut host = Fake::default();
        apply(&store, &mut host, spec.clone()).unwrap();
        let mut record = store.read(UID).unwrap().unwrap();
        record.zvol_guid = Some("999".into());
        store.write(&record).unwrap();
        assert!(apply(&store, &mut host, spec).is_err());
        let path = store.manifest_path(UID);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.read(UID).is_err());
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink("missing", &path).unwrap();
        assert!(store.read(UID).is_err());
    }
    #[test]
    fn active_manifest_without_guid_is_rejected_before_host_operations() {
        let (_dir, store, spec) = fixture();
        let mut host = Fake::default();
        apply(&store, &mut host, spec.clone()).unwrap();
        let mut record = store.read(UID).unwrap().unwrap();
        record.zvol_guid = None;
        assert!(record.validate().is_err());
        std::fs::write(
            store.manifest_path(UID),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        let calls = host.pool_calls;
        assert!(apply(&store, &mut host, spec).is_err());
        assert_eq!(host.pool_calls, calls);
    }
}
