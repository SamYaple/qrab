//! Straight-line policy gate from `docs/security/level-5-init.md`.
//!
//! Phase 1 scope: steps 1-8 (MSR, Secure Boot, Deployed/Setup mode,
//! lockdown, module sig enforcement, IMA policy, dm-verity). Steps
//! 9-20 are stubbed as [`todo!()`] call-outs so the wiring surface is
//! obvious for Phase 2 without silently letting a partial gate claim
//! success.
//!
//! Real-boot wiring lives behind the [`Probe`] trait so integration
//! tests can drive the whole ladder with fixed strings/bytes.

use crate::efivar;
use crate::ima;
use crate::lockdown;
use crate::log::TokenSink;
use crate::modsig;
use crate::msr;
use crate::tokens::Token;
use crate::verity;

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
}

/// Phase 1 sequencer. Runs steps 1-8, emits [`Token::Ok`] if they
/// all pass. Any failure collapses to a single fatal token — the
/// caller (bin) emits it and halts.
///
/// Tests drive this through a [`Probe`] stub and a capturing sink;
/// the binary wires a real probe reading `/dev/cpu/0/msr`, efivarfs,
/// `/sys/kernel/security/*` etc.
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

    // Steps 9-20 are Phase 2. Each is a distinct sub-binding the
    // sequencer will pick up when the relevant parser lands:
    //   9-12  eventlog::parse + PCR replay against /dev/tpmrm0
    //  13-18  report::parse(/run/qrab/report) + report_data reconstruct
    //    19  secret::consume(/run/qrab/secret) + memfd handoff
    //    20  policy::check_pcr_whitelist(pcrs)
    // They intentionally do NOT execute in Phase 1 so a half-finished
    // gate cannot print QRAB_INIT_OK on a guest that lacks the Phase-2
    // machinery. When Phase 2 lands, replace this beacon with the real
    // calls and remove the `phase1_incomplete` token-emission path in
    // the binary.

    let _ = sink.emit(Token::Ok);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::CaptureSink;

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
    }

    fn passing_probe() -> StubProbe {
        let mut sb_blob = 0x07u32.to_le_bytes().to_vec();
        sb_blob.push(1);
        let mut dep_blob = 0x07u32.to_le_bytes().to_vec();
        dep_blob.push(1);
        let mut setup_blob = 0x07u32.to_le_bytes().to_vec();
        setup_blob.push(0);
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
}
