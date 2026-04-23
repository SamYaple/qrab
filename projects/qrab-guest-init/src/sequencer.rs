//! Straight-line policy gate from `docs/security/level-5-init.md`.
//!
//! Phase 2b scope: steps 1-18. Extends Phase 2a (steps 1-12) with the
//! SNP attestation report parse + vmpl/measurement/policy/tcb/
//! report_data checks.
//!
//! Step 19 (secret consume + memfd handoff) and step 20 (per-PCR
//! workload whitelist) remain deferred to Phase 2c.
//!
//! Real-boot wiring lives behind the [`Probe`] trait so integration
//! tests can drive the whole ladder with fixed strings/bytes. The
//! TPM handle is a `Box<dyn Read+Write+Send>` — we can't hold a
//! non-dyn generic on the trait without infecting every caller with a
//! type parameter, and the indirection is fine at steady state (one
//! call, one round-trip, done). `Send` is added only to keep the
//! trait object usable from any thread; the trait itself isn't
//! `async` and the real binary runs it single-threaded.

use sha2::{Digest, Sha256};

use crate::efivar;
use crate::eventlog;
use crate::ima;
use crate::lockdown;
use crate::log::TokenSink;
use crate::modsig;
use crate::msr;
use crate::pcr;
use crate::policy::{self, Policy};
use crate::report;
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

    /// Raw bytes of `/run/qrab/report` (`MAGIC || body`). The dracut
    /// pre-pivot hook writes this as an atomic single-shot; the
    /// sequencer does not tolerate a partially-written file.
    fn read_report(&self) -> std::io::Result<Vec<u8>>;

    /// Raw 64-byte session binary: `client_nonce || server_nonce`.
    /// Paired with [`Probe::read_report`] by the dracut hook so both
    /// files appear atomically before qrab-guest-init runs.
    fn read_session(&self) -> std::io::Result<Vec<u8>>;

    /// Read `/run/qrab/secret` WITHOUT unlinking. Phase 2b computes
    /// `sha256(secret)` for the report_data reconstruct; Phase 2c
    /// will add a distinct `consume_secret` method that does the
    /// single-shot read + unlink. Keeping the two separate means
    /// Phase 2b can hash the secret and compare the PSP report
    /// binding before the secret is actually handed to the
    /// workload — a verification fault never touches the filesystem.
    fn read_secret(&self) -> std::io::Result<Vec<u8>>;
}

