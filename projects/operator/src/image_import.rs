//! The image importer (`qrab-operator image-importer`): drives each
//! BaseImage from Pending to Sealed (container/image-service §4).
//!
//! Inspect Job → `vm-image-stage-create` → fresh CHAP in Secret
//! `<bimg>-stage` → `vm-image-stage-export` → write Job → `vm-image-stage-seal`.
//! The Jobs hold no Kubernetes token; the storage host hashes the volume
//! itself when it seals. A generation that already exists is adopted.
use crate::{
    image::{BaseImage, BaseImageStatus, ImagePhase},
    image_job::{self, Failure, Inspected, Written},
    storage::{ChapSecret, SharedImageStorage, StagePhase, StageReport, StorageError},
};
use anyhow::{Context as _, Result};
use k8s_openapi::api::{
    batch::v1::Job,
    core::v1::{Pod, Secret},
};
use kube::{
    api::{DeleteParams, ListParams, Patch, PatchParams, PostParams},
    Api, Resource, ResourceExt,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};

pub const FINALIZER: &str = "qrab.yapplesauce.com/image-stage";
pub const MANAGER: &str = "qrab-image-importer";
pub const MAX_ATTEMPTS: u32 = 3;
pub const CONTAINER: &str = "image-job";
const MIB: u64 = 1 << 20;
const MIN_STAGE: u64 = 16 * MIB;
const BUSY: Duration = Duration::from_secs(10);
const IDLE: Duration = Duration::from_secs(600);

/// A Job as the importer sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobState {
    Running,
    /// The container's termination message.
    Succeeded(String),
    Failed(String),
}

#[async_trait::async_trait]
pub trait ImportCluster: Send + Sync {
    async fn job(&self, name: &str) -> Result<Option<JobState>>;
    async fn create_job(&self, job: &serde_json::Value) -> Result<()>;
    /// Deletes a Job and its Pods; absent is fine.
    async fn delete_job(&self, name: &str) -> Result<()>;
    /// Server-side applies the stage credential Secret (never read back).
    async fn apply_secret(&self, secret: &serde_json::Value) -> Result<()>;
    async fn delete_secret(&self, name: &str) -> Result<()>;
    async fn patch_status(&self, name: &str, status: &BaseImageStatus) -> Result<()>;
    async fn set_finalizers(&self, name: &str, finalizers: &[String]) -> Result<()>;
}

pub struct Importer {
    pub cluster: Arc<dyn ImportCluster>,
    pub storage: SharedImageStorage,
    pub namespace: String,
    /// Pinned image for the Job containers (qrab-operator with qemu-img).
    pub job_image: String,
    pub service_account: String,
}

pub fn inspect_job_name(name: &str) -> String {
    format!("{name}-inspect")
}
pub fn write_job_name(name: &str) -> String {
    format!("{name}-write")
}
pub fn secret_name(name: &str) -> String {
    format!("{name}-stage")
}

/// The stage capacity for a raw size: whole MiB, at least 16 MiB.
///
/// ```
/// assert_eq!(qrab_operator::image_import::stage_capacity(117440512), 117440512);
/// assert_eq!(qrab_operator::image_import::stage_capacity(512), 16 << 20);
/// assert_eq!(qrab_operator::image_import::stage_capacity((20 << 20) + 512), 21 << 20);
/// ```
pub fn stage_capacity(size: u64) -> u64 {
    size.div_ceil(MIB).saturating_mul(MIB).max(MIN_STAGE)
}

fn owner(image: &BaseImage) -> serde_json::Value {
    json!([{
        "apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "BaseImage",
        "name": image.name_any(), "uid": image.uid().unwrap_or_default(),
        "controller": true, "blockOwnerDeletion": false,
    }])
}

fn labels(image: &BaseImage, component: &str) -> serde_json::Value {
    json!({
        "app.kubernetes.io/name": "qrab-image-job",
        "app.kubernetes.io/component": component,
        "app.kubernetes.io/instance": image.name_any(),
        "app.kubernetes.io/managed-by": MANAGER,
        "qrab.yapplesauce.com/owner-uid": image.uid().unwrap_or_default(),
    })
}

