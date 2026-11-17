//! Kubernetes effects are conditional on the BaseImage and child incarnations.
use super::*;
use anyhow::{bail, ensure};
use k8s_openapi::{
    api::{batch::v1::Job, core::v1::Pod},
    apimachinery::pkg::apis::meta::v1::ObjectMeta,
};
use kube::{
    api::{DeleteParams, ListParams, Patch, PatchParams, PostParams, Preconditions},
    Api,
};

pub struct KubeImportCluster {
    pub jobs: Api<Job>,
    pub pods: Api<Pod>,
    pub secrets: Api<Secret>,
    pub images: Api<BaseImage>,
}

fn identity(metadata: &ObjectMeta) -> Result<Preconditions> {
    ensure!(
        metadata.uid.as_deref().is_some_and(|s| !s.is_empty())
            && metadata
                .resource_version
                .as_deref()
                .is_some_and(|s| !s.is_empty()),
        "object UID and resourceVersion are required"
    );
    Ok(Preconditions {
        uid: metadata.uid.clone(),
        resource_version: metadata.resource_version.clone(),
    })
}

fn owned(metadata: &ObjectMeta, image: &BaseImage, name: &str) -> Result<()> {
    let uid = image
        .uid()
        .filter(|s| !s.is_empty())
        .context("BaseImage UID is required")?;
    ensure!(
        metadata.name.as_deref() == Some(name)
            && metadata.namespace == image.namespace()
            && metadata.owner_references.as_deref().is_some_and(|owners| {
                matches!(owners, [owner] if owner.api_version == "qrab.yapplesauce.com/v1alpha1"
                    && owner.kind == "BaseImage" && owner.name == image.name_any()
                    && owner.uid == uid && owner.controller == Some(true))
            }),
        "{name} is not owned by this BaseImage incarnation"
    );
    Ok(())
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

    /// Refresh only metadata/status progress. An old reconcile must not cross
    /// recreation, spec changes or the beginning of deletion. Refreshing the
    /// version permits multiple status writes within one reconcile.
    async fn current(&self, observed: &BaseImage) -> Result<BaseImage> {
        identity(&observed.metadata)?;
        ensure!(
            observed.metadata.generation.is_some(),
            "BaseImage generation is required"
        );
        let current = self.images.get(&observed.name_any()).await?;
        identity(&current.metadata)?;
        ensure!(
            current.uid() == observed.uid()
                && current.namespace() == observed.namespace()
                && current.metadata.generation == observed.metadata.generation
                && current.spec == observed.spec
                && current.metadata.deletion_timestamp.is_some()
                    == observed.metadata.deletion_timestamp.is_some(),
            "BaseImage changed while the importer was reconciling"
        );
        Ok(current)
    }

    async fn owned_job(&self, image: &BaseImage, name: &str) -> Result<Option<Job>> {
        let job = self.jobs.get_opt(name).await?;
        if let Some(job) = &job {
            owned(&job.metadata, image, name)?;
            identity(&job.metadata)?;
        }
        Ok(job)
    }
}

fn termination_message(pods: &[Pod], job: &Job, succeeded: bool) -> String {
    pods.iter()
        .filter(|pod| {
            pod.namespace() == job.namespace()
                && pod.owner_references().iter().any(|owner| {
                    owner.api_version == "batch/v1"
                        && owner.kind == "Job"
                        && owner.name == job.name_any()
                        && Some(&owner.uid) == job.metadata.uid.as_ref()
                        && owner.controller == Some(true)
                })
        })
        .filter_map(|pod| pod.status.as_ref()?.container_statuses.as_ref())
        .flatten()
        .filter(|container| container.name == CONTAINER)
        .filter_map(|container| container.state.as_ref()?.terminated.as_ref())
        .filter(|terminated| !succeeded || terminated.exit_code == 0)
        .find_map(|terminated| terminated.message.clone())
        .unwrap_or_default()
}

#[async_trait::async_trait]
impl ImportCluster for KubeImportCluster {
    async fn check_image(&self, image: &BaseImage) -> Result<()> {
        self.current(image).await.map(drop)
    }

    async fn job(&self, image: &BaseImage, name: &str) -> Result<Option<ObservedJob>> {
        self.current(image).await?;
        let Some(job) = self.owned_job(image, name).await? else {
            return Ok(None);
        };
        let done = |kind: &str| {
            job.status
                .as_ref()
                .and_then(|s| s.conditions.as_ref())
                .is_some_and(|conditions| {
                    conditions
                        .iter()
                        .any(|c| c.type_ == kind && c.status == "True")
                })
        };
        let (succeeded, failed) = (done("Complete"), done("Failed"));
        let state = if succeeded || failed {
            let pods = self
                .pods
                .list(
                    &ListParams::default().labels(&format!("batch.kubernetes.io/job-name={name}")),
                )
                .await?;
            let message = termination_message(&pods.items, &job, succeeded);
            if succeeded {
                JobState::Succeeded(message)
            } else {
                JobState::Failed(message)
            }
        } else {
            JobState::Running
        };
        Ok(Some(ObservedJob {
            state,
            uid: job.metadata.uid.expect("checked UID"),
            resource_version: job.metadata.resource_version.expect("checked version"),
        }))
    }

