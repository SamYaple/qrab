use super::*;
use crate::{
    image::BaseImageSpec,
    storage::{ImageReport, ImageStorage, ImportDigest, StorageResult},
};
use std::sync::Mutex;

#[derive(Default)]
struct FakeCluster {
    jobs: Mutex<Vec<(String, JobState)>>,
    created: Mutex<Vec<serde_json::Value>>,
    secrets: Mutex<Vec<serde_json::Value>>,
    deleted_secrets: Mutex<Vec<String>>,
    status: Mutex<Option<BaseImageStatus>>,
    finalizers: Mutex<Option<Vec<String>>>,
}

#[async_trait::async_trait]
impl ImportCluster for FakeCluster {
    async fn job(&self, name: &str) -> Result<Option<JobState>> {
        Ok(self
            .jobs
            .lock()
            .unwrap()
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s.clone()))
    }
    async fn create_job(&self, job: &serde_json::Value) -> Result<()> {
        let name = job["metadata"]["name"].as_str().unwrap().to_owned();
        self.jobs.lock().unwrap().push((name, JobState::Running));
        self.created.lock().unwrap().push(job.clone());
        Ok(())
    }
    async fn delete_job(&self, name: &str) -> Result<()> {
        self.jobs.lock().unwrap().retain(|(n, _)| n != name);
        Ok(())
    }
    async fn apply_secret(&self, secret: &serde_json::Value) -> Result<()> {
        self.secrets.lock().unwrap().push(secret.clone());
        Ok(())
    }
    async fn delete_secret(&self, name: &str) -> Result<()> {
        self.deleted_secrets.lock().unwrap().push(name.into());
        Ok(())
    }
    async fn patch_status(&self, _: &str, status: &BaseImageStatus) -> Result<()> {
        *self.status.lock().unwrap() = Some(status.clone());
        Ok(())
    }
    async fn set_finalizers(&self, _: &str, finalizers: &[String]) -> Result<()> {
        *self.finalizers.lock().unwrap() = Some(finalizers.to_vec());
        Ok(())
    }
}

#[derive(Default)]
struct FakeStorage {
    image: Mutex<Option<ImageReport>>,
    stage: Mutex<Option<StageReport>>,
    calls: Mutex<Vec<String>>,
    secrets: Mutex<Vec<ChapSecret>>,
    seal_refusal: Mutex<Option<String>>,
}

fn stage(phase: StagePhase, generation: u64) -> StageReport {
    StageReport {
        image: "cirros".into(),
        image_generation: 2,
        capacity_bytes: Some(117440512),
        target_iqn: "iqn.2026-11.com.yapplesauce:qrab.stage.cirros.g2".into(),
        initiator_iqn: "iqn.2026-11.com.yapplesauce:qrab.importer.cirros.g2".into(),
        portal: "10.203.0.1:3260".into(),
        lun: 0,
        chap_username: "qrab-stage-cirros-g2".into(),
        generation,
        phase,
    }
}

fn sealed(sha: &str, staged: bool) -> ImageReport {
    ImageReport {
        image: "cirros".into(),
        generation: 2,
        capacity_bytes: 117440512,
        phase: "sealed".into(),
        import: Some(ImportDigest {
            sha256: sha.into(),
            size_bytes: 117440512,
        }),
        staged,
    }
}

