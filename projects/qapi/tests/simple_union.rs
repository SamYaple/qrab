//! Historical simple unions keep arbitrary branch payloads under `data`.
use serde_json::{json, Value};

#[qapi::qapi]
enum Kind {
    #[qapi(name = "text")]
    Text,
    #[qapi(name = "number")]
    Number,
    #[qapi(name = "list")]
    List,
    #[qapi(name = "object")]
    Object,
    #[qapi(name = "null")]
    Null,
}

#[qapi::qapi]
struct Payload {
    required: String,
}

#[qapi::qapi(union_branch)]
enum Branch {
    #[qapi(name = "text")]
    Text(String),
    #[qapi(name = "number")]
    Number(i64),
    #[qapi(name = "list")]
    List(Vec<i64>),
    #[qapi(name = "object")]
    Object(Payload),
    #[qapi(name = "null")]
    Null(()),
}

#[qapi::qapi]
#[qapi(simple_union)]
struct Simple {
    #[qapi(name = "type", discriminator)]
    kind: Kind,
    #[qapi(flatten, union)]
    u: Option<Box<Branch>>,
}

#[qapi::qapi(simple_union(removed = "6.2.0"))]
struct Transition {
    #[qapi(name = "type", discriminator)]
    kind: Kind,
    #[qapi(flatten, union)]
    u: Option<Branch>,
}

#[test]
fn simple_unions_round_trip_scalar_array_object_and_null_payloads() {
    for (kind, payload) in [
        ("text", json!("value")),
        ("number", json!(42)),
        ("list", json!([1, 2, 3])),
        ("object", json!({"required":"value"})),
        ("null", Value::Null),
    ] {
        let value = json!({"type":kind,"data":payload});
        let parsed: Simple = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(&parsed).unwrap(), value);
    }
    for invalid in [
        json!({"type":"text","data":1}),
        json!({"type":"text"}),
        json!({"type":"object","data":{}}),
        json!({"type":"unknown","data":{}}),
    ] {
        assert!(serde_json::from_value::<Simple>(invalid).is_err());
    }
    let missing = Simple {
        kind: Kind::Text,
        u: None,
    };
    assert!(serde_json::to_value(missing).is_err());
}

#[test]
fn simple_to_flat_transition_selects_the_peer_layout() {
    let historical = json!({"type":"object","data":{"required":"old"}});
    qapi::with_version((6, 1, 0), || {
        let parsed: Transition = serde_json::from_value(historical.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), historical);
    });
    let current = json!({"type":"object","required":"new"});
    qapi::with_version((6, 2, 0), || {
        let parsed: Transition = serde_json::from_value(current.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), current);
    });
    // Unscoped inspection defaults to the newest representation.
    let parsed: Transition = serde_json::from_value(current.clone()).unwrap();
    assert_eq!(qapi::to_value((11, 1, 3), &parsed).unwrap(), current);
}

#[test]
fn generated_socket_addresses_use_the_pre_2_10_nested_layout() {
    for (version, value) in [
        (
            (1, 3, 0),
            json!({"type":"inet","data":{"host":"127.0.0.1","port":"10809"}}),
        ),
        (
            (2, 9, 0),
            json!({"type":"inet","data":{"host":"127.0.0.1","port":"10809"}}),
        ),
        (
            (11, 1, 3),
            json!({"type":"inet","host":"127.0.0.1","port":"10809"}),
        ),
    ] {
        qapi::with_version(version, || {
            let address: qapi::SocketAddress = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(address).unwrap(), value);
        });
    }
}

#[test]
fn historical_generated_tags_stay_separate_from_later_schema_enums() {
    qapi::with_version((2, 1, 0), || {
        let wire = json!({"type":"node","data":{}});
        let value: qapi::NumaOptions = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
        assert_eq!(
            serde_json::to_value(qapi::NumaOptionsTypeGenerated::Node).unwrap(),
            json!("node")
        );
        assert!(serde_json::to_value(qapi::NumaOptionsType::Node).is_err());
    });
    qapi::with_version((2, 7, 0), || {
        let wire = json!({"type":"none","data":{}});
        let value: qapi::NetLegacyOptions = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
        assert!(serde_json::to_value(qapi::NetLegacyOptionsType::None).is_err());
    });
}

#[test]
fn scalar_payloads_cannot_silently_disappear_from_flat_unions() {
    let value = Transition {
        kind: Kind::List,
        u: Some(Branch::List(vec![1, 2])),
    };
    let error = qapi::to_value((6, 2, 0), &value).unwrap_err().to_string();
    assert!(
        error.contains("flattened QAPI payload must be an object"),
        "{error}"
    );
}
