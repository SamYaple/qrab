//! Straight-line policy gate from `docs/security/level-5-init.md`.
//!
//! Phase 2a scope: steps 1-12. Adds event-log parse + SHA-256 replay
//! + TPM 2.0 PCR read + per-PCR compare on top of the Phase 1 checks.
//!
//! Steps 13-20 stay deferred to Phase 2b.
//!
//! Real-boot wiring lives behind the [`Probe`] trait so integration
//! tests can drive the whole ladder with fixed strings/bytes. The
//! TPM handle is a `Box<dyn Read+Write+Send>` — we can't hold a
//! non-dyn generic on the trait without infecting every caller with a
//! type parameter, and the indirection is fine at steady state (one
//! call, one round-trip, done). `Send` is added only to keep the
//! trait object usable from any thread; the trait itself isn't
//! `async` and the real binary runs it single-threaded.

use crate::efivar;
use crate::eventlog;
use crate::ima;
use crate::lockdown;
use crate::log::TokenSink;
use crate::modsig;
use crate::msr;
use crate::pcr;
use crate::tokens::Token;
use crate::verity;

/// Composite trait for TPM device handles. Rust only permits one
/// non-auto trait in a `dyn` bound, so we conjoin `Read + Write` into
/// a single supertrait and let a blanket impl cover anything that
/// satisfies both. `Send` is an auto trait so it rides along cleanly.
pub trait TpmDevice: std::io::Read + std::io::Write + Send {}

impl<T: std::io::Read + std::io::Write + Send> TpmDevice for T {}

/// Boot-time sources the sequencer reads. Every method returns the
/// raw file contents (or MSR 64-bit value) — policy decoding lives
/// in the per-module check functions.
pub trait Probe {
    fn read_sev_status_msr(&self) -> std::io::Result<u64>;
    fn read_secure_boot(&self) -> std::io::Result<Vec<u8>>;
    fn read_deployed_mode(&self) -> std::io::Result<Vec<u8>>;
    fn read_setup_mode(&self) -> std::io::Result<Vec<u8>>;
    fn read_lockdown(&self) -> std::io::Result<String>;
    fn read_sig_enforce(&self) -> std::io::Result<String>;
    fn read_ima_policy(&self) -> std::io::Result<String>;
    fn read_mountinfo(&self) -> std::io::Result<String>;
    /// Looks up the dm uuid for the `/` mount's source device.
    /// Implementations resolve `/dev/mapper/<N>` or `/dev/dm-<N>`
    /// and read `/sys/block/dm-<N>/dm/uuid`.
    fn read_root_dm_uuid(&self) -> std::io::Result<String>;
    /// Raw TCG 2.0 crypto-agile event-log bytes — normally
    /// `/sys/kernel/security/tpm0/binary_bios_measurements`.
    fn read_eventlog(&self) -> std::io::Result<Vec<u8>>;
    /// Open (or fabricate) a handle on the TPM 2.0 resource manager,
    /// `/dev/tpmrm0`. Returned as a boxed [`TpmDevice`] trait object so
    /// a unit test can substitute a `Vec<u8>`-backed fake without
    /// forcing a type parameter onto the rest of the probe surface.
    fn open_tpm(&self) -> std::io::Result<Box<dyn TpmDevice>>;
}

