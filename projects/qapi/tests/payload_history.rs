//! Versioned branches sharing a wire tag must use the peer's payload shape.
use serde_json::json;

#[qapi::qapi]
enum NumericChoice {
    Floating(f64),
    #[qapi(removed = "2.10.0")]
    Integer(i8),
}

#[qapi::qapi(union_branch)]
enum UniqueBranches {
    #[qapi(name = "new", added = "10.2.0")]
    New(After),
    #[qapi(name = "old", removed = "11.0.0")]
    Old,
}

#[test]
fn scoped_union_dispatch_checks_unique_branch_lifetimes() {
    use qapi::QapiFlatUnion;
    let payload: serde_json::Map<String, serde_json::Value> =
        serde_json::from_value(json!({"new":"value"})).unwrap();
    qapi::with_version((10, 1, 0), || {
        assert!(UniqueBranches::deserialize_variant("new", payload.clone()).is_err());
    });
    qapi::with_version((10, 2, 0), || {
        assert!(UniqueBranches::deserialize_variant("new", payload.clone()).is_ok());
        assert!(UniqueBranches::deserialize_variant("old", Default::default()).is_ok());
    });
    qapi::with_version((11, 0, 0), || {
        assert!(UniqueBranches::deserialize_variant("old", Default::default()).is_err());
    });
    assert!(UniqueBranches::deserialize_variant("new", payload).is_ok());
    assert!(UniqueBranches::deserialize_variant("old", Default::default()).is_ok());
}

#[test]
fn legacy_numeric_alternates_preserve_shape_and_reject_integer_range_errors() {
    qapi::with_version((2, 9, 0), || {
        for number in [-128, 0, 127] {
            let value: NumericChoice = serde_json::from_value(json!(number)).unwrap();
            assert!(matches!(value, NumericChoice::Integer(_)));
            assert_eq!(serde_json::to_value(value).unwrap(), json!(number));
        }
        for number in [-129i64, 128, 9_007_199_254_740_993] {
            assert!(serde_json::from_value::<NumericChoice>(json!(number)).is_err());
        }
        for number in [42.0, 42.5, -0.0] {
            let value: NumericChoice = serde_json::from_value(json!(number)).unwrap();
            assert!(matches!(value, NumericChoice::Floating(_)));
            assert_eq!(serde_json::to_value(value).unwrap(), json!(number));
        }
    });
    qapi::with_version((2, 10, 0), || {
        for number in [42, 128] {
            let value: NumericChoice = serde_json::from_value(json!(number)).unwrap();
            assert!(matches!(value, NumericChoice::Floating(_)));
        }
    });
    // With no asserted peer, recorded numeric values remain inspectable.
    assert!(serde_json::from_value::<NumericChoice>(json!(128)).is_ok());
}

#[qapi::qapi]
enum Kind {
    #[qapi(name = "value")]
    Value,
}
#[qapi::qapi]
struct Before {
    old: String,
}
#[qapi::qapi]
struct After {
    new: String,
}

#[qapi::qapi(union_branch)]
enum Branch {
    #[qapi(name = "value", added = "6.2.0")]
    Value(After),
    #[qapi(name = "value", removed = "6.2.0")]
    ValueLegacy(Before),
}

#[qapi::qapi]
struct Union {
    #[qapi(discriminator)]
    kind: Kind,
    #[qapi(flatten, union)]
    u: Option<Branch>,
}

#[qapi::qapi(union_branch)]
enum PreviouslyEmpty {
    #[qapi(name = "value", added = "6.2.0")]
    Value(After),
    #[qapi(name = "value", removed = "6.2.0")]
    ValueLegacy,
}

#[test]
fn discriminator_dispatch_chooses_the_payload_at_the_version_boundary() {
    qapi::with_version((6, 1, 0), || {
        let value = json!({"kind":"value","old":"before"});
        let union: Union = serde_json::from_value(value.clone()).unwrap();
        assert!(matches!(union.u, Some(Branch::ValueLegacy(_))));
        assert_eq!(serde_json::to_value(union).unwrap(), value);
        let direct: Branch = serde_json::from_value(json!({"old":"before"})).unwrap();
        assert!(matches!(direct, Branch::ValueLegacy(_)));
    });
    qapi::with_version((6, 2, 0), || {
        let value = json!({"kind":"value","new":"after"});
        let union: Union = serde_json::from_value(value.clone()).unwrap();
        assert!(matches!(union.u, Some(Branch::Value(_))));
        assert_eq!(serde_json::to_value(union).unwrap(), value);
        assert!(serde_json::from_value::<Union>(json!({"kind":"value","old":"before"})).is_err());
        assert!(serde_json::from_value::<Branch>(json!({"old":"before"})).is_err());
    });
}

#[test]
fn unscoped_inspection_uses_latest_shape_without_falling_back_after_a_decode_error() {
    let union: Union = serde_json::from_value(json!({"kind":"value","new":"after"})).unwrap();
    assert!(matches!(union.u, Some(Branch::Value(_))));
    assert!(serde_json::from_value::<Union>(json!({"kind":"value","old":"before"})).is_err());
    assert!(serde_json::from_value::<Branch>(json!({"old":"before"})).is_err());
    assert!(
        <PreviouslyEmpty as qapi::QapiFlatUnion>::deserialize_variant("value", Default::default())
            .is_err()
    );
    qapi::with_version((6, 1, 0), || {
        let empty = <PreviouslyEmpty as qapi::QapiFlatUnion>::deserialize_variant(
            "value",
            Default::default(),
        )
        .unwrap();
        assert!(matches!(empty, PreviouslyEmpty::ValueLegacy));
    });
}

#[test]
fn generated_legacy_socket_addresses_keep_original_and_wrapper_payloads() {
    for version in [(6, 1, 0), (6, 2, 0), (11, 1, 3)] {
        qapi::with_version(version, || {
            let value = json!({"type":"inet","data":{"host":"127.0.0.1","port":"10809"}});
            let address: qapi::SocketAddressLegacy = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(address).unwrap(), value);
        });
    }
}

#[test]
fn recent_dbus_audio_payloads_remain_usable_before_the_10_0_type_change() {
    for version in [(9, 2, 0), (10, 0, 0), (11, 1, 3)] {
        qapi::with_version(version, || {
            let value = json!({"driver":"dbus","id":"audio0"});
            let audio: qapi::Audiodev = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(
                matches!(audio.u, Some(qapi::AudiodevBranch::DbusLegacy(_))),
                version < (10, 0, 0)
            );
            assert_eq!(serde_json::to_value(audio).unwrap(), value);
        });
    }
}

#[test]
fn generated_enum_wire_renames_preserve_historical_spellings() {
    for (version, wire) in [
        ((2, 5, 0), "l1_grow.activate_table"),
        ((2, 6, 0), "l1_grow_activate_table"),
    ] {
        let value = json!(wire);
        let event: qapi::BlkdebugEvent = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(qapi::to_value(version, &event).unwrap(), value);
    }
}

#[test]
fn legacy_empty_discriminator_alternates_use_bare_values_at_their_original_versions() {
    let reference = qapi::BlockdevRef::Reference("node0".into());
    assert_eq!(
        qapi::to_value((1, 7, 0), &reference).unwrap(),
        json!("node0")
    );
    let mode = qapi::Qcow2OverlapChecks::Mode(qapi::Qcow2OverlapCheckMode::Cached);
    assert_eq!(qapi::to_value((2, 2, 0), &mode).unwrap(), json!("cached"));
}
