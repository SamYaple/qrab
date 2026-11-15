//! Sanity tests for the generated `qapi::metadata` module.
//!
//! Asserts the shape contract that downstream consumers (notably the BMC UI)
//! rely on: every command/struct/enum is present, flat unions are flagged,
//! alternates and branch enums are distinguishable, and the type-ref pool
//! resolves correctly.

use qapi::metadata::{EnumKind, COMMANDS, ENUMS, STRUCTS};

#[test]
fn historical_discriminator_fields_do_not_hide_current_unions() {
    for (name, wire_name) in [
        ("SocketAddress", "type"),
        ("NumaOptions", "type"),
        ("NetLegacyOptions", "type"),
        ("BlockdevOptions", "driver"),
        ("ChardevBackend", "type"),
        ("CpuInfoFast", "target"),
        ("ImageInfoSpecific", "type"),
        ("InputEvent", "type"),
    ] {
        let info = STRUCTS.get(name).unwrap();
        assert!(info.is_union, "{name}");
        assert!(info.discriminator_field.is_some(), "{name}");
        let branch = ENUMS.get(info.branch_enum.unwrap()).unwrap();
        assert_eq!(branch.kind, EnumKind::FlatUnionBranch, "{name}");
        let discriminator = info
            .fields
            .iter()
            .find(|field| Some(field.name) == info.discriminator_field)
            .unwrap();
        assert_eq!(discriminator.wire_name, wire_name);
        assert!(discriminator.until.is_none(), "{name}");
    }
}

#[test]
fn metadata_references_resolve_including_enum_payloads() {
    use qapi::metadata::{TypeKind, TYPE_REFS};
    for (index, ty) in TYPE_REFS.iter().enumerate() {
        match ty.kind {
            TypeKind::Struct => assert!(STRUCTS.contains_key(ty.name), "{}", ty.name),
            TypeKind::Enum => assert!(ENUMS.contains_key(ty.name), "{}", ty.name),
            TypeKind::List => assert!((ty.inner as usize) < index),
            TypeKind::Primitive => {}
        }
    }
    for metadata in STRUCTS.values() {
        for field in metadata.fields {
            assert!((field.ty as usize) < TYPE_REFS.len());
        }
    }
    for metadata in ENUMS.values() {
        for variant in metadata.variants {
            if metadata.kind == EnumKind::Unit {
                assert!(variant.payload_type.is_none());
            }
            if let Some(payload) = variant.payload_type {
                let ty = &TYPE_REFS[payload as usize];
                if let Some(name) = variant.payload_struct {
                    assert_eq!(ty.kind, TypeKind::Struct);
                    assert_eq!(ty.name, name);
                }
            } else {
                assert!(variant.payload_struct.is_none());
            }
        }
    }
}

#[test]
fn commands_table_has_full_qapi_surface() {
    // QEMU 10.2 carries roughly 280 QMP commands. A floor of 250 prevents
    // accidental schema-export regressions without needing to bump the
    // expected count on every QEMU release.
    assert!(
        COMMANDS.len() >= 250,
        "expected >=250 commands, got {}",
        COMMANDS.len()
    );
}

#[test]
fn structs_and_enums_populated() {
    assert!(
        STRUCTS.len() >= 800,
        "expected >=800 structs, got {}",
        STRUCTS.len()
    );
    assert!(
        ENUMS.len() >= 100,
        "expected >=100 enums, got {}",
        ENUMS.len()
    );
}

#[test]
fn blockdev_add_is_a_flat_union() {
    let s = STRUCTS.get("BlockdevAdd").expect("BlockdevAdd missing");
    assert!(s.is_union, "BlockdevAdd should be detected as a union");
    assert!(
        s.discriminator_field.is_some(),
        "BlockdevAdd should expose its discriminator field"
    );
    assert!(
        s.branch_enum.is_some(),
        "BlockdevAdd should reference a branch enum"
    );
}

#[test]
fn empty_args_command_present() {
    // `query-status` returns StatusInfo and takes no arguments.
    let cmd = COMMANDS
        .iter()
        .find(|c| c.wire_name == "query-status")
        .expect("query-status command missing");
    assert_eq!(
        cmd.args.fields.len(),
        0,
        "query-status should have zero fields, got {}",
        cmd.args.fields.len()
    );
    assert!(
        cmd.returns.is_some(),
        "query-status should have a return type"
    );
}

#[test]
fn alternate_enum_classified() {
    let e = ENUMS
        .get("StrOrNull")
        .expect("StrOrNull alternate missing from ENUMS");
    assert_eq!(
        e.kind,
        EnumKind::Alternate,
        "StrOrNull should be classified as Alternate"
    );
}

#[test]
fn flat_union_branch_enum_classified() {
    let e = ENUMS
        .get("BlockdevOptionsBranch")
        .expect("BlockdevOptionsBranch missing from ENUMS");
    assert_eq!(
        e.kind,
        EnumKind::FlatUnionBranch,
        "BlockdevOptionsBranch should be classified as FlatUnionBranch"
    );
    assert!(
        e.variants.iter().any(|v| v.payload_struct.is_some()),
        "at least one BlockdevOptionsBranch variant should carry a payload struct"
    );
}

#[test]
fn unit_enum_classified() {
    // Pick a known unit-only enum.
    let e = ENUMS
        .get("RunState")
        .or_else(|| ENUMS.get("BlockdevDriver"))
        .expect("expected at least one unit enum");
    assert_eq!(e.kind, EnumKind::Unit);
}

#[test]
fn changed_branch_payloads_expose_distinct_lifetimes_and_original_wire_tags() {
    let branches = ENUMS.get("SocketAddressLegacyBranch").unwrap();
    let current = branches
        .variants
        .iter()
        .find(|variant| variant.name == "Inet")
        .unwrap();
    let historical = branches
        .variants
        .iter()
        .find(|variant| variant.name == "InetLegacy")
        .unwrap();
    assert_eq!(current.wire_name, "inet");
    assert_eq!(historical.wire_name, "inet");
    assert_eq!(current.since, (6, 2, 0));
    assert_eq!(historical.until, Some((6, 2, 0)));
    assert_eq!(current.payload_struct, Some("InetSocketAddressWrapper"));
    assert_eq!(historical.payload_struct, Some("InetSocketAddress"));
}

#[test]
fn required_wire_fields_stay_required_in_metadata_after_version_merging() {
    let info = STRUCTS.get("VersionInfo").unwrap();
    let qemu = info
        .fields
        .iter()
        .find(|field| field.name == "qemu")
        .unwrap();
    assert!(
        !qemu.optional,
        "version merging must not turn wire-required into schema-optional"
    );
    let value: qapi::VersionInfo = serde_json::from_value(serde_json::json!({
        "qemu":{"major":11,"minor":1,"micro":3},"package":""
    }))
    .unwrap();
    assert!(
        value.qemu.is_some(),
        "Rust uses Option to accommodate the older inline representation"
    );
}
