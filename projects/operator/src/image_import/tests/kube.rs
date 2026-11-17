//! Real adapter requests against a private, version-checking API fixture.
use super::*;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::Value;
use std::future::IntoFuture;

const IMAGE_PATH: &str = "/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/baseimages/cirros-g2";
const JOB_PATH: &str = "/apis/batch/v1/namespaces/qrab/jobs/cirros-g2-inspect";
const SECRET_PATH: &str = "/api/v1/namespaces/qrab/secrets/cirros-g2-stage";

struct ApiObjects {
    image: Value,
    job: Option<Value>,
    pods: Vec<Value>,
    secret: Option<Value>,
    job_deletes: Vec<Value>,
    secret_deletes: Vec<Value>,
    secret_tests: usize,
    image_patches: Vec<Value>,
    job_delete_replacement: Option<Value>,
    secret_delete_replacement: Option<Value>,
    image_patch_conflict: bool,
}

#[derive(Clone)]
struct ApiState(Arc<Mutex<ApiObjects>>);

fn failure(code: StatusCode) -> Response {
    let reason = match code {
        StatusCode::NOT_FOUND => "NotFound",
        StatusCode::CONFLICT => "Conflict",
        StatusCode::UNPROCESSABLE_ENTITY => "Invalid",
        _ => "InternalError",
    };
    (
        code,
        Json(json!({"apiVersion":"v1","kind":"Status","status":"Failure",
        "code":code.as_u16(),"reason":reason,"message":"fixture refused request"})),
    )
        .into_response()
}

async fn current_image(State(state): State<ApiState>) -> Json<Value> {
    Json(state.0.lock().unwrap().image.clone())
}

async fn patch_image(State(state): State<ApiState>, Json(patch): Json<Value>) -> Response {
    let mut state = state.0.lock().unwrap();
    if state.image_patch_conflict {
        state.image["metadata"]["resourceVersion"] = json!("concurrent-22");
    }
    if patch["metadata"]["uid"] != state.image["metadata"]["uid"]
        || patch["metadata"]["resourceVersion"] != state.image["metadata"]["resourceVersion"]
    {
        return failure(StatusCode::CONFLICT);
    }
    state.image_patches.push(patch.clone());
    json_patch::merge(&mut state.image, &patch);
    state.image["metadata"]["resourceVersion"] =
        json!(format!("write-{}", state.image_patches.len()));
    Json(state.image.clone()).into_response()
}

async fn current_job(State(state): State<ApiState>) -> Response {
    match &state.0.lock().unwrap().job {
        Some(job) => Json(job.clone()).into_response(),
        None => failure(StatusCode::NOT_FOUND),
    }
}

async fn create_job() -> Response {
    failure(StatusCode::CONFLICT)
}

async fn delete_job(State(state): State<ApiState>, Json(params): Json<Value>) -> Response {
    let mut state = state.0.lock().unwrap();
    state.job_deletes.push(params.clone());
    if let Some(replacement) = state.job_delete_replacement.take() {
        state.job = Some(replacement);
    }
    let Some(job) = &state.job else {
        return failure(StatusCode::NOT_FOUND);
    };
    if params["preconditions"]["uid"] != job["metadata"]["uid"]
        || params["preconditions"]["resourceVersion"] != job["metadata"]["resourceVersion"]
    {
        return failure(StatusCode::CONFLICT);
    }
    Json(state.job.take().unwrap()).into_response()
}

async fn pods(State(state): State<ApiState>) -> Json<Value> {
    Json(
        json!({"apiVersion":"v1","kind":"PodList","metadata":{},"items":state.0.lock().unwrap().pods}),
    )
}

