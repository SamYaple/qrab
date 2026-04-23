//! Production [`Probe`] implementation. Reads the real sysfs,
//! procfs, efivarfs, and MSR device paths.
//!
//! Kept isolated so the pure decoders and the sequencer stay free of
//! unit-test-hostile IO.

use std::os::fd::RawFd;
use std::path::PathBuf;

use crate::efivar::build_efivar_filename;
use crate::msr;
use crate::secret;
use crate::sequencer::{Probe, TpmDevice};
use crate::verity;

/// Paths real boot consults. Exposed on the struct so a container or
/// integration test can point at a fixture root.
pub struct RealProbe {
    pub msr_path: PathBuf,
    pub efivars_dir: PathBuf,
    pub lockdown_path: PathBuf,
    pub sig_enforce_path: PathBuf,
    pub ima_policy_path: PathBuf,
    pub mountinfo_path: PathBuf,
    pub sys_block_dir: PathBuf,
    pub dev_dir: PathBuf,
    pub eventlog_path: PathBuf,
    pub tpm_device_path: PathBuf,
    /// `/run/qrab/report` — magic-prefixed SNP attestation report
    /// written by the dracut pre-pivot hook. Phase 2b step 13.
    pub read_report_path: PathBuf,
    /// `/run/qrab/session` — `client_nonce || server_nonce`, 64 B
    /// total. Phase 2b step 18.
    pub read_session_path: PathBuf,
    /// `/run/qrab/secret` — secret body from attest-server. Phase 2b
    /// only hashes it for the report_data reconstruct (non-
    /// destructive read); Phase 2c will add a consuming read that
    /// unlinks on success.
    pub read_secret_path: PathBuf,
}

impl RealProbe {
    /// Defaults that match `docs/security/level-5-init.md` §Components.
    pub fn live_defaults() -> Self {
        Self {
            msr_path: PathBuf::from("/dev/cpu/0/msr"),
            efivars_dir: PathBuf::from("/sys/firmware/efi/efivars"),
            lockdown_path: PathBuf::from("/sys/kernel/security/lockdown"),
            sig_enforce_path: PathBuf::from(
                "/sys/module/module/parameters/sig_enforce",
            ),
            ima_policy_path: PathBuf::from("/sys/kernel/security/ima/policy"),
            mountinfo_path: PathBuf::from("/proc/self/mountinfo"),
            sys_block_dir: PathBuf::from("/sys/block"),
            dev_dir: PathBuf::from("/dev"),
            eventlog_path: PathBuf::from(
                "/sys/kernel/security/tpm0/binary_bios_measurements",
            ),
            tpm_device_path: PathBuf::from("/dev/tpmrm0"),
            read_report_path: PathBuf::from("/run/qrab/report"),
            read_session_path: PathBuf::from("/run/qrab/session"),
            read_secret_path: PathBuf::from("/run/qrab/secret"),
        }
    }

    fn efivar_path(&self, name: &str) -> PathBuf {
        self.efivars_dir.join(build_efivar_filename(name))
    }
}

impl Probe for RealProbe {
    fn read_sev_status_msr(&self) -> std::io::Result<u64> {
        msr::read_msr_file(&self.msr_path, msr::MSR_AMD64_SEV)
    }

    fn read_secure_boot(&self) -> std::io::Result<Vec<u8>> {
        std::fs::read(self.efivar_path("SecureBoot"))
    }

    fn read_deployed_mode(&self) -> std::io::Result<Vec<u8>> {
        std::fs::read(self.efivar_path("DeployedMode"))
    }

    fn read_setup_mode(&self) -> std::io::Result<Vec<u8>> {
        std::fs::read(self.efivar_path("SetupMode"))
    }

    fn read_lockdown(&self) -> std::io::Result<String> {
        std::fs::read_to_string(&self.lockdown_path)
    }

    fn read_sig_enforce(&self) -> std::io::Result<String> {
        std::fs::read_to_string(&self.sig_enforce_path)
    }

    fn read_ima_policy(&self) -> std::io::Result<String> {
        std::fs::read_to_string(&self.ima_policy_path)
    }

    fn read_mountinfo(&self) -> std::io::Result<String> {
        std::fs::read_to_string(&self.mountinfo_path)
    }

    fn read_root_dm_uuid(&self) -> std::io::Result<String> {
        let mi = self.read_mountinfo()?;
        let source = verity::parse_root_source(&mi).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no `/` entry in mountinfo",
            )
        })?;
        let dm_name = resolve_dm_name(source, &self.dev_dir)?;
        let uuid_path = self.sys_block_dir.join(&dm_name).join("dm").join("uuid");
        std::fs::read_to_string(uuid_path)
    }

    fn read_eventlog(&self) -> std::io::Result<Vec<u8>> {
        std::fs::read(&self.eventlog_path)
    }

    fn open_tpm(&self) -> std::io::Result<Box<dyn TpmDevice>> {
        let f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.tpm_device_path)?;
        Ok(Box::new(f))
    }

    fn read_report(&self) -> std::io::Result<Vec<u8>> {
        std::fs::read(&self.read_report_path)
    }

    fn read_session(&self) -> std::io::Result<Vec<u8>> {
        std::fs::read(&self.read_session_path)
    }

    fn read_secret(&self) -> std::io::Result<Vec<u8>> {
        std::fs::read(&self.read_secret_path)
    }

    fn consume_secret(&self, expected_len: usize) -> std::io::Result<RawFd> {
        // Delegate to the `secret` module, flattening every variant
        // of `ConsumeError` to a generic `std::io::Error` with
        // `ErrorKind::Other`. The sequencer collapses all of these
        // into `Token::FailSecret` anyway — the specific cause
        // surfaces only in dev-mode stderr via `Debug`. Keeping the
        // surface narrow on-console is the L5 contract: an attacker
        // must not be able to probe which sub-step they tripped.
        secret::consume(&self.read_secret_path, expected_len)
            .map_err(|e| std::io::Error::other(format!("{e:?}")))
    }
}

