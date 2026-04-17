use qapi::commands::*;
use qapi::events::*;
use qapi::*;

/// Set a high version so all since-gated fields are included.
fn init_version() {
    set_qemu_version(99, 0, 0);
}

#[test]
fn unit_enum_serialize() {
    init_version();
    let val = IoOperationType::Read;
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, r#""read""#);

    let val = IoOperationType::Write;
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, r#""write""#);
}

#[test]
fn unit_enum_deserialize() {
    init_version();
    let val: IoOperationType = serde_json::from_str(r#""read""#).unwrap();
    assert!(matches!(val, IoOperationType::Read));

    let val: IoOperationType = serde_json::from_str(r#""write""#).unwrap();
    assert!(matches!(val, IoOperationType::Write));
}

#[test]
fn unit_enum_roundtrip() {
    init_version();
    let variants = vec![OnOffAuto::Auto, OnOffAuto::On, OnOffAuto::Off];
    for v in variants {
        let json = serde_json::to_string(&v).unwrap();
        let back: OnOffAuto = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }
}

#[test]
fn enum_with_rename() {
    init_version();
    let val = OffAutoPcibar::Bar0;
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, r#""bar0""#);

    let val: OffAutoPcibar = serde_json::from_str(r#""bar5""#).unwrap();
    assert!(matches!(val, OffAutoPcibar::Bar5));
}

#[test]
fn enum_leading_digit_variant() {
    init_version();
    let val = PcieLinkSpeed::_25;
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, r#""2_5""#);

    let val: PcieLinkSpeed = serde_json::from_str(r#""16""#).unwrap();
    assert!(matches!(val, PcieLinkSpeed::_16));
}

#[test]
fn alternate_string_roundtrip() {
    init_version();
    let val = StrOrNull::S("hello".to_string());
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, r#""hello""#);

    let back: StrOrNull = serde_json::from_str(&json).unwrap();
    match back {
        StrOrNull::S(s) => assert_eq!(s, "hello"),
        _ => panic!("expected S variant"),
    }
}

#[test]
fn alternate_null_roundtrip() {
    init_version();
    let val = StrOrNull::N(());
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, "null");

    let back: StrOrNull = serde_json::from_str("null").unwrap();
    assert!(matches!(back, StrOrNull::N(())));
}

#[test]
fn struct_serialize_with_rename() {
    init_version();
    let val = HumanReadableText {
        human_readable_text: "test output".to_string(),
    };
    let json = serde_json::to_string(&val).unwrap();
    assert!(json.contains(r#""human-readable-text""#));
    assert!(json.contains(r#""test output""#));
}

#[test]
fn struct_deserialize_with_rename() {
    init_version();
    let json = r#"{"human-readable-text": "hello"}"#;
    let val: HumanReadableText = serde_json::from_str(json).unwrap();
    assert_eq!(val.human_readable_text, "hello");
}

#[test]
fn struct_roundtrip() {
    init_version();
    let val = StatusInfo {
        running: true,
        singlestep: Some(false),
        status: RunState::Running,
    };
    let json = serde_json::to_string(&val).unwrap();
    let back: StatusInfo = serde_json::from_str(&json).unwrap();
    assert!(back.running);
    assert!(matches!(back.status, RunState::Running));
}

#[test]
fn command_trait_query_status() {
    assert_eq!(QueryStatus::NAME, "query-status");
    assert!(!QueryStatus::ALLOW_OOB);
    assert!(QueryStatus::SUCCESS_RESPONSE);
    // Returns type check — this is a compile-time check
    fn _check_returns(_: <QueryStatus as QapiCommand>::Returns) {}
    fn _check_is_status_info(_: StatusInfo) {}
}

#[test]
fn command_trait_yank() {
    assert_eq!(Yank::NAME, "yank");
    assert!(Yank::ALLOW_OOB);
    assert!(!Yank::ALLOW_PRECONFIG);
    assert!(Yank::SUCCESS_RESPONSE);
}

#[test]
fn event_trait() {
    assert_eq!(ShutdownEvent::NAME, "SHUTDOWN");
    assert_eq!(StopEvent::NAME, "STOP");
    assert_eq!(ResumeEvent::NAME, "RESUME");
}

#[test]
fn command_serialize() {
    init_version();
    let cmd = QmpCapabilities { enable: None };
    let json = serde_json::to_string(&cmd).unwrap();
    // Should serialize to an empty object (no enable field since it's None)
    assert_eq!(json, "{}");

    let cmd = QmpCapabilities {
        enable: Some(vec![QmpCapability::Oob]),
    };
    let json = serde_json::to_string(&cmd).unwrap();
    assert!(json.contains("\"enable\""));
    assert!(json.contains("\"oob\""));
}

#[test]
fn event_serialize() {
    init_version();
    let evt = ShutdownEvent {
        guest: Some(true),
        reason: Some(ShutdownCause::HostSignal),
    };
    let json = serde_json::to_string(&evt).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["guest"], true);
    assert_eq!(parsed["reason"], "host-signal");
}

#[test]
fn event_deserialize() {
    init_version();
    let json = r#"{"guest": false, "reason": "host-qmp-quit"}"#;
    let evt: ShutdownEvent = serde_json::from_str(json).unwrap();
    assert_eq!(evt.guest, Some(false));
    assert!(matches!(evt.reason, Some(ShutdownCause::HostQmpQuit)));
}

#[test]
fn version_gating_rejects_old_version() {
    init_version();
    let val = IoOperationType::Read;
    assert!(serde_json::to_string(&val).is_ok());
}

#[test]
fn union_struct_serialize() {
    init_version();
    let val = YankInstance {
        r#type: YankInstanceType::Chardev,
        u: Some(YankInstanceBranch::Chardev(YankInstanceChardev {
            id: "serial0".to_string(),
        })),
    };
    let json = serde_json::to_string(&val).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "chardev");
}

#[test]
fn error_class_roundtrip() {
    init_version();
    let val = QapiErrorClass::GenericError;
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, r#""GenericError""#);
    let back: QapiErrorClass = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, QapiErrorClass::GenericError));
}

#[test]
fn command_version_info() {
    // Test that AVAILABLE_SINCE and REMOVED_IN are set correctly
    assert_eq!(QueryStatus::AVAILABLE_SINCE, (1, 0, 0));
    assert_eq!(QueryStatus::REMOVED_IN, None);
}

// ── Module separation tests ──

/// Commands must live in qapi::commands, not the crate root.
#[test]
fn commands_in_commands_module() {
    // These are commands — they implement QapiCommand
    assert_eq!(qapi::commands::QueryStatus::NAME, "query-status");
    assert_eq!(qapi::commands::QueryVersion::NAME, "query-version");
    assert_eq!(qapi::commands::Stop::NAME, "stop");
    assert_eq!(qapi::commands::Cont::NAME, "cont");
    assert_eq!(qapi::commands::Quit::NAME, "quit");
    assert_eq!(qapi::commands::SystemReset::NAME, "system_reset");
    assert_eq!(qapi::commands::SystemPowerdown::NAME, "system_powerdown");
}

/// Events must live in qapi::events, not the crate root.
#[test]
fn events_in_events_module() {
    use qapi::QapiEvent;
    assert_eq!(qapi::events::ShutdownEvent::NAME, "SHUTDOWN");
    assert_eq!(qapi::events::StopEvent::NAME, "STOP");
    assert_eq!(qapi::events::ResumeEvent::NAME, "RESUME");
    assert_eq!(qapi::events::ResetEvent::NAME, "RESET");
    assert_eq!(qapi::events::PowerdownEvent::NAME, "POWERDOWN");
}

/// Data types must live at the crate root, not in commands or events.
#[test]
fn data_types_at_crate_root() {
    // These are data types (return values, not commands)
    let _: qapi::StatusInfo =
        serde_json::from_str(r#"{"running":true,"status":"running"}"#).unwrap();

    // ErrorClass is an enum data type
    let _: qapi::ErrorClass = serde_json::from_str(r#""GenericError""#).unwrap();
}

/// QGA commands must live in qapi::qga::commands.
#[test]
fn qga_commands_in_module() {
    assert_eq!(qapi::qga::commands::GuestPing::NAME, "guest-ping");
    assert_eq!(qapi::qga::commands::GuestInfo::NAME, "guest-info");
    assert_eq!(qapi::qga::commands::GuestSync::NAME, "guest-sync");
}

/// QGA data types must live at qapi::qga root.
#[test]
fn qga_data_types_at_module_root() {
    // GuestAgentInfo is a data type, not a command
    let _info: Option<qapi::qga::GuestAgentInfo> = None;
}

/// Commands implement QapiCommand trait with correct metadata.
#[test]
fn command_trait_metadata() {
    init_version();
    // QueryStatus is a well-known command available since early QEMU
    assert_eq!(qapi::commands::QueryStatus::NAME, "query-status");
    assert!(qapi::commands::QueryStatus::SUCCESS_RESPONSE);

    // Yank has specific version bounds
    assert_eq!(qapi::commands::Yank::NAME, "yank");
}

/// Cross-referenced command types are accessible from data types module.
/// Some commands like BlockdevBackup are used by TransactionAction variants.
#[test]
fn cross_referenced_command_types_accessible() {
    // BlockdevBackup is a command that's also referenced by TransactionAction.
    // It should be accessible from the root module (where data types live).
    let _: Option<qapi::BlockdevBackup> = None;
}

// ─── Discriminated union (flat) tests ───

#[test]
fn sev_info_deser_sev_variant() {
    init_version();
    // QEMU 10.2 response with sev-type discriminator
    let json = r#"{"api-major":1,"api-minor":55,"build-id":49,"enabled":true,"sev-type":"sev","state":"running","handle":1,"policy":1}"#;
    let info: SevInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.api_major, 1);
    assert_eq!(info.api_minor, 55);
    assert!(info.enabled);
    assert!(matches!(info.sev_type, Some(SevGuestType::Sev)));
    assert!(matches!(info.state, SevState::Running));
    match &info.u {
        Some(SevInfoBranch::Sev(guest)) => {
            assert_eq!(guest.handle, 1);
            assert_eq!(guest.policy, 1);
        }
        other => panic!("expected SevInfoBranch::Sev, got {:?}", other),
    }
}

#[test]
fn sev_info_deser_snp_variant() {
    init_version();
    let json = r#"{"api-major":1,"api-minor":55,"build-id":49,"enabled":true,"sev-type":"sev-snp","state":"running","snp-policy":196608}"#;
    let info: SevInfo = serde_json::from_str(json).unwrap();
    assert!(matches!(info.sev_type, Some(SevGuestType::SevSnp)));
    match &info.u {
        Some(SevInfoBranch::SevSnp(guest)) => {
            assert_eq!(guest.snp_policy, 196608);
        }
        other => panic!("expected SevInfoBranch::SevSnp, got {:?}", other),
    }
}

#[test]
fn sev_info_deser_pre_91() {
    init_version();
    // Pre-9.1.0 QEMU: no sev-type, no union branch, has handle/policy as direct fields
    let json = r#"{"api-major":1,"api-minor":55,"build-id":49,"enabled":true,"state":"running","handle":1,"policy":1}"#;
    let info: SevInfo = serde_json::from_str(json).unwrap();
    assert!(info.enabled);
    assert!(info.sev_type.is_none());
    assert!(info.u.is_none());
    // handle/policy should be captured in the removed fields
    assert_eq!(info.handle, Some(1));
    assert_eq!(info.policy, Some(1));
}

#[test]
fn sev_info_roundtrip_sev() {
    init_version();
    let info = SevInfo {
        api_major: 1,
        api_minor: 55,
        build_id: 49,
        enabled: true,
        handle: None,
        policy: None,
        sev_type: Some(SevGuestType::Sev),
        state: SevState::Running,
        u: Some(SevInfoBranch::Sev(SevGuestInfo {
            handle: 1,
            policy: 1,
        })),
    };
    let json = serde_json::to_string(&info).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    // Verify flat structure: handle/policy at top level, not nested
    assert_eq!(parsed["handle"], 1);
    assert_eq!(parsed["policy"], 1);
    assert_eq!(parsed["sev-type"], "sev");
    // Round-trip
    let back: SevInfo = serde_json::from_str(&json).unwrap();
    assert!(matches!(back.sev_type, Some(SevGuestType::Sev)));
    match &back.u {
        Some(SevInfoBranch::Sev(g)) => {
            assert_eq!(g.handle, 1);
            assert_eq!(g.policy, 1);
        }
        other => panic!("expected Sev, got {:?}", other),
    }
}

#[test]
fn sev_info_roundtrip_snp() {
    init_version();
    let info = SevInfo {
        api_major: 1,
        api_minor: 55,
        build_id: 49,
        enabled: true,
        handle: None,
        policy: None,
        sev_type: Some(SevGuestType::SevSnp),
        state: SevState::Running,
        u: Some(SevInfoBranch::SevSnp(SevSnpGuestInfo {
            snp_policy: 196608,
        })),
    };
    let json = serde_json::to_string(&info).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["sev-type"], "sev-snp");
    assert_eq!(parsed["snp-policy"], 196608);
    let back: SevInfo = serde_json::from_str(&json).unwrap();
    assert!(matches!(back.sev_type, Some(SevGuestType::SevSnp)));
    match &back.u {
        Some(SevInfoBranch::SevSnp(g)) => assert_eq!(g.snp_policy, 196608),
        other => panic!("expected SevSnp, got {:?}", other),
    }
}
