//! Host-testable helpers used by `qrab-boot::measure::require_and_probe_vtpm`
//! to reason about `EFI_TCG2_PROTOCOL` state.
//!
//! Separated from the firmware call site so the `BootServiceCapability`
//! classification, the `TPM2_PCR_Read` command encoding, and the
//! response parsing can be exercised on the host via
//! `cargo test -p qrab-boot-verify`.
//!
//! Three pieces live here:
//!
//! 1. [`classify_capability`] — maps `(tpm_present, active_pcr_banks)`
//!    to a terminal [`VtpmStatus`]. qrab-boot halts on any variant
//!    other than [`VtpmStatus::Enforcing`].
//! 2. [`PCR_READ_PCR0_SHA256_CMD`] — the 20-byte big-endian command
//!    buffer submitted to `EFI_TCG2_PROTOCOL.SubmitCommand` to fetch
//!    PCR 0 of the SHA-256 bank. Locked at a fixed value by
//!    [`tests::pcr_read_cmd_matches_tpm_spec_bytes`].
//! 3. [`parse_pcr_read_response`] — strips the 10-byte TPM header,
//!    checks `responseCode == TPM_RC_SUCCESS`, walks the
//!    `TPML_PCR_SELECTION` echo + `TPML_DIGEST` wrapper, and returns
//!    the 32-byte SHA-256 digest.
//!
//! ## TPM2_PCR_Read wire format
//!
//! TPM 2.0 Architecture Part 2 / Part 3 §22.4. All multi-byte integers
//! big-endian.
//!
//! ### Command (20 bytes for our fixed PCR 0 / SHA-256 request)
//!
//! ```text
//! [2]  tag         = 0x8001                         TPM_ST_NO_SESSIONS
//! [4]  commandSize = 0x00000014 (= 20)
//! [4]  commandCode = 0x0000017E                     TPM_CC_PCR_Read
//! [4]  TPML_PCR_SELECTION.count = 0x00000001
//! [2]  pcrSelections[0].hash        = 0x000B        TPM_ALG_SHA256
//! [1]  pcrSelections[0].sizeofSelect = 0x03
//! [3]  pcrSelections[0].pcrSelect   = 0x01 0x00 0x00  bit 0 = PCR 0
//! ```
//!
//! ### Response (62 bytes for success on a single SHA-256 PCR)
//!
//! ```text
//! [2]  tag                          (echo of TPM_ST_NO_SESSIONS)
//! [4]  responseSize                 (= 0x0000003E = 62)
//! [4]  responseCode                 (0 = TPM_RC_SUCCESS)
//! [4]  pcrUpdateCounter
//! [4]  TPML_PCR_SELECTION.count     (echo, = 1)
//! [2]  pcrSelections[0].hash        (echo, = 0x000B)
//! [1]  pcrSelections[0].sizeofSelect (echo, = 3)
//! [3]  pcrSelections[0].pcrSelect   (echo)
//! [4]  TPML_DIGEST.count            (= 1)
//! [2]  digests[0].size              (= 0x0020 = 32)
//! [32] digests[0].buffer            SHA-256 digest of PCR 0
//! ```

/// Hash-algorithm bitmap bit for SHA-256 in `EFI_TCG2_BOOT_SERVICE_CAPABILITY.
/// active_pcr_banks` (TCG2 Protocol Spec §6.5.2.1 — `EFI_TCG2_BOOT_HASH_ALG_SHA256`).
pub const HASH_ALG_SHA256: u32 = 0x0000_0002;

/// Serial-log token literals emitted by `qrab-boot::measure` +
/// `efi_main` for the vTPM probe path. Single source of truth so
/// every `contrib/smoke-*.sh` grep pattern matches exactly one set
/// of strings and a silent rename in the bootloader source fails a
/// host-side unit test before it reaches a live smoke. See
/// [`tests::measure_rs_uses_tokens_verbatim`] for the enforcement.
pub mod tokens {
    /// Happy-path — `require_and_probe_vtpm` successfully read a
    /// non-zero PCR 0 from the SVSM-backed vTPM.
    pub const VTPM_SVSM_OK: &str = "QRAB_VTPM_SVSM_OK";
    /// Capability diagnostic line (protocol version, max cmd/rsp,
    /// manufacturer ID, active banks, present flag).
    pub const VTPM_CAP: &str = "QRAB_VTPM_CAP";
    /// Fatal halt — vTPM probe failed under the default
    /// `qrab_require_vtpm` cfg.
    pub const FAIL_NO_VTPM: &str = "QRAB_FAIL_NO_VTPM";
    /// Non-fatal log — vTPM probe failed under the
    /// `QRAB_ALLOW_NO_VTPM=1` opt-out build; the bootloader continues
    /// past the probe and past any extend failures.
    pub const TCG2_SOFT_ABSENT: &str = "QRAB_TCG2_SOFT_ABSENT";

