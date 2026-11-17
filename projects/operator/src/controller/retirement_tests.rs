//! Exercise actual retirement and deletion requests against a conditional API.
use super::*;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::patch,
    Json, Router,
};
use serde_json::Value;
use std::{future::IntoFuture, sync::Mutex};

#[derive(Clone)]
struct ApiState {
    pod: Arc<Mutex<Option<Value>>>,
    requests: Arc<Mutex<usize>>,
}

fn status(code: StatusCode) -> Response {
    (code,Json(json!({"apiVersion":"v1","kind":"Status","status":"Failure","code":code.as_u16(),
        "reason":if code==StatusCode::NOT_FOUND {"NotFound"} else {"Conflict"},"message":"fixture refused mutation"}))).into_response()
}

async fn annotate(State(state): State<ApiState>, Json(body): Json<Value>) -> Response {
    *state.requests.lock().unwrap() += 1;
    let mut pod = state.pod.lock().unwrap();
    let Some(pod) = pod.as_mut() else {
        return status(StatusCode::NOT_FOUND);
    };
    for field in ["uid", "resourceVersion"] {
        if !body["metadata"][field].is_null() && body["metadata"][field] != pod["metadata"][field] {
            return status(StatusCode::CONFLICT);
        }
    }
    json_patch::merge(pod, &body);
    pod["metadata"]["resourceVersion"] = json!("after-patch");
    Json(pod.clone()).into_response()
}

async fn remove(State(state): State<ApiState>, Json(body): Json<Value>) -> Response {
    *state.requests.lock().unwrap() += 1;
    let mut pod = state.pod.lock().unwrap();
    let Some(current) = pod.as_ref() else {
        return status(StatusCode::NOT_FOUND);
    };
    for field in ["uid", "resourceVersion"] {
        if !body["preconditions"][field].is_null()
            && body["preconditions"][field] != current["metadata"][field]
        {
            return status(StatusCode::CONFLICT);
        }
    }
    *pod = None;
    Json(json!({"apiVersion":"v1","kind":"Status","status":"Success"})).into_response()
}

struct Fixture {
    vm: VirtualMachine,
    observed: Pod,
    state: ApiState,
    ctx: Context,
    pods: Api<Pod>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl Fixture {
    async fn new() -> Self {
        let vm: VirtualMachine = serde_json::from_value(
            json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
            "metadata":{"name":"vm","namespace":"qrab","uid":"vm-uid","resourceVersion":"1"},
            "spec":{"stateVolumeClaim":"state","components":{"bmc":{"enabled":false}}}}),
        )
        .unwrap();
        let observed:Pod=serde_json::from_value(json!({"apiVersion":"v1","kind":"Pod","metadata":{
            "name":"vm-nic-old","namespace":"qrab","uid":"anchor-1","resourceVersion":"11",
            "ownerReferences":[{"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine","name":"vm","uid":"vm-uid","controller":true}]},
            "spec":{"nodeName":"node-a","containers":[{"name":"anchor","image":"fixture"}]}})).unwrap();
        let state = ApiState {
            pod: Arc::new(Mutex::new(Some(serde_json::to_value(&observed).unwrap()))),
            requests: Arc::new(Mutex::new(0)),
        };
        let app = Router::new()
            .route(
                "/api/v1/namespaces/qrab/pods/vm-nic-old",
                patch(annotate).delete(remove),
            )
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = Client::try_from(kube::Config::new(
            format!("http://{}", listener.local_addr().unwrap())
                .parse()
                .unwrap(),
        ))
        .unwrap();
        let server =
            tokio::spawn(async move { axum::serve(listener, app).into_future().await.unwrap() });
        let pods = Api::namespaced(client.clone(), "qrab");
        let ctx = Context {
            client,
            settings: Settings {
                namespace: "qrab".into(),
                vm_image: format!("fixture@sha256:{}", "a".repeat(64)),
                oidc_issuer: "https://issuer.example".into(),
                oidc_client_id: "qrab".into(),
                idp_ip: None,
                redfish_oidc_issuer: "https://issuer.example/redfish".into(),
                redfish_oidc_audience: "redfish".into(),
                bmc_bearer: None,
                nic_addresses: None,
                guest_devid_ca_cn: guest_identity::DEFAULT_DEVID_CA_CN.into(),
                tpm_issuer: Default::default(),
            },
            storage: None,
            metrics: None,
            evictor: None,
        };
        Self {
            vm,
            observed,
            state,
            ctx,
            pods,
            server,
        }
    }
    async fn retire(&self) -> Result<()> {
        retire_other_anchors(
            &self.ctx,
            &self.pods,
            &self.vm,
            std::iter::once(&self.observed),
            "vm-nic-current",
        )
        .await
    }
}

