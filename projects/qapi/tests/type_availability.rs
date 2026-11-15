use serde::de::DeserializeOwned;
use serde_json::{json, Value};

#[qapi::qapi(added = "2.0.0", removed = "3.0.0")]
struct Record {
    value: String,
}

#[qapi::qapi(added = "2.0.0", removed = "3.0.0")]
struct Flattened {
    #[qapi(flatten)]
    properties: Option<std::collections::HashMap<String, Value>>,
}

#[qapi::qapi(added = "2.0.0", removed = "3.0.0")]
enum Kind {
    #[qapi(name = "value")]
    Value,
}

#[qapi::qapi(added = "2.0.0", removed = "3.0.0")]
enum Alternate {
    Text(String),
    Null(()),
}

fn check<T: DeserializeOwned>(wire: Value) {
    for version in [(1, 9, 0), (3, 0, 0)] {
        let error = qapi::with_version(version, || serde_json::from_value::<T>(wire.clone()))
            .err()
            .unwrap();
        assert!(error.to_string().contains("not available"), "{error}");
    }
    qapi::with_version((2, 0, 0), || serde_json::from_value::<T>(wire.clone())).unwrap();
    // Recorded values can still be inspected without asserting a peer release.
    serde_json::from_value::<T>(wire).unwrap();
}

#[test]
fn explicit_peer_contexts_enforce_type_lifetimes_for_all_wire_shapes() {
    check::<Record>(json!({"value":"record"}));
    check::<Flattened>(json!({"value":"flattened"}));
    check::<Kind>(json!("value"));
    check::<Alternate>(json!("text"));
    check::<Alternate>(Value::Null);
}
