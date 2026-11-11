//! Wire-shape regressions exposed by the current QEMU schema.
use qapi::{with_version, BlockExportIothreads};

#[test]
fn alternate_preserves_scalar_and_array_payloads() {
    with_version((11, 1, 3), || {
        let single = BlockExportIothreads::Single("io0".into());
        assert_eq!(
            serde_json::to_value(single).unwrap(),
            serde_json::json!("io0")
        );
        let multi = BlockExportIothreads::Multi(vec!["io0".into(), "io1".into()]);
        let value = serde_json::json!(["io0", "io1"]);
        assert_eq!(serde_json::to_value(multi).unwrap(), value);
        match serde_json::from_value::<BlockExportIothreads>(value).unwrap() {
            BlockExportIothreads::Multi(threads) => assert_eq!(threads, ["io0", "io1"]),
            _ => panic!("Array payload decoded as scalar alternative"),
        }
        assert!(serde_json::from_value::<BlockExportIothreads>(serde_json::json!(42)).is_err());
    });
}
