//! Valid field names must not capture generated serde locals.
use serde_json::json;

#[qapi::qapi]
struct Tail {
    tail: String,
}

#[qapi::qapi]
struct Flattened {
    map: String,
    full_map: String,
    __qapi_map: String,
    #[qapi(flatten)]
    extra: Tail,
}

#[qapi::qapi]
struct Ordinary {
    __field_value: String,
    __qapi_field_value: String,
    value: String,
    value2: String,
}

#[test]
fn flattened_field_names_and_internal_prefixes_round_trip() {
    let wire = json!({"map":"map","full_map":"full","__qapi_map":"reserved","tail":"tail"});
    let value: Flattened = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), wire);
}

#[test]
fn ordinary_field_names_cannot_capture_another_fields_storage() {
    let wire = json!({"__field_value":"internal-looking","__qapi_field_value":"second","value":"value","value2":"second value"});
    let value: Ordinary = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), wire);
}

#[qapi::qapi]
struct D {
    text: String,
}
#[qapi::qapi]
enum S {
    Value(D),
}
#[qapi::qapi]
struct __Visitor {
    text: String,
}
#[qapi::qapi]
struct __QapiDeserializer {
    text: String,
}
#[qapi::qapi]
struct WithD {
    data: D,
}

#[test]
fn schema_type_names_do_not_capture_visitor_or_serde_generic_names() {
    let value: S = serde_json::from_value(json!({"text":"value"})).unwrap();
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        json!({"text":"value"})
    );
    let value: __Visitor = serde_json::from_value(json!({"text":"visitor"})).unwrap();
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        json!({"text":"visitor"})
    );
    let value: __QapiDeserializer = serde_json::from_value(json!({"text":"deserializer"})).unwrap();
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        json!({"text":"deserializer"})
    );
    let value: WithD = serde_json::from_value(json!({"data":{"text":"nested"}})).unwrap();
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        json!({"data":{"text":"nested"}})
    );
}

#[qapi::qapi]
struct SharedStorage {
    __qapi_raw_76616c7565: String,
    #[qapi(name = "value", added = "10.0.0")]
    current: Option<String>,
    #[qapi(name = "value", removed = "10.0.0")]
    historical: Option<i64>,
}
#[qapi::qapi]
enum Kind {
    #[qapi(name = "one")]
    One,
}
#[qapi::qapi(union_branch)]
enum Branch {
    #[qapi(name = "one")]
    One(Tail),
}
#[qapi::qapi]
struct UnionStorage {
    #[qapi(discriminator)]
    kind: Kind,
    __qapi_union_payload: String,
    #[qapi(flatten, union)]
    u: Option<Branch>,
}

#[test]
fn user_fields_cannot_capture_shared_wire_or_union_payload_storage() {
    qapi::with_version((11, 1, 3), || {
        let wire = json!({"__qapi_raw_76616c7565":"user","value":"current"});
        let value: SharedStorage = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
        let wire = json!({"kind":"one","__qapi_union_payload":"user","tail":"payload"});
        let value: UnionStorage = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    });
}

#[qapi::qapi]
struct __QapiVisitor {
    text: String,
}
#[qapi::qapi]
struct ReferencedVisitor {
    value: __QapiVisitor,
}

#[test]
fn helper_type_names_cannot_shadow_referenced_schema_types() {
    let wire = json!({"value":{"text":"schema type"}});
    let value: ReferencedVisitor = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), wire);
}