    /// Every token the Phase 3 probe may emit. Consumers that want the
    /// full set (e.g. smoke regex builders) should iterate this rather
    /// than copy-pasting.
    pub const ALL: &[&str] = &[VTPM_SVSM_OK, VTPM_CAP, FAIL_NO_VTPM, TCG2_SOFT_ABSENT];
}

/// Short-suffix strings appended to the halt log for each failure
/// class so operators can grep the precise reason without parsing the
/// `Display` output of `VtpmProbeError`.
///
/// Pinned by [`tests::every_fail_class_has_a_distinct_suffix`].
pub mod fail_suffix {
    pub const TCG2_ABSENT: &str = "TCG2_ABSENT";
    pub const TCG2_OPEN: &str = "TCG2_OPEN";
    pub const TCG2_GET_CAPABILITY: &str = "TCG2_GET_CAPABILITY";
    pub const TPM_NOT_PRESENT: &str = "TPM_NOT_PRESENT";
    pub const NO_SHA256_BANK: &str = "NO_SHA256_BANK";
    pub const PCR_READ: &str = "PCR_READ";
    pub const PCR_PARSE: &str = "PCR_PARSE";
    pub const PCR0_ALL_ZERO: &str = "PCR0_ALL_ZERO";

    pub const ALL: &[&str] = &[
        TCG2_ABSENT,
        TCG2_OPEN,
        TCG2_GET_CAPABILITY,
        TPM_NOT_PRESENT,
        NO_SHA256_BANK,
        PCR_READ,
        PCR_PARSE,
        PCR0_ALL_ZERO,
    ];
}

/// TPM2_PCR_Read for PCR 0, SHA-256 bank. Fixed 20 bytes, big-endian.
/// Submitted verbatim to `EFI_TCG2_PROTOCOL.SubmitCommand`.
pub const PCR_READ_PCR0_SHA256_CMD: [u8; 20] = [
    0x80, 0x01, // tag           = TPM_ST_NO_SESSIONS
    0x00, 0x00, 0x00, 0x14, // commandSize = 20
    0x00, 0x00, 0x01, 0x7E, // commandCode = TPM_CC_PCR_Read
    0x00, 0x00, 0x00, 0x01, // TPML_PCR_SELECTION.count = 1
    0x00, 0x0B, // pcrSelections[0].hash = TPM_ALG_SHA256
    0x03, // pcrSelections[0].sizeofSelect
    0x01, 0x00, 0x00, // pcrSelect bits: bit 0 = PCR 0
];

/// Minimum well-formed response size for our fixed request. Parser
/// rejects anything shorter.
pub const PCR_READ_RESPONSE_MIN_LEN: usize = 62;

/// Terminal verdict from [`classify_capability`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VtpmStatus {
    /// `tpm_present == true && active_pcr_banks & SHA256 != 0`.
    /// Safe to proceed with the `TPM2_PCR_Read` probe.
    Enforcing { active_pcr_banks: u32 },
    /// `BootServiceCapability.present_flag` is zero — the TCG2 DXE is
    /// loaded but reports no TPM behind it (typical on a build where
    /// `Tcg2InstanceLibDTpmSvsm` wasn't wired into the DSC, or where
    /// the SVSM vTPM handshake silently failed during PEI).
    NotPresent,
    /// TPM present but SHA-256 bank not active. qrab-boot extends PCR
    /// 8/9 with the TCG2 protocol, which internally hashes on every
    /// active bank — with no SHA-256 bank the digest qrab-guest-init
    /// later replays would not match.
    NoSha256Bank { active_pcr_banks: u32 },
}

pub fn classify_capability(tpm_present: bool, active_pcr_banks: u32) -> VtpmStatus {
    if !tpm_present {
        return VtpmStatus::NotPresent;
    }
    if active_pcr_banks & HASH_ALG_SHA256 == 0 {
        return VtpmStatus::NoSha256Bank { active_pcr_banks };
    }
    VtpmStatus::Enforcing { active_pcr_banks }
}

