//! Exercise actual status publication against conditional private API semantics.
use super::*;
use axum::{
    extract::State, http::StatusCode, response::IntoResponse, routing::patch, Json, Router,
};
use serde_json::Value;
use std::{future::IntoFuture, sync::Arc, sync::Mutex};

fn profile(uid: &str, version: &str) -> VpnProfile {
    serde_json::from_value(json!({
        "apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VpnProfile",
        "metadata":{"name":"reused","namespace":"qrab","uid":uid,
            "resourceVersion":version,"generation":1},
        "spec":{"proton":{}},"status":{"server":"current"}
    }))
    .unwrap()
}

struct Stored {
    object: Value,
    patches: Vec<Value>,
}

type Store = Arc<Mutex<Stored>>;

async fn publish(State(store): State<Store>, Json(patch): Json<Value>) -> axum::response::Response {
    let mut store = store.lock().unwrap();
    store.patches.push(patch.clone());
    // Unconditional patches deliberately succeed, so old code must demonstrate
    // the actual cross-incarnation overwrite rather than just a client error.
    if ["uid", "resourceVersion"].iter().any(|key| {
        patch["metadata"]
            .get(*key)
            .is_some_and(|value| value != &store.object["metadata"][key])
    }) {
        return (
            StatusCode::CONFLICT,
            Json(json!({"apiVersion":"v1","kind":"Status","status":"Failure",
                "code":409,"reason":"Conflict","message":"profile changed"})),
        )
            .into_response();
    }
    store.object["status"] = patch["status"].clone();
    Json(store.object.clone()).into_response()
}

struct Server {
    client: kube::Client,
    store: Store,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn server(current: &VpnProfile) -> Server {
    let store = Arc::new(Mutex::new(Stored {
        object: serde_json::to_value(current).unwrap(),
        patches: Vec::new(),
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = kube::Client::try_from(kube::Config::new(
        format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap(),
    ))
    .unwrap();
    let app = Router::new()
        .route(
            "/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/vpnprofiles/reused/status",
            patch(publish),
        )
        .with_state(store.clone());
    Server {
        client,
        store,
        task: tokio::spawn(async move { axum::serve(listener, app).into_future().await.unwrap() }),
    }
}

fn outcome() -> VpnProfileStatus {
    VpnProfileStatus {
        observed_generation: Some(1),
        server: Some("late-provider-result".into()),
        conditions: vec![ready(true, "Configured", "old snapshot")],
        ..Default::default()
    }
}

#[tokio::test]
async fn status_snapshot_cannot_overwrite_a_replacement_or_newer_profile() {
    let observed = profile("old-uid", "11");
    for current in [profile("new-uid", "20"), profile("old-uid", "20")] {
        let server = server(&current).await;
        let broker = Broker::new(
            server.client.clone(),
            "qrab",
            Client::with_base("http://127.0.0.1:9").unwrap(),
        );
        broker.set_status(&observed, &outcome()).await;
        let stored = server.store.lock().unwrap();
        assert_eq!(
            stored.object["status"],
            serde_json::to_value(current.status).unwrap(),
            "a delayed result overwrote current profile status"
        );
    }
}

#[tokio::test]
async fn status_snapshot_matching_identity_publishes_conditionally() {
    let observed = profile("old-uid", "11");
    let server = server(&observed).await;
    let broker = Broker::new(
        server.client.clone(),
        "qrab",
        Client::with_base("http://127.0.0.1:9").unwrap(),
    );
    broker.set_status(&observed, &outcome()).await;
    let stored = server.store.lock().unwrap();
    assert_eq!(
        stored.object["status"],
        serde_json::to_value(outcome()).unwrap()
    );
    assert_eq!(
        stored.patches[0]["metadata"],
        json!({"uid":"old-uid","resourceVersion":"11"})
    );
}

#[tokio::test]
async fn status_snapshot_without_complete_identity_never_writes() {
    for missing in 0..4 {
        let current = profile("old-uid", "11");
        let mut observed = current.clone();
        match missing {
            0 => observed.metadata.uid = None,
            1 => observed.metadata.resource_version = None,
            2 => observed.metadata.uid = Some(String::new()),
            _ => observed.metadata.resource_version = Some(String::new()),
        }
        let server = server(&current).await;
        let broker = Broker::new(
            server.client.clone(),
            "qrab",
            Client::with_base("http://127.0.0.1:9").unwrap(),
        );
        broker.set_status(&observed, &outcome()).await;
        assert!(server.store.lock().unwrap().patches.is_empty());
    }
}