async fn check_secret(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(tests): Json<Value>,
) -> Response {
    let mut state = state.0.lock().unwrap();
    state.secret_tests += 1;
    let accept = headers.get("accept").unwrap().to_str().unwrap();
    assert!(
        accept.contains("as=PartialObjectMetadata"),
        "must request metadata only"
    );
    assert!(
        !accept.contains(','),
        "must not negotiate a Secret payload fallback"
    );
    assert_eq!(headers["content-type"], "application/json-patch+json");
    let tests: json_patch::Patch = serde_json::from_value(tests).unwrap();
    assert!(tests
        .0
        .iter()
        .all(|op| matches!(op, json_patch::PatchOperation::Test(_))));
    let Some(secret) = &state.secret else {
        return failure(StatusCode::NOT_FOUND);
    };
    let mut checked = secret.clone();
    if json_patch::patch(&mut checked, &tests).is_err() {
        return failure(StatusCode::UNPROCESSABLE_ENTITY);
    }
    Json(json!({"apiVersion":"meta.k8s.io/v1","kind":"PartialObjectMetadata","metadata":checked["metadata"]})).into_response()
}

async fn delete_secret(State(state): State<ApiState>, Json(params): Json<Value>) -> Response {
    let mut state = state.0.lock().unwrap();
    state.secret_deletes.push(params.clone());
    if let Some(replacement) = state.secret_delete_replacement.take() {
        state.secret = Some(replacement);
    }
    let Some(secret) = &state.secret else {
        return failure(StatusCode::NOT_FOUND);
    };
    if params["preconditions"]["uid"] != secret["metadata"]["uid"]
        || params["preconditions"]["resourceVersion"] != secret["metadata"]["resourceVersion"]
    {
        return failure(StatusCode::CONFLICT);
    }
    state.secret = None;
    Json(json!({"apiVersion":"v1","kind":"Status","status":"Success"})).into_response()
}

struct Fixture {
    image: BaseImage,
    state: ApiState,
    cluster: Arc<KubeImportCluster>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn owned_job(image: &BaseImage) -> Value {
    json!({"apiVersion":"batch/v1","kind":"Job",
        "metadata":{"name":"cirros-g2-inspect","namespace":"qrab","uid":"job-1","resourceVersion":"3",
            "ownerReferences":owner(image)},
        "status":{"conditions":[{"type":"Complete","status":"True"}]}})
}

fn report_pod(job_uid: &str, message: &str) -> Value {
    json!({"apiVersion":"v1","kind":"Pod",
    "metadata":{"name":format!("pod-{job_uid}"),"namespace":"qrab","uid":format!("pod-{job_uid}"),
        "ownerReferences":[{"apiVersion":"batch/v1","kind":"Job","name":"cirros-g2-inspect","uid":job_uid,"controller":true}]},
    "status":{"phase":"Succeeded","containerStatuses":[{
        "name":CONTAINER,"image":"fixture","imageID":"fixture","ready":false,"restartCount":0,
        "state":{"terminated":{"exitCode":0,"message":message}}
    }]}})
}

impl Fixture {
    async fn new() -> Self {
        let mut image = bimg("cirros-g2");
        image.metadata.resource_version = Some("11".into());
        let mut secret = render_secret(&image, "qrab", &ChapSecret("not-returned".into()));
        secret["metadata"]["uid"] = json!("secret-1");
        secret["metadata"]["resourceVersion"] = json!("7");
        let state = ApiState(Arc::new(Mutex::new(ApiObjects {
            image: serde_json::to_value(&image).unwrap(),
            job: Some(owned_job(&image)),
            pods: vec![report_pod("job-1", "{\"virtualSize\":16777216}")],
            secret: Some(secret),
            job_deletes: vec![],
            secret_deletes: vec![],
            secret_tests: 0,
            image_patches: vec![],
            job_delete_replacement: None,
            secret_delete_replacement: None,
            image_patch_conflict: false,
        })));
        let app = Router::new()
            .route(IMAGE_PATH, get(current_image).patch(patch_image))
            .route(
                &format!("{IMAGE_PATH}/status"),
                axum::routing::patch(patch_image),
            )
            .route(JOB_PATH, get(current_job).delete(delete_job))
            .route(
                "/apis/batch/v1/namespaces/qrab/jobs",
                axum::routing::post(create_job),
            )
            .route("/api/v1/namespaces/qrab/pods", get(pods))
            .route(
                SECRET_PATH,
                axum::routing::patch(check_secret).delete(delete_secret),
            )
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = ::kube::Client::try_from(::kube::Config::new(
            format!("http://{}", listener.local_addr().unwrap())
                .parse()
                .unwrap(),
        ))
        .unwrap();
        let server =
            tokio::spawn(async move { axum::serve(listener, app).into_future().await.unwrap() });
        Self {
            image,
            state,
            cluster: Arc::new(KubeImportCluster::new(client, "qrab")),
            server,
        }
    }
}

#[tokio::test]
async fn kube_adapter_never_consumes_or_deletes_a_foreign_same_name_job() {
    let fixture = Fixture::new().await;
    fixture.state.0.lock().unwrap().job.as_mut().unwrap()["metadata"]["ownerReferences"] = json!([{"apiVersion":"batch/v1","kind":"CronJob","name":"other","uid":"other-owner","controller":true}]);
    let importer = Importer {
        cluster: fixture.cluster.clone(),
        storage: Arc::new(FakeStorage::default()),
        namespace: "qrab".into(),
        job_image: "fixture".into(),
        service_account: "fixture".into(),
    };
    let error = importer.reconcile(&fixture.image).await.unwrap_err();
    assert!(error.to_string().contains("not owned"), "{error}");
    assert!(fixture.state.0.lock().unwrap().job_deletes.is_empty());
}

#[tokio::test]
async fn job_report_is_bound_to_the_job_uid_and_successful_container() {
    let fixture = Fixture::new().await;
    let expected = "{\"virtualSize\":16777216}";
    let mut wrong_attempt = report_pod("job-1", "failed attempt");
    wrong_attempt["status"]["containerStatuses"][0]["state"]["terminated"]["exitCode"] = json!(1);
    fixture.state.0.lock().unwrap().pods = vec![
        report_pod("old-job", "foreign report"),
        wrong_attempt,
        report_pod("job-1", expected),
    ];
    let observed = fixture
        .cluster
        .job(&fixture.image, "cirros-g2-inspect")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed.state, JobState::Succeeded(expected.into()));
    assert_eq!(observed.uid, "job-1");
    assert_eq!(observed.resource_version, "3");
}

