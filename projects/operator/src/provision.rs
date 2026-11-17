//! Converge `spec.disks` through the storage service.
//!
//! Each disk is created (blank) or CoW-cloned (base image), grown on size
//! increase and exported with an operator-generated CHAP secret stored in an
//! owned Secret. Credentials are write-only: the operator never reads Secret
//! data. Cleanup checks ownership with a test-only metadata patch and receives
//! metadata only. Every export uses a fresh secret written before the call, so an
//! Active export's Secret is the one it was exported with; an interrupted
//! export or fence is finished with a fence and the next generation. A fenced
//! disk is re-exported only after the VM process that held the old secret is
//! gone. Server-side admission (contrib/kubernetes/operator/secret-policy.yaml)
//! confines the operator's Secret writes to these VM-owned objects.
use crate::{
    api::{DiskStatus, ProvisionedDisk, VirtualMachine},
    storage::{ChapSecret, DiskPhase, DiskReport, Origin, StorageApi, StorageError},
    Error, Result,
};
use k8s_openapi::{
    api::core::v1::{Pod, Secret},
    ByteString,
};
use kube::{
    api::{DeleteParams, Patch, PatchParams, Preconditions},
    Api, Resource, ResourceExt,
};
use serde_json::json;
use std::{collections::BTreeMap, io::Read};

#[cfg(test)]
mod secret_tests;

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

/// Write-only per-disk credential storage (Kubernetes Secrets in production).
#[async_trait::async_trait]
pub trait CredentialStore: Send + Sync {
    async fn put(&self, disk: &str, credential: &Credential) -> Result<()>;
    /// Absent is success.
    async fn delete(&self, disk: &str) -> Result<()>;
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
    // An interrupted export or fence: the secret it recorded is unknown, so
    // finish the fence and rotate. Never while a VM may still use the export.
    if matches!(report.phase, DiskPhase::Preparing | DiskPhase::Fencing) {
        if launched {
            return Ok((
                Step::Wait(format!(
                    "disk {id} is {:?}; stop the VM to fence and rotate its credential",
                    report.phase
                )),
                Some(status(id, &report)),
            ));
        }
        report = match api.fence(vm_uid, id).await {
            Ok(report) => report,
            Err(error @ StorageError::Unavailable(_)) => {
                return Ok((
                    Step::Wait(format!("disk {id} fence is still in progress: {error}")),
                    Some(status(id, &report)),
                ))
            }
            Err(error) => return Err(storage(error)),
        };
        verified(vm_uid, disk, &report)?;
    }
    let generation = match report.phase {
        DiskPhase::Active => report.generation,
        DiskPhase::Provisioned | DiskPhase::Fenced => {
            if report.phase == DiskPhase::Fenced && launched {
                return Ok((
                    Step::Restart(format!(
                        "disk {id} was fenced; replacing the VM instance before credential rotation"
                    )),
                    Some(status(id, &report)),
                ));
            }
            let fresh = Credential::generate(report.generation + 1)?;
            // Durable before use: the target records exactly this secret.
            credentials.put(id, &fresh).await?;
            report = api
                .export(vm_uid, id, fresh.generation, &fresh.secret)
                .await
                .map_err(storage)?;
            fresh.generation
        }
        _ => {
            return Ok((
                Step::Wait(format!("disk {id} is {:?}", report.phase)),
                Some(status(id, &report)),
            ))
        }
    };
    verified(vm_uid, disk, &report)?;
    if report.phase != DiskPhase::Active || report.generation != generation {
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
    let (uid, ids) = disk_ids(vm)?;
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

/// The VM's UID and every disk ID it has in `spec.disks` or recorded status.
fn disk_ids(vm: &VirtualMachine) -> Result<(String, Vec<&str>)> {
    let uid = vm
        .uid()
        .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
    let mut ids: Vec<&str> = vm.spec.disks.iter().map(|d| d.id.as_str()).collect();
    for disk in vm.status.iter().flat_map(|s| s.disks.iter()) {
        if !ids.contains(&disk.id.as_str()) {
            ids.push(&disk.id);
        }
    }
    Ok((uid, ids))
}

/// After [`fence_all`] on a deleted VM: retire every disk, so the storage
/// host's reaper destroys it once its grace period passes. Returns the disks
/// the storage service refused to retire, with the refusal code. They stay
/// fenced and retained, as before retirement existed. An outage is an error.
pub async fn retire_all(
    api: &dyn StorageApi,
    vm: &VirtualMachine,
) -> Result<Vec<(String, String)>> {
    let (uid, ids) = disk_ids(vm)?;
    let mut refused = Vec::new();
    for id in ids {
        match api.status(&uid, id).await.map_err(storage)? {
            None => continue,
            Some(report) if report.phase == DiskPhase::Retired => continue,
            Some(_) => {}
        }
        match api.retire_disk(&uid, id).await {
            Ok(report) if report.phase == DiskPhase::Retired => {}
            Ok(_) => {
                return Err(Error::Storage(StorageError::Unavailable(format!(
                    "disk {id} did not report retirement"
                ))))
            }
            Err(StorageError::Refused(code)) => refused.push((id.to_string(), code)),
            Err(error) => return Err(storage(error)),
        }
    }
    Ok(refused)
}

/// Fence a disk no longer in `spec.disks` and delete its credential Secret.
/// The zvol is retained and not retired, so re-adding the ID reattaches it.
pub async fn release(
    api: &dyn StorageApi,
    credentials: &dyn CredentialStore,
    vm_uid: &str,
    id: &str,
) -> Result<()> {
    if api.status(vm_uid, id).await.map_err(storage)?.is_some() {
        let report = api.fence(vm_uid, id).await.map_err(storage)?;
        if !matches!(
            report.phase,
            DiskPhase::Fenced | DiskPhase::Provisioned | DiskPhase::Retired
        ) {
            return Err(Error::Storage(StorageError::Unavailable(format!(
                "disk {id} did not report a completed fence"
            ))));
        }
    }
    credentials.delete(id).await
}

/// Disks recorded in status but no longer in `spec.disks` whose export no
/// launched VM process can still be using.
pub fn removable(vm: &VirtualMachine, pod: Option<&Pod>, launched: bool) -> Vec<String> {
    vm.status
        .iter()
        .flat_map(|s| s.disks.iter())
        .filter(|gone| !vm.spec.disks.iter().any(|d| d.id == gone.id))
        .filter(|gone| {
            !(launched
                && pod.and_then(|p| p.spec.as_ref()).is_some_and(|s| {
                    s.volumes
                        .iter()
                        .flatten()
                        .any(|v| v.name == format!("iscsi-auth-{}", gone.id))
                }))
        })
        .map(|gone| gone.id.clone())
        .collect()
}

/// A storage-service outage or timeout while fencing is progress, not a
/// refusal: the target journal stays `Fencing` and the next call resumes it.
pub fn fence_in_progress(error: &Error) -> bool {
    matches!(error, Error::Storage(StorageError::Unavailable(_)))
}

/// True when the operator itself can hard-fence every native disk of this VM.
pub fn self_fencing(vm: &VirtualMachine, api: Option<&dyn StorageApi>) -> bool {
    api.is_some() && vm.spec.iscsi_disks.is_empty() && !vm.spec.disks.is_empty()
}

/// Secrets named `<vm>-disk-<id>`, controller-owned by the VM, holding only
/// the CHAP password and its generation annotation. Never read data back.
pub struct SecretStore<'a> {
    pub api: Api<Secret>,
    pub vm: &'a VirtualMachine,
}

#[async_trait::async_trait]
impl CredentialStore for SecretStore<'_> {
    async fn put(&self, disk: &str, credential: &Credential) -> Result<()> {
        let secret = self.render(disk, credential)?;
        // Server-side apply creates or replaces without a read; admission
        // refuses it unless any existing object is this VM's credential.
        self.api
            .patch(
                &secret.name_any(),
                &PatchParams::apply("qrab-operator").force(),
                &Patch::Apply(&secret),
            )
            .await?;
        Ok(())
    }

