//! Shared wire fields select a single peer-appropriate representation.
use serde_json::json;

#[test]
fn stable_branch_backports_do_not_enable_missing_fields_in_other_series() {
    for version in [(10, 0, 10), (10, 2, 3), (11, 0, 1), (11, 1, 3)] {
        qapi::with_version(version, || {
            let wire = json!({"event":"read_aio","delay-ns":7});
            let value: qapi::BlkdebugInjectErrorOptions =
                serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        });
    }
    for version in [(10, 0, 9), (10, 1, 0), (10, 2, 2), (11, 0, 0)] {
        qapi::with_version(version, || {
            assert!(serde_json::from_value::<qapi::BlkdebugInjectErrorOptions>(
                json!({"event":"read_aio","delay-ns":7})
            )
            .is_err());
            let wire = json!({"event":"read_aio"});
            let value: qapi::BlkdebugInjectErrorOptions =
                serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        });
    }
}

#[test]
fn object_add_catchall_maps_keep_their_separate_lifetimes() {
    for version in [(2, 4, 0), (5, 0, 0)] {
        qapi::with_version(version, || {
            let wire = json!({"id":"rng","qom-type":"rng-random","filename":"/dev/urandom"});
            let value: qapi::commands::ObjectAdd = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), wire);
        });
    }
    qapi::with_version((3, 0, 0), || {
        let wire = json!({"id":"rng","qom-type":"rng-random","props":{"filename":"/dev/urandom"}});
        let value: qapi::commands::ObjectAdd = serde_json::from_value(wire.clone()).unwrap();
        assert!(value.arguments.is_none() && value.arguments_legacy.is_none());
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    });
}

#[test]
fn schema_introspection_keeps_the_historical_required_oob_flag() {
    let minimal = json!({"arg-type":"q_empty","ret-type":"q_empty"});
    qapi::with_version((2, 12, 0), || {
        assert!(serde_json::from_value::<qapi::SchemaInfoCommand>(minimal.clone()).is_err());
        let mut wire = minimal.clone();
        wire["allow-oob"] = json!(false);
        let value: qapi::SchemaInfoCommand = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(value.allow_oob_legacy, Some(false));
        assert!(value.allow_oob.is_none());
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    });
    qapi::with_version((3, 0, 0), || {
        let value: qapi::SchemaInfoCommand = serde_json::from_value(minimal.clone()).unwrap();
        assert!(value.allow_oob.is_none() && value.allow_oob_legacy.is_none());
        assert_eq!(serde_json::to_value(value).unwrap(), minimal);
    });
}

#[qapi::qapi]
struct Versioned {
    #[qapi(name = "value", added = "10.0.0")]
    value: Option<String>,
    #[qapi(name = "value", removed = "10.0.0")]
    value_legacy: Option<i64>,
}

#[qapi::qapi]
struct Punctuation {
    #[qapi(name = "foo-bar", added = "10.0.0")]
    left: Option<String>,
    #[qapi(name = "foo-bar", removed = "10.0.0")]
    left_legacy: Option<i64>,
    #[qapi(name = "foo.bar", added = "10.0.0")]
    right: Option<String>,
    #[qapi(name = "foo.bar", removed = "10.0.0")]
    right_legacy: Option<i64>,
}

#[test]
fn fields_do_not_populate_inactive_representations_or_retry_invalid_current_values() {
    qapi::with_version((9, 2, 0), || {
        let value: Versioned = serde_json::from_value(json!({"value":42})).unwrap();
        assert!(value.value.is_none());
        assert_eq!(value.value_legacy, Some(42));
        assert_eq!(serde_json::to_value(value).unwrap(), json!({"value":42}));
    });
    qapi::with_version((10, 0, 0), || {
        let value: Versioned = serde_json::from_value(json!({"value":"current"})).unwrap();
        assert!(value.value_legacy.is_none());
        assert_eq!(
            serde_json::to_value(value).unwrap(),
            json!({"value":"current"})
        );
        assert!(serde_json::from_value::<Versioned>(json!({"value":42})).is_err());
    });
    assert!(serde_json::from_value::<Versioned>(json!({"value":42})).is_err());
}

#[test]
fn distinct_wire_punctuation_never_aliases_generated_temporary_variables() {
    let wire = json!({"foo-bar":"left","foo.bar":"right"});
    let value: Punctuation = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(value.left.as_deref(), Some("left"));
    assert_eq!(value.right.as_deref(), Some("right"));
    assert!(value.left_legacy.is_none() && value.right_legacy.is_none());
    assert_eq!(qapi::to_value((11, 1, 3), &value).unwrap(), wire);
    qapi::with_version((9, 2, 0), || {
        let wire = json!({"foo-bar":1,"foo.bar":2});
        let value: Punctuation = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(value.left_legacy, Some(1));
        assert_eq!(value.right_legacy, Some(2));
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    });
}

