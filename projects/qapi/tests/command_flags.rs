use qapi::QapiCommand;

#[qapi::qapi(command, returns = "()")]
#[qapi(allow_oob(added = "6.0.0", removed = "9.0.0"))]
#[qapi(allow_preconfig(added = "7.0.0", removed = "10.0.0"))]
#[qapi(no_success_response(added = "8.0.0", removed = "11.0.0"))]
struct Changing {}

#[test]
fn command_flags_follow_inclusive_additions_and_exclusive_removals() {
    for (major, oob, preconfig, reply) in [
        (5, false, false, true),
        (6, true, false, true),
        (7, true, true, true),
        (8, true, true, false),
        (9, false, true, false),
        (10, false, false, false),
        (11, false, false, true),
    ] {
        let version = (major, 0, 0);
        assert_eq!(Changing::allows_oob(version), oob, "{version:?}");
        assert_eq!(
            Changing::allows_preconfig(version),
            preconfig,
            "{version:?}"
        );
        assert_eq!(
            Changing::sends_success_response(version),
            reply,
            "{version:?}"
        );
    }
    assert!(!Changing::ALLOW_OOB);
    assert!(!Changing::ALLOW_PRECONFIG);
    assert!(Changing::SUCCESS_RESPONSE);
    assert_eq!(Changing::PRECONFIG_SINCE, Some((7, 0, 0)));
}

#[test]
fn real_historical_reply_flags_and_bare_flags_remain_queryable() {
    use qapi::{
        commands::Yank,
        qga::commands::{GuestShutdown, GuestSuspendRam},
    };
    assert!(GuestShutdown::sends_success_response((1, 0, 0)));
    assert!(!GuestShutdown::sends_success_response((1, 1, 0)));
    assert!(!GuestSuspendRam::sends_success_response((1, 1, 0)));
    assert!(Yank::allows_oob((11, 1, 3)));
    assert!(!Yank::allows_preconfig((11, 1, 3)));
}
