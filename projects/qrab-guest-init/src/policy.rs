//! Workload-specific whitelists + report_data reconstruct.
//!
//! Drives L5 steps 15, 16, 17, and 18 of
//! `docs/security/level-5-init.md`. The whitelists live in a
//! declarative [`Policy`] struct so tests can dial in expected
//! measurements / TCB floors without touching the sequencer.
//! Production boots populate the policy at compile time (or from a
//! locked-down build-time file) — a dynamic policy source is
//! explicitly out of scope for phase 5.
//!
//! No serde, no `toml`, no `thiserror`. The L5 threat model says a
//! policy that can be rewritten from the guest rootfs at runtime is
//! not a policy; we keep the surface plain enough that future phases
//! can bake the values straight into the UKI.
//!
//! The `check_*` functions deliberately return `Result<(), ()>`:
//! every failure maps to one fixed, spec-pinned token
//! (`FailMeasurement` / `FailPolicy` / `FailTcb`) at the sequencer
//! call-site. Carrying a richer error surface here would just get
//! discarded by the sequencer's `.map_err(|_| Token::…)` wrap and
//! give an attacker a finer-grained oracle into which field
//! rejected them — exactly the ambiguity the L5 spec sets out to
//! collapse.
#![allow(clippy::result_unit_err)]

use sha2::{Digest, Sha256};

use qrab_boot_proto::PROTO_TAG;

/// The VMPL the guest OS expects to be running at. qrab-guest-init
/// requires it to be 0 so the SVSM-served PSP report is authoritative.
///
/// VMPL > 0 would mean a lower-privileged paravisor issued the
/// report, which is exactly the state a malicious SVSM replacement
/// would try to forge into.
pub const EXPECTED_VMPL: u32 = 0;

/// One entry in the Phase 2c per-PCR workload whitelist.
///
/// `pcr_index` indexes a SHA-256 bank slot in the `[[u8; 32]; 24]`
/// array returned by [`crate::pcr::read_sha256_bank`]. `expected` is
/// the digest the live TPM must read back as — i.e. the post-replay
/// state after every event-log extend has been folded in. The
/// whitelist runs AFTER step 12's replay-vs-TPM compare, so a
/// matching entry here is a double assertion: the event log extends
/// to `expected`, AND the live TPM agrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcrWhitelistEntry {
    pub pcr_index: u32,
    pub expected: [u8; 32],
}

/// Workload policy — the whitelist of acceptable report fields.
/// Populated by the binary (`main.rs`) with values baked into the
/// UKI at build time.
#[derive(Debug, Clone)]
pub struct Policy {
    /// Acceptable SNP measurement digests (48 B each). An empty set
    /// is explicitly refused — production MUST pin at least one.
    /// A future "panic if empty at policy-construction" check can
    /// harden this further, but for now the check lives in
    /// [`Policy::check_measurement`] so tests can round-trip an
    /// empty policy through the constructor and exercise the
    /// rejection path.
    pub allowed_measurements: Vec<[u8; 48]>,
    /// Mask of bits that MUST be set in `SnpAttestationReport.policy`.
    /// The report's `policy` field is a 64-bit bitmask; a report
    /// passes if `report.policy & required_policy_mask ==
    /// required_policy_mask`.
    pub required_policy_mask: u64,
    /// Mask of bits that MUST NOT be set. A report passes if
    /// `report.policy & forbidden_policy_mask == 0`. Kept as a
    /// separate field so the common case (require a small set, allow
    /// everything else) doesn't require listing every forbidden bit.
    pub forbidden_policy_mask: u64,
    /// Minimum `reported_tcb` as a packed u64 per the SNP ABI
    /// (committed_microcode/snp/tee/bootloader). Any report with
    /// `reported_tcb < min_tcb` is rejected.
    ///
    /// AMD packs the four octets at bits [63:56] / [47:40] / [39:32] /
    /// [7:0] respectively; lexicographic comparison on the packed u64
    /// therefore matches "newer or equal TCB" so long as each field
    /// stays within its byte.
    pub min_tcb: u64,
    /// Per-PCR whitelist. Each entry pins the SHA-256 digest the
    /// guest's live TPM PCR must read back as. Production pins at
    /// least PCRs 0, 4, 7, 8, 9: UEFI secure-boot evidence (0, 7),
    /// bootloader code (4), UKI measurement (8), secret commitment
    /// (9). An empty `Vec` is allowed (used by some unit tests /
    /// dev builds) but production MUST pin at least one.
    pub allowed_pcrs: Vec<PcrWhitelistEntry>,
}