/// Distinct error classes from [`parse_pcr_read_response`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// Response buffer shorter than [`PCR_READ_RESPONSE_MIN_LEN`].
    Truncated { got: usize, need: usize },
    /// `responseCode` non-zero. Bubble the raw `TPM_RC` so operators
    /// can look it up in TPM 2.0 Part 2 §6.6.
    TpmRc(u32),
    /// `TPML_DIGEST.count` not 1 — the response structure doesn't
    /// match our fixed request (we asked for exactly one PCR).
    WrongDigestCount(u32),
    /// `digests[0].size` not 32 bytes — expected SHA-256 digest.
    WrongDigestSize(u16),
}

/// Parse a TPM2_PCR_Read response for our fixed [`PCR_READ_PCR0_SHA256_CMD`]
/// request. On success returns the 32-byte SHA-256 PCR 0 digest.
///
/// Callers typically feed [`summarize`] next to turn the digest into a
/// serial-log-friendly prefix (and reject the all-zero case).
pub fn parse_pcr_read_response(bytes: &[u8]) -> Result<[u8; 32], ParseError> {
    const HEADER_LEN: usize = 10; // tag(2) + size(4) + rc(4)
    const UPDATE_COUNTER_LEN: usize = 4;
    // Mirrors the request: count(4) + hash(2) + sizeofSelect(1) + pcrSelect(3).
    const SELECTION_OUT_LEN: usize = 10;
    const DIGEST_COUNT_LEN: usize = 4;
    const DIGEST_SIZE_LEN: usize = 2;
    const DIGEST_BUFFER_LEN: usize = 32;

    if bytes.len() < PCR_READ_RESPONSE_MIN_LEN {
        return Err(ParseError::Truncated {
            got: bytes.len(),
            need: PCR_READ_RESPONSE_MIN_LEN,
        });
    }

    let rc = u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
    if rc != 0 {
        return Err(ParseError::TpmRc(rc));
    }

    const DIGEST_COUNT_OFF: usize = HEADER_LEN + UPDATE_COUNTER_LEN + SELECTION_OUT_LEN;
    let digest_count = u32::from_be_bytes([
        bytes[DIGEST_COUNT_OFF],
        bytes[DIGEST_COUNT_OFF + 1],
        bytes[DIGEST_COUNT_OFF + 2],
        bytes[DIGEST_COUNT_OFF + 3],
    ]);
    if digest_count != 1 {
        return Err(ParseError::WrongDigestCount(digest_count));
    }

    const DIGEST_SIZE_OFF: usize = DIGEST_COUNT_OFF + DIGEST_COUNT_LEN;
    let digest_size = u16::from_be_bytes([bytes[DIGEST_SIZE_OFF], bytes[DIGEST_SIZE_OFF + 1]]);
    if digest_size as usize != DIGEST_BUFFER_LEN {
        return Err(ParseError::WrongDigestSize(digest_size));
    }

    const DIGEST_BUF_OFF: usize = DIGEST_SIZE_OFF + DIGEST_SIZE_LEN;
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes[DIGEST_BUF_OFF..DIGEST_BUF_OFF + DIGEST_BUFFER_LEN]);
    Ok(out)
}

/// Terminal verdict from [`summarize`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VtpmSummary {
    /// Every byte of the digest is zero. A just-booted vTPM that has
    /// not had S-CRTM firmware measurements extended into PCR 0 will
    /// look like this — it is a canary for a broken TCG2 PEI wiring
    /// (either the SVSM vTPM wasn't reachable during PEI, or the
    /// firmware's Tcg2Pei driver failed to run).
    AllZero,
    /// First 8 bytes of the digest, ready to log as hex.
    NonZero { prefix: [u8; 8] },
}

