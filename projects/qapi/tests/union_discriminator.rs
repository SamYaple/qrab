//! A union's explicit tag must describe its selected typed payload.
use serde_json::json;

#[qapi::qapi]
enum Tag {
    A,
    B,
}

#[qapi::qapi]
struct Data {
    value: String,
}

#[qapi::qapi(union_branch)]
enum Branch {
    A(Data),
    B(Data),
}

#[qapi::qapi]
struct Flat {
    #[qapi(discriminator)]
    kind: Tag,
    #[qapi(flatten, union)]
    u: Option<Box<Branch>>,
}

#[qapi::qapi(simple_union)]
struct Nested {
    #[qapi(name = "type", discriminator)]
    kind: Tag,
    #[qapi(flatten, union)]
    u: Option<Branch>,
}

#[test]
fn outgoing_tags_match_flat_and_nested_payloads() {
    qapi::with_version((11, 1, 3), || {
        let payload = || {
            Branch::A(Data {
                value: "payload".into(),
            })
        };
        let flat = Flat {
            kind: Tag::B,
            u: Some(Box::new(payload())),
        };
        let nested = Nested {
            kind: Tag::B,
            u: Some(payload()),
        };
        let error = serde_json::to_value(flat).unwrap_err().to_string();
        assert!(error.contains("Flat: discriminator 'kind'"), "{error}");
        assert!(error.contains("payload selects 'A'"), "{error}");
        let error = serde_json::to_value(nested).unwrap_err().to_string();
        assert!(error.contains("Nested: discriminator 'type'"), "{error}");
        assert!(error.contains("payload selects 'A'"), "{error}");
        let flat = Flat {
            kind: Tag::A,
            u: Some(Box::new(payload())),
        };
        let nested = Nested {
            kind: Tag::A,
            u: Some(payload()),
        };
        assert_eq!(
            serde_json::to_value(flat).unwrap(),
            json!({"kind":"A","value":"payload"})
        );
        assert_eq!(
            serde_json::to_value(nested).unwrap(),
            json!({"type":"A","data":{"value":"payload"}})
        );
    });
}
