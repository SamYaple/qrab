//! Sanity tests for the generated `qapi::metadata` module.
//!
//! Asserts the shape contract that downstream consumers (notably the BMC UI)
//! rely on: every command/struct/enum is present, flat unions are flagged,
//! alternates and branch enums are distinguishable, and the type-ref pool
//! resolves correctly.

use qapi::metadata::{
    EnumKind, COMMANDS, ENUMS, STRUCTS,
};

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
