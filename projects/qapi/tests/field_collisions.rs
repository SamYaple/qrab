//! Historical wire names remain distinct after Rust field-name conversion.
use serde_json::json;

#[test]
fn cpu_info_preserves_both_instruction_pointer_fields() {
    for version in [(1, 0, 0), (1, 4, 1), (2, 5, 1)] {
        qapi::with_version(version, || {
            let mut wire =
                json!({"CPU":0,"current":true,"halted":false,"thread_id":123,"pc":11,"PC":22});
            if version >= (2, 4, 0) {
                wire["qom_path"] = json!("/machine/cpu0");
            }
            let value: qapi::CpuInfo = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(value.pc, Some(11));
            assert_eq!(value.pc_alt, Some(22));
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        });
    }
    let fields: Vec<_> = qapi::metadata::STRUCTS
        .get("CpuInfo")
        .unwrap()
        .fields
        .iter()
        .filter(|field| matches!(field.wire_name, "pc" | "PC"))
        .collect();
    assert_eq!(fields.len(), 2);
    for field in fields {
        assert_eq!(field.since, (1, 0, 0));
        assert_eq!(field.until, Some((2, 6, 0)));
    }
}