#[tokio::test]
async fn create_conflict_requires_the_current_baseimage_owner() {
    let fixture = Fixture::new().await;
    let desired = owned_job(&fixture.image);
    fixture
        .cluster
        .create_job(&fixture.image, &desired)
        .await
        .unwrap();
    fixture.state.0.lock().unwrap().job.as_mut().unwrap()["metadata"]["ownerReferences"][0]
        ["uid"] = json!("previous-baseimage");
    assert!(fixture
        .cluster
        .create_job(&fixture.image, &desired)
        .await
        .is_err());
}

#[tokio::test]
async fn consuming_a_report_cannot_delete_a_replacement_owned_job() {
    let fixture = Fixture::new().await;
    let observed = fixture
        .cluster
        .job(&fixture.image, "cirros-g2-inspect")
        .await
        .unwrap()
        .unwrap();
    fixture.state.0.lock().unwrap().job.as_mut().unwrap()["metadata"]["uid"] = json!("job-2");
    assert!(fixture
        .cluster
        .delete_job(&fixture.image, "cirros-g2-inspect", Some(&observed))
        .await
        .is_err());
    assert!(fixture.state.0.lock().unwrap().job_deletes.is_empty());
}

#[tokio::test]
async fn job_cleanup_delete_checks_uid_and_version_at_the_api() {
    for field in ["uid", "resourceVersion"] {
        let fixture = Fixture::new().await;
        let mut replacement = owned_job(&fixture.image);
        replacement["metadata"][field] = json!("replacement");
        fixture.state.0.lock().unwrap().job_delete_replacement = Some(replacement);
        assert!(fixture
            .cluster
            .delete_job(&fixture.image, "cirros-g2-inspect", None)
            .await
            .is_err());
        let state = fixture.state.0.lock().unwrap();
        assert_eq!(
            state.job.as_ref().unwrap()["metadata"][field],
            "replacement"
        );
        assert_eq!(
            state.job_deletes[0]["preconditions"],
            json!({"uid":"job-1","resourceVersion":"3"})
        );
        assert_eq!(state.job_deletes[0]["propagationPolicy"], "Foreground");
    }
}