/// Secret `<bimg>-stage`: the stage's CHAP password, key `password` only.
pub fn render_secret(image: &BaseImage, namespace: &str, secret: &ChapSecret) -> serde_json::Value {
    let mut object: Secret = serde_json::from_value(json!({
        "apiVersion": "v1", "kind": "Secret",
        "metadata": {"name": secret_name(&image.name_any()), "namespace": namespace,
            "labels": labels(image, "image-stage-credential"), "ownerReferences": owner(image)},
        "type": "Opaque",
    }))
    .expect("static Secret shape");
    object.data = Some(std::collections::BTreeMap::from([(
        "password".to_owned(),
        k8s_openapi::ByteString(secret.0.clone().into_bytes()),
    )]));
    serde_json::to_value(object).expect("Secret serializes")
}

impl Importer {
    /// One Job Pod: no token, no privileges, a sized scratch volume, and the
    /// report in the termination message.
    pub fn render_job(
        &self,
        image: &BaseImage,
        component: &str,
        args: Vec<String>,
        scratch: u64,
    ) -> serde_json::Value {
        let name = image.name_any();
        let job = if component == "inspect" {
            inspect_job_name(&name)
        } else {
            write_job_name(&name)
        };
        let mut command = vec![
            "/usr/local/bin/qrab-operator".to_owned(),
            "image-job".into(),
            component.into(),
        ];
        command.extend(args);
        let mut mounts = vec![json!({"name": "work", "mountPath": image_job::WORK_DIR})];
        let mut volumes = vec![
            json!({"name": "work", "emptyDir": {"sizeLimit": format!("{}Mi", scratch.div_ceil(MIB))}}),
        ];
        if component == "write" {
            mounts.push(json!({"name": "stage-credential", "mountPath": "/run/qrab-stage", "readOnly": true}));
            volumes.push(json!({"name": "stage-credential", "secret": {
                "secretName": secret_name(&name), "defaultMode": 0o440,
                "items": [{"key": "password", "path": "password"}]}}));
        }
        json!({
            "apiVersion": "batch/v1", "kind": "Job",
            "metadata": {"name": job, "namespace": self.namespace, "labels": labels(image, component), "ownerReferences": owner(image)},
            "spec": {
                "backoffLimit": 0,
                "activeDeadlineSeconds": 7200,
                "template": {
                    "metadata": {"labels": labels(image, component)},
                    "spec": {
                        "serviceAccountName": self.service_account,
                        "automountServiceAccountToken": false,
                        "enableServiceLinks": false,
                        "restartPolicy": "Never",
                        "securityContext": {"runAsNonRoot": true, "runAsUser": 10001, "runAsGroup": 10001,
                            "fsGroup": 10001, "seccompProfile": {"type": "RuntimeDefault"}},
                        "containers": [{
                            "name": CONTAINER,
                            "image": self.job_image,
                            "command": command,
                            "terminationMessagePolicy": "File",
                            "resources": {"requests": {"cpu": "250m", "memory": "256Mi"}, "limits": {"memory": "1Gi"}},
                            "securityContext": {"allowPrivilegeEscalation": false, "readOnlyRootFilesystem": true,
                                "capabilities": {"drop": ["ALL"]}},
                            "volumeMounts": mounts,
                        }],
                        "volumes": volumes,
                    }
                }
            }
        })
    }

    fn source_args(image: &BaseImage) -> Vec<String> {
        let s = &image.spec.source;
        vec![
            "--url".into(),
            s.url.clone(),
            "--sha256".into(),
            s.sha256.clone(),
            "--format".into(),
            s.format.as_str().into(),
        ]
    }

    async fn save(&self, image: &BaseImage, status: &BaseImageStatus) -> Result<()> {
        if image.status.as_ref() != Some(status) {
            self.cluster.patch_status(&image.name_any(), status).await?;
        }
        Ok(())
    }

    async fn clean(&self, name: &str) -> Result<()> {
        self.cluster.delete_job(&inspect_job_name(name)).await?;
        self.cluster.delete_job(&write_job_name(name)).await?;
        self.cluster.delete_secret(&secret_name(name)).await
    }

    /// Aborts an unsealed stage. `false` while a seal is in progress.
    async fn abort(&self, image: &BaseImage) -> Result<bool> {
        let (n, g) = (&image.spec.image, image.spec.generation);
        let stage = self.storage.stage_status(n, g).await?;
        match stage.phase {
            StagePhase::Absent => Ok(true),
            StagePhase::Sealing => Ok(false),
            _ => {
                self.storage.stage_abort(n, g).await?;
                Ok(true)
            }
        }
    }