    async fn delete(&self, disk: &str) -> Result<()> {
        let name = secret_name(&self.vm.name_any(), disk);
        let uid = self
            .vm
            .uid()
            .ok_or_else(|| Error::Invalid("VM UID is required".into()))?;
        // Names are reusable, including collisions between valid VM/disk pairs.
        // Atomically test the intended owner without Secret GET permission or
        // a response containing data. Every operation is a no-op JSON test.
        let tests = serde_json::from_value(json!([
            {"op":"test","path":"/metadata/ownerReferences/0/apiVersion","value":"qrab.yapplesauce.com/v1alpha1"},
            {"op":"test","path":"/metadata/ownerReferences/0/kind","value":"VirtualMachine"},
            {"op":"test","path":"/metadata/ownerReferences/0/name","value":self.vm.name_any()},
            {"op":"test","path":"/metadata/ownerReferences/0/uid","value":uid},
            {"op":"test","path":"/metadata/ownerReferences/0/controller","value":true},
            {"op":"test","path":"/metadata/labels/app.kubernetes.io~1component","value":"disk-credential"},
            {"op":"test","path":"/metadata/labels/qrab.yapplesauce.com~1owner-uid","value":uid},
            {"op":"test","path":"/metadata/annotations/qrab.yapplesauce.com~1disk","value":disk}
        ]))?;
        let checked = match self
            .api
            .patch_metadata(&name, &PatchParams::default(), &Patch::Json::<()>(tests))
            .await
        {
            Ok(checked) => checked,
            Err(kube::Error::Api(status)) if status.code == 404 => return Ok(()),
            Err(kube::Error::Api(status)) if status.code == 422 => {
                return Err(Error::Ownership(format!(
                    "disk credential {name} does not match VM {uid} and disk {disk}"
                )))
            }
            Err(error) => return Err(error.into()),
        };
        let metadata = checked.metadata;
        if metadata.name.as_deref() != Some(name.as_str())
            || metadata.namespace != self.vm.namespace()
            || !metadata.owner_references.as_deref().is_some_and(|refs| {
                matches!(refs, [owner] if owner.api_version == "qrab.yapplesauce.com/v1alpha1"
                    && owner.kind == "VirtualMachine" && owner.name == self.vm.name_any()
                    && owner.uid == uid && owner.controller == Some(true))
            })
            || metadata.uid.as_deref().is_none_or(str::is_empty)
            || metadata
                .resource_version
                .as_deref()
                .is_none_or(str::is_empty)
        {
            return Err(Error::Ownership(format!(
                "disk credential {name} lacks exact owned object metadata"
            )));
        }
        // Preserve the checked object through the effect. A new Secret or a
        // credential rotation between PATCH and DELETE must cause a conflict.
        let params = DeleteParams::default().preconditions(Preconditions {
            uid: metadata.uid,
            resource_version: metadata.resource_version,
        });
        match self.api.delete(&name, &params).await {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(status)) if status.code == 404 => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl SecretStore<'_> {
    /// The exact object `put` applies; admission accepts nothing else.
    pub fn render(&self, disk: &str, credential: &Credential) -> Result<Secret> {
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
        Ok(secret)
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
        /// Simulates a daemon wrapper timeout during a slow kernel drain.
        pub fence_timeout: std::sync::atomic::AtomicBool,
        /// Simulates an identity without `storage_retire`.
        pub retire_forbidden: std::sync::atomic::AtomicBool,
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
            let timeout = self.fence_timeout.load(std::sync::atomic::Ordering::SeqCst);
            self.with("fence", uid, id, |m, k| {
                if timeout {
                    if let Some(d) = m.get_mut(&k) {
                        d.phase = DiskPhase::Fencing;
                    }
                    return Err(StorageError::Unavailable("wrapper-timeout".into()));
                }
                let d = m
                    .get_mut(&k)
                    .ok_or(StorageError::Refused("disk-not-found".into()))?;
                if d.phase != DiskPhase::Provisioned && d.phase != DiskPhase::Retired {
                    d.phase = DiskPhase::Fenced;
                }
                Ok(Self::report(uid, id, d))
            })
        }
        async fn retire_disk(&self, uid: &str, id: &str) -> StorageResult<DiskReport> {
            let forbidden = self
                .retire_forbidden
                .load(std::sync::atomic::Ordering::SeqCst);
            self.with("retire", uid, id, |m, k| {
                if forbidden {
                    return refused("forbidden");
                }
                let d = m
                    .get_mut(&k)
                    .ok_or(StorageError::Refused("disk-not-found".into()))?;
                match d.phase {
                    DiskPhase::Fenced | DiskPhase::Provisioned | DiskPhase::Retired => {
                        d.phase = DiskPhase::Retired
                    }
                    _ => return refused("fence-required"),
                }
                Ok(Self::report(uid, id, d))
            })
        }
    }

