use qapi::{qemu_version, with_version};

#[test]
fn serialization_context_is_nested_and_restored_on_panic() {
    let original = qemu_version();
    with_version((11, 1, 3), || {
        assert_eq!(qemu_version(), (11, 1, 3));
        with_version((10, 2, 3), || assert_eq!(qemu_version(), (10, 2, 3)));
        assert_eq!(qemu_version(), (11, 1, 3));
        let result = std::panic::catch_unwind(|| with_version((1, 0, 0), || panic!("test unwind")));
        assert!(result.is_err());
        assert_eq!(qemu_version(), (11, 1, 3));
        std::thread::spawn(move || assert_eq!(qemu_version(), original))
            .join()
            .unwrap();
    });
    assert_eq!(qemu_version(), original);
}

#[test]
fn out_of_context_serialization_names_the_fix_and_the_helper_works() {
    let props: qapi::CpuInstanceProperties =
        serde_json::from_value(serde_json::json!({"core-id": 2, "thread-id": 0})).unwrap();
    let error = serde_json::to_value(&props).unwrap_err().to_string();
    assert!(error.contains("CpuInstanceProperties"), "{error}");
    assert!(error.contains("qapi::with_version"), "{error}");
    let value = qapi::to_value((11, 1, 3), &props).unwrap();
    assert_eq!(value["core-id"], 2);
    assert_eq!(qemu_version(), (0, 0, 0), "the helper restores the context");

    // Inside a context, a type outside its lifetime names the peer version.
    let removed: qapi::TargetInfo =
        serde_json::from_value(serde_json::json!({"arch": "x86_64"})).unwrap();
    let error = qapi::to_value((11, 1, 3), &removed)
        .unwrap_err()
        .to_string();
    assert!(error.contains("not available in QEMU 11.1.3"), "{error}");
    assert!(qapi::to_value((10, 0, 0), &removed).is_ok());
}

#[qapi::qapi(added = "9.2.0", removed = "12.0.0")]
enum VersionedUnitEnum {
    Stable,
    #[qapi(name = "introduced", added = "10.2.0")]
    Introduced,
    #[qapi(name = "retired", removed = "11.0.0")]
    Retired,
}

#[qapi::qapi(added = "9.2.0", removed = "12.0.0")]
enum VersionedAlternate {
    #[qapi(added = "10.2.0")]
    Text(String),
    #[qapi(removed = "11.0.0")]
    Number(i64),
    #[qapi(added = "10.0.0")]
    Null,
}

#[test]
fn enum_serialization_checks_variant_introduction_and_removal_boundaries() {
    assert!(qapi::to_value((10, 1, 0), &VersionedUnitEnum::Introduced).is_err());
    assert_eq!(
        qapi::to_value((10, 2, 0), &VersionedUnitEnum::Introduced).unwrap(),
        "introduced"
    );
    assert_eq!(
        qapi::to_value((10, 2, 0), &VersionedUnitEnum::Retired).unwrap(),
        "retired"
    );
    let error = qapi::to_value((11, 0, 0), &VersionedUnitEnum::Retired)
        .unwrap_err()
        .to_string();
    assert!(error.contains("VersionedUnitEnum::Retired"), "{error}");
    assert!(error.contains("11.0.0"), "{error}");
    assert!(qapi::to_value((12, 0, 0), &VersionedUnitEnum::Stable).is_err());
}

#[test]
fn alternate_serialization_checks_type_and_variant_lifetimes() {
    assert!(qapi::to_value((10, 1, 0), &VersionedAlternate::Text("value".into())).is_err());
    assert_eq!(
        qapi::to_value((10, 2, 0), &VersionedAlternate::Text("value".into())).unwrap(),
        "value"
    );
    assert_eq!(
        qapi::to_value((10, 2, 0), &VersionedAlternate::Number(1)).unwrap(),
        1
    );
    assert!(qapi::to_value((11, 0, 0), &VersionedAlternate::Number(1)).is_err());
    assert!(qapi::to_value((9, 2, 0), &VersionedAlternate::Null).is_err());
    assert!(qapi::to_value((10, 0, 0), &VersionedAlternate::Null)
        .unwrap()
        .is_null());
    assert!(qapi::to_value((9, 1, 0), &VersionedAlternate::Number(1)).is_err());
    assert!(qapi::to_value((12, 0, 0), &VersionedAlternate::Text("value".into())).is_err());
    let error = serde_json::to_value(&VersionedAlternate::Number(1))
        .unwrap_err()
        .to_string();
    assert!(error.contains("qapi::with_version"), "{error}");
}

#[test]
fn scoped_enum_and_alternate_decoding_enforces_unique_variant_lifetimes() {
    for (version, wire) in [((10, 1, 0), "introduced"), ((11, 0, 0), "retired")] {
        with_version(version, || {
            assert!(serde_json::from_value::<VersionedUnitEnum>(wire.into()).is_err());
        });
        assert!(serde_json::from_value::<VersionedUnitEnum>(wire.into()).is_ok());
    }
    for (version, wire) in [
        ((10, 1, 0), serde_json::json!("value")),
        ((11, 0, 0), serde_json::json!(1)),
        ((9, 2, 0), serde_json::Value::Null),
    ] {
        with_version(version, || {
            assert!(serde_json::from_value::<VersionedAlternate>(wire.clone()).is_err());
        });
        assert!(serde_json::from_value::<VersionedAlternate>(wire).is_ok());
    }
}

#[test]
fn generated_migration_commands_cannot_send_capabilities_outside_their_lifetime() {
    let command = qapi::commands::MigrateSetCapabilities {
        capabilities: vec![qapi::MigrationCapabilityStatus {
            capability: qapi::MigrationCapability::MappedRam,
            state: true,
        }],
    };
    assert!(qapi::to_value((8, 2, 0), &command).is_err());
    assert_eq!(
        qapi::to_value((9, 0, 0), &command).unwrap(),
        serde_json::json!({
            "capabilities": [{ "capability": "mapped-ram", "state": true }]
        })
    );
    #[allow(deprecated)]
    let retired = qapi::commands::MigrateSetCapabilities {
        capabilities: vec![qapi::MigrationCapabilityStatus {
            capability: qapi::MigrationCapability::ZeroBlocks,
            state: true,
        }],
    };
    assert!(qapi::to_value((10, 2, 0), &retired).is_ok());
    assert!(qapi::to_value((11, 0, 0), &retired).is_err());
}