pub fn summarize(digest: &[u8; 32]) -> VtpmSummary {
    if digest.iter().all(|&b| b == 0) {
        return VtpmSummary::AllZero;
    }
    let mut prefix = [0u8; 8];
    prefix.copy_from_slice(&digest[..8]);
    VtpmSummary::NonZero { prefix }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden_response(digest: [u8; 32]) -> [u8; PCR_READ_RESPONSE_MIN_LEN] {
        let mut buf = [0u8; PCR_READ_RESPONSE_MIN_LEN];
        buf[0..2].copy_from_slice(&[0x80, 0x01]);
        buf[2..6].copy_from_slice(&(PCR_READ_RESPONSE_MIN_LEN as u32).to_be_bytes());
        buf[6..10].copy_from_slice(&0u32.to_be_bytes());
        buf[10..14].copy_from_slice(&42u32.to_be_bytes());
        buf[14..18].copy_from_slice(&1u32.to_be_bytes());
        buf[18..20].copy_from_slice(&[0x00, 0x0B]);
        buf[20] = 0x03;
        buf[21..24].copy_from_slice(&[0x01, 0x00, 0x00]);
        buf[24..28].copy_from_slice(&1u32.to_be_bytes());
        buf[28..30].copy_from_slice(&32u16.to_be_bytes());
        buf[30..62].copy_from_slice(&digest);
        buf
    }

    #[test]
    fn classify_enforcing_happy_path() {
        let status = classify_capability(true, HASH_ALG_SHA256 | 0x1);
        assert_eq!(
            status,
            VtpmStatus::Enforcing {
                active_pcr_banks: HASH_ALG_SHA256 | 0x1
            }
        );
    }

    #[test]
    fn classify_not_present_when_flag_clear() {
        assert_eq!(
            classify_capability(false, HASH_ALG_SHA256),
            VtpmStatus::NotPresent
        );
    }

    #[test]
    fn classify_no_sha256_bank() {
        // SHA-1 (0x1) active, SHA-256 bit clear.
        assert_eq!(
            classify_capability(true, 0x1),
            VtpmStatus::NoSha256Bank {
                active_pcr_banks: 0x1
            }
        );
    }

    #[test]
    fn pcr_read_cmd_matches_tpm_spec_bytes() {
        // Locks the 20-byte command buffer. A regression here means
        // the TPM wire format changed unexpectedly; review before
        // bumping the golden bytes.
        assert_eq!(PCR_READ_PCR0_SHA256_CMD.len(), 20);
        // commandSize field encodes 20.
        let cmd_size = u32::from_be_bytes([
            PCR_READ_PCR0_SHA256_CMD[2],
            PCR_READ_PCR0_SHA256_CMD[3],
            PCR_READ_PCR0_SHA256_CMD[4],
            PCR_READ_PCR0_SHA256_CMD[5],
        ]);
        assert_eq!(cmd_size as usize, PCR_READ_PCR0_SHA256_CMD.len());
        // commandCode field is TPM_CC_PCR_Read = 0x17E.
        let cc = u32::from_be_bytes([
            PCR_READ_PCR0_SHA256_CMD[6],
            PCR_READ_PCR0_SHA256_CMD[7],
            PCR_READ_PCR0_SHA256_CMD[8],
            PCR_READ_PCR0_SHA256_CMD[9],
        ]);
        assert_eq!(cc, 0x0000_017E);
    }

    #[test]
    fn parse_response_round_trip() {
        let mut want = [0u8; 32];
        for (i, b) in want.iter_mut().enumerate() {
            *b = i as u8;
        }
        let buf = golden_response(want);
        assert_eq!(parse_pcr_read_response(&buf).unwrap(), want);
    }

    #[test]
    fn parse_response_truncated() {
        let buf = [0u8; PCR_READ_RESPONSE_MIN_LEN - 1];
        assert_eq!(
            parse_pcr_read_response(&buf),
            Err(ParseError::Truncated {
                got: PCR_READ_RESPONSE_MIN_LEN - 1,
                need: PCR_READ_RESPONSE_MIN_LEN,
            })
        );
    }

    #[test]
    fn parse_response_tpm_rc_surfaces() {
        let mut buf = golden_response([1u8; 32]);
        // TPM_RC_FAILURE = 0x101.
        buf[6..10].copy_from_slice(&0x0000_0101u32.to_be_bytes());
        assert_eq!(
            parse_pcr_read_response(&buf),
            Err(ParseError::TpmRc(0x0000_0101))
        );
    }

    #[test]
    fn parse_response_rejects_wrong_digest_count() {
        let mut buf = golden_response([1u8; 32]);
        buf[24..28].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(
            parse_pcr_read_response(&buf),
            Err(ParseError::WrongDigestCount(0))
        );
    }

    #[test]
    fn parse_response_rejects_wrong_digest_size() {
        let mut buf = golden_response([1u8; 32]);
        // Pretend server returned a SHA-1 (20 bytes) digest.
        buf[28..30].copy_from_slice(&20u16.to_be_bytes());
        assert_eq!(
            parse_pcr_read_response(&buf),
            Err(ParseError::WrongDigestSize(20))
        );
    }

    #[test]
    fn summarize_all_zero_rejected() {
        assert_eq!(summarize(&[0u8; 32]), VtpmSummary::AllZero);
    }

    #[test]
    fn summarize_non_zero_extracts_prefix() {
        let mut d = [0u8; 32];
        d[0] = 0xde;
        d[1] = 0xad;
        d[2] = 0xbe;
        d[3] = 0xef;
        d[4] = 0x00;
        d[5] = 0x11;
        d[6] = 0x22;
        d[7] = 0x33;
        assert_eq!(
            summarize(&d),
            VtpmSummary::NonZero {
                prefix: [0xde, 0xad, 0xbe, 0xef, 0x00, 0x11, 0x22, 0x33]
            }
        );
    }

    #[test]
    fn summarize_non_zero_late_byte_still_detected() {
        let mut d = [0u8; 32];
        d[31] = 1;
        assert_eq!(
            summarize(&d),
            VtpmSummary::NonZero {
                prefix: [0, 0, 0, 0, 0, 0, 0, 0]
            }
        );
    }

    // -- tokens + fail-suffix regression guardrails --------------------

    #[test]
    fn tokens_locked_against_rename() {
        // If any of these equalities change, every contrib/smoke-*.sh
        // grep pattern needs to be updated in the same commit.
        assert_eq!(tokens::VTPM_SVSM_OK, "QRAB_VTPM_SVSM_OK");
        assert_eq!(tokens::VTPM_CAP, "QRAB_VTPM_CAP");
        assert_eq!(tokens::FAIL_NO_VTPM, "QRAB_FAIL_NO_VTPM");
        assert_eq!(tokens::TCG2_SOFT_ABSENT, "QRAB_TCG2_SOFT_ABSENT");
        assert_eq!(tokens::ALL.len(), 4);
        for t in tokens::ALL {
            assert!(t.starts_with("QRAB_"), "token `{t}` lost QRAB_ prefix");
        }
    }

    #[test]
    fn fail_suffixes_are_unique() {
        // Grep on serial expects each probe failure to carry a single,
        // unambiguous suffix — no overlap, no prefix collisions.
        let all = fail_suffix::ALL;
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b, "duplicate suffix `{a}`");
                // Also guard against prefix collisions like `PCR_READ`
                // / `PCR_READ_X` that would make grep ambiguous.
                assert!(!a.starts_with(b), "`{a}` is a superstring of `{b}`");
                assert!(!b.starts_with(a), "`{b}` is a superstring of `{a}`");
            }
            assert!(!a.is_empty());
        }
        assert_eq!(all.len(), 8);
    }

    #[test]
    fn every_fail_class_has_a_distinct_suffix() {
        // Every failure class pinned above must be referenced in the
        // `VtpmProbeError::Display` impl under `qrab-boot::measure`.
        // If a variant is added / renamed but the impl is forgotten,
        // this grep test fails before any live smoke does.
        let measure = include_str!("../../qrab-boot/src/measure.rs");
        for suffix in fail_suffix::ALL {
            assert!(
                measure.contains(suffix),
                "fail-suffix `{suffix}` missing from qrab-boot/src/measure.rs — \
                 did someone rename a `VtpmProbeError` variant's Display form?"
            );
        }
    }

    #[test]
    fn measure_rs_uses_tokens_verbatim() {
        // Keeps the `log::info!("QRAB_VTPM_SVSM_OK ...")` / `log::warn!(
        // "QRAB_FAIL_NO_VTPM ...")` format-string literals in the
        // bootloader source synced with the `tokens::*` consts that
        // every smoke greps. `include_str!` hard-fails at compile time
        // if the source path drifts, which is also what we want.
        //
        // Per-file `contains` avoids heap allocation so this test stays
        // no_std-clean under the default (featureless) build.
        let measure = include_str!("../../qrab-boot/src/measure.rs");
        let main = include_str!("../../qrab-boot/src/main.rs");
        for tok in tokens::ALL {
            assert!(
                measure.contains(tok) || main.contains(tok),
                "token `{tok}` missing from qrab-boot source — \
                 did someone rename it in `measure.rs` / `main.rs` \
                 without updating `tokens::*`?"
            );
        }
    }
}