    #[derive(Default)]
    pub struct FakeCredentials(pub Mutex<BTreeMap<String, Credential>>);

    #[async_trait::async_trait]
    impl CredentialStore for FakeCredentials {
        async fn delete(&self, disk: &str) -> Result<()> {
            self.0.lock().unwrap().remove(disk);
            Ok(())
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
    async fn every_export_writes_a_fresh_secret_before_the_call() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let first = creds.0.lock().unwrap()["root"].clone();
        api.fence(UID, "root").await.unwrap();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let second = creds.0.lock().unwrap()["root"].clone();
        assert_ne!(first.secret, second.secret);
        assert_eq!(second.generation, 2);
        assert_eq!(api.disk(UID, "root").unwrap().secret, Some(second.secret.0));
    }

    #[tokio::test]
    async fn interrupted_export_waits_while_live_then_fences_and_rotates_when_stopped() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let mut interrupted = api.disk(UID, "root").unwrap();
        interrupted.phase = DiskPhase::Preparing;
        api.set(UID, "root", interrupted);
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
    async fn a_fence_timeout_is_reported_as_progress_and_resumed() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        api.fence_timeout
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let error = release(&api, &creds, UID, "root").await.unwrap_err();
        assert!(fence_in_progress(&error));
        assert!(
            creds.0.lock().unwrap().contains_key("root"),
            "kept until fenced"
        );
        let (step, status) = run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        assert!(matches!(step, Step::Wait(ref m) if m.contains("in progress")));
        assert_eq!(status.unwrap().phase, "Fencing");
        api.fence_timeout
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let (step, _) = run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let Step::Ready(attached) = step else {
            panic!("not ready")
        };
        assert_eq!(
            attached.generation, 2,
            "resumed fence rotates the credential"
        );
        assert!(!fence_in_progress(&Error::Storage(StorageError::Refused(
            "fence-required".into()
        ))));
    }

