pub use qapi_runtime::{
    PreconfigSafe, QapiCommand, QapiEvent, QapiFlatUnion, qemu_version, scoped_qemu_version,
    unavailable_type_message, with_version,
};
mod generated;
use generated::*;

fn main() {
    with_version((6, 1, 0), || {
        let value: <commands::QueryChangingDirect as QapiCommand>::Returns =
            ChangingTypeLegacy2::Old;
        assert_eq!(
            serde_json::to_value(value).unwrap(),
            serde_json::json!("old")
        );
        let values: <commands::QueryChangingArray as QapiCommand>::Returns =
            vec![ChangingTypeLegacy2::Old];
        assert_eq!(
            serde_json::to_value(values).unwrap(),
            serde_json::json!(["old"])
        );
        let value: CycleHolder =
            serde_json::from_value(serde_json::json!({"item":{"value":"middle"}})).unwrap();
        assert_eq!(
            serde_json::to_value(value).unwrap(),
            serde_json::json!({"item":{"value":"middle"}})
        );
    });
}
