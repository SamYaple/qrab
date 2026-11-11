//! Fixed-scope root administration for qrab's dedicated ZFS/LIO targets.
#![deny(unsafe_code)]

pub mod disk;
mod host;
mod store;

use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::PathBuf};

pub use disk::Image;
pub use host::LinuxHost;
pub use store::Store;

pub const PARENT: &str = "tank/qrab-iscsi";
pub const ROOT: &str = "/var/lib/qrab-iscsi-target";
pub const PORTAL: &str = "10.203.0.1:3260";
pub const HBA: &str = "iblock_47";
pub const OWNER: &str = "qrab-iscsi-target-v1";
pub const MIN_VOLUME: u64 = 16 * 1024 * 1024;
/// Legacy single-disk demo bound. Provisioned disks use [`MAX_DISK`].
pub const MAX_VOLUME: u64 = 1024 * 1024 * 1024;
/// Per-disk bound; the aggregate logical budget is [`PARENT_QUOTA`].
pub const MAX_DISK: u64 = 1 << 40;
pub const PARENT_QUOTA: u64 = 8 * 1024 * 1024 * 1024;

/// A stable machine-readable refusal reported to the storage daemon caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusal(pub &'static str);
impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Refusal {}

/// The first refusal code in an error chain, if any.
pub fn refusal(error: &anyhow::Error) -> Option<&'static str> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<Refusal>())
        .map(|r| r.0)
}

