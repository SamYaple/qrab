//! Private API fixtures for the real write-only Kubernetes credential store.
use super::*;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::patch,
    Json, Router,
};
use serde_json::Value;
use std::{future::IntoFuture, sync::Arc, sync::Mutex};

const VM_UID: &str = "00000000-0000-4000-8000-000000000001";
const OTHER_VM_UID: &str = "00000000-0000-4000-8000-000000000002";

fn vm(name: &str, uid: &str) -> VirtualMachine {
    serde_json::from_value(json!({
        "apiVersion":"qrab.yapplesauce.com/v1alpha1", "kind":"VirtualMachine",
        "metadata":{"namespace":"qrab", "name":name, "uid":uid},
        "spec":{"running":false,"stateVolumeClaim":"state"}
    }))
    .unwrap()
}

fn credential(vm: &VirtualMachine, disk: &str) -> Value {
    let secret = SecretStore {
        api: Api::namespaced(test_client("127.0.0.1:9"), "qrab"),
        vm,
    }
    .render(
        disk,
        &Credential {
            generation: 1,
            secret: ChapSecret("a".repeat(32)),
        },
    )
    .unwrap();
    let mut secret = serde_json::to_value(secret).unwrap();
    secret["metadata"]["uid"] = "secret-original".into();
    secret["metadata"]["resourceVersion"] = "11".into();
    secret
}

fn test_client(address: &str) -> kube::Client {
    kube::Client::try_from(kube::Config::new(
        format!("http://{address}").parse().unwrap(),
    ))
    .unwrap()
}

#[derive(Default)]
struct Fixture {
    secret: Option<Value>,
    after_check: Option<Value>,
    checks: usize,
    deletes: Vec<Value>,
    metadata_only: bool,
}

type Store = Arc<Mutex<Fixture>>;

fn status(code: StatusCode) -> Response {
    (
        code,
        Json(json!({
            "apiVersion":"v1","kind":"Status","status":"Failure",
            "reason":code.canonical_reason().unwrap(),"message":"fixture refusal",
            "code":code.as_u16()
        })),
    )
        .into_response()
}

async fn check(
    State(store): State<Store>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let mut store = store.lock().unwrap();
    store.checks += 1;
    store.metadata_only = headers
        .get("accept")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.contains("as=PartialObjectMetadata"));
    assert_eq!(
        headers.get("content-type").unwrap(),
        "application/json-patch+json"
    );
    let Some(secret) = &store.secret else {
        return status(StatusCode::NOT_FOUND);
    };
    let operations = body.as_array().expect("JSON patch operations");
    assert!(
        !operations.is_empty(),
        "empty patch establishes no ownership"
    );
    for operation in operations {
        assert_eq!(operation["op"], "test", "ownership check must not mutate");
        let pointer = operation["path"].as_str().unwrap();
        assert!(
            pointer.starts_with("/metadata/"),
            "no credential data access"
        );
        if secret.pointer(pointer) != operation.get("value") {
            return status(StatusCode::UNPROCESSABLE_ENTITY);
        }
    }
    let reply = json!({
        "apiVersion":"meta.k8s.io/v1","kind":"PartialObjectMetadata",
        "metadata":secret["metadata"]
    });
    if let Some(replacement) = store.after_check.take() {
        store.secret = Some(replacement);
    }
    Json(reply).into_response()
}

