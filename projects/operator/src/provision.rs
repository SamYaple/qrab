//! Converge `spec.disks` through the storage service.
//!
//! Each disk is created (blank) or CoW-cloned (base image), grown on size
//! increase and exported with an operator-generated CHAP secret stored in an
//! owned Secret. The secret is persisted before the export that uses it, so a
//! retry always presents the credential the target recorded. A fenced disk
//! is re-exported only with the next generation and a new secret, and only
//! after the VM process that held the old one is gone.
use crate::{
    api::{DiskStatus, ProvisionedDisk, VirtualMachine},
    storage::{ChapSecret, DiskPhase, DiskReport, Origin, StorageApi, StorageError},
    Error, Result,
};
use k8s_openapi::{api::core::v1::Secret, ByteString};
use kube::{
    api::{Patch, PatchParams, PostParams},
    Api, Resource, ResourceExt,
};
use serde_json::json;
use std::{collections::BTreeMap, io::Read};

pub const SECRET_KEY: &str = "password";
pub const GENERATION_ANNOTATION: &str = "qrab.yapplesauce.com/credential-generation";
pub const DISK_ANNOTATION: &str = "qrab.yapplesauce.com/disk";

pub fn secret_name(vm: &str, disk: &str) -> String {
    format!("{vm}-disk-{disk}")
}

pub fn initiator(uid: &str) -> String {
    format!("iqn.2026-11.com.yapplesauce:qrab.vm.{uid}")
}

/// A verified export the Pod renderer may attach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    pub id: String,
    pub portal: String,
    pub target: String,
    pub username: String,
    pub secret_name: String,
    pub generation: u64,
    pub capacity_bytes: u64,
    pub read_only: bool,
    pub boot_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    pub generation: u64,
    pub secret: ChapSecret,
}

impl Credential {
    pub fn generate(generation: u64) -> Result<Self> {
        let mut bytes = [0u8; 16];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .map_err(|_| Error::Invalid("no entropy source for CHAP generation".into()))?;
        Ok(Self {
            generation,
            secret: ChapSecret(bytes.iter().map(|b| format!("{b:02x}")).collect()),
        })
    }
}

/// Owned per-disk credential storage (Kubernetes Secrets in production).
#[async_trait::async_trait]
pub trait CredentialStore: Send + Sync {
    async fn get(&self, disk: &str) -> Result<Option<Credential>>;
    async fn put(&self, disk: &str, credential: &Credential) -> Result<()>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Ready(Attached),
    /// Not attachable yet; retry later without touching a running Pod.
    Wait(String),
    /// The running VM holds a revoked credential; replace its instance.
    Restart(String),
}

fn storage(error: StorageError) -> Error {
    Error::Storage(error)
}

fn status(id: &str, report: &DiskReport) -> DiskStatus {
    DiskStatus {
        id: id.into(),
        capacity_bytes: report.capacity_bytes,
        generation: report.generation,
        phase: report.phase.as_str().into(),
    }
}

fn verified(vm_uid: &str, disk: &ProvisionedDisk, report: &DiskReport) -> Result<()> {
    let want = disk.base_image.as_ref().map(|b| Origin {
        image: b.name.clone(),
        generation: b.generation,
    });
    if report.origin != want {
        return Err(Error::Invalid(format!(
            "disk {} already exists with another base image; use a new disk id",
            disk.id
        )));
    }
    if report.initiator_iqn != initiator(vm_uid)
        || report.portal.parse::<std::net::SocketAddr>().is_err()
        || !crate::resources::valid_iqn(&report.target_iqn)
        || report.chap_username.is_empty()
        || report.chap_username.len() > 223
        || !report.chap_username.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(Error::Ownership(format!(
            "storage report for disk {} names another initiator or endpoint",
            disk.id
        )));
    }
    Ok(())
}