pub fn valid_uid(uid: &str) -> bool {
    uid.len() == 36
        && uid.bytes().enumerate().all(|(i, byte)| {
            if [8, 13, 18, 23].contains(&i) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

/// Lowercase letters/digits/hyphens with alphanumeric ends, 1..=`max` bytes.
pub fn valid_name(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
}

pub fn valid_password(password: &str) -> bool {
    password.len() == 32
        && password
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn valid_guid(guid: &Option<String>) -> bool {
    guid.as_ref()
        .is_none_or(|g| !g.is_empty() && g.bytes().all(|b| b.is_ascii_digit()))
}

/// A sealed base-image generation that CoW clones descend from.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Origin {
    pub image: String,
    pub generation: u32,
}
impl Origin {
    pub fn new(image: String, generation: u32) -> Result<Self> {
        let origin = Self { image, generation };
        origin.validate()?;
        Ok(origin)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            valid_name(&self.image, 32) && (1..=999_999).contains(&self.generation),
            Refusal("invalid-image")
        );
        Ok(())
    }
    pub fn key(&self) -> String {
        format!("image-{}-g{}", self.image, self.generation)
    }
    pub fn dataset(&self) -> String {
        format!("{PARENT}/{}", self.key())
    }
    pub fn snapshot(&self) -> String {
        format!("{}@sealed", self.dataset())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub vm_uid: String,
    pub capacity_bytes: u64,
    /// Absent only for the legacy single-disk demo record (schema 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
}
impl Spec {
    pub fn new(vm_uid: String, capacity_bytes: u64) -> Result<Self> {
        let spec = Self {
            vm_uid,
            capacity_bytes,
            disk: None,
            origin: None,
        };
        spec.validate()?;
        Ok(spec)
    }
    pub fn disk(
        vm_uid: String,
        disk: String,
        capacity_bytes: u64,
        origin: Option<Origin>,
    ) -> Result<Self> {
        let spec = Self {
            vm_uid,
            capacity_bytes,
            disk: Some(disk),
            origin,
        };
        spec.validate()?;
        Ok(spec)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(valid_uid(&self.vm_uid), Refusal("invalid-vm-uid"));
        let most = if self.disk.is_some() {
            MAX_DISK
        } else {
            MAX_VOLUME
        };
        ensure!(
            (MIN_VOLUME..=most).contains(&self.capacity_bytes)
                && self.capacity_bytes.is_multiple_of(1024 * 1024),
            Refusal("invalid-capacity")
        );
        if let Some(disk) = &self.disk {
            ensure!(valid_name(disk, 20), Refusal("invalid-disk-id"));
        }
        if let Some(origin) = &self.origin {
            ensure!(self.disk.is_some(), "legacy demo disks have no origin");
            origin.validate()?;
        }
        Ok(())
    }
    /// The journal key: `<uid>` (legacy demo) or `<uid>.<disk>`.
    pub fn key(&self) -> String {
        match &self.disk {
            None => self.vm_uid.clone(),
            Some(disk) => format!("{}.{disk}", self.vm_uid),
        }
    }
    pub fn dataset(&self) -> String {
        match &self.disk {
            None => format!("{PARENT}/vm-{}", self.vm_uid),
            Some(disk) => format!("{PARENT}/disk-{}-{disk}", self.vm_uid),
        }
    }
    pub fn device(&self) -> PathBuf {
        PathBuf::from("/dev/zvol").join(self.dataset())
    }
    pub fn target(&self) -> String {
        match &self.disk {
            None => format!("iqn.2026-11.com.yapplesauce:qrab.disk.{}", self.vm_uid),
            Some(disk) => format!(
                "iqn.2026-11.com.yapplesauce:qrab.disk.{}.{disk}",
                self.vm_uid
            ),
        }
    }
    pub fn initiator(&self) -> String {
        format!("iqn.2026-11.com.yapplesauce:qrab.vm.{}", self.vm_uid)
    }
    pub fn username(&self) -> String {
        match &self.disk {
            None => format!("qrab-{}", self.vm_uid.replace('-', "")),
            Some(disk) => format!("qrab-{}-{disk}", self.vm_uid.replace('-', "")),
        }
    }
    pub fn object(&self) -> String {
        match &self.disk {
            None => format!("qrab-{}", self.vm_uid),
            Some(disk) => format!("qrab-{}-{disk}", self.vm_uid),
        }
    }
    /// The hold tag a clone keeps on its sealed origin snapshot.
    pub fn clone_hold(&self) -> String {
        format!("qrab-clone-{}", self.key())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Provisioned storage that has never been exported (generation 0).
    Provisioned,
    Preparing,
    Active,
    Fencing,
    Fenced,
    /// Export objects removed; data retained for explicit offline disposal.
    Retired,
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
    /// Recorded GUID of the sealed image snapshot a clone descends from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_guid: Option<String>,
    pub chap_password: String,
}
impl Record {
    fn validate(&self) -> Result<()> {
        self.spec.validate()?;
        let legacy = self.spec.disk.is_none();
        ensure!(
            self.owner == OWNER
                && if legacy {
                    self.schema == 1
                        && self.generation > 0
                        && self.origin_guid.is_none()
                        && !matches!(self.phase, Phase::Provisioned | Phase::Retired)
                } else {
                    self.schema == 2
                },
            "unknown target manifest owner/version"
        );
        // Generation 0 means never exported and therefore no credential.
        ensure!(
            if self.generation == 0 {
                self.chap_password.is_empty()
                    && matches!(self.phase, Phase::Provisioned | Phase::Retired)
            } else {
                valid_password(&self.chap_password) && self.phase != Phase::Provisioned
            },
            "target secret state is malformed"
        );
        ensure!(
            valid_guid(&self.zvol_guid) && valid_guid(&self.origin_guid),
            "target GUID state is malformed"
        );
        ensure!(
            matches!(self.phase, Phase::Provisioned | Phase::Preparing) || self.zvol_guid.is_some(),
            "active target has no recorded zvol GUID"
        );
        ensure!(
            self.spec.origin.is_some() == self.origin_guid.is_some() || self.zvol_guid.is_none(),
            "clone origin GUID state is malformed"
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
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
            schema: if spec.disk.is_some() { 2 } else { 1 },
            vm_uid: spec.vm_uid.clone(),
            disk: spec.disk.clone(),
            origin: spec.origin.clone(),
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
    /// Creates a thick blank zvol, or verifies the recorded one.
    fn ensure_volume(&mut self, spec: &Spec, guid: Option<&str>) -> Result<String>;
    fn activate(&mut self, record: &Record) -> Result<()>;
    fn verify_active(&mut self, record: &Record) -> Result<()>;
    fn verify_fenced(&mut self, record: &Record) -> Result<()>;
    fn fence(&mut self, record: &Record) -> Result<()>;
    fn restore_active(&mut self, record: &Record) -> Result<()>;
    /// Sum of every zvol's logical size below the dedicated parent.
    fn logical_bytes(&mut self) -> Result<u64>;
    /// Holds the sealed origin snapshot, then CoW-clones it (or verifies the
    /// recorded clone and its origin). Returns the clone's zvol GUID.
    fn ensure_clone(&mut self, spec: &Spec, image: &Image, guid: Option<&str>) -> Result<String>;
    /// Ownership, GUID, geometry and (for clones) origin snapshot identity.
    fn verify_volume(&mut self, record: &Record) -> Result<()>;
    /// Grow only. Resumable when ZFS already has the requested size.
    fn expand(&mut self, record: &Record, capacity_bytes: u64) -> Result<()>;
    /// Copies a fenced source disk into a sealed, held, readonly image.
    /// Records the image dataset and snapshot GUIDs before returning.
    fn publish_image(&mut self, source: &Record, image: &mut Image) -> Result<()>;
    fn verify_image(&mut self, image: &Image) -> Result<()>;
    /// Removes the fenced export objects and marks the zvol retained.
    fn retire(&mut self, record: &Record) -> Result<()>;
}

fn password() -> Result<String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Establishes or verifies the dedicated parent. The caller holds the lock.
fn prepare_pool<B: Backend>(store: &Store, host: &mut B) -> Result<()> {
    let pool = store.pool_guid()?;
    if pool.is_none() && !store.pool_intent_exists()? {
        host.preflight_new_scope()?;
        store.write_pool(None)?;
    }
    let guid = host.ensure_pool(pool.as_deref())?;
    store.write_pool(Some(&guid))?;
    Ok(())
}

pub fn apply<B: Backend>(store: &Store, host: &mut B, spec: Spec) -> Result<Report> {
    spec.validate()?;
    ensure!(
        spec.disk.is_none(),
        "apply manages only the legacy demo target"
    );
    let _lock = store.lock()?;
    let existing = store.read(&spec.vm_uid)?;
    if let Some(record) = &existing {
        ensure!(record.spec==spec,"existing VM target has another capacity/identity; adoption and resize are not supported");
        ensure!(
            record.phase != Phase::Fencing,
            "an interrupted fence must finish before recovery/apply"
        );
    }
    prepare_pool(store, host)?;
    let mut record = match existing {
        Some(record) => record,
        None => {
            host.preflight_new_volume(&spec)?;
            let record = Record {
                schema: 1,
                owner: OWNER.into(),
                spec,
                generation: 1,
                phase: Phase::Preparing,
                zvol_guid: None,
                origin_guid: None,
                chap_password: password()?,
            };
            store.write(&record)?;
            record
        }
    };
    match record.phase {
        Phase::Fencing => bail!("an interrupted fence must finish before recovery/apply"),
        Phase::Provisioned | Phase::Retired => bail!("legacy target has a provisioned-disk phase"),
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

/// Persist `Fencing`, drain kernel references, then persist `Fenced`. A
/// never-exported disk has no export to drain and stays `Provisioned`.
fn fence_record<B: Backend>(store: &Store, host: &mut B, mut record: Record) -> Result<Report> {
    match record.phase {
        Phase::Fenced => {
            host.verify_fenced(&record)?;
            return Ok(record.report());
        }
        Phase::Provisioned => {
            host.verify_volume(&record)?;
            return Ok(record.report());
        }
        Phase::Retired => bail!(Refusal("disk-retired")),
        _ => {}
    }
    record.phase = Phase::Fencing;
    store.write(&record)?;
    host.fence(&record)?;
    host.verify_fenced(&record)?;
    record.phase = Phase::Fenced;
    store.write(&record)?;
    Ok(record.report())
}

pub fn fence<B: Backend>(store: &Store, host: &mut B, vm_uid: &str) -> Result<Report> {
    let _lock = store.lock()?;
    let record = store
        .read(vm_uid)?
        .ok_or_else(|| anyhow::anyhow!("no owned target manifest exists"))?;
    fence_record(store, host, record)
}

/// Boot restoration preserves active credentials. Every other exported phase
/// stays off; never-exported and retired disks have nothing to restore.
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
        match record.phase {
            Phase::Active => host.restore_active(&record)?,
            Phase::Provisioned | Phase::Retired => {}
            _ => {
                record.phase = Phase::Fencing;
                store.write(&record)?;
                host.fence(&record)?;
                host.verify_fenced(&record)?;
                record.phase = Phase::Fenced;
                store.write(&record)?;
            }
        }
        reports.push(record.report());
    }
    Ok(reports)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    pub const UID: &str = "e309f751-5dda-4271-a0f4-4454c9527a59";

    /// Hermetic host: per-journal-key export state and a toy ZFS inventory.
    #[derive(Default)]
    pub struct Fake {
        pub active: bool,
        pub fail_fence: bool,
        pub foreign: bool,
        pub secret: String,
        pub calls: Vec<&'static str>,
        pub pool_calls: u32,
        pub exports: BTreeMap<String, String>,
        pub volumes: BTreeMap<String, u64>,
        pub holds: BTreeMap<String, Vec<String>>,
        pub images: BTreeMap<String, (u64, String, String)>,
        pub retired: Vec<String>,
        pub replaced_origin: bool,
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
        fn ensure_volume(&mut self, spec: &Spec, _: Option<&str>) -> Result<String> {
            ensure!(!self.foreign, Refusal("foreign-dataset"));
            self.volumes
                .entry(spec.dataset())
                .or_insert(spec.capacity_bytes);
            Ok("34".into())
        }
        fn activate(&mut self, r: &Record) -> Result<()> {
            self.calls.push("activate");
            self.active = true;
            self.secret = r.chap_password.clone();
            self.exports.insert(r.spec.key(), r.chap_password.clone());
            Ok(())
        }
        fn verify_active(&mut self, r: &Record) -> Result<()> {
            ensure!(
                self.exports.get(&r.spec.key()) == Some(&r.chap_password),
                "not active"
            );
            Ok(())
        }
        fn verify_fenced(&mut self, r: &Record) -> Result<()> {
            ensure!(!self.exports.contains_key(&r.spec.key()), "not drained");
            Ok(())
        }
        fn fence(&mut self, r: &Record) -> Result<()> {
            self.calls.push("fence");
            ensure!(!self.fail_fence, "drain failed");
            self.active = false;
            self.exports.remove(&r.spec.key());
            Ok(())
        }
        fn restore_active(&mut self, r: &Record) -> Result<()> {
            self.activate(r)
        }
        fn logical_bytes(&mut self) -> Result<u64> {
            Ok(self.volumes.values().sum())
        }
        fn ensure_clone(&mut self, spec: &Spec, image: &Image, _: Option<&str>) -> Result<String> {
            self.verify_image(image)?;
            ensure!(!self.foreign, Refusal("foreign-dataset"));
            self.holds
                .entry(image.origin.snapshot())
                .or_default()
                .push(spec.clone_hold());
            self.volumes
                .entry(spec.dataset())
                .or_insert(spec.capacity_bytes);
            Ok("56".into())
        }
        fn verify_volume(&mut self, r: &Record) -> Result<()> {
            ensure!(
                self.volumes.get(&r.spec.dataset()) == Some(&r.spec.capacity_bytes),
                "geometry drift"
            );
            if let Some(origin) = &r.spec.origin {
                ensure!(
                    !self.replaced_origin
                        && self.images.get(&origin.key()).map(|i| &i.2) == r.origin_guid.as_ref(),
                    Refusal("clone-origin-mismatch")
                );
            }
            Ok(())
        }
        fn expand(&mut self, r: &Record, capacity: u64) -> Result<()> {
            self.calls.push("expand");
            self.volumes.insert(r.spec.dataset(), capacity);
            Ok(())
        }
        fn publish_image(&mut self, source: &Record, image: &mut Image) -> Result<()> {
            self.calls.push("publish");
            ensure!(
                !self.exports.contains_key(&source.spec.key()),
                "source live"
            );
            image.dataset_guid = Some("77".into());
            image.snapshot_guid = Some("78".into());
            self.images.insert(
                image.origin.key(),
                (image.capacity_bytes, "77".into(), "78".into()),
            );
            self.volumes
                .insert(image.origin.dataset(), image.capacity_bytes);
            self.holds
                .entry(image.origin.snapshot())
                .or_default()
                .push("qrab-image".into());
            Ok(())
        }
        fn verify_image(&mut self, image: &Image) -> Result<()> {
            let Some((size, dataset, snapshot)) = self.images.get(&image.origin.key()) else {
                bail!(Refusal("image-not-published"))
            };
            ensure!(
                *size == image.capacity_bytes
                    && image.dataset_guid.as_ref() == Some(dataset)
                    && image.snapshot_guid.as_ref() == Some(snapshot),
                Refusal("image-replaced")
            );
            Ok(())
        }
        fn retire(&mut self, r: &Record) -> Result<()> {
            self.verify_fenced(r)?;
            self.retired.push(r.spec.key());
            Ok(())
        }
    }
    pub fn fixture() -> (tempfile::TempDir, Store, Spec) {
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
        for disk in ["", "-a", "a-", "A", "a/b", "a.b", "abcdefghijklmnopqrstu"] {
            assert!(Spec::disk(UID.into(), disk.into(), MIN_VOLUME, None).is_err());
        }
        assert!(Spec::disk(UID.into(), "root".into(), MAX_DISK + (1 << 20), None).is_err());
        assert!(Origin::new("../x".into(), 1).is_err());
        assert!(Origin::new("debian".into(), 0).is_err());
    }
    #[test]
    fn provisioned_disk_names_never_collide_with_the_legacy_demo() {
        let legacy = Spec::new(UID.into(), MIN_VOLUME).unwrap();
        let disk = Spec::disk(UID.into(), "root".into(), MIN_VOLUME, None).unwrap();
        assert_ne!(legacy.dataset(), disk.dataset());
        assert_ne!(legacy.target(), disk.target());
        assert_ne!(legacy.object(), disk.object());
        assert_ne!(legacy.username(), disk.username());
        assert_eq!(legacy.initiator(), disk.initiator());
        assert_eq!(disk.dataset(), format!("tank/qrab-iscsi/disk-{UID}-root"));
        assert_eq!(
            disk.target(),
            format!("iqn.2026-11.com.yapplesauce:qrab.disk.{UID}.root")
        );
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
        assert!(!host.exports.contains_key(UID));
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
        assert!(!public.contains("\"disk\""));
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
        host.exports.clear();
        restore(&store, &mut host).unwrap();
        assert_eq!(host.exports.get(UID), Some(&secret));
        assert_eq!(store.read(UID).unwrap().unwrap().generation, 1);
        fence(&store, &mut host, UID).unwrap();
        restore(&store, &mut host).unwrap();
        assert!(!host.exports.contains_key(UID));
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
    #[test]
    fn legacy_schema_one_journal_still_parses_without_new_fields() {
        let (_dir, store, _spec) = fixture();
        let _lock = store.lock().unwrap();
        let legacy = format!(
            r#"{{"schema":1,"owner":"{OWNER}","spec":{{"vm_uid":"{UID}","capacity_bytes":134217728}},"generation":2,"phase":"active","zvol_guid":"34","chap_password":"0123456789abcdef0123456789abcdef"}}"#
        );
        let path = store.manifest_path(UID);
        std::fs::write(&path, legacy).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let record = store.read(UID).unwrap().unwrap();
        assert_eq!(record.spec.disk, None);
        assert_eq!(record.generation, 2);
    }
}
