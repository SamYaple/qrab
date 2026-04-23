//! Fixed token strings emitted on `/dev/console` per
//! `docs/security/level-5-init.md` §Failure tokens.

/// Every outcome — success or specific failure — qrab-guest-init can
/// emit. Wire format is the raw UTF-8 bytes of the string returned by
/// [`Token::as_str`] plus a trailing newline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Start,
    Ok,
    FailNoSnp,
    FailSbOff,
    FailSbSetup,
    FailLockdown,
    FailModsig,
    FailImaPolicy,
    FailVerity,
    FailEventlogParse,
    /// `QRAB_INIT_FAIL_PCR_<N>` for N in 0..=10. Carried as a byte so
    /// the token formatter doesn't have to allocate.
    FailPcr(u8),
    FailReportParse,
    FailVmpl,
    FailMeasurement,
    FailPolicy,
    FailTcb,
    FailReportData,
    FailSecret,
    FailSecretLen,
    FailSecretConsumed,
    FailPolicyPcr,
}

impl Token {
    /// Raw wire string without trailing newline.
    pub fn as_str(self) -> &'static str {
        match self {
            Token::Start => "QRAB_INIT_START",
            Token::Ok => "QRAB_INIT_OK",
            Token::FailNoSnp => "QRAB_INIT_FAIL_NOSNP",
            Token::FailSbOff => "QRAB_INIT_FAIL_SB_OFF",
            Token::FailSbSetup => "QRAB_INIT_FAIL_SB_SETUP",
            Token::FailLockdown => "QRAB_INIT_FAIL_LOCKDOWN",
            Token::FailModsig => "QRAB_INIT_FAIL_MODSIG",
            Token::FailImaPolicy => "QRAB_INIT_FAIL_IMA_POLICY",
            Token::FailVerity => "QRAB_INIT_FAIL_VERITY",
            Token::FailEventlogParse => "QRAB_INIT_FAIL_EVENTLOG_PARSE",
            Token::FailPcr(n) => match n {
                0 => "QRAB_INIT_FAIL_PCR_0",
                1 => "QRAB_INIT_FAIL_PCR_1",
                2 => "QRAB_INIT_FAIL_PCR_2",
                3 => "QRAB_INIT_FAIL_PCR_3",
                4 => "QRAB_INIT_FAIL_PCR_4",
                5 => "QRAB_INIT_FAIL_PCR_5",
                6 => "QRAB_INIT_FAIL_PCR_6",
                7 => "QRAB_INIT_FAIL_PCR_7",
                8 => "QRAB_INIT_FAIL_PCR_8",
                9 => "QRAB_INIT_FAIL_PCR_9",
                10 => "QRAB_INIT_FAIL_PCR_10",
                // Out-of-range PCR index is itself a protocol violation —
                // report the event-log parse token rather than silently
                // fabricating a string for an invalid bank.
                _ => "QRAB_INIT_FAIL_EVENTLOG_PARSE",
            },
            Token::FailReportParse => "QRAB_INIT_FAIL_REPORT_PARSE",
            Token::FailVmpl => "QRAB_INIT_FAIL_VMPL",
            Token::FailMeasurement => "QRAB_INIT_FAIL_MEASUREMENT",
            Token::FailPolicy => "QRAB_INIT_FAIL_POLICY",
            Token::FailTcb => "QRAB_INIT_FAIL_TCB",
            Token::FailReportData => "QRAB_INIT_FAIL_REPORT_DATA",
            Token::FailSecret => "QRAB_INIT_FAIL_SECRET",
            Token::FailSecretLen => "QRAB_INIT_FAIL_SECRET_LEN",
            Token::FailSecretConsumed => "QRAB_INIT_FAIL_SECRET_CONSUMED",
            Token::FailPolicyPcr => "QRAB_INIT_FAIL_POLICY_PCR",
        }
    }

    /// True for success/beacon tokens; false for any failure the
    /// sequencer should halt on.
    pub fn is_fatal(self) -> bool {
        !matches!(self, Token::Start | Token::Ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_token_has_qrab_init_prefix() {
        for t in [
            Token::Start,
            Token::Ok,
            Token::FailNoSnp,
            Token::FailSbOff,
            Token::FailSbSetup,
            Token::FailLockdown,
            Token::FailModsig,
            Token::FailImaPolicy,
            Token::FailVerity,
            Token::FailEventlogParse,
            Token::FailPcr(0),
            Token::FailPcr(10),
            Token::FailReportParse,
            Token::FailVmpl,
            Token::FailMeasurement,
            Token::FailPolicy,
            Token::FailTcb,
            Token::FailReportData,
            Token::FailSecret,
            Token::FailSecretLen,
            Token::FailSecretConsumed,
            Token::FailPolicyPcr,
        ] {
            assert!(
                t.as_str().starts_with("QRAB_INIT"),
                "token {t:?} must carry QRAB_INIT_ prefix per spec",
            );
        }
    }

    #[test]
    fn pcr_indices_map_distinctly() {
        let strs: Vec<_> = (0u8..=10).map(|n| Token::FailPcr(n).as_str()).collect();
        let deduped: std::collections::HashSet<_> = strs.iter().collect();
        assert_eq!(deduped.len(), 11, "PCR tokens 0..=10 must be distinct");
    }

    #[test]
    fn out_of_range_pcr_falls_back_to_eventlog_parse() {
        // Rather than panicking or printing `QRAB_INIT_FAIL_PCR_255`,
        // out-of-range banks collapse into the generic event-log parse
        // token so an attacker can't induce a novel token by feeding a
        // malformed log.
        assert_eq!(
            Token::FailPcr(255).as_str(),
            "QRAB_INIT_FAIL_EVENTLOG_PARSE",
        );
    }

    #[test]
    fn start_and_ok_are_non_fatal() {
        assert!(!Token::Start.is_fatal());
        assert!(!Token::Ok.is_fatal());
    }

    #[test]
    fn every_failure_is_fatal() {
        for t in [
            Token::FailNoSnp,
            Token::FailLockdown,
            Token::FailPcr(7),
            Token::FailReportData,
            Token::FailSecretConsumed,
        ] {
            assert!(t.is_fatal(), "failure token {t:?} must be fatal");
        }
    }
}
