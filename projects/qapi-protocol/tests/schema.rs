//! Non-QEMU serde tests (trait metadata, enum coverage, field renames)
//! plus QMP schema introspection (needs QEMU x86_64).

#[allow(dead_code, unused_imports)]
mod common;

#[allow(unused_imports)]
use common::*;
use qapi::*;

// ── Command trait metadata ───────────────────────────────────────────────────

#[test]
fn command_trait_metadata() {
    ensure_version_set();
    assert_eq!(QueryStatus::NAME, "query-status");
    assert!(!QueryStatus::ALLOW_OOB);
    assert!(QueryStatus::SUCCESS_RESPONSE);

    assert_eq!(QueryVersion::NAME, "query-version");
    assert_eq!(QueryName::NAME, "query-name");
    assert_eq!(QueryKvm::NAME, "query-kvm");
    assert_eq!(QueryUuid::NAME, "query-uuid");
    assert_eq!(QueryCommands::NAME, "query-commands");
    assert_eq!(QueryChardev::NAME, "query-chardev");
    assert_eq!(QueryBlock::NAME, "query-block");
    assert_eq!(QueryBlockJobs::NAME, "query-block-jobs");
    assert_eq!(QueryMigrate::NAME, "query-migrate");
    assert_eq!(QueryMigrateCapabilities::NAME, "query-migrate-capabilities");
    assert_eq!(QueryMigrateParameters::NAME, "query-migrate-parameters");
    assert_eq!(QueryPci::NAME, "query-pci");
    assert_eq!(QueryTpmModels::NAME, "query-tpm-models");
    assert_eq!(QueryTpmTypes::NAME, "query-tpm-types");
    assert_eq!(QueryQmpSchema::NAME, "query-qmp-schema");
    assert_eq!(QueryDirtyRate::NAME, "query-dirty-rate");
    assert_eq!(QueryYank::NAME, "query-yank");

    assert!(Yank::ALLOW_OOB);
}

// ── Event trait metadata ─────────────────────────────────────────────────────

#[test]
fn event_trait_metadata() {
    ensure_version_set();
    assert_eq!(ShutdownEvent::NAME, "SHUTDOWN");
    assert_eq!(StopEvent::NAME, "STOP");
    assert_eq!(ResumeEvent::NAME, "RESUME");
    assert_eq!(ResetEvent::NAME, "RESET");
    assert_eq!(PowerdownEvent::NAME, "POWERDOWN");
    assert_eq!(SuspendEvent::NAME, "SUSPEND");
    assert_eq!(WakeupEvent::NAME, "WAKEUP");
    assert_eq!(WatchdogEvent::NAME, "WATCHDOG");
    assert_eq!(GuestPanickedEvent::NAME, "GUEST_PANICKED");
}

// ── Enum serialization coverage ──────────────────────────────────────────────

#[test]
fn enum_run_state_all_variants() {
    ensure_version_set();
    let variants: Vec<(&str, RunState)> = vec![
        ("\"debug\"", RunState::Debug),
        ("\"finish-migrate\"", RunState::FinishMigrate),
        ("\"inmigrate\"", RunState::Inmigrate),
        ("\"internal-error\"", RunState::InternalError),
        ("\"io-error\"", RunState::IoError),
        ("\"paused\"", RunState::Paused),
        ("\"postmigrate\"", RunState::Postmigrate),
        ("\"prelaunch\"", RunState::Prelaunch),
        ("\"restore-vm\"", RunState::RestoreVm),
        ("\"running\"", RunState::Running),
        ("\"save-vm\"", RunState::SaveVm),
        ("\"shutdown\"", RunState::Shutdown),
        ("\"suspended\"", RunState::Suspended),
        ("\"watchdog\"", RunState::Watchdog),
        ("\"guest-panicked\"", RunState::GuestPanicked),
        ("\"colo\"", RunState::Colo),
    ];
    for (expected_json, variant) in variants {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected_json, "RunState serialized wrong for {}", expected_json);
        let back: RunState = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), expected_json);
    }
}

#[test]
fn enum_on_off_auto() {
    ensure_version_set();
    for (expected, variant) in [("\"auto\"", OnOffAuto::Auto), ("\"on\"", OnOffAuto::On), ("\"off\"", OnOffAuto::Off)] {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected);
        let back: OnOffAuto = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), expected);
    }
}