/// Phase 2b sequencer. Runs steps 1-18, emits [`Token::Ok`] if they
/// all pass. Any failure collapses to a single fatal token — the
/// caller (bin) emits it and halts.
///
/// Tests drive this through a [`Probe`] stub, a [`Policy`] fixture,
/// and a capturing sink; the binary wires a real probe reading
/// `/dev/cpu/0/msr`, efivarfs, `/sys/kernel/security/*`,
/// `/sys/kernel/security/tpm0/…`, `/dev/tpmrm0`, `/run/qrab/report`,
/// `/run/qrab/session`, and `/run/qrab/secret`.
pub fn run_gate<P, S>(probe: &P, policy: &Policy, sink: &mut S) -> Result<(), Token>
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

    // Step 9: parse the TCG 2.0 crypto-agile event log. The parsed
    // `events` vector stays alive through step 18 — it feeds both
    // the PCR replay (step 10) AND the unique-PCR-8-event lookup
    // that step 18 needs for the report_data reconstruct. Phase 2a
    // originally dropped it after step 10; Phase 2b keeps the bind
    // so we walk the log exactly once.
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

    // Step 13: parse /run/qrab/report.
    let report_bytes = probe.read_report().map_err(|_| Token::FailReportParse)?;
    let report = report::parse(&report_bytes).map_err(|_| Token::FailReportParse)?;

    // Step 14: require vmpl == 0. Block expression around the
    // packed-struct field read — taking a reference to a
    // `#[repr(C, packed)]` field is UB; a block-scoped copy is the
    // documented idiom.
    if { report.vmpl } != policy::EXPECTED_VMPL {
        return Err(Token::FailVmpl);
    }

    // Step 15: measurement whitelist.
    let measurement = { report.measurement };
    policy
        .check_measurement(&measurement)
        .map_err(|_| Token::FailMeasurement)?;

    // Step 16: policy-bits whitelist.
    let policy_bits = { report.policy };
    policy
        .check_policy_bits(policy_bits)
        .map_err(|_| Token::FailPolicy)?;

    // Step 17: reported_tcb floor.
    let reported_tcb = { report.reported_tcb };
    policy
        .check_tcb(reported_tcb)
        .map_err(|_| Token::FailTcb)?;

    // Step 18: report_data reconstruct.
    //
    // `uki_sha256` is the event-level digest that qrab-boot extended
    // into PCR 8 — NOT the replayed PCR value. Phase 2a's replay
    // folds `sha256(zero || uki_digest)` into PCR 8; step 18 needs
    // the raw `uki_digest` that the bootloader originally hashed
    // and the attestation server committed to as `uki_sha256`.
    // Re-use the parsed event vec (kept alive since step 9) and
    // pull the single SHA-256 entry for PCR 8. Zero or more-than-
    // one PCR-8 events is itself a protocol violation — map both to
    // FailReportData so the sequencer has a deterministic token.
    let uki_sha256 = eventlog::unique_sha256_digest_for_pcr(&events, 8)
        .ok_or(Token::FailReportData)?;

    let session = probe.read_session().map_err(|_| Token::FailReportData)?;
    let (client_nonce, server_nonce) =
        policy::parse_session(&session).map_err(|_| Token::FailReportData)?;

    // Non-destructive read. Phase 2c will add a separate
    // `consume_secret` accessor that does the single-shot
    // read + unlink on `/run/qrab/secret`; Phase 2b only needs the
    // hash.
    let secret_bytes = probe.read_secret().map_err(|_| Token::FailReportData)?;
    let secret_sha256: [u8; 32] = Sha256::digest(&secret_bytes).into();

    let reconstructed = policy::reconstruct_report_data(
        &client_nonce,
        &server_nonce,
        &uki_sha256,
        &secret_sha256,
    );
    let report_data_full = { report.report_data };
    let mut report_data_prefix = [0u8; 32];
    report_data_prefix.copy_from_slice(&report_data_full[..32]);
    if reconstructed != report_data_prefix {
        return Err(Token::FailReportData);
    }

    // Phase 2c plugs in here:
    //   19. secret::consume(/run/qrab/secret, expected_len)
    //       → unlink + memfd handoff to workload via execve.
    //   20. policy::check_pcr_whitelist(pcr_values)
    //       → L5 workload-specific per-PCR allowlist against
    //         `read` (the live `/dev/tpmrm0` bank from step 11).

    let _ = sink.emit(Token::Ok);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventlog::{SPEC_ID_SIGNATURE, TPM_ALG_SHA256};
    use crate::log::CaptureSink;
    use crate::policy::Policy;
    use crate::report::REPORT_MAGIC;
    use qrab_snp_proto::{SNP_ATTESTATION_REPORT_SIZE, SnpAttestationReport};
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    use zerocopy::{FromBytes, IntoBytes};

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
        report: std::io::Result<Vec<u8>>,
        session: std::io::Result<Vec<u8>>,
        secret: std::io::Result<Vec<u8>>,
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

    /// Reference measurement pinned by the fixture policy. Made into
    /// a named constant so failure-mode tests can mutate the report
    /// to a DIFFERENT 48-byte value without rewriting the whitelist.
    const FIXTURE_MEASUREMENT: [u8; 48] = [0xA1; 48];
    /// `SnpAttestationReport.policy` bitmap the fixture reports emit.
    /// Required bits [0,1,2] set, no forbidden bits.
    const FIXTURE_POLICY_BITS: u64 = 0b0111;
    /// `reported_tcb` the fixture reports emit. Arbitrary but equal
    /// to the policy's `min_tcb` so the edge passes.
    const FIXTURE_MIN_TCB: u64 = 0x0000_0004_0003_0002;
    /// Client + server nonces the fixture session carries.
    const FIXTURE_CLIENT_NONCE: [u8; 32] = [0x11; 32];
    const FIXTURE_SERVER_NONCE: [u8; 32] = [0x22; 32];
    /// `sha256(uki_bytes)` the fixture bootloader committed to. Must
    /// equal the SHA-256 digest of the PCR-8 event in the fixture
    /// log so step 18 lands on equality.
    const FIXTURE_UKI_SHA256: [u8; 32] = [0x33; 32];
    /// Secret body the fixture probe hands back.
    fn fixture_secret_body() -> Vec<u8> {
        b"fixture-secret-canary".to_vec()
    }

    fn fixture_policy() -> Policy {
        Policy {
            allowed_measurements: vec![FIXTURE_MEASUREMENT],
            required_policy_mask: 0b0111,
            forbidden_policy_mask: 0b1000_0000,
            min_tcb: FIXTURE_MIN_TCB,
        }
    }

    /// Construct a zero-initialised report mutated in the fields
    /// Phase 2b checks. Returns the struct + the pre-computed
    /// report_data the sequencer will reconstruct — tests that care
    /// about one failure mode leave the rest as-is and re-serialise.
    fn fixture_report() -> SnpAttestationReport {
        let buf = [0u8; SNP_ATTESTATION_REPORT_SIZE];
        let mut r =
            SnpAttestationReport::read_from_bytes(&buf).expect("zero cast");
        r.vmpl = 0;
        r.measurement = FIXTURE_MEASUREMENT;
        r.policy = FIXTURE_POLICY_BITS;
        r.reported_tcb = FIXTURE_MIN_TCB;
        // report_data is set by the caller after the reconstruct is
        // known (the SHA depends on secret body + uki digest + nonces).
        r
    }

    /// Fold the fixture report + magic prefix into a `Vec<u8>` that
    /// satisfies [`crate::report::parse`].
    fn make_report_blob(r: &SnpAttestationReport) -> Vec<u8> {
        let mut blob = Vec::with_capacity(REPORT_MAGIC.len() + SNP_ATTESTATION_REPORT_SIZE);
        blob.extend_from_slice(REPORT_MAGIC);
        blob.extend_from_slice(r.as_bytes());
        blob
    }

    /// TCG log with a spec-id header + one PCR-8 event carrying
    /// [`FIXTURE_UKI_SHA256`] + extra events on other PCRs. Used by
    /// the passing probe so both step 12 (PCR compare) and step 18
    /// (unique PCR-8 digest) pass.
    fn fixture_event_log() -> Vec<u8> {
        let mut out = spec_id_only_log();
        // TCG_PCR_EVENT2 for PCR 8. Same shape as `log_extending_pcr4`
        // but parameterised to an arbitrary PCR.
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&0x0000_0005u32.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&TPM_ALG_SHA256.to_le_bytes());
        out.extend_from_slice(&FIXTURE_UKI_SHA256);
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }

    /// Build a TPM response whose PCR 8 = extend(zero, FIXTURE_UKI_SHA256)
    /// (matching the fixture event log) and all other PCRs stay zero.
    fn fixture_tpm_response() -> Vec<u8> {
        let mut banks = [[0u8; 32]; 10];
        banks[8] = extend_sha256([0u8; 32], &FIXTURE_UKI_SHA256);
        tpm_response_for_bank(banks)
    }

    fn passing_probe() -> StubProbe {
        let mut sb_blob = 0x07u32.to_le_bytes().to_vec();
        sb_blob.push(1);
        let mut dep_blob = 0x07u32.to_le_bytes().to_vec();
        dep_blob.push(1);
        let mut setup_blob = 0x07u32.to_le_bytes().to_vec();
        setup_blob.push(0);

        // Phase 2b fixture: event log AND TPM bank align on a
        // single PCR-8 event whose SHA-256 digest is
        // `FIXTURE_UKI_SHA256`. Step 12 passes because the replay
        // matches the TPM read; step 18 pulls the unique PCR-8
        // digest and folds it with the fixture nonces + secret
        // hash to reconstruct `report_data`.
        let eventlog = fixture_event_log();
        let tpm = fixture_tpm_response();

        // Build a report whose report_data is the correct
        // reconstruct for the fixture session + secret.
        let secret = fixture_secret_body();
        let secret_sha256: [u8; 32] = Sha256::digest(&secret).into();
        let reconstructed = policy::reconstruct_report_data(
            &FIXTURE_CLIENT_NONCE,
            &FIXTURE_SERVER_NONCE,
            &FIXTURE_UKI_SHA256,
            &secret_sha256,
        );
        let mut report = fixture_report();
        report.report_data[..32].copy_from_slice(&reconstructed);
        let report_blob = make_report_blob(&report);

        let mut session = Vec::with_capacity(64);
        session.extend_from_slice(&FIXTURE_CLIENT_NONCE);
        session.extend_from_slice(&FIXTURE_SERVER_NONCE);

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
            eventlog: Ok(eventlog),
            tpm_response: std::sync::Mutex::new(Some(tpm)),
            tpm_open_err: None,
            report: Ok(report_blob),
            session: Ok(session),
            secret: Ok(secret),
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
        fn read_report(&self) -> std::io::Result<Vec<u8>> {
            clone_io(&self.report)
        }
        fn read_session(&self) -> std::io::Result<Vec<u8>> {
            clone_io(&self.session)
        }
        fn read_secret(&self) -> std::io::Result<Vec<u8>> {
            clone_io(&self.secret)
        }
    }

    #[test]
    fn happy_path_emits_start_then_ok() {
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&passing_probe(), &fixture_policy(), &mut sink), Ok(()));
        assert_eq!(cap.tokens(), vec![Token::Start, Token::Ok]);
    }

    #[test]
    fn msr_missing_snp_fails_nosnp() {
        let mut p = passing_probe();
        p.sev_status = Ok(0b011); // SEV + SEV-ES but no SNP
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailNoSnp));
        assert_eq!(cap.tokens(), vec![Token::Start]);
    }

    #[test]
    fn msr_read_error_fails_nosnp() {
        let mut p = passing_probe();
        p.sev_status = Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailNoSnp));
    }

    #[test]
    fn secure_boot_off_fails_sb_off() {
        let mut p = passing_probe();
        let mut sb_blob = 0x07u32.to_le_bytes().to_vec();
        sb_blob.push(0);
        p.secure_boot = Ok(sb_blob);
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailSbOff));
    }

    #[test]
    fn setup_mode_set_fails_sb_setup() {
        let mut p = passing_probe();
        let mut setup_blob = 0x07u32.to_le_bytes().to_vec();
        setup_blob.push(1);
        p.setup = Ok(setup_blob);
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailSbSetup));
    }

    #[test]
    fn lockdown_none_fails_lockdown() {
        let mut p = passing_probe();
        p.lockdown = Ok("[none] integrity confidentiality\n".into());
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailLockdown));
    }

    #[test]
    fn sig_enforce_n_fails_modsig() {
        let mut p = passing_probe();
        p.sig_enforce = Ok("N\n".into());
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailModsig));
    }

    #[test]
    fn ima_empty_fails_ima_policy() {
        let mut p = passing_probe();
        p.ima = Ok(String::new());
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailImaPolicy));
    }

    #[test]
    fn raw_block_rootfs_fails_verity() {
        let mut p = passing_probe();
        p.mountinfo = Ok(
            "28 1 253:0 / / rw,relatime shared:1 - ext4 /dev/vda2 rw\n".into(),
        );
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailVerity));
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
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailNoSnp));
        assert_eq!(cap.tokens(), vec![Token::Start]);
    }

    // ------ Phase 2a (steps 9-12) sequencer tests ------

    /// Happy path across a 3-event log (spec-id header + one PCR-4
    /// extend + one PCR-8 extend carrying `FIXTURE_UKI_SHA256`)
    /// where the TPM returns exactly the replayed bank.
    ///
    /// Phase 2b: the sequencer now runs all the way through step 18,
    /// so the happy path needs a valid PCR-8 event (step 18 pulls
    /// `uki_sha256` from it) AND a TPM bank that matches the replay
    /// (step 12 still compares every PCR).
    #[test]
    fn phase2a_happy_with_pcr4_and_pcr8_extends() {
        let event_digest = {
            let mut h = Sha256::new();
            h.update(b"some-event");
            <[u8; 32]>::from(h.finalize())
        };
        let mut p = passing_probe();

        // Event log: spec-id + PCR-4 extend + PCR-8 extend of the
        // fixture uki digest. Handwritten to avoid growing the
        // helper catalogue.
        let mut log = spec_id_only_log();
        log.extend_from_slice(&4u32.to_le_bytes());
        log.extend_from_slice(&5u32.to_le_bytes());
        log.extend_from_slice(&1u32.to_le_bytes());
        log.extend_from_slice(&TPM_ALG_SHA256.to_le_bytes());
        log.extend_from_slice(&event_digest);
        log.extend_from_slice(&0u32.to_le_bytes());
        log.extend_from_slice(&8u32.to_le_bytes());
        log.extend_from_slice(&5u32.to_le_bytes());
        log.extend_from_slice(&1u32.to_le_bytes());
        log.extend_from_slice(&TPM_ALG_SHA256.to_le_bytes());
        log.extend_from_slice(&FIXTURE_UKI_SHA256);
        log.extend_from_slice(&0u32.to_le_bytes());
        p.eventlog = Ok(log);

        // TPM bank: PCR 4 replayed, PCR 8 replayed, rest zero.
        let mut banks = [[0u8; 32]; 10];
        banks[4] = extend_sha256([0u8; 32], &event_digest);
        banks[8] = extend_sha256([0u8; 32], &FIXTURE_UKI_SHA256);
        p.tpm_response = std::sync::Mutex::new(Some(tpm_response_for_bank(banks)));

        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Ok(()));
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
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailEventlogParse));
    }

    /// Event log IO error → FailEventlogParse.
    #[test]
    fn phase2a_eventlog_read_error_fails_parse() {
        let mut p = passing_probe();
        p.eventlog = Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailEventlogParse));
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
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailPcr(4)));
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
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailPcr(0)));
    }

    /// TPM open fails → FailPcr(0) (deterministic index; a TPM-
    /// unreachable guest has no attestation chain at all).
    #[test]
    fn phase2a_tpm_open_error_fails_pcr_0() {
        let mut p = passing_probe();
        p.tpm_open_err = Some(std::io::ErrorKind::NotFound);
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailPcr(0)));
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
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailPcr(0)));
    }

    /// Bank 9 mismatch surfaces FailPcr(9), not rolled-up to FailPcr(0).
    /// Fixture log extends PCR 8 with `FIXTURE_UKI_SHA256`, so the
    /// overridden bank must preserve that replay at slot 8 or step 12
    /// would fail on PCR 8 before reaching PCR 9.
    #[test]
    fn phase2a_pcr9_mismatch_reports_failpcr_9() {
        let mut p = passing_probe();
        let mut banks = [[0u8; 32]; 10];
        banks[8] = extend_sha256([0u8; 32], &FIXTURE_UKI_SHA256);
        banks[9] = [0xCCu8; 32];
        p.tpm_response = std::sync::Mutex::new(Some(tpm_response_for_bank(banks)));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(run_gate(&p, &fixture_policy(), &mut sink), Err(Token::FailPcr(9)));
    }

    // ------ Phase 2b (steps 13-18) sequencer tests ------

    /// The happy path is already covered by `happy_path_emits_start_then_ok`
    /// — `passing_probe()` + `fixture_policy()` exercise steps 1-18.
    /// The tests below each twist one input to isolate a single
    /// failure mode.

    #[test]
    fn phase2b_garbled_report_fails_report_parse() {
        let mut p = passing_probe();
        // 20 zero bytes — shorter than MAGIC + 1184.
        p.report = Ok(vec![0u8; 20]);
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailReportParse)
        );
    }

    #[test]
    fn phase2b_report_read_error_fails_report_parse() {
        // A failure reading the file is indistinguishable from a
        // file that didn't parse — both route to FailReportParse.
        let mut p = passing_probe();
        p.report = Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailReportParse)
        );
    }

    #[test]
    fn phase2b_wrong_magic_fails_report_parse() {
        // Right total length, wrong magic prefix.
        let mut blob = vec![0u8; REPORT_MAGIC.len() + SNP_ATTESTATION_REPORT_SIZE];
        blob[..REPORT_MAGIC.len()].copy_from_slice(b"QRAB_REPORT_V2");
        let mut p = passing_probe();
        p.report = Ok(blob);
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailReportParse)
        );
    }

    #[test]
    fn phase2b_vmpl_one_fails_vmpl() {
        // Rebuild the fixture report with vmpl = 1 and keep every
        // other field passing.
        let secret = fixture_secret_body();
        let secret_sha256: [u8; 32] = Sha256::digest(&secret).into();
        let reconstructed = policy::reconstruct_report_data(
            &FIXTURE_CLIENT_NONCE,
            &FIXTURE_SERVER_NONCE,
            &FIXTURE_UKI_SHA256,
            &secret_sha256,
        );
        let mut report = fixture_report();
        report.vmpl = 1;
        report.report_data[..32].copy_from_slice(&reconstructed);

        let mut p = passing_probe();
        p.report = Ok(make_report_blob(&report));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailVmpl)
        );
    }

    #[test]
    fn phase2b_measurement_not_in_whitelist_fails_measurement() {
        // Flip the report's measurement to a value NOT in the
        // fixture policy's whitelist. Report_data still commits to
        // the fixture nonces so step 15 is the first diverging
        // check, not step 18.
        let secret = fixture_secret_body();
        let secret_sha256: [u8; 32] = Sha256::digest(&secret).into();
        let reconstructed = policy::reconstruct_report_data(
            &FIXTURE_CLIENT_NONCE,
            &FIXTURE_SERVER_NONCE,
            &FIXTURE_UKI_SHA256,
            &secret_sha256,
        );
        let mut report = fixture_report();
        report.measurement = [0x99u8; 48];
        report.report_data[..32].copy_from_slice(&reconstructed);

        let mut p = passing_probe();
        p.report = Ok(make_report_blob(&report));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailMeasurement)
        );
    }

    #[test]
    fn phase2b_missing_required_policy_bit_fails_policy() {
        // Drop bit 0 from the fixture policy bitmap while keeping
        // every other field green. The check in step 16 demands
        // every required bit, so a missing bit trips FailPolicy.
        let secret = fixture_secret_body();
        let secret_sha256: [u8; 32] = Sha256::digest(&secret).into();
        let reconstructed = policy::reconstruct_report_data(
            &FIXTURE_CLIENT_NONCE,
            &FIXTURE_SERVER_NONCE,
            &FIXTURE_UKI_SHA256,
            &secret_sha256,
        );
        let mut report = fixture_report();
        report.policy = 0b0110;
        report.report_data[..32].copy_from_slice(&reconstructed);

        let mut p = passing_probe();
        p.report = Ok(make_report_blob(&report));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailPolicy)
        );
    }

    #[test]
    fn phase2b_reported_tcb_below_min_fails_tcb() {
        // Report one less than the floor. `check_tcb` uses a strict
        // < comparison, so min_tcb - 1 is rejected but min_tcb passes.
        let secret = fixture_secret_body();
        let secret_sha256: [u8; 32] = Sha256::digest(&secret).into();
        let reconstructed = policy::reconstruct_report_data(
            &FIXTURE_CLIENT_NONCE,
            &FIXTURE_SERVER_NONCE,
            &FIXTURE_UKI_SHA256,
            &secret_sha256,
        );
        let mut report = fixture_report();
        report.reported_tcb = FIXTURE_MIN_TCB - 1;
        report.report_data[..32].copy_from_slice(&reconstructed);

        let mut p = passing_probe();
        p.report = Ok(make_report_blob(&report));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailTcb)
        );
    }

    #[test]
    fn phase2b_wrong_client_nonce_fails_report_data() {
        // Swap the client_nonce in the session file. Every step up
        // through 17 still passes (the report itself is unchanged);
        // only the reconstruct at step 18 diverges from
        // `report.report_data`.
        let mut p = passing_probe();
        let mut bad_session = Vec::with_capacity(64);
        let mut wrong_nonce = FIXTURE_CLIENT_NONCE;
        wrong_nonce[0] ^= 0xFF;
        bad_session.extend_from_slice(&wrong_nonce);
        bad_session.extend_from_slice(&FIXTURE_SERVER_NONCE);
        p.session = Ok(bad_session);

        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailReportData)
        );
    }

    #[test]
    fn phase2b_wrong_server_nonce_fails_report_data() {
        let mut p = passing_probe();
        let mut bad_session = Vec::with_capacity(64);
        let mut wrong_nonce = FIXTURE_SERVER_NONCE;
        wrong_nonce[0] ^= 0xFF;
        bad_session.extend_from_slice(&FIXTURE_CLIENT_NONCE);
        bad_session.extend_from_slice(&wrong_nonce);
        p.session = Ok(bad_session);

        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailReportData)
        );
    }

    #[test]
    fn phase2b_wrong_secret_fails_report_data() {
        // Swap the secret body → sha256(secret) changes → reconstruct
        // diverges → step 18 fails FailReportData. Report itself is
        // unchanged so steps 13-17 still pass.
        let mut p = passing_probe();
        p.secret = Ok(b"tampered-secret-body".to_vec());

        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailReportData)
        );
    }

    #[test]
    fn phase2b_session_wrong_length_fails_report_data() {
        // SessionError maps to FailReportData by the sequencer's
        // error-collapse rule — a mis-sized session file is no less
        // tamperable than one with the wrong bytes.
        let mut p = passing_probe();
        p.session = Ok(vec![0u8; 63]);
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailReportData)
        );
    }

    #[test]
    fn phase2b_zero_pcr8_events_fails_report_data() {
        // Drop the PCR-8 event entirely — spec-id header only — so
        // `unique_sha256_digest_for_pcr` returns None. We also have
        // to zero out the TPM's PCR 8 bank so step 12 (which runs
        // first) doesn't fail on a different code path.
        let mut p = passing_probe();
        p.eventlog = Ok(spec_id_only_log());
        p.tpm_response = std::sync::Mutex::new(Some(tpm_response_for_bank([[0u8; 32]; 10])));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailReportData)
        );
    }

    #[test]
    fn phase2b_multiple_pcr8_events_fails_report_data() {
        // Two events on PCR 8 — the lookup refuses. Build a log
        // with spec-id + two PCR-8 extends; update the TPM bank so
        // step 12 passes (`expected[8]` = extend(extend(0, d1), d2)).
        let d1 = FIXTURE_UKI_SHA256;
        let d2 = [0x44u8; 32];

        let mut log = spec_id_only_log();
        for d in &[d1, d2] {
            log.extend_from_slice(&8u32.to_le_bytes());
            log.extend_from_slice(&5u32.to_le_bytes());
            log.extend_from_slice(&1u32.to_le_bytes());
            log.extend_from_slice(&TPM_ALG_SHA256.to_le_bytes());
            log.extend_from_slice(d);
            log.extend_from_slice(&0u32.to_le_bytes());
        }

        let mut banks = [[0u8; 32]; 10];
        let folded = extend_sha256(extend_sha256([0u8; 32], &d1), &d2);
        banks[8] = folded;

        let mut p = passing_probe();
        p.eventlog = Ok(log);
        p.tpm_response = std::sync::Mutex::new(Some(tpm_response_for_bank(banks)));

        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailReportData)
        );
    }

    #[test]
    fn phase2b_secret_read_error_fails_report_data() {
        // Any IO failure on `/run/qrab/secret` collapses to
        // FailReportData — Phase 2b only reads the body to hash it
        // for the reconstruct; Phase 2c's `consume_secret` will
        // distinguish secret-specific failures with FailSecret.
        let mut p = passing_probe();
        p.secret = Err(std::io::Error::from(std::io::ErrorKind::NotFound));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailReportData)
        );
    }

    #[test]
    fn phase2b_empty_measurement_whitelist_fails_measurement() {
        // Runtime policy with an empty whitelist must reject every
        // measurement — the "production MUST pin at least one"
        // invariant is enforced inside `check_measurement`, not at
        // construction.
        let mut empty_policy = fixture_policy();
        empty_policy.allowed_measurements.clear();
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&passing_probe(), &empty_policy, &mut sink),
            Err(Token::FailMeasurement)
        );
    }

    #[test]
    fn phase2b_forbidden_policy_bit_set_fails_policy() {
        // All required bits present but a forbidden bit also set.
        let secret = fixture_secret_body();
        let secret_sha256: [u8; 32] = Sha256::digest(&secret).into();
        let reconstructed = policy::reconstruct_report_data(
            &FIXTURE_CLIENT_NONCE,
            &FIXTURE_SERVER_NONCE,
            &FIXTURE_UKI_SHA256,
            &secret_sha256,
        );
        let mut report = fixture_report();
        report.policy = FIXTURE_POLICY_BITS | 0b1000_0000;
        report.report_data[..32].copy_from_slice(&reconstructed);

        let mut p = passing_probe();
        p.report = Ok(make_report_blob(&report));
        let cap = CaptureSink::new();
        let mut sink: &CaptureSink = &cap;
        assert_eq!(
            run_gate(&p, &fixture_policy(), &mut sink),
            Err(Token::FailPolicy)
        );
    }
}