async fn delete(State(store): State<Store>, Json(body): Json<Value>) -> Response {
    let mut store = store.lock().unwrap();
    store.deletes.push(body.clone());
    let Some(secret) = &store.secret else {
        return status(StatusCode::NOT_FOUND);
    };
    // This endpoint deliberately accepts an unconditioned DELETE. The old
    // implementation must demonstrate the actual cross-owner deletion.
    for (condition, field) in [("uid", "uid"), ("resourceVersion", "resourceVersion")] {
        if let Some(expected) = body["preconditions"].get(condition) {
            if expected != &secret["metadata"][field] {
                return status(StatusCode::CONFLICT);
            }
        }
    }
    store.secret = None;
    Json(json!({"apiVersion":"v1","kind":"Status","status":"Success"})).into_response()
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

async fn server(secret: Option<Value>, after_check: Option<Value>) -> Server {
    let store = Arc::new(Mutex::new(Fixture {
        secret,
        after_check,
        ..Default::default()
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = test_client(&listener.local_addr().unwrap().to_string());
    let app = Router::new()
        .route(
            "/api/v1/namespaces/qrab/secrets/{name}",
            patch(check).delete(delete),
        )
        .with_state(store.clone());
    Server {
        client,
        store,
        task: tokio::spawn(async move { axum::serve(listener, app).into_future().await.unwrap() }),
    }
}

#[tokio::test]
async fn secret_cleanup_refuses_a_valid_other_vm_with_the_same_generated_name() {
    let first = vm("a", VM_UID);
    let second = vm("a-disk-b", OTHER_VM_UID);
    assert_eq!(secret_name("a", "b-disk-c"), secret_name("a-disk-b", "c"));
    let foreign = credential(&second, "c");
    let server = server(Some(foreign.clone()), None).await;
    let secrets = SecretStore {
        api: Api::namespaced(server.client.clone(), "qrab"),
        vm: &first,
    };
    let result = secrets.delete("b-disk-c").await;
    let store = server.store.lock().unwrap();
    assert_eq!(
        store.secret.as_ref(),
        Some(&foreign),
        "foreign credential deleted"
    );
    assert!(result.is_err());
    assert!(store.deletes.is_empty());
}

#[tokio::test]
async fn secret_cleanup_retry_never_deletes_a_colliding_new_vm() {
    let first = vm("a", VM_UID);
    let second = vm("a-disk-b", OTHER_VM_UID);
    let server = server(Some(credential(&first, "b-disk-c")), None).await;
    let secrets = SecretStore {
        api: Api::namespaced(server.client.clone(), "qrab"),
        vm: &first,
    };
    let backend = fake::FakeStorage::default();
    release(&backend, &secrets, VM_UID, "b-disk-c")
        .await
        .unwrap();
    // The caller's status write was lost. Meanwhile a different valid VM
    // creates its own credential at the now-free generated name.
    let foreign = credential(&second, "c");
    server.store.lock().unwrap().secret = Some(foreign.clone());
    assert!(release(&backend, &secrets, VM_UID, "b-disk-c")
        .await
        .is_err());
    let store = server.store.lock().unwrap();
    assert_eq!(store.secret.as_ref(), Some(&foreign));
    assert_eq!(store.checks, 2);
    assert_eq!(store.deletes.len(), 1, "retry reached foreign DELETE");
}

#[tokio::test]
async fn secret_cleanup_deletes_only_the_checked_uid_and_version_without_reading_data() {
    let vm = vm("a", VM_UID);
    let server = server(Some(credential(&vm, "root")), None).await;
    let secrets = SecretStore {
        api: Api::namespaced(server.client.clone(), "qrab"),
        vm: &vm,
    };
    secrets.delete("root").await.unwrap();
    let store = server.store.lock().unwrap();
    assert!(store.secret.is_none());
    assert_eq!(store.checks, 1);
    assert!(store.metadata_only);
    assert_eq!(store.deletes.len(), 1);
    assert_eq!(
        store.deletes[0]["preconditions"],
        json!({"uid":"secret-original","resourceVersion":"11"})
    );
}

#[tokio::test]
async fn secret_cleanup_cannot_delete_a_replacement_or_a_newer_credential() {
    let vm = vm("a", VM_UID);
    for replaced_uid in [false, true] {
        let original = credential(&vm, "root");
        let mut replacement = original.clone();
        replacement["metadata"]["resourceVersion"] = "12".into();
        if replaced_uid {
            replacement["metadata"]["uid"] = "secret-replacement".into();
        }
        let server = server(Some(original), Some(replacement.clone())).await;
        let secrets = SecretStore {
            api: Api::namespaced(server.client.clone(), "qrab"),
            vm: &vm,
        };
        assert!(secrets.delete("root").await.is_err());
        assert_eq!(
            server.store.lock().unwrap().secret.as_ref(),
            Some(&replacement)
        );
    }
}

#[tokio::test]
async fn secret_cleanup_missing_is_idempotent() {
    let vm = vm("a", VM_UID);
    let server = server(None, None).await;
    let secrets = SecretStore {
        api: Api::namespaced(server.client.clone(), "qrab"),
        vm: &vm,
    };
    secrets.delete("root").await.unwrap();
    assert!(server.store.lock().unwrap().secret.is_none());
}

#[tokio::test]
async fn secret_cleanup_refuses_missing_or_ambiguous_ownership_and_object_identity() {
    let vm = vm("a", VM_UID);
    for (pointer, value) in [
        ("/metadata/ownerReferences/0/kind", json!("OtherKind")),
        ("/metadata/ownerReferences/0/controller", json!(false)),
        ("/metadata/ownerReferences/0/name", json!("another-vm")),
        (
            "/metadata/labels/app.kubernetes.io~1component",
            json!("other"),
        ),
        (
            "/metadata/labels/qrab.yapplesauce.com~1owner-uid",
            json!(OTHER_VM_UID),
        ),
        (
            "/metadata/annotations/qrab.yapplesauce.com~1disk",
            json!("other"),
        ),
        ("/metadata/uid", json!(null)),
        ("/metadata/resourceVersion", json!(null)),
    ] {
        let mut secret = credential(&vm, "root");
        *secret.pointer_mut(pointer).unwrap() = value;
        let server = server(Some(secret.clone()), None).await;
        let secrets = SecretStore {
            api: Api::namespaced(server.client.clone(), "qrab"),
            vm: &vm,
        };
        assert!(secrets.delete("root").await.is_err(), "accepted {pointer}");
        let store = server.store.lock().unwrap();
        assert_eq!(store.secret.as_ref(), Some(&secret), "changed {pointer}");
        assert!(store.deletes.is_empty(), "deleted with {pointer}");
    }
    let mut shared = credential(&vm, "root");
    let mut other = shared["metadata"]["ownerReferences"][0].clone();
    other["uid"] = OTHER_VM_UID.into();
    shared["metadata"]["ownerReferences"]
        .as_array_mut()
        .unwrap()
        .push(other);
    let server = server(Some(shared.clone()), None).await;
    let secrets = SecretStore {
        api: Api::namespaced(server.client.clone(), "qrab"),
        vm: &vm,
    };
    assert!(secrets.delete("root").await.is_err());
    let store = server.store.lock().unwrap();
    assert_eq!(store.secret.as_ref(), Some(&shared));
    assert!(store.deletes.is_empty());
}