    /// Counts a failed attempt; the last one fails the image.
    async fn attempt_failed(
        &self,
        image: &BaseImage,
        status: &mut BaseImageStatus,
        message: String,
        fatal: bool,
    ) -> Result<Duration> {
        status.attempts += 1;
        status.message = message;
        if fatal || status.attempts >= MAX_ATTEMPTS {
            status.phase = ImagePhase::Failed;
            self.abort(image).await?;
            self.clean(&image.name_any()).await?;
            self.save(image, status).await?;
            return Ok(IDLE);
        }
        status.phase = ImagePhase::Pending;
        self.save(image, status).await?;
        Ok(Duration::from_secs(30 * u64::from(status.attempts)))
    }

    /// One reconcile; returns when to look again.
    pub async fn reconcile(&self, image: &BaseImage) -> Result<Duration> {
        let name = image.name_any();
        let finalizers = image.finalizers().to_vec();
        if image.meta().deletion_timestamp.is_some() {
            if finalizers.iter().any(|f| f == FINALIZER) {
                let sealed = matches!(image.phase(), ImagePhase::Sealed | ImagePhase::Retired);
                if !sealed && !self.abort(image).await? {
                    return Ok(BUSY);
                }
                self.clean(&name).await?;
                let rest: Vec<String> = finalizers.into_iter().filter(|f| f != FINALIZER).collect();
                self.cluster.set_finalizers(&name, &rest).await?;
            }
            return Ok(IDLE);
        }
        let mut status = image.status.clone().unwrap_or_default();
        status.observed_generation = image.meta().generation;
        let expected = BaseImage::expected_name(&image.spec);
        if name != expected {
            status.phase = ImagePhase::Failed;
            status.message = format!(
                "a BaseImage for {} generation {} must be named {expected}",
                image.spec.image, image.spec.generation
            );
            self.save(image, &status).await?;
            return Ok(IDLE);
        }
        if !finalizers.iter().any(|f| f == FINALIZER) {
            let mut with = finalizers.clone();
            with.push(FINALIZER.into());
            self.cluster.set_finalizers(&name, &with).await?;
        }
        let (n, g) = (image.spec.image.as_str(), image.spec.generation);
        match status.phase {
            ImagePhase::Retired | ImagePhase::Failed => return Ok(IDLE),
            ImagePhase::Sealed => {
                if image.spec.retired {
                    match self.storage.retire(n, g).await {
                        Ok(_) => {
                            status.phase = ImagePhase::Retired;
                            status.message = "Retired: no new VMs clone this generation.".into();
                        }
                        Err(StorageError::Refused(code)) => {
                            status.message = format!("Retiring is refused ({code}); retrying.")
                        }
                        Err(error) => return Err(error.into()),
                    }
                    self.save(image, &status).await?;
                    return Ok(BUSY.max(Duration::from_secs(60)));
                }
                return Ok(IDLE);
            }
            _ => {}
        }
        if let Some(report) = self.storage.image_status(n, g).await? {
            let ours = matches!(status.phase, ImagePhase::Sealing | ImagePhase::Writing);
            status.phase = match report.phase.as_str() {
                "sealed" => ImagePhase::Sealed,
                "retired" => ImagePhase::Retired,
                _ => {
                    status.message =
                        format!("Generation {g} is being published on the storage host.");
                    self.save(image, &status).await?;
                    return Ok(BUSY);
                }
            };
            status.adopted = !ours;
            status.capacity_bytes = Some(report.capacity_bytes);
            if let Some(import) = report.import {
                status.size_bytes = Some(import.size_bytes);
                status.sha256 = Some(import.sha256);
            }
            status.message = if ours {
                "Sealed.".into()
            } else {
                "Adopted: the generation already exists on the storage host.".into()
            };
            self.clean(&name).await?;
            self.save(image, &status).await?;
            return Ok(BUSY);
        }
        if image.spec.retired {
            if !self.abort(image).await? {
                return Ok(BUSY);
            }
            self.clean(&name).await?;
            status.phase = ImagePhase::Retired;
            status.message = "Retired before it was sealed.".into();
            self.save(image, &status).await?;
            return Ok(IDLE);
        }
        // 1. Inspect: download, verify and measure.
        let size = match status.size_bytes {
            Some(size) => size,
            None => match self.cluster.job(&inspect_job_name(&name)).await? {
                None => {
                    let job = self.render_job(
                        image,
                        "inspect",
                        Self::source_args(image),
                        image_job::MAX_IMAGE_BYTES + (1 << 30),
                    );
                    self.cluster.create_job(&job).await?;
                    status.phase = ImagePhase::Inspecting;
                    status.message = "Downloading and checking the image.".into();
                    self.save(image, &status).await?;
                    return Ok(BUSY);
                }
                Some(JobState::Running) => return Ok(BUSY),
                Some(JobState::Failed(message)) => {
                    self.cluster.delete_job(&inspect_job_name(&name)).await?;
                    let failure = serde_json::from_str::<Failure>(&message).ok();
                    let fatal = failure
                        .as_ref()
                        .is_some_and(|f| f.error == "sha256-mismatch");
                    let text = failure
                        .map(|f| f.message)
                        .unwrap_or_else(|| "the inspect Job failed".into());
                    return self
                        .attempt_failed(image, &mut status, format!("Inspect: {text}"), fatal)
                        .await;
                }
                Some(JobState::Succeeded(message)) => {
                    self.cluster.delete_job(&inspect_job_name(&name)).await?;
                    match serde_json::from_str::<Inspected>(&message) {
                        Ok(inspected) if inspected.virtual_size <= image_job::MAX_IMAGE_BYTES => {
                            status.size_bytes = Some(inspected.virtual_size);
                            status.capacity_bytes = Some(stage_capacity(inspected.virtual_size));
                            self.save(image, &status).await?;
                            inspected.virtual_size
                        }
                        _ => {
                            return self
                                .attempt_failed(
                                    image,
                                    &mut status,
                                    "Inspect: no valid report".into(),
                                    false,
                                )
                                .await
                        }
                    }
                }
            },
        };
        let capacity = stage_capacity(size);
        // 2. Stage and write.
        match self.cluster.job(&write_job_name(&name)).await? {
            None => {
                let mut stage = self.storage.stage_status(n, g).await?;
                if stage.phase == StagePhase::Absent {
                    stage = self.storage.stage_create(n, g, capacity).await?;
                }
                if !matches!(stage.phase, StagePhase::Provisioned | StagePhase::Fenced) {
                    // An earlier write left it exported (or mid-seal): start over.
                    self.abort(image).await?;
                    return Ok(BUSY);
                }
                let secret = crate::provision::Credential::generate(stage.generation + 1)
                    .context("generate the stage credential")?
                    .secret;
                self.cluster
                    .apply_secret(&render_secret(image, &self.namespace, &secret))
                    .await?;
                let stage = self
                    .storage
                    .stage_export(n, g, stage.generation + 1, &secret)
                    .await?;
                let job = self.render_job(
                    image,
                    "write",
                    self.write_args(image, &stage, size),
                    size.saturating_mul(2) + (1 << 30),
                );
                self.cluster.create_job(&job).await?;
                status.phase = ImagePhase::Writing;
                status.message = "Converting the image and writing it to the stage volume.".into();
                self.save(image, &status).await?;
                Ok(BUSY)
            }
            Some(JobState::Running) => Ok(BUSY),
            Some(JobState::Failed(message)) => {
                let text = serde_json::from_str::<Failure>(&message)
                    .map(|f| f.message)
                    .unwrap_or_else(|_| "the write Job failed".into());
                if !self.abort(image).await? {
                    return Ok(BUSY);
                }
                self.cluster.delete_job(&write_job_name(&name)).await?;
                self.cluster.delete_secret(&secret_name(&name)).await?;
                self.attempt_failed(image, &mut status, format!("Write: {text}"), false)
                    .await
            }
            Some(JobState::Succeeded(message)) => {
                let written = match serde_json::from_str::<Written>(&message) {
                    Ok(w) if w.size_bytes == size && w.sha256.len() == 64 => w,
                    _ => {
                        self.abort(image).await?;
                        self.cluster.delete_job(&write_job_name(&name)).await?;
                        return self
                            .attempt_failed(
                                image,
                                &mut status,
                                "Write: no valid report".into(),
                                false,
                            )
                            .await;
                    }
                };
                if status.phase != ImagePhase::Sealing {
                    status.phase = ImagePhase::Sealing;
                    status.message = "Fencing, hashing and sealing on the storage host.".into();
                    self.save(image, &status).await?;
                }
                match self
                    .storage
                    .stage_seal(n, g, written.size_bytes, &written.sha256)
                    .await
                {
                    Ok(report) => {
                        status.phase = ImagePhase::Sealed;
                        status.adopted = false;
                        status.capacity_bytes = Some(report.capacity_bytes);
                        status.size_bytes = Some(written.size_bytes);
                        status.sha256 =
                            Some(report.import.map(|i| i.sha256).unwrap_or(written.sha256));
                        status.message = "Sealed.".into();
                        self.clean(&name).await?;
                        self.save(image, &status).await?;
                        Ok(BUSY)
                    }
                    Err(StorageError::Refused(code)) if code == "sha256-mismatch" => {
                        self.abort(image).await?;
                        self.clean(&name).await?;
                        self.attempt_failed(
                            image,
                            &mut status,
                            "Seal: the volume's SHA-256 differs from what the write Job wrote"
                                .into(),
                            false,
                        )
                        .await
                    }
                    Err(error) => Err(error.into()),
                }
            }
        }
    }

