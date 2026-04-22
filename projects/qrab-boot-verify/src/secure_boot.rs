//! UEFI `SecureBoot` / `SetupMode` variable-state evaluation.
//!
//! Separated from the firmware call sites so the logic is exercised
//! on the host via `cargo test -p qrab-boot-verify`. qrab-boot reads
//! the two global variables, hands the raw byte slices to
//! [`evaluate`], and halts with a distinct `QRAB_FAIL_SB_*` serial
//! token when the returned status is not [`SbStatus::Enforcing`].
//!
//! UEFI 2.11 §3.3 / Platform Firmware Profile 1.06 §4.1:
//! * `SecureBoot`: UINT8. `1` = enforcing, `0` = off or not active.
//! * `SetupMode`:  UINT8. `0` = user mode, `1` = setup mode (PK
//!   missing — anyone can enroll).
//!
//! We require **both** conditions (`SecureBoot == 1 && SetupMode == 0`)
//! before the firmware LoadImage path is worth trusting.

#![cfg_attr(not(feature = "std"), no_std)]

/// Terminal verdict from [`evaluate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbStatus {
    /// `SecureBoot == 1 && SetupMode == 0`. Firmware is enforcing db.
    Enforcing,
    /// `SecureBoot == 0`. Either never enabled, or disabled at runtime.
    Disabled,
    /// `SecureBoot == 1 && SetupMode == 1`. Setup mode bypasses auth
    /// for PK writes — effectively unprotected even if the byte says 1.
    SetupMode,
    /// `SecureBoot` byte held a value other than 0 or 1.
    SecureBootMalformed { value: u8 },
    /// `SetupMode` byte held a value other than 0 or 1.
    SetupModeMalformed { value: u8 },
    /// One of the two variables was unreadable (missing, zero-length,
    /// or firmware returned an error). Callers distinguish via
    /// [`SbMissing`].
    VarMissing(SbMissing),
}

/// Which UEFI global variable was missing when [`evaluate_bytes`] saw
/// an empty slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbMissing {
    SecureBoot,
    SetupMode,
}

/// Pure-logic evaluator. Callers pass the first byte (or `None` for a
/// missing/zero-length variable) of each global variable.
pub fn evaluate(secure_boot: Option<u8>, setup_mode: Option<u8>) -> SbStatus {
    let sb = match secure_boot {
        Some(b) => b,
        None => return SbStatus::VarMissing(SbMissing::SecureBoot),
    };
    let sm = match setup_mode {
        Some(b) => b,
        None => return SbStatus::VarMissing(SbMissing::SetupMode),
    };

    match (sb, sm) {
        (0, 0) | (0, 1) => SbStatus::Disabled,
        (1, 0) => SbStatus::Enforcing,
        (1, 1) => SbStatus::SetupMode,
        (1, other) => SbStatus::SetupModeMalformed { value: other },
        (other, _) => SbStatus::SecureBootMalformed { value: other },
    }
}

/// Convenience wrapper that accepts raw byte slices as they come from
/// `runtime::get_variable`. Empty slices become `None`.
pub fn evaluate_bytes(secure_boot: &[u8], setup_mode: &[u8]) -> SbStatus {
    evaluate(secure_boot.first().copied(), setup_mode.first().copied())
}

/// Serial-token for the matching `SbStatus`, including the OK path.
/// Callers should `log::warn!("{}", sb_token(status))` then halt on
/// anything other than `QRAB_SB_OK`.
pub fn sb_token(status: SbStatus) -> &'static str {
    match status {
        SbStatus::Enforcing => "QRAB_SB_OK",
        SbStatus::Disabled => "QRAB_FAIL_SB_DISABLED",
        SbStatus::SetupMode => "QRAB_FAIL_SB_SETUPMODE",
        SbStatus::SecureBootMalformed { .. } => "QRAB_FAIL_SB_MALFORMED",
        SbStatus::SetupModeMalformed { .. } => "QRAB_FAIL_SB_MALFORMED",
        SbStatus::VarMissing(SbMissing::SecureBoot) => "QRAB_FAIL_SB_VAR_MISSING_SB",
        SbStatus::VarMissing(SbMissing::SetupMode) => "QRAB_FAIL_SB_VAR_MISSING_SM",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforcing_happy_path() {
        assert_eq!(evaluate(Some(1), Some(0)), SbStatus::Enforcing);
        assert_eq!(sb_token(SbStatus::Enforcing), "QRAB_SB_OK");
    }

    #[test]
    fn secure_boot_off_is_disabled() {
        assert_eq!(evaluate(Some(0), Some(0)), SbStatus::Disabled);
        assert_eq!(evaluate(Some(0), Some(1)), SbStatus::Disabled);
    }

    #[test]
    fn setup_mode_is_not_enforcing() {
        assert_eq!(evaluate(Some(1), Some(1)), SbStatus::SetupMode);
    }

    #[test]
    fn secure_boot_byte_not_bool_is_malformed() {
        assert_eq!(
            evaluate(Some(2), Some(0)),
            SbStatus::SecureBootMalformed { value: 2 }
        );
    }

    #[test]
    fn setup_mode_byte_not_bool_is_malformed() {
        // sb = 1 required to reach SetupModeMalformed branch.
        assert_eq!(
            evaluate(Some(1), Some(7)),
            SbStatus::SetupModeMalformed { value: 7 }
        );
    }

    #[test]
    fn missing_secure_boot_reported_distinctly() {
        let s = evaluate(None, Some(0));
        assert_eq!(s, SbStatus::VarMissing(SbMissing::SecureBoot));
        assert_eq!(sb_token(s), "QRAB_FAIL_SB_VAR_MISSING_SB");
    }

    #[test]
    fn missing_setup_mode_reported_distinctly() {
        let s = evaluate(Some(1), None);
        assert_eq!(s, SbStatus::VarMissing(SbMissing::SetupMode));
        assert_eq!(sb_token(s), "QRAB_FAIL_SB_VAR_MISSING_SM");
    }

    #[test]
    fn evaluate_bytes_honours_first_byte_only() {
        // UEFI variables for SecureBoot / SetupMode are UINT8, but the
        // spec does not preclude firmware returning more bytes. Our
        // evaluator looks at byte 0 only; downstream bytes are ignored.
        assert_eq!(evaluate_bytes(&[1, 0xff, 0xff], &[0]), SbStatus::Enforcing);
        assert_eq!(evaluate_bytes(&[], &[0]), SbStatus::VarMissing(SbMissing::SecureBoot));
        assert_eq!(evaluate_bytes(&[1], &[]), SbStatus::VarMissing(SbMissing::SetupMode));
    }

    #[test]
    fn every_status_has_a_token() {
        // Lock the enum-to-token mapping so new variants don't silently
        // fall through to a default.
        let variants = [
            SbStatus::Enforcing,
            SbStatus::Disabled,
            SbStatus::SetupMode,
            SbStatus::SecureBootMalformed { value: 9 },
            SbStatus::SetupModeMalformed { value: 9 },
            SbStatus::VarMissing(SbMissing::SecureBoot),
            SbStatus::VarMissing(SbMissing::SetupMode),
        ];
        for v in variants {
            let t = sb_token(v);
            assert!(t.starts_with("QRAB_"), "token {t} missing QRAB_ prefix");
        }
    }
}