impl Policy {
    /// Accept any measurement that appears verbatim in
    /// [`Policy::allowed_measurements`]. Rejects an empty whitelist
    /// outright — production MUST pin at least one digest, otherwise
    /// the measurement check is a no-op and the gate trivially
    /// passes.
    pub fn check_measurement(&self, m: &[u8; 48]) -> Result<(), ()> {
        if self.allowed_measurements.is_empty() {
            return Err(());
        }
        if self.allowed_measurements.iter().any(|allowed| allowed == m) {
            Ok(())
        } else {
            Err(())
        }
    }

    /// Accept a report policy bitmap iff every required bit is set
    /// and no forbidden bit is. A crafted `policy` field that sets
    /// ONLY required bits still fails if `min_tcb` or the
    /// measurement check rejects the surrounding report — each
    /// check is independent.
    pub fn check_policy_bits(&self, p: u64) -> Result<(), ()> {
        if (p & self.required_policy_mask) != self.required_policy_mask {
            return Err(());
        }
        if (p & self.forbidden_policy_mask) != 0 {
            return Err(());
        }
        Ok(())
    }

    /// Accept any reported TCB at or above [`Policy::min_tcb`].
    /// Strict less-than rejection — equality passes.
    pub fn check_tcb(&self, t: u64) -> Result<(), ()> {
        if t < self.min_tcb {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Check every [`PcrWhitelistEntry`] against the live TPM SHA-256
    /// bank. Each entry pins `read[pcr_index] == expected`.
    ///
    /// An empty whitelist is allowed — this is used by some tests
    /// and the dev build has no workload-specific PCR pins yet.
    /// Production is expected to pin at least PCRs 0, 4, 7, 8, 9.
    ///
    /// Returns `Err(pcr_index)` on the first mismatching entry so
    /// the sequencer can emit `Token::FailPolicyPcr` and the
    /// post-mortem log line carries the diverging index. Iteration
    /// order follows `allowed_pcrs`'s declaration order — callers
    /// who want deterministic "lowest-index wins" should sort the
    /// vector at construction.
    pub fn check_pcr_whitelist(&self, read: &[[u8; 32]; 24]) -> Result<(), u32> {
        for entry in &self.allowed_pcrs {
            let idx = entry.pcr_index as usize;
            if idx >= read.len() {
                // An entry pinning a PCR the kernel bank doesn't
                // expose is itself a policy error. Return the
                // stated index so the forensics log shows which
                // malformed entry tripped.
                return Err(entry.pcr_index);
            }
            if read[idx] != entry.expected {
                return Err(entry.pcr_index);
            }
        }
        Ok(())
    }
}

/// Reconstruct the 32-byte `report_data` prefix the server bound
/// into the PSP report, per [`qrab_boot_proto::report_data`]. Uses
/// [`qrab_boot_proto::PROTO_TAG`] so any future PROTO_TAG bump can't
/// silently drift between boot and init.
///
/// The bootloader side computes
/// `sha256(PROTO_TAG || client_nonce || server_nonce || uki_sha256
///         || secret_sha256)` and the PSP signs it inside the 64-byte
/// `report.report_data` field (padded to 64 B with zeros). L5 replays
/// the hash over the exact same domain and compares against the
/// first 32 bytes of `report.report_data`.
pub fn reconstruct_report_data(
    client_nonce: &[u8; 32],
    server_nonce: &[u8; 32],
    uki_sha256: &[u8; 32],
    secret_sha256: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PROTO_TAG);
    hasher.update(client_nonce);
    hasher.update(server_nonce);
    hasher.update(uki_sha256);
    hasher.update(secret_sha256);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// Session file layout: 64 raw bytes = `client_nonce || server_nonce`.
/// Dracut hook (contrib/qrab-boot-initrd-module/) writes this from
/// the same two fields qrab-boot published to the guest. Binary
/// instead of hex/PEM because the dracut hook is a minimal shell
/// script and we don't want to require hex-decode tooling.
pub fn parse_session(raw: &[u8]) -> Result<([u8; 32], [u8; 32]), SessionError> {
    if raw.len() != 64 {
        return Err(SessionError::WrongLength(raw.len()));
    }
    let mut client_nonce = [0u8; 32];
    let mut server_nonce = [0u8; 32];
    client_nonce.copy_from_slice(&raw[..32]);
    server_nonce.copy_from_slice(&raw[32..]);
    Ok((client_nonce, server_nonce))
}

/// Errors surfaced by [`parse_session`].
#[derive(Debug, PartialEq, Eq)]
pub enum SessionError {
    /// `/run/qrab/session` was not exactly 64 bytes. Carries the
    /// observed length for forensics — the dracut hook is expected
    /// to always produce exactly 64 bytes; any drift is a bug there
    /// or a tampering attempt.
    WrongLength(usize),
}

impl core::fmt::Display for SessionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::WrongLength(n) => {
                write!(f, "/run/qrab/session length {n} != 64")
            }
        }
    }
}

