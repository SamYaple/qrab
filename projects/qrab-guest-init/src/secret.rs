//! Single-shot consume of `/run/qrab/secret`.
//!
//! Phase 2c step 19 of `docs/security/level-5-init.md`. The flow is:
//!
//! 1. Open `secret_path` and check that its mode bits are exactly
//!    `0o600`. Dracut writes the file atomically with that mode;
//!    anything else is a protocol violation we refuse to work with
//!    (a world-readable file a human left behind in `/tmp` is NOT
//!    the same thing as the real secret, and we must never unlink
//!    such a file on their behalf).
//! 2. Read the file into a bounded buffer. If the length doesn't
//!    match `expected_len`, the dracut hook wrote a truncated /
//!    padded file — another protocol violation.
//! 3. `memfd_create()` a fresh anonymous tmpfs-backed file.
//! 4. Copy the secret into the memfd.
//! 5. Apply `F_SEAL_WRITE | F_SEAL_SHRINK | F_SEAL_GROW | F_SEAL_SEAL`
//!    so the workload (post-`execve`) cannot truncate/extend/re-seal.
//! 6. `unlink` the source file. Any error here AFTER a successful
//!    memfd is still returned as `Unlink` — the secret is already in
//!    the memfd, so the workload will still get it, but failing to
//!    unlink leaves the on-disk copy for anyone who can read it.
//!
//! Returns the raw memfd FD so `main.rs` can thread it into
//! `execve`'s envp as `QRAB_SECRET_FD=<n>`. The workload's entrypoint
//! is responsible for reading the memfd and zeroizing it (plus
//! closing the fd) before dropping privilege.
//!
//! Length gate: the caller supplies `expected_len`. If the on-disk
//! blob is NOT exactly that many bytes, return `WrongLen` — a
//! partial write from dracut or a truncated EFI variable is a strict
//! protocol violation.

use std::ffi::CString;
use std::fs;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use nix::fcntl::{FcntlArg, SealFlag, fcntl};
use nix::sys::memfd::{MemFdCreateFlag, memfd_create};

/// Failure modes for [`consume`]. Every variant carries enough
/// context for a post-mortem log line but the sequencer collapses
/// them all to `Token::FailSecret` — the tight error-to-token map
/// keeps the on-console output ambiguous for an attacker probing
/// which branch they hit.
#[derive(Debug)]
pub enum ConsumeError {
    /// `/run/qrab/secret` absent, or read failed.
    Io(std::io::Error),
    /// On-disk blob length != expected.
    WrongLen { expected: usize, actual: usize },
    /// Mode bits != 0o600.
    WrongMode { actual: u32 },
    /// `memfd_create` failed.
    MemfdCreate(nix::Error),
    /// Writing into the memfd failed.
    MemfdWrite(std::io::Error),
    /// Sealing the memfd failed.
    Seal(nix::Error),
    /// Unlink the source file failed.
    Unlink(std::io::Error),
}

impl std::fmt::Display for ConsumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "read secret: {e}"),
            Self::WrongLen { expected, actual } => {
                write!(f, "secret length {actual} != expected {expected}")
            }
            Self::WrongMode { actual } => {
                write!(f, "secret mode {actual:#o} != 0o600")
            }
            Self::MemfdCreate(e) => write!(f, "memfd_create: {e}"),
            Self::MemfdWrite(e) => write!(f, "memfd write: {e}"),
            Self::Seal(e) => write!(f, "memfd F_ADD_SEALS: {e}"),
            Self::Unlink(e) => write!(f, "unlink secret: {e}"),
        }
    }
}

impl std::error::Error for ConsumeError {}