/// Advance one disk by at most the transitions needed this reconcile.
/// `launched` is true when a VM process may still hold the current credential.
pub async fn converge(
    api: &dyn StorageApi,
    credentials: &dyn CredentialStore,
    vm_name: &str,
    vm_uid: &str,
    disk: &ProvisionedDisk,
    launched: bool,
) -> Result<(Step, Option<DiskStatus>)> {
    let id = disk.id.as_str();
    let mut report = match api.status(vm_uid, id).await.map_err(storage)? {
        Some(report) => report,
        None => match &disk.base_image {
            None => api.create(vm_uid, id, disk.size_bytes).await,
            Some(image) => api.clone_image(vm_uid, id, image, disk.size_bytes).await,
        }
        .map_err(storage)?,
    };
    verified(vm_uid, disk, &report)?;
    if report.capacity_bytes > disk.size_bytes {
        return Err(Error::Invalid(format!(
            "disk {id} is {} bytes; sizeBytes may only grow",
            report.capacity_bytes
        )));
    }
    if report.phase == DiskPhase::Retired {
        return Err(Error::Invalid(format!(
            "disk {id} was retired by an administrator; use a new disk id"
        )));
    }
    if report.capacity_bytes < disk.size_bytes {
        report = api
            .expand(vm_uid, id, disk.size_bytes)
            .await
            .map_err(storage)?;
        verified(vm_uid, disk, &report)?;
    }
    let current = credentials.get(id).await?;
    let matches = |c: &Option<Credential>| {
        c.as_ref()
            .is_some_and(|c| c.generation == report.generation && report.generation > 0)
    };
    // An export whose secret we no longer hold, or an interrupted fence, is
    // finished with a fence before rotation. Never while a VM may use it.
    if report.phase == DiskPhase::Fencing
        || (matches!(report.phase, DiskPhase::Active | DiskPhase::Preparing) && !matches(&current))
    {
        if launched {
            return Ok((
                Step::Wait(format!(
                    "disk {id} credential state is {:?} without a matching Secret; stop the VM to fence and rotate it",
                    report.phase
                )),
                Some(status(id, &report)),
            ));
        }
        report = api.fence(vm_uid, id).await.map_err(storage)?;
        verified(vm_uid, disk, &report)?;
    }
    let held = current
        .clone()
        .filter(|c| c.generation == report.generation && report.generation > 0);
    let credential = match (report.phase, held) {
        (DiskPhase::Active, Some(credential)) => credential,
        (DiskPhase::Preparing, Some(credential)) => {
            report = api
                .export(vm_uid, id, credential.generation, &credential.secret)
                .await
                .map_err(storage)?;
            credential
        }
        (DiskPhase::Provisioned | DiskPhase::Fenced, _) => {
            if report.phase == DiskPhase::Fenced && launched {
                return Ok((
                    Step::Restart(format!(
                        "disk {id} was fenced; replacing the VM instance before credential rotation"
                    )),
                    Some(status(id, &report)),
                ));
            }
            let next = report.generation + 1;
            let credential = match current {
                Some(c) if c.generation == next => c,
                _ => {
                    let fresh = Credential::generate(next)?;
                    // Durable before use: the target records exactly this secret.
                    credentials.put(id, &fresh).await?;
                    fresh
                }
            };
            report = api
                .export(vm_uid, id, next, &credential.secret)
                .await
                .map_err(storage)?;
            credential
        }
        _ => {
            return Ok((
                Step::Wait(format!(
                    "disk {id} is {:?} without a usable credential",
                    report.phase
                )),
                Some(status(id, &report)),
            ))
        }
    };
    verified(vm_uid, disk, &report)?;
    if report.phase != DiskPhase::Active || report.generation != credential.generation {
        return Ok((
            Step::Wait(format!("disk {id} export is not active yet")),
            Some(status(id, &report)),
        ));
    }
    Ok((
        Step::Ready(Attached {
            id: id.into(),
            portal: report.portal.clone(),
            target: report.target_iqn.clone(),
            username: report.chap_username.clone(),
            secret_name: secret_name(vm_name, id),
            generation: report.generation,
            capacity_bytes: report.capacity_bytes,
            read_only: disk.read_only,
            boot_index: disk.boot_index,
        }),
        Some(status(id, &report)),
    ))
}

/// Hard-fence every provisioned disk this VM has or had. Kernel ACL/LUN
/// teardown completes inside the storage service before this returns.
pub async fn fence_all(api: &dyn StorageApi, vm: &VirtualMachine) -> Result<()> {
    let uid = vm
        .uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    let mut ids: Vec<&str> = vm.spec.disks.iter().map(|d| d.id.as_str()).collect();
    for disk in vm.status.iter().flat_map(|s| s.disks.iter()) {
        if !ids.contains(&disk.id.as_str()) {
            ids.push(&disk.id);
        }
    }
    for id in ids {
        if api.status(&uid, id).await.map_err(storage)?.is_none() {
            continue;
        }
        let report = api.fence(&uid, id).await.map_err(storage)?;
        if !matches!(
            report.phase,
            DiskPhase::Fenced | DiskPhase::Provisioned | DiskPhase::Retired
        ) {
            return Err(Error::Storage(StorageError::Unavailable(format!(
                "disk {id} did not report a completed fence"
            ))));
        }
    }
    Ok(())
}