#[test]
fn enum_io_operation_type() {
    ensure_version_set();
    assert_eq!(serde_json::to_string(&IoOperationType::Read).unwrap(), "\"read\"");
    assert_eq!(serde_json::to_string(&IoOperationType::Write).unwrap(), "\"write\"");
}

#[test]
fn enum_error_class() {
    ensure_version_set();
    let json = serde_json::to_string(&QapiErrorClass::GenericError).unwrap();
    assert_eq!(json, "\"GenericError\"");
    let json = serde_json::to_string(&QapiErrorClass::CommandNotFound).unwrap();
    assert_eq!(json, "\"CommandNotFound\"");
}

#[test]
fn enum_migration_capability_all_variants() {
    ensure_version_set();
    let variants = [
        "xbzrle", "rdma-pin-all", "auto-converge", "zero-blocks", "events",
        "postcopy-ram", "x-colo", "release-ram", "return-path", "pause-before-switchover",
        "multifd", "dirty-bitmaps", "postcopy-blocktime", "late-block-activate",
        "x-ignore-shared", "validate-uuid", "background-snapshot",
        "zero-copy-send", "postcopy-preempt", "switchover-ack",
        "dirty-limit", "mapped-ram",
    ];
    for name in &variants {
        let json_str = format!("\"{}\"", name);
        if let Ok(cap) = serde_json::from_str::<MigrationCapability>(&json_str) {
            let back = serde_json::to_string(&cap).unwrap();
            assert_eq!(back, json_str, "Roundtrip failed for MigrationCapability '{}'", name);
        }
    }
}

#[test]
fn enum_dump_guest_memory_format() {
    ensure_version_set();
    for name in &["elf", "kdump-zlib", "kdump-lzo", "kdump-snappy", "kdump-raw-zlib",
                  "kdump-raw-lzo", "kdump-raw-snappy", "win-dmp"] {
        let json_str = format!("\"{}\"", name);
        if let Ok(fmt) = serde_json::from_str::<DumpGuestMemoryFormat>(&json_str) {
            let back = serde_json::to_string(&fmt).unwrap();
            assert_eq!(back, json_str);
        }
    }
}

// ── Alternate types ──────────────────────────────────────────────────────────

#[test]
fn alternate_str_or_null() {
    ensure_version_set();
    let val = StrOrNull::S("hello".to_string());
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, "\"hello\"");
    let back: StrOrNull = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, StrOrNull::S(s) if s == "hello"));

    let val = StrOrNull::N(());
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, "null");
    let back: StrOrNull = serde_json::from_str("null").unwrap();
    assert!(matches!(back, StrOrNull::N(())));
}

// ── Struct field renames ─────────────────────────────────────────────────────

#[test]
fn struct_field_renames() {
    ensure_version_set();
    let val = HumanReadableText { human_readable_text: "test".to_string() };
    let json = serde_json::to_string(&val).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("human-readable-text").is_some(), "Expected 'human-readable-text' key");

    let val = UuidInfo { uuid: "abc".to_string() };
    let json = serde_json::to_string(&val).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("UUID").is_some(), "Expected 'UUID' key");
}

// ── QMP schema introspection (needs QEMU) ────────────────────────────────────

arch_tests!(query_qmp_schema, [X86_64], |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let schema: Vec<SchemaInfo> = t.execute(&QueryQmpSchema {}).await.unwrap();
    assert!(schema.len() > 100, "QMP schema should have >100 entries, got {}", schema.len());

    let mut commands = 0u32;
    let mut objects = 0u32;
    let mut enums = 0u32;
    let mut events = 0u32;
    for entry in &schema {
        match serde_json::to_string(&entry.meta_type).unwrap().as_str() {
            "\"command\"" => commands += 1,
            "\"object\"" => objects += 1,
            "\"enum\"" => enums += 1,
            "\"event\"" => events += 1,
            _ => {}
        }
    }
    assert!(commands > 50, "Should have >50 commands, got {}", commands);
    assert!(objects > 50, "Should have >50 objects, got {}", objects);
    assert!(enums > 10, "Should have >10 enums, got {}", enums);
    assert!(events > 5, "Should have >5 events, got {}", events);

    t.shutdown().await;
});