    #[test]
    fn removed_disks_are_released_only_once_no_launched_pod_mounts_them() {
        let mut vm: VirtualMachine = serde_json::from_value(serde_json::json!({
            "apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
            "metadata":{"name":"vm","namespace":"qrab","uid":UID},
            "spec":{"stateVolumeClaim":"state","console":{"publicUrl":"https://vm.example/","allowedSubjects":[]},
                "disks":[{"id":"root","sizeBytes":67108864}]},
            "status":{"observedGeneration":1,"phase":"Running","ready":true,"reason":"","message":"",
                "disks":[{"id":"root","capacityBytes":67108864,"generation":1,"phase":"Active"},
                         {"id":"old","capacityBytes":67108864,"generation":1,"phase":"Active"}]}
        }))
        .unwrap();
        let pod: Pod = serde_json::from_value(serde_json::json!({
            "metadata":{"name":"vm"},
            "spec":{"containers":[{"name":"manager"}],"volumes":[{"name":"iscsi-auth-old","secret":{"secretName":"vm-disk-old"}}]}
        }))
        .unwrap();
        assert!(
            removable(&vm, Some(&pod), true).is_empty(),
            "still mounted by a live QEMU"
        );
        assert_eq!(removable(&vm, Some(&pod), false), vec!["old"]);
        assert_eq!(removable(&vm, None, false), vec!["old"]);
        vm.spec.disks.clear();
        assert_eq!(removable(&vm, None, false), vec!["root", "old"]);
    }

    #[tokio::test]
    async fn a_removed_disk_is_fenced_before_its_secret_is_deleted() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        release(&api, &creds, UID, "root").await.unwrap();
        assert_eq!(api.disk(UID, "root").unwrap().phase, DiskPhase::Fenced);
        assert!(creds.0.lock().unwrap().is_empty());
        assert_eq!(api.calls().last().map(String::as_str), Some("fence root"));
        // Already gone from storage: only the Secret is removed.
        release(&api, &creds, UID, "never").await.unwrap();
        // Re-adding the same id reattaches the retained data with a new secret.
        let (step, _) = run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        assert!(matches!(step, Step::Ready(ref a) if a.generation == 2));
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

        // Deletion then retires both; a second pass calls nothing new.
        api.retire_forbidden
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let refused = retire_all(&api, &vm).await.unwrap();
        assert_eq!(
            refused,
            [
                ("root".to_string(), "forbidden".to_string()),
                ("old".into(), "forbidden".into())
            ]
        );
        assert_eq!(api.disk(UID, "root").unwrap().phase, DiskPhase::Fenced);
        api.retire_forbidden
            .store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(retire_all(&api, &vm).await.unwrap().is_empty());
        assert_eq!(api.disk(UID, "root").unwrap().phase, DiskPhase::Retired);
        assert_eq!(api.disk(UID, "old").unwrap().phase, DiskPhase::Retired);
        let before = api
            .calls()
            .iter()
            .filter(|c| c.starts_with("retire"))
            .count();
        assert!(retire_all(&api, &vm).await.unwrap().is_empty());
        let after = api
            .calls()
            .iter()
            .filter(|c| c.starts_with("retire"))
            .count();
        assert_eq!(before, after, "already retired disks are not retired again");
    }

    #[tokio::test]
    async fn retire_all_refuses_an_unfenced_disk() {
        let api = FakeStorage::default();
        let creds = FakeCredentials::default();
        run(&api, &creds, &disk(64 * MIB, None), false)
            .await
            .unwrap();
        let mut vm = VirtualMachine::new(
            "vm",
            serde_json::from_value(serde_json::json!({
                "stateVolumeClaim":"state","console":{"publicUrl":"https://vm.example/","allowedSubjects":[]},
                "disks":[{"id":"root","sizeBytes":67108864}]
            }))
            .unwrap(),
        );
        vm.metadata.uid = Some(UID.into());
        assert_eq!(api.disk(UID, "root").unwrap().phase, DiskPhase::Active);
        assert_eq!(
            retire_all(&api, &vm).await.unwrap(),
            [("root".to_string(), "fence-required".to_string())]
        );
        assert_eq!(api.disk(UID, "root").unwrap().phase, DiskPhase::Active);
    }
}