/// Phase 2a sequencer. Runs steps 1-12, emits [`Token::Ok`] if they
/// all pass. Any failure collapses to a single fatal token — the
/// caller (bin) emits it and halts.
///
/// Tests drive this through a [`Probe`] stub and a capturing sink;
/// the binary wires a real probe reading `/dev/cpu/0/msr`, efivarfs,
/// `/sys/kernel/security/*`, `/sys/kernel/security/tpm0/…`, and
/// `/dev/tpmrm0`.
pub fn run_gate<P, S>(probe: &P, sink: &mut S) -> Result<(), Token>
where
    P: Probe,
    S: TokenSink,
{
    // Step 1: beacon.
    let _ = sink.emit(Token::Start);

    // Step 2: SEV status MSR bits 0|1|2.
    let msr_val = probe.read_sev_status_msr().map_err(|_| Token::FailNoSnp)?;
    msr::check_sev_status(msr_val)?;

    // Step 3: Secure Boot enabled.
    let sb = probe.read_secure_boot().map_err(|_| Token::FailSbOff)?;
    efivar::check_secure_boot(&sb)?;

    // Step 4: Deployed=1, Setup=0.
    let dep = probe.read_deployed_mode().map_err(|_| Token::FailSbSetup)?;
    let setup = probe.read_setup_mode().map_err(|_| Token::FailSbSetup)?;
    efivar::check_deployed_and_setup_mode(&dep, &setup)?;

    // Step 5: lockdown >= integrity.
    let lockdown = probe.read_lockdown().map_err(|_| Token::FailLockdown)?;
    lockdown::check_lockdown(&lockdown)?;

    // Step 6: module.sig_enforce == Y.
    let sig = probe.read_sig_enforce().map_err(|_| Token::FailModsig)?;
    modsig::check_sig_enforce(&sig)?;

    // Step 7: IMA policy loaded with appraise.
    let ima = probe.read_ima_policy().map_err(|_| Token::FailImaPolicy)?;
    ima::check_ima_policy(&ima)?;

    // Step 8: dm-verity on /.
    let mi = probe.read_mountinfo().map_err(|_| Token::FailVerity)?;
    let uuid = probe.read_root_dm_uuid().map_err(|_| Token::FailVerity)?;
    verity::check_root_verity(&mi, &uuid)?;

    // Step 9: parse the TCG 2.0 crypto-agile event log.
    let log_bytes = probe.read_eventlog().map_err(|_| Token::FailEventlogParse)?;
    let events = eventlog::parse(&log_bytes).map_err(|_| Token::FailEventlogParse)?;

    // Step 10: replay the SHA-256 bank from the log (pure function,
    // no failure mode of its own).
    let expected = eventlog::replay_sha256(&events, 9);

    // Step 11: read the live SHA-256 bank from /dev/tpmrm0.
    //
    // A TPM I/O error isn't strictly a "wrong PCR" — but the spec
    // deliberately routes every PCR-chain failure through a single
    // FailPcr token family so attackers can't narrow the failure mode
    // by provoking specific errors. Pick PCR 0 as the deterministic
    // index for TPM-unreachable so the test story is predictable; any
    // real such failure means the attestation chain is already dead.
    let mut tpm = probe.open_tpm().map_err(|_| Token::FailPcr(0))?;
    let read = pcr::read_sha256_bank(tpm.as_mut(), 9).map_err(|_| Token::FailPcr(0))?;

    // Step 12: compare expected vs. read for every PCR in 0..=9. The
    // first mismatch wins — emit its PCR index so the failure token
    // carries enough forensic data for post-mortem.
    for n in 0u32..=9 {
        if expected[n as usize] != read[n as usize] {
            return Err(Token::FailPcr(n as u8));
        }
    }

    // Steps 13-20 remain Phase 2b:
    //  13-18  report::parse(/run/qrab/report) + report_data reconstruct
    //    19  secret::consume(/run/qrab/secret) + memfd handoff
    //    20  policy::check_pcr_whitelist(pcrs)

    let _ = sink.emit(Token::Ok);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventlog::{SPEC_ID_SIGNATURE, TPM_ALG_SHA256};
    use crate::log::CaptureSink;
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};

    /// Stub probe where every accessor returns a passing fixture by
    /// default. Individual tests mutate one field to exercise a
    /// specific failure mode.
    struct StubProbe {
        sev_status: std::io::Result<u64>,
        secure_boot: std::io::Result<Vec<u8>>,
        deployed: std::io::Result<Vec<u8>>,
        setup: std::io::Result<Vec<u8>>,
        lockdown: std::io::Result<String>,
        sig_enforce: std::io::Result<String>,
        ima: std::io::Result<String>,
        mountinfo: std::io::Result<String>,
        dm_uuid: std::io::Result<String>,
        eventlog: std::io::Result<Vec<u8>>,
        /// Wrapped in a `std::sync::Mutex<Option<…>>` so we can hand
        /// ownership to the returned trait object without taking
        /// `&mut self` on `open_tpm` (the trait is `&self`-only).
        tpm_response: std::sync::Mutex<Option<Vec<u8>>>,
        tpm_open_err: Option<std::io::ErrorKind>,
    }

    /// Boxed fake TPM. Writes are swallowed; reads drain `response`
    /// until empty then return 0.
    struct StubTpm {
        response: Vec<u8>,
    }

    impl Read for StubTpm {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = std::cmp::min(buf.len(), self.response.len());
            buf[..n].copy_from_slice(&self.response[..n]);
            self.response.drain(..n);
            Ok(n)
        }
    }

    impl Write for StubTpm {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Produce a minimal spec-id-only TCG 2.0 log.
    fn spec_id_only_log() -> Vec<u8> {
        let mut out = Vec::new();
        // legacy TCG_PCR_EVENT
        out.extend_from_slice(&0u32.to_le_bytes()); // pcr
        out.extend_from_slice(&0x0000_0003u32.to_le_bytes()); // event type
        out.extend_from_slice(&[0u8; 20]); // sha1 digest
        let mut data = Vec::new();
        data.extend_from_slice(SPEC_ID_SIGNATURE);
        data.extend_from_slice(&0u32.to_le_bytes()); // platformClass
        data.push(0); // minor
        data.push(2); // major
        data.push(0);
        data.push(2);
        data.extend_from_slice(&1u32.to_le_bytes()); // one alg
        data.extend_from_slice(&TPM_ALG_SHA256.to_le_bytes());
        data.extend_from_slice(&32u16.to_le_bytes());
        data.push(0); // vendorInfoSize
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    /// Produce a log that extends PCR 4 by a single SHA-256 digest.
    fn log_extending_pcr4(event_digest: [u8; 32]) -> Vec<u8> {
        let mut out = spec_id_only_log();
        // TCG_PCR_EVENT2: pcr=4, eventType=5, count=1, alg=SHA256,
        //   digest(32), eventSize=0, no eventData.
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(&5u32.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&TPM_ALG_SHA256.to_le_bytes());
        out.extend_from_slice(&event_digest);
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    /// Build a TPM2_PCR_Read response that returns the passed banks
    /// for PCRs 0..=9.
    fn tpm_response_for_bank(banks: [[u8; 32]; 10]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0x8001u16.to_be_bytes()); // tag
        out.extend_from_slice(&0u32.to_be_bytes()); // size placeholder
        out.extend_from_slice(&0u32.to_be_bytes()); // rc
        out.extend_from_slice(&42u32.to_be_bytes()); // pcrUpdateCounter
        out.extend_from_slice(&1u32.to_be_bytes()); // selCount
        out.extend_from_slice(&0x000Bu16.to_be_bytes()); // SHA256
        out.push(3);
        out.extend_from_slice(&[0xFF, 0x03, 0x00]);
        out.extend_from_slice(&10u32.to_be_bytes());
        for bank in &banks {
            out.extend_from_slice(&32u16.to_be_bytes());
            out.extend_from_slice(bank);
        }
        let sz = out.len() as u32;
        out[2..6].copy_from_slice(&sz.to_be_bytes());
        out
    }

    fn extend_sha256(state: [u8; 32], digest: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(state);
        h.update(digest);
        h.finalize().into()
    }

    fn passing_probe() -> StubProbe {
        let mut sb_blob = 0x07u32.to_le_bytes().to_vec();
        sb_blob.push(1);
        let mut dep_blob = 0x07u32.to_le_bytes().to_vec();
        dep_blob.push(1);
        let mut setup_blob = 0x07u32.to_le_bytes().to_vec();
        setup_blob.push(0);
        // Default: empty spec-id log + TPM response where all 10
        // banks match the replay (zero, since no events extend).
        let tpm = tpm_response_for_bank([[0u8; 32]; 10]);
        StubProbe {
            sev_status: Ok(0b111),
            secure_boot: Ok(sb_blob),
            deployed: Ok(dep_blob),
            setup: Ok(setup_blob),
            lockdown: Ok("none [integrity] confidentiality\n".into()),
            sig_enforce: Ok("Y\n".into()),
            ima: Ok("appraise func=BPRM_CHECK appraise_type=imasig\n".into()),
            mountinfo: Ok(
                "28 1 254:0 / / ro,relatime shared:1 - ext4 /dev/mapper/root ro\n"
                    .into(),
            ),
            dm_uuid: Ok("CRYPT-VERITY-abcdef-root\n".into()),
            eventlog: Ok(spec_id_only_log()),
            tpm_response: std::sync::Mutex::new(Some(tpm)),
            tpm_open_err: None,
        }
    }

    fn clone_io<T: Clone>(r: &std::io::Result<T>) -> std::io::Result<T> {
        match r {
            Ok(v) => Ok(v.clone()),
            Err(e) => Err(std::io::Error::new(e.kind(), format!("{e}"))),
        }
    }

    impl Probe for StubProbe {
        fn read_sev_status_msr(&self) -> std::io::Result<u64> {
            clone_io(&self.sev_status)
        }
        fn read_secure_boot(&self) -> std::io::Result<Vec<u8>> {
            clone_io(&self.secure_boot)
        }
        fn read_deployed_mode(&self) -> std::io::Result<Vec<u8>> {
            clone_io(&self.deployed)
        }
        fn read_setup_mode(&self) -> std::io::Result<Vec<u8>> {
            clone_io(&self.setup)
        }
        fn read_lockdown(&self) -> std::io::Result<String> {
            clone_io(&self.lockdown)
        }
        fn read_sig_enforce(&self) -> std::io::Result<String> {
            clone_io(&self.sig_enforce)
        }
        fn read_ima_policy(&self) -> std::io::Result<String> {
            clone_io(&self.ima)
        }
        fn read_mountinfo(&self) -> std::io::Result<String> {
            clone_io(&self.mountinfo)
        }
        fn read_root_dm_uuid(&self) -> std::io::Result<String> {
            clone_io(&self.dm_uuid)
        }
        fn read_eventlog(&self) -> std::io::Result<Vec<u8>> {
            clone_io(&self.eventlog)
        }
        fn open_tpm(&self) -> std::io::Result<Box<dyn TpmDevice>> {
            if let Some(k) = self.tpm_open_err {
                return Err(std::io::Error::from(k));
            }
            let response = self
                .tpm_response
                .lock()
                .expect("mutex")
                .take()
                .unwrap_or_default();
            Ok(Box::new(StubTpm { response }))
        }
    }

    #[test]
    fn happy_path_emits_start_then_ok() {
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&passing_probe(), &mut sink), Ok(()));
        assert_eq!(cap.tokens(), vec![Token::Start, Token::Ok]);
    }

    #[test]
    fn msr_missing_snp_fails_nosnp() {
        let mut p = passing_probe();
        p.sev_status = Ok(0b011); // SEV + SEV-ES but no SNP
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailNoSnp));
        assert_eq!(cap.tokens(), vec![Token::Start]);
    }

    #[test]
    fn msr_read_error_fails_nosnp() {
        let mut p = passing_probe();
        p.sev_status = Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailNoSnp));
    }

    #[test]
    fn secure_boot_off_fails_sb_off() {
        let mut p = passing_probe();
        let mut sb_blob = 0x07u32.to_le_bytes().to_vec();
        sb_blob.push(0);
        p.secure_boot = Ok(sb_blob);
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailSbOff));
    }

    #[test]
    fn setup_mode_set_fails_sb_setup() {
        let mut p = passing_probe();
        let mut setup_blob = 0x07u32.to_le_bytes().to_vec();
        setup_blob.push(1);
        p.setup = Ok(setup_blob);
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailSbSetup));
    }

    #[test]
    fn lockdown_none_fails_lockdown() {
        let mut p = passing_probe();
        p.lockdown = Ok("[none] integrity confidentiality\n".into());
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailLockdown));
    }

    #[test]
    fn sig_enforce_n_fails_modsig() {
        let mut p = passing_probe();
        p.sig_enforce = Ok("N\n".into());
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailModsig));
    }

    #[test]
    fn ima_empty_fails_ima_policy() {
        let mut p = passing_probe();
        p.ima = Ok(String::new());
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailImaPolicy));
    }

    #[test]
    fn raw_block_rootfs_fails_verity() {
        let mut p = passing_probe();
        p.mountinfo = Ok(
            "28 1 253:0 / / rw,relatime shared:1 - ext4 /dev/vda2 rw\n".into(),
        );
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailVerity));
    }

    #[test]
    fn first_failure_short_circuits_remaining_checks() {
        // A probe that fails at MSR AND returns garbage for every
        // subsequent field should still emit exactly `Start` +
        // `FailNoSnp`. Nothing else gets a chance to print a token.
        let mut p = passing_probe();
        p.sev_status = Ok(0);
        p.secure_boot = Err(std::io::Error::other("should not be read"));
        p.lockdown = Err(std::io::Error::other("should not be read"));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailNoSnp));
        assert_eq!(cap.tokens(), vec![Token::Start]);
    }

    // ------ Phase 2a (steps 9-12) sequencer tests ------

    /// Happy path across a 2-event log (spec-id header + one PCR-4
    /// extend) where the TPM returns exactly the replayed bank.
    #[test]
    fn phase2a_happy_with_pcr4_extend() {
        let event_digest = {
            let mut h = Sha256::new();
            h.update(b"some-event");
            <[u8; 32]>::from(h.finalize())
        };
        let mut p = passing_probe();
        p.eventlog = Ok(log_extending_pcr4(event_digest));

        // Build the TPM bank: PCR 4 = sha256(zero || event_digest);
        // every other PCR stays zero.
        let mut banks = [[0u8; 32]; 10];
        banks[4] = extend_sha256([0u8; 32], &event_digest);
        p.tpm_response = std::sync::Mutex::new(Some(tpm_response_for_bank(banks)));

        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Ok(()));
        assert_eq!(cap.tokens(), vec![Token::Start, Token::Ok]);
    }

    /// Truncated event log → FailEventlogParse.
    #[test]
    fn phase2a_truncated_log_fails_eventlog_parse() {
        let mut p = passing_probe();
        // 10 bytes is well short of a TCG_PCR_EVENT header (32 B).
        p.eventlog = Ok(vec![0u8; 10]);
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailEventlogParse));
    }

    /// Event log IO error → FailEventlogParse.
    #[test]
    fn phase2a_eventlog_read_error_fails_parse() {
        let mut p = passing_probe();
        p.eventlog = Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailEventlogParse));
    }

    /// TPM returns a wrong PCR 4 digest → FailPcr(4).
    #[test]
    fn phase2a_pcr4_mismatch_reports_failpcr_4() {
        let event_digest = [0x77u8; 32];
        let mut p = passing_probe();
        p.eventlog = Ok(log_extending_pcr4(event_digest));

        // TPM says PCR 4 is all-ones; replay says
        // sha256(zero || 0x77×32). Mismatch → FailPcr(4).
        let mut banks = [[0u8; 32]; 10];
        banks[4] = [0xFFu8; 32];
        p.tpm_response = std::sync::Mutex::new(Some(tpm_response_for_bank(banks)));

        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailPcr(4)));
    }

    /// First mismatch wins: PCR 0 AND PCR 4 both diverge → FailPcr(0).
    #[test]
    fn phase2a_first_mismatch_wins() {
        let event_digest = [0x22u8; 32];
        let mut p = passing_probe();
        p.eventlog = Ok(log_extending_pcr4(event_digest));

        let mut banks = [[0u8; 32]; 10];
        banks[0] = [0xAAu8; 32]; // diverges first (replay says zeros)
        banks[4] = [0xBBu8; 32]; // diverges too, but PCR 0 reports first
        p.tpm_response = std::sync::Mutex::new(Some(tpm_response_for_bank(banks)));

        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailPcr(0)));
    }

    /// TPM open fails → FailPcr(0) (deterministic index; a TPM-
    /// unreachable guest has no attestation chain at all).
    #[test]
    fn phase2a_tpm_open_error_fails_pcr_0() {
        let mut p = passing_probe();
        p.tpm_open_err = Some(std::io::ErrorKind::NotFound);
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailPcr(0)));
    }

    /// TPM returns an error response (rc != 0) → FailPcr(0).
    #[test]
    fn phase2a_tpm_rc_nonzero_fails_pcr_0() {
        let mut p = passing_probe();
        // Rewrite the default zero-bank response to carry rc=0x101.
        let mut bad = tpm_response_for_bank([[0u8; 32]; 10]);
        bad[6..10].copy_from_slice(&0x0000_0101u32.to_be_bytes());
        p.tpm_response = std::sync::Mutex::new(Some(bad));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailPcr(0)));
    }

    /// Bank 9 mismatch surfaces FailPcr(9), not rolled-up to FailPcr(0).
    #[test]
    fn phase2a_pcr9_mismatch_reports_failpcr_9() {
        let mut p = passing_probe();
        let mut banks = [[0u8; 32]; 10];
        banks[9] = [0xCCu8; 32];
        p.tpm_response = std::sync::Mutex::new(Some(tpm_response_for_bank(banks)));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &mut sink), Err(Token::FailPcr(9)));
    }
}