#[test]
fn values_in_an_inactive_shared_field_are_errors_instead_of_silently_disappearing() {
    let wrong = Versioned {
        value: Some("new".into()),
        value_legacy: None,
    };
    let error = qapi::to_value((9, 2, 0), &wrong).unwrap_err().to_string();
    assert!(
        error.contains("inactive representation of field 'value'"),
        "{error}"
    );
    let conflicting = Versioned {
        value: Some("new".into()),
        value_legacy: Some(42),
    };
    assert!(qapi::to_value((11, 1, 3), &conflicting).is_err());
}

#[qapi::qapi]
struct ConditionalRequired {
    #[qapi(name = "value", added = "10.0.0", required)]
    current: Option<serde_json::Value>,
    #[qapi(name = "value", removed = "10.0.0", required)]
    legacy: Option<String>,
}

#[qapi::qapi]
struct Nullable {
    value: Option<serde_json::Value>,
}
#[qapi::qapi]
struct NonNullable {
    value: Option<String>,
}

#[test]
fn conditional_required_fields_are_checked_in_a_peer_context() {
    qapi::with_version((11, 1, 3), || {
        assert!(serde_json::from_value::<ConditionalRequired>(json!({})).is_err());
        let value: ConditionalRequired = serde_json::from_value(json!({"value":null})).unwrap();
        assert_eq!(value.current, Some(serde_json::Value::Null));
        assert_eq!(serde_json::to_value(value).unwrap(), json!({"value":null}));
    });
    qapi::with_version((9, 2, 0), || {
        let value: ConditionalRequired = serde_json::from_value(json!({"value":"old"})).unwrap();
        assert_eq!(value.legacy.as_deref(), Some("old"));
        assert!(serde_json::from_value::<ConditionalRequired>(json!({"value":null})).is_err());
    });
    let record: ConditionalRequired = serde_json::from_value(json!({})).unwrap();
    assert!(qapi::to_value((11, 1, 3), &record).is_err());
}

#[test]
fn optional_nullable_values_preserve_presence_and_nonnullable_values_refuse_null() {
    let present: Nullable = serde_json::from_value(json!({"value":null})).unwrap();
    assert_eq!(present.value, Some(serde_json::Value::Null));
    assert_eq!(
        serde_json::to_value(present).unwrap(),
        json!({"value":null})
    );
    let absent: Nullable = serde_json::from_value(json!({})).unwrap();
    assert!(absent.value.is_none());
    assert!(serde_json::from_value::<NonNullable>(json!({"value":null})).is_err());
    let info: qapi::ObjectPropertyInfo =
        serde_json::from_value(json!({"name":"property","type":"str","default-value":null}))
            .unwrap();
    assert_eq!(info.default_value, Some(serde_json::Value::Null));
}

#[qapi::qapi]
struct FlattenedLifecycle {
    id: String,
    #[qapi(flatten, removed = "10.0.0")]
    extra: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[test]
fn flattened_maps_capture_only_unclaimed_keys_and_follow_their_lifetime() {
    qapi::with_version((9, 2, 0), || {
        let wire = json!({"id":"object","unknown":true});
        let value: FlattenedLifecycle = serde_json::from_value(wire.clone()).unwrap();
        let extra = value.extra.as_ref().unwrap();
        assert_eq!(extra.len(), 1);
        assert!(!extra.contains_key("id"));
        assert_eq!(serde_json::to_value(value).unwrap(), wire);
    });
    let value: FlattenedLifecycle = serde_json::from_value(json!({"id":"object"})).unwrap();
    assert!(value.extra.is_none());
    let invalid = FlattenedLifecycle {
        id: "object".into(),
        extra: Some(Default::default()),
    };
    assert!(qapi::to_value((10, 0, 0), &invalid).is_err());
}

#[test]
fn synthetic_gen_false_catchall_is_not_a_required_wire_property() {
    let value = qapi::commands::QomSet {
        path: "/object".into(),
        property: "property".into(),
        value: serde_json::Value::Null,
        arguments: None,
    };
    assert_eq!(
        qapi::to_value((2, 4, 0), &value).unwrap(),
        json!({"path":"/object","property":"property","value":null})
    );
}
