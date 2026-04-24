//! Stage efivarfs-backed payloads into `/run/qrab/` at PID-1 time.
//!
//! `qrab-boot` publishes three EFI variables before handing the UKI
//! off to the kernel: `QrabSecret`, `QrabReport`, `QrabSession`. Each
//! file under `/sys/firmware/efi/efivars/<Name>-<GUID>` is a 4-byte
//! little-endian attribute word followed by the variable body. This
//! module strips the attribute prefix and copies the body into
//! `/run/qrab/<name>` so the sequencer (`qrab-guest-init`) can consume
//! them via the existing `Probe::read_*` calls.
//!
//! The `public` variants (report + session) leave the source intact
//! — downstream debugging can still inspect the efivar. The `secret`
//! variant unlinks the source after staging so the plaintext lives in
//! kernel memory exactly once per boot.
//!
//! Separated from the bin entry point so all copy semantics are
//! covered by unit tests against a tempdir, without needing root or a
//! real efivarfs mount.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Four-byte `EFI_VARIABLE_*` attribute prefix length.
const EFIVAR_ATTR_PREFIX_LEN: usize = 4;

#[derive(Debug)]
pub enum StashError {
    Io(io::Error),
    Truncated { src_len: usize },
    Empty,
}

impl From<io::Error> for StashError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Read an efivarfs file, strip the 4-byte attribute prefix, return
/// the body as a fresh `Vec<u8>`. A file shorter than the prefix is
/// `Truncated`; an empty body (prefix only) is `Empty`.
pub fn read_efivar_body(src: &Path) -> Result<Vec<u8>, StashError> {
    let mut f = File::open(src)?;
    let mut raw = Vec::new();
    f.read_to_end(&mut raw)?;
    if raw.len() < EFIVAR_ATTR_PREFIX_LEN {
        return Err(StashError::Truncated { src_len: raw.len() });
    }
    let body = raw.split_off(EFIVAR_ATTR_PREFIX_LEN);
    if body.is_empty() {
        return Err(StashError::Empty);
    }
    Ok(body)
}

/// Write `body` to `dst` with `mode`, creating the parent dir if
/// needed. Uses `O_CREAT|O_WRONLY|O_TRUNC` so an existing file is
/// replaced atomically from the consumer's point of view only if the
/// caller hands us the final path; callers that care about atomicity
/// should stage into a `.tmp` and rename.
fn write_with_mode(dst: &Path, body: &[u8], mode: u32) -> Result<(), StashError> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(mode)
        .open(dst)?;
    f.write_all(body)?;
    f.sync_all()?;
    // `mode()` on `OpenOptions` only applies at create time; re-chmod
    // so a pre-existing target is also tightened to `mode` and we
    // don't silently inherit a laxer permission from last boot.
    fs::set_permissions(dst, fs::Permissions::from_mode(mode))?;
    Ok(())
}

/// Stage `QrabReport` / `QrabSession`: read-and-copy, leave source.
/// Public data, mode 0644 so downstream tools can re-read without
/// dropping privileges.
pub fn stage_public(src: &Path, dst: &Path) -> Result<usize, StashError> {
    let body = read_efivar_body(src)?;
    write_with_mode(dst, &body, 0o644)?;
    Ok(body.len())
}