/// True when the operator itself can hard-fence every native disk of this VM.
pub fn self_fencing(vm: &VirtualMachine, api: Option<&dyn StorageApi>) -> bool {
    api.is_some() && vm.spec.iscsi_disks.is_empty() && !vm.spec.disks.is_empty()
}

/// Secrets named `<vm>-disk-<id>`, controller-owned by the VM, holding only
/// the CHAP password and its generation annotation.
pub struct SecretStore<'a> {
    pub api: Api<Secret>,
    pub vm: &'a VirtualMachine,
}

impl SecretStore<'_> {
    fn owned(&self, secret: &Secret, disk: &str) -> bool {
        crate::resources::owned(secret, self.vm)
            && secret
                .annotations()
                .get(DISK_ANNOTATION)
                .map(String::as_str)
                == Some(disk)
    }
}

#[async_trait::async_trait]
impl CredentialStore for SecretStore<'_> {
    async fn get(&self, disk: &str) -> Result<Option<Credential>> {
        let name = secret_name(&self.vm.name_any(), disk);
        let Some(secret) = self.api.get_opt(&name).await? else {
            return Ok(None);
        };
        if !self.owned(&secret, disk) {
            return Err(Error::Ownership(format!(
                "Secret {name} is not this VM's disk credential"
            )));
        }
        let generation = secret
            .annotations()
            .get(GENERATION_ANNOTATION)
            .and_then(|g| g.parse::<u64>().ok());
        let password = secret
            .data
            .as_ref()
            .and_then(|d| d.get(SECRET_KEY))
            .and_then(|v| String::from_utf8(v.0.clone()).ok())
            .filter(|p| crate::resources::valid_chap(p));
        Ok(match (generation, password) {
            (Some(generation), Some(password)) => Some(Credential {
                generation,
                secret: ChapSecret(password),
            }),
            _ => None,
        })
    }

    async fn put(&self, disk: &str, credential: &Credential) -> Result<()> {
        let name = secret_name(&self.vm.name_any(), disk);
        let uid = self
            .vm
            .uid()
            .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
        let mut owner = self
            .vm
            .controller_owner_ref(&())
            .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
        owner.block_owner_deletion = Some(false);
        let mut secret: Secret = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "type": "Opaque",
            "metadata": {
                "name": name,
                "namespace": self.vm.namespace(),
                "labels": {
                    "app.kubernetes.io/name": "qrab",
                    "app.kubernetes.io/component": "disk-credential",
                    "app.kubernetes.io/instance": self.vm.name_any(),
                    "qrab.yapplesauce.com/owner-uid": uid,
                },
                "annotations": {
                    DISK_ANNOTATION: disk,
                    GENERATION_ANNOTATION: credential.generation.to_string(),
                },
                "ownerReferences": [owner],
            },
        }))?;
        secret.data = Some(BTreeMap::from([(
            SECRET_KEY.to_owned(),
            ByteString(credential.secret.0.clone().into_bytes()),
        )]));
        match self.api.get_opt(&name).await? {
            None => {
                self.api.create(&PostParams::default(), &secret).await?;
            }
            Some(existing) => {
                if !self.owned(&existing, disk) {
                    return Err(Error::Ownership(format!(
                        "Secret {name} is not this VM's disk credential"
                    )));
                }
                secret.metadata.uid = existing.uid();
                secret.metadata.resource_version = existing.resource_version();
                self.api
                    .patch(
                        &name,
                        &PatchParams::apply("qrab-operator").force(),
                        &Patch::Apply(&secret),
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use crate::api::BaseImage;
    use crate::storage::StorageResult;
    use std::sync::Mutex;

    #[derive(Clone)]
    pub struct Disk {
        pub origin: Option<Origin>,
        pub capacity: u64,
        pub generation: u64,
        pub phase: DiskPhase,
        pub secret: Option<String>,
    }

    /// Mirrors qrab-iscsi-target's export/fence/rotation contract.
    #[derive(Default)]
    pub struct FakeStorage {
        pub disks: Mutex<BTreeMap<String, Disk>>,
        pub calls: Mutex<Vec<String>>,
    }

    impl FakeStorage {
        pub fn disk(&self, uid: &str, id: &str) -> Option<Disk> {
            self.disks
                .lock()
                .unwrap()
                .get(&format!("{uid}.{id}"))
                .cloned()
        }
        pub fn set(&self, uid: &str, id: &str, disk: Disk) {
            self.disks
                .lock()
                .unwrap()
                .insert(format!("{uid}.{id}"), disk);
        }
        pub fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
        fn report(uid: &str, id: &str, d: &Disk) -> DiskReport {
            DiskReport {
                vm_uid: uid.into(),
                disk: id.into(),
                origin: d.origin.clone(),
                capacity_bytes: d.capacity,
                target_iqn: format!("iqn.2026-11.com.yapplesauce:qrab.disk.{uid}.{id}"),
                initiator_iqn: initiator(uid),
                portal: "10.203.0.1:3260".into(),
                lun: 0,
                chap_username: format!("qrab-{}-{id}", uid.replace('-', "")),
                generation: d.generation,
                phase: d.phase,
            }
        }
        fn with<T>(
            &self,
            call: &str,
            uid: &str,
            id: &str,
            f: impl FnOnce(&mut BTreeMap<String, Disk>, String) -> StorageResult<T>,
        ) -> StorageResult<T> {
            self.calls.lock().unwrap().push(format!("{call} {id}"));
            f(&mut self.disks.lock().unwrap(), format!("{uid}.{id}"))
        }
    }

    fn refused<T>(code: &str) -> StorageResult<T> {
        Err(StorageError::Refused(code.into()))
    }

    #[async_trait::async_trait]
    impl StorageApi for FakeStorage {
        async fn status(&self, uid: &str, id: &str) -> StorageResult<Option<DiskReport>> {
            self.with("status", uid, id, |m, k| {
                Ok(m.get(&k).map(|d| Self::report(uid, id, d)))
            })
        }
        async fn create(&self, uid: &str, id: &str, capacity: u64) -> StorageResult<DiskReport> {
            self.with("create", uid, id, |m, k| {
                let d = m.entry(k).or_insert(Disk {
                    origin: None,
                    capacity,
                    generation: 0,
                    phase: DiskPhase::Provisioned,
                    secret: None,
                });
                Ok(Self::report(uid, id, d))
            })
        }
        async fn clone_image(
            &self,
            uid: &str,
            id: &str,
            image: &BaseImage,
            capacity: u64,
        ) -> StorageResult<DiskReport> {
            self.with("clone", uid, id, |m, k| {
                if image.name == "missing" {
                    return refused("image-not-published");
                }
                let d = m.entry(k).or_insert(Disk {
                    origin: Some(Origin {
                        image: image.name.clone(),
                        generation: image.generation,
                    }),
                    capacity,
                    generation: 0,
                    phase: DiskPhase::Provisioned,
                    secret: None,
                });
                Ok(Self::report(uid, id, d))
            })
        }
        async fn expand(&self, uid: &str, id: &str, capacity: u64) -> StorageResult<DiskReport> {
            self.with("expand", uid, id, |m, k| {
                let d = m
                    .get_mut(&k)
                    .ok_or(StorageError::Refused("disk-not-found".into()))?;
                if capacity < d.capacity {
                    return refused("shrink-refused");
                }
                d.capacity = capacity;
                Ok(Self::report(uid, id, d))
            })
        }
        async fn export(
            &self,
            uid: &str,
            id: &str,
            generation: u64,
            secret: &ChapSecret,
        ) -> StorageResult<DiskReport> {
            self.with("export", uid, id, |m, k| {
                let d = m
                    .get_mut(&k)
                    .ok_or(StorageError::Refused("disk-not-found".into()))?;
                match d.phase {
                    DiskPhase::Active => {
                        if generation != d.generation || d.secret.as_ref() != Some(&secret.0) {
                            return refused("fence-required");
                        }
                    }
                    DiskPhase::Provisioned if generation != 1 => {
                        return refused("generation-mismatch")
                    }
                    DiskPhase::Fenced if generation != d.generation + 1 => {
                        return refused("generation-mismatch")
                    }
                    DiskPhase::Fenced if d.secret.as_ref() == Some(&secret.0) => {
                        return refused("credential-reused")
                    }
                    DiskPhase::Fencing => return refused("fence-incomplete"),
                    DiskPhase::Retired => return refused("disk-retired"),
                    _ => {}
                }
                d.generation = generation;
                d.secret = Some(secret.0.clone());
                d.phase = DiskPhase::Active;
                Ok(Self::report(uid, id, d))
            })
        }
        async fn fence(&self, uid: &str, id: &str) -> StorageResult<DiskReport> {
            self.with("fence", uid, id, |m, k| {
                let d = m
                    .get_mut(&k)
                    .ok_or(StorageError::Refused("disk-not-found".into()))?;
                if d.phase != DiskPhase::Provisioned && d.phase != DiskPhase::Retired {
                    d.phase = DiskPhase::Fenced;
                }
                Ok(Self::report(uid, id, d))
            })
        }
    }

    #[derive(Default)]
    pub struct FakeCredentials(pub Mutex<BTreeMap<String, Credential>>);

    #[async_trait::async_trait]
    impl CredentialStore for FakeCredentials {
        async fn get(&self, disk: &str) -> Result<Option<Credential>> {
            Ok(self.0.lock().unwrap().get(disk).cloned())
        }
        async fn put(&self, disk: &str, credential: &Credential) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .insert(disk.into(), credential.clone());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::*;
    use super::*;
    use crate::api::BaseImage;

    const UID: &str = "e309f751-5dda-4271-a0f4-4454c9527a59";
    const MIB: u64 = 1 << 20;

    fn disk(size: u64, image: Option<(&str, u32)>) -> ProvisionedDisk {
        ProvisionedDisk {
            id: "root".into(),
            size_bytes: size,
            base_image: image.map(|(name, generation)| BaseImage {
                name: name.into(),
                generation,
            }),
            read_only: false,
            boot_index: Some(0),
        }
    }

    async fn run(
        api: &FakeStorage,
        creds: &FakeCredentials,
        d: &ProvisionedDisk,
        launched: bool,
    ) -> Result<(Step, Option<DiskStatus>)> {
        converge(api, creds, "vm", UID, d, launched).await
    }

    #[tokio::test]
    async fn blank_disk_is_created_secret_persisted_then_exported_once() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        let (step, status) = run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let Step::Ready(attached) = step else {
            panic!("not ready: {step:?}")
        };
        assert_eq!(attached.generation, 1);
        assert_eq!(attached.secret_name, "vm-disk-root");
        assert_eq!(
            attached.target,
            format!("iqn.2026-11.com.yapplesauce:qrab.disk.{UID}.root")
        );
        assert_eq!(status.unwrap().phase, "Active");
        let stored = creds.0.lock().unwrap()["root"].clone();
        assert_eq!(
            api.disk(UID, "root").unwrap().secret,
            Some(stored.secret.0.clone())
        );
        assert!(crate::resources::valid_chap(&stored.secret.0));
        // Steady state: status only, no further mutation.
        run(&api, &creds, &disk(64 * MIB, None), true)
            .await
            .unwrap();
        assert_eq!(
            api.calls(),
            vec!["status root", "create root", "export root", "status root"]
        );
    }

    #[tokio::test]
    async fn base_image_clone_is_immutable_and_missing_images_surface_refusals() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        let (step, _) = run(&api, &creds, &disk(64 * MIB, Some(("debian", 3))), false)
            .await
            .unwrap();
        assert!(matches!(step, Step::Ready(_)));
        assert!(api.calls().contains(&"clone root".to_string()));
        let other = run(&api, &creds, &disk(64 * MIB, Some(("debian", 4))), true).await;
        assert!(matches!(other, Err(Error::Invalid(_))));
        let blank = run(&api, &creds, &disk(64 * MIB, None), true).await;
        assert!(matches!(blank, Err(Error::Invalid(_))));
        let api = FakeStorage::default();
        let missing = run(&api, &creds, &disk(64 * MIB, Some(("missing", 1))), false).await;
        assert!(matches!(
            missing,
            Err(Error::Storage(StorageError::Refused(code))) if code == "image-not-published"
        ));
    }

    #[tokio::test]
    async fn growth_expands_and_shrink_is_refused_without_storage_mutation() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let (step, status) = run(&api, &creds, &disk(128 * MIB, None), true)
            .await
            .unwrap();
        let Step::Ready(attached) = step else {
            panic!("not ready")
        };
        assert_eq!(attached.capacity_bytes, 128 * MIB);
        assert_eq!(status.unwrap().capacity_bytes, 128 * MIB);
        // Growth keeps the credential and export: no fence or rotation.
        assert_eq!(attached.generation, 1);
        let calls = api.calls().len();
        assert!(matches!(
            run(&api, &creds, &disk(64 * MIB, None), true).await,
            Err(Error::Invalid(_))
        ));
        assert_eq!(api.calls().len(), calls + 1);
        assert_eq!(api.disk(UID, "root").unwrap().capacity, 128 * MIB);
    }

    #[tokio::test]
    async fn fenced_disk_restarts_a_live_vm_then_rotates_to_a_new_generation() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let old = creds.0.lock().unwrap()["root"].clone();
        api.fence(UID, "root").await.unwrap();
        let (step, _) = run(&api, &creds, &disk(64 * MIB, None), true)
            .await
            .unwrap();
        assert!(matches!(step, Step::Restart(_)));
        assert_eq!(api.disk(UID, "root").unwrap().phase, DiskPhase::Fenced);
        let (step, _) = run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let Step::Ready(attached) = step else {
            panic!("not ready")
        };
        assert_eq!(attached.generation, 2);
        let new = creds.0.lock().unwrap()["root"].clone();
        assert_eq!(new.generation, 2);
        assert_ne!(new.secret, old.secret);
        assert_eq!(api.disk(UID, "root").unwrap().secret, Some(new.secret.0));
    }

    #[tokio::test]
    async fn a_persisted_rotation_is_reused_after_a_failed_export() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        api.fence(UID, "root").await.unwrap();
        let pending = Credential::generate(2).unwrap();
        creds
            .0
            .lock()
            .unwrap()
            .insert("root".into(), pending.clone());
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        assert_eq!(
            api.disk(UID, "root").unwrap().secret,
            Some(pending.secret.0)
        );
    }

    #[tokio::test]
    async fn lost_secret_for_a_live_export_waits_then_fences_and_rotates_when_stopped() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        creds.0.lock().unwrap().clear();
        let (step, _) = run(&api, &creds, &disk(64 * MIB, None), true)
            .await
            .unwrap();
        assert!(matches!(step, Step::Wait(_)));
        assert!(!api.calls().iter().any(|c| c.starts_with("fence")));
        let (step, _) = run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let Step::Ready(attached) = step else {
            panic!("not ready")
        };
        assert_eq!(attached.generation, 2);
        let fence = api.calls().iter().position(|c| c == "fence root").unwrap();
        let export = api
            .calls()
            .iter()
            .rposition(|c| c == "export root")
            .unwrap();
        assert!(fence < export);
    }

    #[tokio::test]
    async fn retired_or_foreign_reports_are_never_attached() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        api.set(
            UID,
            "root",
            Disk {
                origin: None,
                capacity: 64 * MIB,
                generation: 1,
                phase: DiskPhase::Retired,
                secret: None,
            },
        );
        assert!(matches!(
            run(&api, &creds, &disk(64 * MIB, None), false).await,
            Err(Error::Invalid(_))
        ));
        let mut vm_disk = disk(64 * MIB, None);
        vm_disk.id = "data".into();
        // A report for another VM's initiator is an ownership failure.
        let foreign = converge(
            &api,
            &creds,
            "vm",
            "0b5c1d0e-1111-4222-8333-944455556666",
            &vm_disk,
            false,
        )
        .await
        .unwrap();
        assert!(matches!(foreign.0, Step::Ready(_)));
        let report = DiskReport {
            initiator_iqn: initiator("0b5c1d0e-1111-4222-8333-944455556666"),
            ..FakeStorage::default()
                .create(UID, "x", 64 * MIB)
                .await
                .unwrap()
        };
        assert!(matches!(
            verified(UID, &disk(64 * MIB, None), &report),
            Err(Error::Ownership(_))
        ));
    }

    #[tokio::test]
    async fn fence_all_covers_spec_and_recorded_disks() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let mut gone = disk(64 * MIB, None);
        gone.id = "old".into();
        run(&api, &creds, &gone, false).await.unwrap();
        let mut vm = VirtualMachine::new(
            "vm",
            serde_json::from_value(serde_json::json!({
                "stateVolumeClaim":"state","console":{"publicUrl":"https://vm.example/","allowedSubjects":[]},
                "disks":[{"id":"root","sizeBytes":67108864}]
            }))
            .unwrap(),
        );
        vm.metadata.uid = Some(UID.into());
        vm.status = Some(crate::api::VirtualMachineStatus {
            disks: vec![DiskStatus {
                id: "old".into(),
                ..Default::default()
            }],
            ..Default::default()
        });
        fence_all(&api, &vm).await.unwrap();
        assert_eq!(api.disk(UID, "root").unwrap().phase, DiskPhase::Fenced);
        assert_eq!(api.disk(UID, "old").unwrap().phase, DiskPhase::Fenced);
    }
}
