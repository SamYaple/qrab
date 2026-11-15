//! A flat union may use only implicit branches with no payload fields.
use serde_json::json;

#[qapi::qapi]
enum EmptyBranchKind {
    #[qapi(name = "first")]
    First,
    #[qapi(name = "second")]
    Second,
}

// Stacked attributes must preserve the generator's branch marker.
#[qapi::qapi]
#[qapi(union_branch)]
enum EmptyBranch {
    #[qapi(name = "first")]
    First,
    #[qapi(name = "second")]
    Second,
}

#[qapi::qapi]
struct EmptyPayloadUnion {
    #[qapi(discriminator)]
    kind: EmptyBranchKind,
    #[qapi(flatten, union)]
    u: Option<EmptyBranch>,
}

#[qapi::qapi(union_branch)]
enum NoCompiledBranches {}

#[test]
fn implicit_empty_branches_use_the_discriminator_and_add_no_wire_fields() {
    for (kind, second) in [("first", false), ("second", true)] {
        let value = json!({"kind":kind});
        let union: EmptyPayloadUnion = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(matches!(union.u, Some(EmptyBranch::Second)), second);
        assert_eq!(serde_json::to_value(&union).unwrap(), value);
        assert!(serde_json::to_value(&union.u).unwrap().is_null());
    }
    assert!(serde_json::from_value::<EmptyPayloadUnion>(json!({"kind":"unknown"})).is_err());
}

#[test]
fn a_branch_enum_with_no_compiled_variants_still_has_a_fallible_dispatcher() {
    assert!(
        <NoCompiledBranches as qapi::QapiFlatUnion>::deserialize_variant(
            "unknown",
            Default::default()
        )
        .is_err()
    );
    assert!(serde_json::from_value::<NoCompiledBranches>(json!(null)).is_err());
}