/// Stage `QrabSecret`: read-and-copy, then unlink the source so the
/// plaintext is not re-readable from any later process. Mode 0600 —
/// only qrab-guest-init (running as UID 0) should be able to read it
/// before it's memfd-sealed and unlinked in turn.
pub fn stage_secret(src: &Path, dst: &Path) -> Result<usize, StashError> {
    let body = read_efivar_body(src)?;
    write_with_mode(dst, &body, 0o600)?;
    // Removing the efivarfs file issues a DeleteVar into the firmware.
    // If it fails (e.g. immutable flag, non-efivarfs source on dev
    // harness) we still surface success for the copy; the caller
    // decides whether to hard-fail. We best-effort the unlink and
    // bubble the errno so the bin can log it as a soft warning.
    match fs::remove_file(src) {
        Ok(()) => Ok(body.len()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(body.len()),
        Err(e) => Err(StashError::Io(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    use tempfile::tempdir;

    fn efivar_blob(body: &[u8]) -> Vec<u8> {
        let mut v = 0x07u32.to_le_bytes().to_vec(); // NV|BS|RT
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn read_body_strips_prefix() {
        let d = tempdir().unwrap();
        let src = d.path().join("FakeVar-guid");
        fs::write(&src, efivar_blob(b"payload")).unwrap();
        assert_eq!(read_efivar_body(&src).unwrap(), b"payload");
    }

    #[test]
    fn read_body_rejects_truncated_source() {
        let d = tempdir().unwrap();
        let src = d.path().join("Short-guid");
        fs::write(&src, &[0u8; 3]).unwrap(); // 3 < 4
        assert!(matches!(
            read_efivar_body(&src),
            Err(StashError::Truncated { src_len: 3 }),
        ));
    }

    #[test]
    fn read_body_rejects_empty_payload() {
        let d = tempdir().unwrap();
        let src = d.path().join("Empty-guid");
        fs::write(&src, 0x07u32.to_le_bytes()).unwrap(); // prefix only
        assert!(matches!(read_efivar_body(&src), Err(StashError::Empty)));
    }

    #[test]
    fn stage_public_keeps_source_and_chmods_0644() {
        let d = tempdir().unwrap();
        let src = d.path().join("QrabReport-g");
        let dst = d.path().join("run/qrab/report");
        fs::write(&src, efivar_blob(&[0xAA; 1184])).unwrap();

        let n = stage_public(&src, &dst).unwrap();
        assert_eq!(n, 1184);
        assert!(src.exists(), "public staging must not unlink source");
        assert_eq!(fs::read(&dst).unwrap(), vec![0xAA; 1184]);
        let mode = fs::metadata(&dst).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }

    #[test]
    fn stage_secret_unlinks_source_and_chmods_0600() {
        let d = tempdir().unwrap();
        let src = d.path().join("QrabSecret-g");
        let dst = d.path().join("run/qrab/secret");
        fs::write(&src, efivar_blob(b"opaque-blob-32-bytes-long-value!")).unwrap();

        let n = stage_secret(&src, &dst).unwrap();
        assert_eq!(n, 32);
        assert!(!src.exists(), "secret staging must unlink source");
        assert_eq!(fs::read(&dst).unwrap(), b"opaque-blob-32-bytes-long-value!");
        let mode = fs::metadata(&dst).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn stage_public_creates_missing_parent_dirs() {
        let d = tempdir().unwrap();
        let src = d.path().join("Src-g");
        let dst = d.path().join("a/b/c/run/qrab/session");
        fs::write(&src, efivar_blob(&[0x11; 64])).unwrap();
        stage_public(&src, &dst).unwrap();
        assert!(dst.exists());
    }

    #[test]
    fn stage_public_overwrites_existing_loose_permissions() {
        let d = tempdir().unwrap();
        let src = d.path().join("Src-g");
        let dst = d.path().join("dst");
        fs::write(&src, efivar_blob(b"x")).unwrap();
        // Pre-existing file with wide mode 0666.
        fs::write(&dst, b"stale").unwrap();
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o666)).unwrap();

        stage_public(&src, &dst).unwrap();
        let mode = fs::metadata(&dst).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o644, "re-chmod must tighten the target");
    }

    #[test]
    fn stage_secret_tolerates_already_missing_source() {
        // Belt-and-braces: our unlink-after-copy sees a NotFound
        // between `open` and `remove` only in pathological races,
        // but we shouldn't panic on it.
        let d = tempdir().unwrap();
        let src = d.path().join("Src-g");
        let dst = d.path().join("dst");
        fs::write(&src, efivar_blob(b"x")).unwrap();

        // Monkey-wrench: remove source out-of-band mid-call by
        // calling stage_secret twice. Second call sees a missing
        // source and should surface `StashError::Io(NotFound)`
        // from the `open`.
        stage_secret(&src, &dst).unwrap();
        assert!(matches!(
            stage_secret(&src, &dst),
            Err(StashError::Io(e)) if e.kind() == io::ErrorKind::NotFound,
        ));
    }
}