/// Resolve `/dev/mapper/<label>` or `/dev/dm-<N>` to the
/// `dm-<N>` basename under `/sys/block/`. For the mapper form we
/// stat the symlink target — the kernel makes it point at the
/// canonical `/dev/dm-<N>`.
fn resolve_dm_name(source: &str, dev_dir: &std::path::Path) -> std::io::Result<String> {
    if let Some(rest) = source.strip_prefix("/dev/") {
        if let Some(name) = rest.strip_prefix("dm-") {
            return Ok(format!("dm-{name}"));
        }
        if let Some(label) = rest.strip_prefix("mapper/") {
            // `/dev/mapper/<label>` is a symlink to `../dm-<N>`.
            // Respect the provided `dev_dir` so tests can swap in
            // a tempdir. `fs::read_link` returns a relative path
            // that we join back into a canonical `/dev/dm-<N>`.
            let link = std::fs::read_link(dev_dir.join("mapper").join(label))?;
            let target = link
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "/dev/mapper link target has no filename",
                    )
                })?;
            if target.starts_with("dm-") {
                return Ok(target.to_string());
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("cannot resolve dm name from source `{source}`"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn efivar_path_uses_global_guid() {
        let probe = RealProbe::live_defaults();
        let got = probe.efivar_path("SecureBoot");
        assert!(got.ends_with(format!(
            "SecureBoot-{}",
            crate::efivar::EFI_GLOBAL_GUID,
        )));
    }

    #[test]
    fn resolve_dm_name_accepts_dm_prefix() {
        let dev = std::path::Path::new("/unused/in/this/branch");
        assert_eq!(resolve_dm_name("/dev/dm-0", dev).unwrap(), "dm-0");
        assert_eq!(resolve_dm_name("/dev/dm-42", dev).unwrap(), "dm-42");
    }

    #[test]
    fn resolve_dm_name_rejects_raw_block() {
        let dev = std::path::Path::new("/unused");
        assert!(resolve_dm_name("/dev/vda2", dev).is_err());
        assert!(resolve_dm_name("/dev/nvme0n1p1", dev).is_err());
    }

    #[test]
    fn resolve_dm_name_follows_mapper_symlink() {
        // Build a fake /dev/mapper/root → ../dm-7 symlink under a
        // tempdir and confirm resolution lands at `dm-7`.
        let td = tempfile::tempdir().unwrap();
        let mapper = td.path().join("mapper");
        std::fs::create_dir(&mapper).unwrap();
        std::os::unix::fs::symlink("../dm-7", mapper.join("root")).unwrap();
        let name = resolve_dm_name("/dev/mapper/root", td.path()).unwrap();
        assert_eq!(name, "dm-7");
    }

    /// End-to-end smoke test of [`RealProbe::consume_secret`] via
    /// the `secret::consume` module. Writes a 16-byte 0o600 file
    /// under a tempdir, points a constructed `RealProbe` at it, and
    /// asserts: (a) the call returns `Ok(fd)`, (b) the tempfile is
    /// gone, (c) the returned fd carries the right bytes.
    #[test]
    fn consume_secret_delegates_to_secret_module() {
        use std::io::{Read, Seek, SeekFrom, Write};
        use std::os::fd::{FromRawFd, OwnedFd};
        use std::os::unix::fs::OpenOptionsExt;

        let td = tempfile::tempdir().unwrap();
        let secret_path = td.path().join("secret");
        let body = b"0123456789abcdef"; // 16 bytes
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&secret_path)
            .unwrap();
        f.write_all(body).unwrap();
        // Ensure mode sticks even if umask stripped it.
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&secret_path).unwrap().permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&secret_path, perms).unwrap();

        // Point a probe at ONLY the tempdir secret. Every other
        // path is irrelevant — consume_secret only touches
        // `read_secret_path`.
        let mut probe = RealProbe::live_defaults();
        probe.read_secret_path = secret_path.clone();

        let fd = probe.consume_secret(body.len()).expect("consume ok");
        assert!(
            !secret_path.exists(),
            "RealProbe::consume_secret must unlink the source"
        );
        // Verify bytes.
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut file = std::fs::File::from(owned);
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut read_back = Vec::new();
        file.read_to_end(&mut read_back).unwrap();
        assert_eq!(read_back, body);
    }

    /// Length-mismatch flattens to a generic `std::io::Error`
    /// (via the `.map_err` in `consume_secret`). The sequencer
    /// turns this into `Token::FailSecret` — here we just confirm
    /// the delegation surfaces an error at all.
    #[test]
    fn consume_secret_wrong_length_surfaces_error() {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let td = tempfile::tempdir().unwrap();
        let secret_path = td.path().join("secret");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&secret_path)
            .unwrap();
        f.write_all(b"short").unwrap(); // 5 bytes
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&secret_path).unwrap().permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&secret_path, perms).unwrap();

        let mut probe = RealProbe::live_defaults();
        probe.read_secret_path = secret_path;
        assert!(probe.consume_secret(16).is_err());
    }
}