#[tokio::test]
async fn secret_cleanup_checks_owner_without_reading_credentials_and_is_idempotent() {
    let fixture = Fixture::new().await;
    fixture.state.0.lock().unwrap().secret.as_mut().unwrap()["metadata"]["ownerReferences"][0]
        ["uid"] = json!("previous-baseimage");
    assert!(fixture
        .cluster
        .delete_secret(&fixture.image, "cirros-g2-stage")
        .await
        .is_err());
    assert!(fixture.state.0.lock().unwrap().secret_deletes.is_empty());
    fixture.state.0.lock().unwrap().secret.as_mut().unwrap()["metadata"]["ownerReferences"][0]
        ["uid"] = json!("uid-bimg");
    fixture
        .cluster
        .delete_secret(&fixture.image, "cirros-g2-stage")
        .await
        .unwrap();
    fixture
        .cluster
        .delete_secret(&fixture.image, "cirros-g2-stage")
        .await
        .unwrap();
    let state = fixture.state.0.lock().unwrap();
    assert_eq!(state.secret_tests, 3);
    assert_eq!(
        state.secret_deletes[0]["preconditions"],
        json!({"uid":"secret-1","resourceVersion":"7"})
    );
    assert!(state.secret.is_none());
}

#[tokio::test]
async fn secret_rotation_and_recreation_between_test_and_delete_are_preserved() {
    for field in ["uid", "resourceVersion"] {
        let fixture = Fixture::new().await;
        let mut replacement = fixture.state.0.lock().unwrap().secret.clone().unwrap();
        replacement["metadata"][field] = json!("replacement");
        fixture.state.0.lock().unwrap().secret_delete_replacement = Some(replacement);
        assert!(fixture
            .cluster
            .delete_secret(&fixture.image, "cirros-g2-stage")
            .await
            .is_err());
        assert_eq!(
            fixture.state.0.lock().unwrap().secret.as_ref().unwrap()["metadata"][field],
            "replacement"
        );
    }
}

#[tokio::test]
async fn parent_recreation_spec_change_and_deletion_stop_stale_effects() {
    for change in ["uid", "generation", "spec", "deleting"] {
        let fixture = Fixture::new().await;
        {
            let mut state = fixture.state.0.lock().unwrap();
            match change {
                "uid" => state.image["metadata"]["uid"] = json!("replacement-baseimage"),
                "generation" => state.image["metadata"]["generation"] = json!(2),
                "spec" => state.image["spec"]["retired"] = json!(true),
                "deleting" => {
                    state.image["metadata"]["deletionTimestamp"] = json!("2026-11-17T00:00:00Z")
                }
                _ => unreachable!(),
            }
        }
        assert!(
            fixture
                .cluster
                .patch_status(&fixture.image, &BaseImageStatus::default())
                .await
                .is_err(),
            "{change}"
        );
        assert!(
            fixture
                .cluster
                .delete_job(&fixture.image, "cirros-g2-inspect", None)
                .await
                .is_err(),
            "{change}"
        );
        assert!(
            fixture
                .cluster
                .delete_secret(&fixture.image, "cirros-g2-stage")
                .await
                .is_err(),
            "{change}"
        );
        assert!(
            fixture
                .cluster
                .set_finalizer(&fixture.image, false)
                .await
                .is_err(),
            "{change}"
        );
        let state = fixture.state.0.lock().unwrap();
        assert!(
            state.image_patches.is_empty()
                && state.job_deletes.is_empty()
                && state.secret_deletes.is_empty()
        );
        assert_eq!(state.secret_tests, 0);
    }
}