/// Read `secret_path`, copy into a sealed memfd, unlink the source.
/// Returns the raw FD of the sealed memfd.
///
/// The returned RawFd is owned by the caller: they must either
/// thread it through `execve` (which inherits it across the exec) or
/// close it explicitly. The function converts an `OwnedFd` to
/// `RawFd` with `IntoRawFd::into_raw_fd` — which forgets the drop —
/// so the fd stays open after this call returns.
///
/// Error handling contract:
///
/// - `WrongMode` is returned BEFORE any unlink. A world-readable
///   file that's NOT ours to consume must be left alone.
/// - Every other error AFTER the mode check still attempts the
///   unlink — the dracut hook guarantees 0o600 mode, and if we've
///   confirmed that mode the file IS ours to consume, even if we
///   later fail to hash it. (Leaving the secret on disk after a
///   failed memfd-create is worse than deleting it.)
pub fn consume(secret_path: &Path, expected_len: usize) -> Result<RawFd, ConsumeError> {
    // Step 1a: mode check. Read the mode BEFORE opening so a
    // mode-mismatched file never touches the memfd.
    let meta = fs::metadata(secret_path).map_err(ConsumeError::Io)?;
    let mode = meta.permissions().mode() & 0o777;
    if mode != 0o600 {
        // Do NOT unlink — we don't own this file.
        return Err(ConsumeError::WrongMode { actual: mode });
    }

    // Everything after this point unlinks the source on the way out.
    // Encapsulate the body so the unlink always runs.
    let inner = || -> Result<RawFd, ConsumeError> {
        // Step 1b: read the file.
        let bytes = fs::read(secret_path).map_err(ConsumeError::Io)?;

        // Step 2: length gate.
        if bytes.len() != expected_len {
            return Err(ConsumeError::WrongLen {
                expected: expected_len,
                actual: bytes.len(),
            });
        }

        // Step 3: create the memfd.
        let memfd_name =
            CString::new("qrab-guest-init-secret").expect("static name has no nul");
        let memfd: OwnedFd = memfd_create(
            memfd_name.as_c_str(),
            MemFdCreateFlag::MFD_CLOEXEC | MemFdCreateFlag::MFD_ALLOW_SEALING,
        )
        .map_err(ConsumeError::MemfdCreate)?;

        // Step 4: write the secret in. Use `nix::unistd::write` so
        // we can keep ownership of `memfd` for the sealing step
        // (converting to `std::fs::File` would consume the `OwnedFd`
        // and we'd have to dup-back just to seal).
        let mut total = 0usize;
        while total < bytes.len() {
            match nix::unistd::write(&memfd, &bytes[total..]) {
                Ok(n) if n > 0 => total += n,
                Ok(_) => {
                    // Zero write with a nonempty buffer is a short-
                    // write failure we can't distinguish from EINTR
                    // on tmpfs. Treat as WriteZero.
                    return Err(ConsumeError::MemfdWrite(
                        std::io::Error::from(std::io::ErrorKind::WriteZero),
                    ));
                }
                Err(e) => {
                    return Err(ConsumeError::MemfdWrite(
                        std::io::Error::from_raw_os_error(e as i32),
                    ));
                }
            }
        }

        // Step 5: seal. Every seal flag in one call so the workload
        // can't shrink, grow, re-seal, or write the secret buffer.
        // nix 0.29's `fcntl` takes `RawFd`, not `AsFd` — we pass
        // `memfd.as_raw_fd()` and the `OwnedFd` keeps the fd alive
        // through the call.
        let seals = SealFlag::F_SEAL_WRITE
            | SealFlag::F_SEAL_SHRINK
            | SealFlag::F_SEAL_GROW
            | SealFlag::F_SEAL_SEAL;
        fcntl(memfd.as_raw_fd(), FcntlArg::F_ADD_SEALS(seals))
            .map_err(ConsumeError::Seal)?;

        // Hand off ownership so the fd outlives `memfd`'s drop.
        Ok(memfd.into_raw_fd())
    };

    let result = inner();

    // Step 6: unlink the source regardless of the inner result (as
    // long as the mode check passed — we already returned above if
    // it didn't). Unlink failure on success-path still surfaces as
    // `Unlink`; on failure-path the original error wins and the
    // unlink failure is silently best-effort (an attacker who can
    // induce a memfd failure AND prevent unlink would be reading
    // the secret via the filesystem anyway, which the workload
    // isn't supposed to do — but we don't want to clobber the
    // original error with a less-informative unlink error).
    let unlink_result = fs::remove_file(secret_path);

    match (result, unlink_result) {
        (Ok(fd), Ok(())) => Ok(fd),
        (Ok(fd), Err(e)) => {
            // Memfd is populated but the file is still on disk. The
            // workload still gets the secret via the fd, but we
            // have to surface the unlink failure — leaving the file
            // behind is exactly the threat the consume step exists
            // to prevent. Close the fd before returning so a caller
            // who ignores the error doesn't leak it.
            // SAFETY: `fd` was just returned by `into_raw_fd` and
            // has not been used since.
            let _ = unsafe { OwnedFd::from_raw_fd(fd) };
            Err(ConsumeError::Unlink(e))
        }
        (Err(e), _) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};

    /// Helper: write `body` to a new file under `dir` at `mode`.
    fn write_at_mode(
        dir: &Path,
        name: &str,
        body: &[u8],
        mode: u32,
    ) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut f = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(mode)
            .open(&path)
            .expect("create");
        f.write_all(body).expect("write");
        // Re-assert the mode — some umasks will have masked it at
        // open time. `set_permissions` is unconditional.
        let mut perms = fs::metadata(&path).expect("meta").permissions();
        perms.set_mode(mode);
        fs::set_permissions(&path, perms).expect("chmod");
        path
    }

    use std::os::unix::fs::OpenOptionsExt;

    #[test]
    fn consume_happy_path() {
        let td = tempfile::tempdir().unwrap();
        let body = b"0123456789abcdef"; // 16 bytes
        let path = write_at_mode(td.path(), "secret", body, 0o600);

        let fd = consume(&path, body.len()).expect("consume ok");

        // Returned FD points at the sealed memfd. Dup it back into
        // a File to read contents + verify size.
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut file = std::fs::File::from(owned);
        let len = file.seek(SeekFrom::End(0)).expect("seek end");
        assert_eq!(len as usize, body.len());
        file.seek(SeekFrom::Start(0)).expect("rewind");
        let mut read_back = Vec::new();
        file.read_to_end(&mut read_back).expect("read");
        assert_eq!(read_back, body);

        // Source file should be gone.
        assert!(!path.exists(), "source secret must be unlinked");
    }

    #[test]
    fn consume_wrong_length() {
        let td = tempfile::tempdir().unwrap();
        // 15 bytes but caller expects 16.
        let path = write_at_mode(td.path(), "secret", b"0123456789abcde", 0o600);
        let err = consume(&path, 16).expect_err("should reject short");
        match err {
            ConsumeError::WrongLen { expected: 16, actual: 15 } => {}
            other => panic!("expected WrongLen {{16,15}}, got {other:?}"),
        }
        // Source still unlinked — the file's mode was 0o600 so it's
        // ours.
        assert!(!path.exists(), "source should still be unlinked");
    }

    #[test]
    fn consume_wrong_mode_0644_rejected() {
        let td = tempfile::tempdir().unwrap();
        let body = b"some-16-byte-blob";
        let path = write_at_mode(td.path(), "secret", body, 0o644);
        let err = consume(&path, body.len()).expect_err("should reject 0644");
        match err {
            ConsumeError::WrongMode { actual: 0o644 } => {}
            other => panic!("expected WrongMode(0o644), got {other:?}"),
        }
        // Source preserved — we only unlink after mode passes.
        assert!(path.exists(), "world-readable file must NOT be unlinked");
    }

    #[test]
    fn consume_missing_file() {
        let td = tempfile::tempdir().unwrap();
        let err = consume(&td.path().join("no-such-secret"), 16)
            .expect_err("should fail");
        match err {
            ConsumeError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            other => panic!("expected Io(NotFound), got {other:?}"),
        }
    }

    #[test]
    fn consume_seals_are_applied() {
        let td = tempfile::tempdir().unwrap();
        let body = b"16-byte-secretXX";
        let path = write_at_mode(td.path(), "secret", body, 0o600);

        let fd = consume(&path, body.len()).expect("consume ok");
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };
        let mut file = std::fs::File::from(owned);
        // F_SEAL_WRITE must make any further write fail with EPERM.
        let err = file.write_all(b"tamper").expect_err("write must fail");
        // Check for EPERM (Operation not permitted = 1). nix re-exports
        // `libc` under `nix::libc`, saving us a direct `libc` dep.
        let kind = err.kind();
        let raw = err.raw_os_error();
        assert!(
            kind == std::io::ErrorKind::PermissionDenied
                || raw == Some(nix::libc::EPERM),
            "expected EPERM / PermissionDenied, got kind={kind:?} raw={raw:?}",
        );
    }

    #[test]
    fn consume_seals_prevent_grow_and_shrink() {
        use nix::unistd::ftruncate;

        let td = tempfile::tempdir().unwrap();
        let body = b"16-byte-secretYY";
        let path = write_at_mode(td.path(), "secret", body, 0o600);

        let fd = consume(&path, body.len()).expect("consume ok");
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };

        // F_SEAL_GROW: ftruncate up must fail.
        let grow = ftruncate(&owned, 32);
        assert!(grow.is_err(), "ftruncate(32) must fail under F_SEAL_GROW");

        // F_SEAL_SHRINK: ftruncate down must fail.
        let shrink = ftruncate(&owned, 8);
        assert!(
            shrink.is_err(),
            "ftruncate(8) must fail under F_SEAL_SHRINK"
        );
    }
}