    fn write_args(&self, image: &BaseImage, stage: &StageReport, size: u64) -> Vec<String> {
        let mut args = Self::source_args(image);
        args.extend([
            "--size-bytes".into(),
            size.to_string(),
            "--portal".into(),
            stage.portal.clone(),
            "--target".into(),
            stage.target_iqn.clone(),
            "--initiator".into(),
            stage.initiator_iqn.clone(),
            "--chap-user".into(),
            stage.chap_username.clone(),
        ]);
        args
    }
}

/// The Kubernetes side of [`ImportCluster`].
pub struct KubeImportCluster {
    pub jobs: Api<Job>,
    pub pods: Api<Pod>,
    pub secrets: Api<Secret>,
    pub images: Api<BaseImage>,
}

impl KubeImportCluster {
    pub fn new(client: kube::Client, namespace: &str) -> Self {
        Self {
            jobs: Api::namespaced(client.clone(), namespace),
            pods: Api::namespaced(client.clone(), namespace),
            secrets: Api::namespaced(client.clone(), namespace),
            images: Api::namespaced(client, namespace),
        }
    }
}

fn termination_message(pods: &[Pod]) -> String {
    pods.iter()
        .filter_map(|pod| pod.status.as_ref()?.container_statuses.as_ref())
        .flatten()
        .filter(|c| c.name == CONTAINER)
        .find_map(|c| c.state.as_ref()?.terminated.as_ref()?.message.clone())
        .unwrap_or_default()
}