impl std::error::Error for SessionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use qrab_boot_proto::{AttestResponse, report_data};

    const M1: [u8; 48] = [0x11; 48];
    const M2: [u8; 48] = [0x22; 48];
    const M3: [u8; 48] = [0x33; 48];

    fn base_policy() -> Policy {
        Policy {
            allowed_measurements: vec![M1, M2],
            required_policy_mask: 0b0000_0111,
            forbidden_policy_mask: 0b1000_0000,
            min_tcb: 0x0000_0003_0002_0001,
            allowed_pcrs: Vec::new(),
        }
    }

    #[test]
    fn expected_vmpl_is_zero() {
        // Explicit constant lock so a future refactor that bumps the
        // expected VMPL has to update this test first.
        assert_eq!(EXPECTED_VMPL, 0);
    }

    #[test]
    fn check_measurement_accepts_allowed() {
        let p = base_policy();
        assert_eq!(p.check_measurement(&M1), Ok(()));
        assert_eq!(p.check_measurement(&M2), Ok(()));
    }

    #[test]
    fn check_measurement_rejects_not_in_whitelist() {
        let p = base_policy();
        assert_eq!(p.check_measurement(&M3), Err(()));
    }

    #[test]
    fn check_measurement_rejects_empty_whitelist() {
        // Production MUST pin at least one. An empty whitelist is
        // explicitly a configuration error, not a "match anything"
        // wildcard.
        let mut p = base_policy();
        p.allowed_measurements.clear();
        assert_eq!(p.check_measurement(&M1), Err(()));
    }

    #[test]
    fn check_policy_bits_accepts_superset() {
        let p = base_policy();
        // Required = 0b0111. A report that also sets bit 3 (not
        // forbidden) passes.
        assert_eq!(p.check_policy_bits(0b0000_1111), Ok(()));
        // Exactly-required bits pass.
        assert_eq!(p.check_policy_bits(0b0000_0111), Ok(()));
    }

    #[test]
    fn check_policy_bits_rejects_missing_required() {
        let p = base_policy();
        // Dropping any required bit fails.
        assert_eq!(p.check_policy_bits(0b0000_0011), Err(()));
        assert_eq!(p.check_policy_bits(0b0000_0110), Err(()));
    }

    #[test]
    fn check_policy_bits_rejects_forbidden() {
        let p = base_policy();
        // All required bits set, but bit 7 is forbidden.
        assert_eq!(p.check_policy_bits(0b1000_0111), Err(()));
    }

    #[test]
    fn check_tcb_edges() {
        let p = base_policy();
        assert_eq!(p.check_tcb(p.min_tcb - 1), Err(()));
        assert_eq!(p.check_tcb(p.min_tcb), Ok(()));
        assert_eq!(p.check_tcb(p.min_tcb + 1), Ok(()));
    }

    #[test]
    fn check_tcb_rejects_zero_when_floor_nonzero() {
        let p = base_policy();
        assert_eq!(p.check_tcb(0), Err(()));
    }

    #[test]
    fn check_tcb_accepts_any_when_floor_zero() {
        // min_tcb = 0 collapses the check to a no-op. Exercised so a
        // test-only Policy that doesn't care about TCB freshness
        // compiles without special casing.
        let mut p = base_policy();
        p.min_tcb = 0;
        assert_eq!(p.check_tcb(0), Ok(()));
        assert_eq!(p.check_tcb(u64::MAX), Ok(()));
    }

    #[test]
    fn reconstruct_matches_qrab_boot_proto_report_data() {
        // Cross-check: our reconstruct and
        // `qrab_boot_proto::report_data` must produce the same first
        // 32 bytes over identical inputs. Pin the domain exactly so
        // a future proto bump on either side can't quietly drift.
        let client_nonce = [0x10u8; 32];
        let server_nonce = [0x20u8; 32];
        let uki_sha256 = [0x30u8; 32];
        let secret_sha256 = [0x40u8; 32];
        let resp = AttestResponse {
            server_nonce,
            uki_url: String::from("vsock://2:1337/fixture"),
            uki_sha256,
            secret_sha256,
            sig: vec![0u8; qrab_boot_proto::SIG_LEN],
        };
        let boot_proto_output = report_data(&client_nonce, &resp);
        let mine = reconstruct_report_data(
            &client_nonce,
            &server_nonce,
            &uki_sha256,
            &secret_sha256,
        );
        // The boot-side returns a 64-byte buffer (SHA-256 || 32 zero
        // bytes); ours returns only the 32-byte digest prefix.
        assert_eq!(&boot_proto_output[..32], &mine[..]);
        assert!(boot_proto_output[32..].iter().all(|&b| b == 0));
    }

    #[test]
    fn reconstruct_is_stable() {
        // Deterministic: same inputs → same output. A flaky hash
        // would break the sequencer's report_data compare on every
        // reboot.
        let cn = [0u8; 32];
        let sn = [0u8; 32];
        let uh = [0u8; 32];
        let sh = [0u8; 32];
        assert_eq!(
            reconstruct_report_data(&cn, &sn, &uh, &sh),
            reconstruct_report_data(&cn, &sn, &uh, &sh),
        );
    }

    #[test]
    fn reconstruct_changes_with_each_input() {
        let cn = [0u8; 32];
        let sn = [1u8; 32];
        let uh = [2u8; 32];
        let sh = [3u8; 32];
        let base = reconstruct_report_data(&cn, &sn, &uh, &sh);

        let mut cn2 = cn;
        cn2[0] ^= 1;
        assert_ne!(base, reconstruct_report_data(&cn2, &sn, &uh, &sh));

        let mut sn2 = sn;
        sn2[0] ^= 1;
        assert_ne!(base, reconstruct_report_data(&cn, &sn2, &uh, &sh));

        let mut uh2 = uh;
        uh2[0] ^= 1;
        assert_ne!(base, reconstruct_report_data(&cn, &sn, &uh2, &sh));

        let mut sh2 = sh;
        sh2[0] ^= 1;
        assert_ne!(base, reconstruct_report_data(&cn, &sn, &uh, &sh2));
    }

    #[test]
    fn parse_session_happy() {
        let mut raw = Vec::with_capacity(64);
        raw.extend_from_slice(&[0xAAu8; 32]);
        raw.extend_from_slice(&[0xBBu8; 32]);
        let (cn, sn) = parse_session(&raw).expect("happy session parses");
        assert_eq!(cn, [0xAAu8; 32]);
        assert_eq!(sn, [0xBBu8; 32]);
    }

    #[test]
    fn parse_session_wrong_length_too_short() {
        let raw = vec![0u8; 63];
        assert_eq!(parse_session(&raw), Err(SessionError::WrongLength(63)));
    }

    #[test]
    fn parse_session_wrong_length_too_long() {
        let raw = vec![0u8; 65];
        assert_eq!(parse_session(&raw), Err(SessionError::WrongLength(65)));
    }

    #[test]
    fn parse_session_empty() {
        // Zero bytes is still a length error, not a panic.
        assert_eq!(parse_session(&[]), Err(SessionError::WrongLength(0)));
    }

    #[test]
    fn session_error_display_is_nonempty() {
        let s = format!("{}", SessionError::WrongLength(42));
        assert!(!s.is_empty());
        assert!(s.contains("42"));
    }

    // ------ Phase 2c: per-PCR whitelist ------

    /// Build a bank with every slot zero except where overridden
    /// by the caller.
    fn zero_bank() -> [[u8; 32]; 24] {
        [[0u8; 32]; 24]
    }

    #[test]
    fn check_pcr_whitelist_empty_accepts_anything() {
        // An empty whitelist is explicitly allowed — dev builds use
        // this to skip the workload-specific check. Production
        // pins at least PCRs 0, 4, 7, 8, 9.
        let p = base_policy();
        assert!(p.allowed_pcrs.is_empty());
        assert_eq!(p.check_pcr_whitelist(&zero_bank()), Ok(()));
        // Even a bank with arbitrary non-zero contents still passes
        // an empty whitelist — the check is an allowlist, not a
        // deny-by-default.
        let mut b = zero_bank();
        b.fill([0xAA; 32]);
        assert_eq!(p.check_pcr_whitelist(&b), Ok(()));
    }

    #[test]
    fn check_pcr_whitelist_single_entry_matching() {
        let mut p = base_policy();
        p.allowed_pcrs = vec![PcrWhitelistEntry {
            pcr_index: 7,
            expected: [0x77; 32],
        }];
        let mut b = zero_bank();
        b[7] = [0x77; 32];
        assert_eq!(p.check_pcr_whitelist(&b), Ok(()));
    }

    #[test]
    fn check_pcr_whitelist_single_entry_mismatching() {
        let mut p = base_policy();
        p.allowed_pcrs = vec![PcrWhitelistEntry {
            pcr_index: 7,
            expected: [0x77; 32],
        }];
        // Bank carries a different PCR 7.
        let mut b = zero_bank();
        b[7] = [0x99; 32];
        assert_eq!(p.check_pcr_whitelist(&b), Err(7));
    }

    #[test]
    fn check_pcr_whitelist_multiple_entries_all_matching() {
        let mut p = base_policy();
        p.allowed_pcrs = vec![
            PcrWhitelistEntry {
                pcr_index: 0,
                expected: [0x00; 32],
            },
            PcrWhitelistEntry {
                pcr_index: 4,
                expected: [0x44; 32],
            },
            PcrWhitelistEntry {
                pcr_index: 7,
                expected: [0x77; 32],
            },
            PcrWhitelistEntry {
                pcr_index: 8,
                expected: [0x88; 32],
            },
            PcrWhitelistEntry {
                pcr_index: 9,
                expected: [0x99; 32],
            },
        ];
        let mut b = zero_bank();
        b[0] = [0x00; 32];
        b[4] = [0x44; 32];
        b[7] = [0x77; 32];
        b[8] = [0x88; 32];
        b[9] = [0x99; 32];
        assert_eq!(p.check_pcr_whitelist(&b), Ok(()));
    }

    #[test]
    fn check_pcr_whitelist_first_mismatch_short_circuits() {
        // Two diverging PCRs — 4 listed BEFORE 8 in allowed_pcrs —
        // should surface PCR 4 (the first mismatch by declaration
        // order).
        let mut p = base_policy();
        p.allowed_pcrs = vec![
            PcrWhitelistEntry {
                pcr_index: 4,
                expected: [0x44; 32],
            },
            PcrWhitelistEntry {
                pcr_index: 8,
                expected: [0x88; 32],
            },
        ];
        let b = zero_bank(); // Both PCRs are zero, neither matches.
        assert_eq!(p.check_pcr_whitelist(&b), Err(4));
    }

    #[test]
    fn check_pcr_whitelist_out_of_range_index_rejected() {
        // PCR 24 doesn't exist in a TPM 2.0 bank array of length 24.
        // An entry pinning a bogus index is itself a policy error;
        // the function must reject rather than panic on the
        // out-of-bounds access.
        let mut p = base_policy();
        p.allowed_pcrs = vec![PcrWhitelistEntry {
            pcr_index: 24,
            expected: [0u8; 32],
        }];
        assert_eq!(p.check_pcr_whitelist(&zero_bank()), Err(24));
    }
}