    async fn create_job(&self, image: &BaseImage, value: &serde_json::Value) -> Result<()> {
        self.current(image).await?;
        let job: Job = serde_json::from_value(value.clone())?;
        let name = job.name_any();
        owned(&job.metadata, image, &name)?;
        match self.jobs.create(&PostParams::default(), &job).await {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(error)) if error.code == 409 => {
                self.owned_job(image, &name)
                    .await?
                    .context("conflicting Job disappeared")?;
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn delete_job(
        &self,
        image: &BaseImage,
        name: &str,
        observed: Option<&ObservedJob>,
    ) -> Result<()> {
        self.current(image).await?;
        let Some(job) = self.owned_job(image, name).await? else {
            return Ok(());
        };
        let preconditions = identity(&job.metadata)?;
        if let Some(observed) = observed {
            ensure!(
                preconditions.uid.as_deref() == Some(&observed.uid)
                    && preconditions.resource_version.as_deref()
                        == Some(&observed.resource_version),
                "Job changed after its report was observed"
            );
        }
        match self
            .jobs
            .delete(
                name,
                &DeleteParams::foreground().preconditions(preconditions),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(error)) if error.code == 404 => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    async fn apply_secret(&self, image: &BaseImage, secret: &serde_json::Value) -> Result<()> {
        self.current(image).await?;
        let metadata: ObjectMeta = serde_json::from_value(secret["metadata"].clone())?;
        let name = secret_name(&image.name_any());
        owned(&metadata, image, &name)?;
        // Existing-object ownership is also checked by image-stage admission.
        self.secrets
            .patch(
                &name,
                &PatchParams::apply(MANAGER).force(),
                &Patch::Apply(secret),
            )
            .await?;
        Ok(())
    }

    async fn delete_secret(&self, image: &BaseImage, name: &str) -> Result<()> {
        self.current(image).await?;
        let uid = image.uid().context("BaseImage UID is required")?;
        // No Secret GET/list/watch or credential payload in the response.
        // JSON tests prove ownership atomically; the response supplies the
        // precise object version for the subsequent conditional deletion.
        let tests = serde_json::from_value(json!([
            {"op":"test","path":"/metadata/ownerReferences/0/apiVersion","value":"qrab.yapplesauce.com/v1alpha1"},
            {"op":"test","path":"/metadata/ownerReferences/0/kind","value":"BaseImage"},
            {"op":"test","path":"/metadata/ownerReferences/0/name","value":image.name_any()},
            {"op":"test","path":"/metadata/ownerReferences/0/uid","value":uid},
            {"op":"test","path":"/metadata/ownerReferences/0/controller","value":true},
            {"op":"test","path":"/metadata/labels/app.kubernetes.io~1component","value":"image-stage-credential"},
            {"op":"test","path":"/metadata/labels/qrab.yapplesauce.com~1owner-uid","value":uid}
        ]))?;
        let checked = match self
            .secrets
            .patch_metadata(name, &PatchParams::default(), &Patch::Json::<()>(tests))
            .await
        {
            Ok(checked) => checked,
            Err(kube::Error::Api(error)) if error.code == 404 => return Ok(()),
            Err(kube::Error::Api(error)) if error.code == 422 => {
                bail!("stage credential does not belong to this BaseImage")
            }
            Err(error) => return Err(error.into()),
        };
        owned(&checked.metadata, image, name)?;
        let params = DeleteParams::default().preconditions(identity(&checked.metadata)?);
        match self.secrets.delete(name, &params).await {
            Ok(_) => Ok(()),
            Err(kube::Error::Api(error)) if error.code == 404 => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    async fn patch_status(&self, image: &BaseImage, status: &BaseImageStatus) -> Result<()> {
        let current = self.current(image).await?;
        self.images.patch_status(&image.name_any(), &PatchParams::default(), &Patch::Merge(json!({
            "metadata":{"uid":current.metadata.uid,"resourceVersion":current.metadata.resource_version},
            "status":status
        }))).await?;
        Ok(())
    }

    async fn set_finalizer(&self, image: &BaseImage, present: bool) -> Result<()> {
        let current = self.current(image).await?;
        let mut finalizers: Vec<_> = current
            .finalizers()
            .iter()
            .filter(|f| *f != FINALIZER)
            .cloned()
            .collect();
        if present {
            finalizers.push(FINALIZER.into());
        }
        if finalizers == current.finalizers() {
            return Ok(());
        }
        self.images.patch(&image.name_any(), &PatchParams::default(), &Patch::Merge(json!({
            "metadata":{"uid":current.metadata.uid,"resourceVersion":current.metadata.resource_version,"finalizers":finalizers}
        }))).await?;
        Ok(())
    }
}