#[tokio::test]
async fn retirement_never_annotates_a_replacement_or_newer_snapshot() {
    for field in ["uid", "resourceVersion"] {
        let fixture = Fixture::new().await;
        {
            let mut pod = fixture.state.pod.lock().unwrap();
            let pod = pod.as_mut().unwrap();
            pod["metadata"][field] = json!("replacement");
            pod["metadata"]["ownerReferences"][0]["uid"] = json!("another-vm");
        }
        let result = fixture.retire().await;
        assert!(
            fixture.state.pod.lock().unwrap().as_ref().unwrap()["metadata"]["annotations"]
                [nic::RETIRE_ANNOTATION]
                .is_null(),
            "retirement annotated a foreign/newer Pod"
        );
        assert!(result.is_err());
    }
}

#[tokio::test]
async fn pod_deletion_never_crosses_a_newer_ownership_snapshot() {
    let fixture = Fixture::new().await;
    {
        let mut pod = fixture.state.pod.lock().unwrap();
        let pod = pod.as_mut().unwrap();
        pod["metadata"]["resourceVersion"] = json!("12");
        pod["metadata"]["ownerReferences"][0]["uid"] = json!("another-vm");
    }
    let result = delete(&fixture.pods, &fixture.observed).await;
    assert!(
        fixture.state.pod.lock().unwrap().is_some(),
        "delete removed a newer foreign ownership snapshot"
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn expired_observation_cannot_delete_a_renewed_retirement() {
    let mut fixture = Fixture::new().await;
    fixture.observed.metadata.annotations = Some(std::collections::BTreeMap::from([(
        nic::RETIRE_ANNOTATION.into(),
        "0".into(),
    )]));
    {
        let mut pod = fixture.state.pod.lock().unwrap();
        let pod = pod.as_mut().unwrap();
        pod["metadata"]["resourceVersion"] = json!("12");
        pod["metadata"]["annotations"][nic::RETIRE_ANNOTATION] = json!("4102444800");
    }
    let result = fixture.retire().await;
    assert!(
        fixture.state.pod.lock().unwrap().is_some(),
        "old expiry deleted a renewed Pod"
    );
    assert!(result.is_err());
}

#[tokio::test]
async fn matching_objects_are_annotated_and_expired_objects_are_deleted() {
    let fixture = Fixture::new().await;
    fixture.retire().await.unwrap();
    let pod = fixture.state.pod.lock().unwrap().clone().unwrap();
    assert!(
        pod["metadata"]["annotations"][nic::RETIRE_ANNOTATION]
            .as_str()
            .unwrap()
            .parse::<i64>()
            .unwrap()
            > 0
    );
    let mut expired = Fixture::new().await;
    expired.observed.metadata.annotations = Some(std::collections::BTreeMap::from([(
        nic::RETIRE_ANNOTATION.into(),
        "0".into(),
    )]));
    *expired.state.pod.lock().unwrap() = Some(serde_json::to_value(&expired.observed).unwrap());
    expired.retire().await.unwrap();
    assert!(expired.state.pod.lock().unwrap().is_none());
}

#[tokio::test]
async fn missing_or_empty_identity_refuses_before_sending_any_mutation() {
    for field in ["uid", "resourceVersion"] {
        for value in [Value::Null, json!("")] {
            let mut fixture = Fixture::new().await;
            let mut pod = serde_json::to_value(&fixture.observed).unwrap();
            pod["metadata"][field] = value;
            fixture.observed = serde_json::from_value(pod).unwrap();
            assert!(fixture.retire().await.is_err());
            assert!(delete(&fixture.pods, &fixture.observed).await.is_err());
            assert_eq!(*fixture.state.requests.lock().unwrap(), 0);
        }
    }
}