#[tokio::test]
async fn status_progress_refreshes_versions_and_finalizer_changes_preserve_other_owners() {
    let fixture = Fixture::new().await;
    fixture.state.0.lock().unwrap().image["metadata"]["finalizers"] =
        json!([FINALIZER, "other.example/finalizer"]);
    fixture
        .cluster
        .set_finalizer(&fixture.image, false)
        .await
        .unwrap();
    fixture
        .cluster
        .patch_status(&fixture.image, &BaseImageStatus::default())
        .await
        .unwrap();
    fixture
        .cluster
        .patch_status(
            &fixture.image,
            &BaseImageStatus {
                phase: ImagePhase::Inspecting,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    fixture
        .cluster
        .set_finalizer(&fixture.image, true)
        .await
        .unwrap();
    let state = fixture.state.0.lock().unwrap();
    assert_eq!(
        state.image["metadata"]["finalizers"],
        json!(["other.example/finalizer", FINALIZER])
    );
    assert_eq!(state.image_patches.len(), 4);
    assert_eq!(state.image_patches[0]["metadata"]["resourceVersion"], "11");
    assert_eq!(
        state.image_patches[2]["metadata"]["resourceVersion"],
        "write-2"
    );
}

#[tokio::test]
async fn status_and_finalizer_changes_conflict_with_a_concurrent_api_update() {
    for finalizer in [false, true] {
        let fixture = Fixture::new().await;
        fixture.state.0.lock().unwrap().image_patch_conflict = true;
        let result = if finalizer {
            fixture.cluster.set_finalizer(&fixture.image, false).await
        } else {
            fixture
                .cluster
                .patch_status(&fixture.image, &BaseImageStatus::default())
                .await
        };
        assert!(result.is_err());
        assert!(fixture.state.0.lock().unwrap().image_patches.is_empty());
    }
}

#[tokio::test]
async fn missing_child_identity_and_ambiguous_secret_ownership_refuse_deletion() {
    for field in ["uid", "resourceVersion"] {
        let fixture = Fixture::new().await;
        fixture.state.0.lock().unwrap().job.as_mut().unwrap()["metadata"][field] = Value::Null;
        assert!(fixture
            .cluster
            .delete_job(&fixture.image, "cirros-g2-inspect", None)
            .await
            .is_err());
        assert!(fixture.state.0.lock().unwrap().job_deletes.is_empty());
        fixture.state.0.lock().unwrap().secret.as_mut().unwrap()["metadata"][field] = Value::Null;
        assert!(fixture
            .cluster
            .delete_secret(&fixture.image, "cirros-g2-stage")
            .await
            .is_err());
        assert!(fixture.state.0.lock().unwrap().secret_deletes.is_empty());
    }
    let fixture = Fixture::new().await;
    {
        let mut state = fixture.state.0.lock().unwrap();
        let owners = state.secret.as_mut().unwrap()["metadata"]["ownerReferences"]
            .as_array_mut()
            .unwrap();
        owners.push(json!({"apiVersion":"v1","kind":"ConfigMap","name":"foreign","uid":"foreign"}));
    }
    assert!(fixture
        .cluster
        .delete_secret(&fixture.image, "cirros-g2-stage")
        .await
        .is_err());
    assert!(fixture.state.0.lock().unwrap().secret_deletes.is_empty());
}

#[tokio::test]
async fn owned_job_deletion_and_absent_cleanup_succeed() {
    let fixture = Fixture::new().await;
    let observed = fixture
        .cluster
        .job(&fixture.image, "cirros-g2-inspect")
        .await
        .unwrap()
        .unwrap();
    fixture
        .cluster
        .delete_job(&fixture.image, "cirros-g2-inspect", Some(&observed))
        .await
        .unwrap();
    fixture
        .cluster
        .delete_job(&fixture.image, "cirros-g2-inspect", None)
        .await
        .unwrap();
    assert_eq!(fixture.state.0.lock().unwrap().job_deletes.len(), 1);
    assert!(fixture.state.0.lock().unwrap().job.is_none());
}

#[tokio::test]
async fn stale_parent_cannot_retire_storage_before_the_status_guard_runs() {
    let mut fixture = Fixture::new().await;
    fixture.image.spec.retired = true;
    fixture.image.status = Some(BaseImageStatus {
        phase: ImagePhase::Sealed,
        ..Default::default()
    });
    let storage = Arc::new(FakeStorage::default());
    *storage.image.lock().unwrap() = Some(sealed(SHA, true));
    let importer = Importer {
        cluster: fixture.cluster.clone(),
        storage: storage.clone(),
        namespace: "qrab".into(),
        job_image: "fixture".into(),
        service_account: "fixture".into(),
    };
    fixture.state.0.lock().unwrap().image["metadata"]["uid"] = json!("replacement-baseimage");
    assert!(importer.reconcile(&fixture.image).await.is_err());
    assert!(
        storage.calls().is_empty(),
        "stale reconcile already mutated storage: {:?}",
        storage.calls()
    );
}