impl FakeStorage {
    fn call(&self, call: &str) {
        self.calls.lock().unwrap().push(call.into());
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl ImageStorage for FakeStorage {
    async fn image_status(&self, _: &str, _: u32) -> StorageResult<Option<ImageReport>> {
        Ok(self.image.lock().unwrap().clone())
    }
    async fn stage_status(&self, _: &str, _: u32) -> StorageResult<StageReport> {
        Ok(self
            .stage
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| stage(StagePhase::Absent, 0)))
    }
    async fn stage_create(&self, _: &str, _: u32, capacity: u64) -> StorageResult<StageReport> {
        self.call(&format!("create {capacity}"));
        let report = stage(StagePhase::Provisioned, 0);
        *self.stage.lock().unwrap() = Some(report.clone());
        Ok(report)
    }
    async fn stage_export(
        &self,
        _: &str,
        _: u32,
        generation: u64,
        secret: &ChapSecret,
    ) -> StorageResult<StageReport> {
        self.call(&format!("export {generation}"));
        self.secrets.lock().unwrap().push(secret.clone());
        let report = stage(StagePhase::Active, generation);
        *self.stage.lock().unwrap() = Some(report.clone());
        Ok(report)
    }
    async fn stage_seal(
        &self,
        _: &str,
        _: u32,
        size: u64,
        sha: &str,
    ) -> StorageResult<ImageReport> {
        self.call(&format!("seal {size} {sha}"));
        if let Some(code) = self.seal_refusal.lock().unwrap().clone() {
            *self.stage.lock().unwrap() = Some(stage(StagePhase::Fenced, 1));
            return Err(StorageError::Refused(code));
        }
        *self.stage.lock().unwrap() = None;
        let report = sealed(sha, true);
        *self.image.lock().unwrap() = Some(report.clone());
        Ok(report)
    }
    async fn stage_abort(&self, _: &str, _: u32) -> StorageResult<StageReport> {
        self.call("abort");
        *self.stage.lock().unwrap() = None;
        Ok(stage(StagePhase::Absent, 0))
    }
    async fn retire(&self, _: &str, _: u32) -> StorageResult<ImageReport> {
        self.call("retire");
        let mut report = self.image.lock().unwrap().clone().unwrap();
        report.phase = "retired".into();
        Ok(report)
    }
}

const SHA: &str = "809dc3e0e993cd8ec736e28d55b8356ccc4ee9051cad9ad1be66fad9a79470f1";

fn bimg(name: &str) -> BaseImage {
    let spec: BaseImageSpec = serde_json::from_value(json!({"image": "cirros", "generation": 2, "title": "CirrOS 0.6.3",
        "os": "cirros", "version": "0.6.3", "source": {"url": "https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img",
        "sha256": "7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b", "format": "qcow2"}}))
    .unwrap();
    let mut image = BaseImage::new(name, spec);
    image.metadata.namespace = Some("qrab".into());
    image.metadata.uid = Some("uid-bimg".into());
    image.metadata.generation = Some(1);
    image.metadata.finalizers = Some(vec![FINALIZER.into()]);
    image
}

struct H {
    cluster: Arc<FakeCluster>,
    storage: Arc<FakeStorage>,
    importer: Importer,
    image: BaseImage,
}

impl H {
    fn new() -> Self {
        let cluster = Arc::new(FakeCluster::default());
        let storage = Arc::new(FakeStorage::default());
        let importer = Importer {
            cluster: cluster.clone(),
            storage: storage.clone(),
            namespace: "qrab".into(),
            job_image: "zot.yapplesauce.com/qrab/operator@sha256:aa".into(),
            service_account: "qrab-image-importer".into(),
        };
        Self {
            cluster,
            storage,
            importer,
            image: bimg("cirros-g2"),
        }
    }
    /// Reconcile once, carrying the written status forward like the API.
    async fn step(&mut self) -> Duration {
        let next = self.importer.reconcile(&self.image).await.unwrap();
        if let Some(status) = self.cluster.status.lock().unwrap().clone() {
            self.image.status = Some(status);
        }
        next
    }
    fn finish(&self, job: &str, state: JobState) {
        let mut jobs = self.cluster.jobs.lock().unwrap();
        let entry = jobs.iter_mut().find(|(n, _)| n == job).expect(job);
        entry.1 = state;
    }
    fn phase(&self) -> ImagePhase {
        self.image.phase()
    }
}

#[tokio::test]
async fn an_import_inspects_stages_writes_and_seals() {
    let mut h = H::new();
    h.step().await;
    assert_eq!(h.phase(), ImagePhase::Inspecting);
    let inspect = h.cluster.created.lock().unwrap()[0].clone();
    let pod = &inspect["spec"]["template"]["spec"];
    assert_eq!(pod["automountServiceAccountToken"], false);
    assert_eq!(
        pod["containers"][0]["securityContext"]["readOnlyRootFilesystem"],
        true
    );
    assert_eq!(
        pod["containers"][0]["command"],
        json!([
            "/usr/local/bin/qrab-operator",
            "image-job",
            "inspect",
            "--url",
            "https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img",
            "--sha256",
            "7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b",
            "--format",
            "qcow2"
        ])
    );
    assert!(
        pod["volumes"].to_string().find("secret").is_none(),
        "inspect gets no credential"
    );
    // Still running: nothing else happens.
    h.step().await;
    assert!(h.storage.calls().is_empty());
    h.finish(
        "cirros-g2-inspect",
        JobState::Succeeded(r#"{"virtualSize":117440512}"#.into()),
    );
    h.step().await;
    assert_eq!(h.phase(), ImagePhase::Writing);
    assert_eq!(h.image.status.as_ref().unwrap().size_bytes, Some(117440512));
    assert_eq!(h.storage.calls(), ["create 117440512", "export 1"]);
    // The credential reaches the Secret and the export, nothing else.
    let secret = h.cluster.secrets.lock().unwrap()[0].clone();
    let password = h.storage.secrets.lock().unwrap()[0].0.clone();
    let typed: Secret = serde_json::from_value(secret.clone()).unwrap();
    assert_eq!(typed.data.unwrap()["password"].0, password.as_bytes());
    assert_eq!(
        secret["metadata"]["labels"]["app.kubernetes.io/component"],
        "image-stage-credential"
    );
    assert_eq!(
        secret["metadata"]["ownerReferences"][0]["kind"],
        "BaseImage"
    );
    let write = h.cluster.created.lock().unwrap()[1].clone();
    let text = write.to_string();
    assert!(!text.contains(&password));
    assert!(
        text.contains("--chap-user")
            && text.contains("qrab-stage-cirros-g2")
            && text.contains("cirros-g2-stage")
    );
    assert!(!h
        .cluster
        .jobs
        .lock()
        .unwrap()
        .iter()
        .any(|(n, _)| n == "cirros-g2-inspect"));
    h.finish(
        "cirros-g2-write",
        JobState::Succeeded(format!(r#"{{"sizeBytes":117440512,"sha256":"{SHA}"}}"#)),
    );
    h.step().await;
    assert_eq!(h.phase(), ImagePhase::Sealed);
    let status = h.image.status.clone().unwrap();
    assert_eq!(
        (status.sha256.as_deref(), status.adopted),
        (Some(SHA), false)
    );
    assert_eq!(
        h.storage.calls().last().unwrap(),
        &format!("seal 117440512 {SHA}")
    );
    assert!(h.cluster.jobs.lock().unwrap().is_empty());
    assert!(h
        .cluster
        .deleted_secrets
        .lock()
        .unwrap()
        .contains(&"cirros-g2-stage".to_owned()));
    // Sealed and idle: no further storage calls.
    let calls = h.storage.calls().len();
    assert_eq!(h.step().await, IDLE);
    assert_eq!(h.storage.calls().len(), calls);
    // Retire.
    h.image.spec.retired = true;
    h.step().await;
    assert_eq!(h.phase(), ImagePhase::Retired);
    assert_eq!(h.storage.calls().last().unwrap(), "retire");
}

#[tokio::test]
async fn an_existing_generation_is_adopted_without_jobs() {
    let mut h = H::new();
    *h.storage.image.lock().unwrap() = Some(sealed(SHA, false));
    h.step().await;
    let status = h.image.status.clone().unwrap();
    assert_eq!((status.phase, status.adopted), (ImagePhase::Sealed, true));
    assert_eq!(status.sha256.as_deref(), Some(SHA));
    assert!(h.cluster.created.lock().unwrap().is_empty());
    assert!(h.storage.calls().is_empty());
}

#[tokio::test]
async fn a_misnamed_image_fails_without_touching_storage() {
    let mut h = H::new();
    h.image = bimg("cirros-latest");
    h.step().await;
    assert_eq!(h.phase(), ImagePhase::Failed);
    assert!(h
        .image
        .status
        .as_ref()
        .unwrap()
        .message
        .contains("must be named cirros-g2"));
    assert!(h.storage.calls().is_empty() && h.cluster.created.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_wrong_upstream_digest_fails_at_once_and_write_failures_retry() {
    let mut h = H::new();
    h.step().await;
    h.finish(
        "cirros-g2-inspect",
        JobState::Failed(r#"{"error":"sha256-mismatch","message":"sha256-mismatch: the download's SHA-256 is x"}"#.into()),
    );
    h.step().await;
    assert_eq!(h.phase(), ImagePhase::Failed);
    assert!(h
        .image
        .status
        .as_ref()
        .unwrap()
        .message
        .starts_with("Inspect: sha256-mismatch"));
    // A write failure aborts the stage and retries with a fresh stage.
    let mut h = H::new();
    h.step().await;
    h.finish(
        "cirros-g2-inspect",
        JobState::Succeeded(r#"{"virtualSize":117440512}"#.into()),
    );
    h.step().await;
    h.finish(
        "cirros-g2-write",
        JobState::Failed(r#"{"error":"import-failed","message":"qemu-img convert failed"}"#.into()),
    );
    h.step().await;
    let status = h.image.status.clone().unwrap();
    assert_eq!((status.phase, status.attempts), (ImagePhase::Pending, 1));
    assert_eq!(h.storage.calls(), ["create 117440512", "export 1", "abort"]);
    h.step().await;
    assert_eq!(h.phase(), ImagePhase::Writing);
    assert_eq!(&h.storage.calls()[3..], ["create 117440512", "export 1"]);
    // A seal that measures another digest counts as a failed attempt too.
    *h.storage.seal_refusal.lock().unwrap() = Some("sha256-mismatch".into());
    h.finish(
        "cirros-g2-write",
        JobState::Succeeded(format!(r#"{{"sizeBytes":117440512,"sha256":"{SHA}"}}"#)),
    );
    h.step().await;
    let status = h.image.status.clone().unwrap();
    assert_eq!((status.phase, status.attempts), (ImagePhase::Pending, 2));
    assert!(status.message.contains("differs"));
    assert_eq!(h.storage.calls().last().unwrap(), "abort");
    // The third failure is final.
    h.step().await;
    h.finish("cirros-g2-write", JobState::Failed("{}".into()));
    h.step().await;
    assert_eq!(h.phase(), ImagePhase::Failed);
}

#[tokio::test]
async fn deleting_an_unsealed_image_aborts_its_stage_and_releases_the_finalizer() {
    let mut h = H::new();
    h.step().await;
    h.finish(
        "cirros-g2-inspect",
        JobState::Succeeded(r#"{"virtualSize":117440512}"#.into()),
    );
    h.step().await;
    h.image.metadata.deletion_timestamp = Some(
        k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(Default::default()),
    );
    h.step().await;
    assert_eq!(h.storage.calls().last().unwrap(), "abort");
    assert!(h.cluster.jobs.lock().unwrap().is_empty());
    assert_eq!(h.cluster.finalizers.lock().unwrap().clone(), Some(vec![]));
    // A sealed image is never aborted on deletion.
    let mut h = H::new();
    *h.storage.image.lock().unwrap() = Some(sealed(SHA, false));
    h.step().await;
    h.image.metadata.deletion_timestamp = Some(
        k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(Default::default()),
    );
    h.step().await;
    assert!(h.storage.calls().is_empty());
    assert_eq!(h.cluster.finalizers.lock().unwrap().clone(), Some(vec![]));
}