#[async_trait::async_trait]
impl ImportCluster for KubeImportCluster {
    async fn job(&self, name: &str) -> Result<Option<JobState>> {
        let Some(job) = self.jobs.get_opt(name).await? else {
            return Ok(None);
        };
        let status = job.status.unwrap_or_default();
        let done = |kind: &str| {
            status
                .conditions
                .as_ref()
                .is_some_and(|c| c.iter().any(|c| c.type_ == kind && c.status == "True"))
        };
        let (succeeded, failed) = (done("Complete"), done("Failed"));
        if !succeeded && !failed {
            return Ok(Some(JobState::Running));
        }
        let pods = self
            .pods
            .list(&ListParams::default().labels(&format!("batch.kubernetes.io/job-name={name}")))
            .await?;
        let message = termination_message(&pods.items);
        Ok(Some(if succeeded {
            JobState::Succeeded(message)
        } else {
            JobState::Failed(message)
        }))
    }
    async fn create_job(&self, job: &serde_json::Value) -> Result<()> {
        let job: Job = serde_json::from_value(job.clone())?;
        match self.jobs.create(&PostParams::default(), &job).await {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(e)) if e.code == 409 => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
    async fn delete_job(&self, name: &str) -> Result<()> {
        match self.jobs.delete(name, &DeleteParams::foreground()).await {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(e)) if e.code == 404 => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
    async fn apply_secret(&self, secret: &serde_json::Value) -> Result<()> {
        let name = secret["metadata"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        self.secrets
            .patch(
                &name,
                &PatchParams::apply(MANAGER).force(),
                &Patch::Apply(secret),
            )
            .await?;
        Ok(())
    }
    async fn delete_secret(&self, name: &str) -> Result<()> {
        match self.secrets.delete(name, &DeleteParams::default()).await {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(e)) if e.code == 404 => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
    async fn patch_status(&self, name: &str, status: &BaseImageStatus) -> Result<()> {
        self.images
            .patch_status(
                name,
                &PatchParams::default(),
                &Patch::Merge(json!({"status": status})),
            )
            .await?;
        Ok(())
    }
    async fn set_finalizers(&self, name: &str, finalizers: &[String]) -> Result<()> {
        self.images
            .patch(
                name,
                &PatchParams::default(),
                &Patch::Merge(json!({"metadata": {"finalizers": finalizers}})),
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
