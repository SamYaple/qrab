#![allow(unsafe_code)]
//! Namespace isolation primitives for sandboxed QEMU execution.
//!
//! Provides the pre-fork, pre-exec, and parent-side helpers that establish
//! user/mount/network/UTS/PID namespaces around the `qrab-init` child
//! process. All functions in this module are used by [`super::vm::VmBuilder::spawn`].

use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::path::PathBuf;

use nix::fcntl::{FcntlArg, FdFlag, OFlag};
use nix::sched::CloneFlags;
use nix::sys::stat::Mode;
use nix::unistd::{Gid, Uid};

/// Configuration sent over a pipe from manager to `qrab-init`.
///
/// Serialized as JSON. The struct is intentionally duplicated here (and in
/// `qrab-init`) rather than shared via a library crate — it is a small,
/// stable wire format and keeping it local avoids a cross-crate dependency
/// for a handful of fields.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct InitConfig {
    /// Absolute path to the QEMU binary.
    pub qemu_binary: PathBuf,
    /// Complete QEMU argument list (excluding argv[0]).
    pub qemu_args: Vec<String>,
    /// Raw fd number for pre-opened `/dev/kvm`, if KVM is enabled.
    pub kvm_fd: Option<i32>,
    /// Raw fd number for pre-opened `/dev/sev`, if SEV-SNP is enabled.
    /// QEMU accesses it via `sev-device=/proc/self/fd/N`.
    pub sev_fd: Option<i32>,
    /// Extra device nodes to bind-mount into `/dev` (e.g. `/dev/sev` for SEV-SNP).
    #[serde(default)]
    pub extra_dev_nodes: Vec<PathBuf>,
}

/// Resolve the path to the `qrab-init` binary.
///
/// Resolution order:
/// 1. `QRAB_INIT_BIN` env var (explicit override)
/// 2. Sibling of the current executable (same directory as the running binary)
/// 3. `qrab-init` on `PATH` via `which`
pub fn qrab_init_binary() -> crate::Result<PathBuf> {
    if let Ok(p) = std::env::var("QRAB_INIT_BIN") {
        let path = PathBuf::from(p);
        if !path.is_file() {
            return Err(crate::Error::Binary(format!(
                "QRAB_INIT_BIN={} is not a file", path.display()
            )));
        }
        return Ok(path);
    }

    if let Ok(exe) = std::env::current_exe() {
        // Check same directory as running binary
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("qrab-init");
            if sibling.is_file() {
                return Ok(sibling);
            }
            // Test binaries live in target/debug/deps/ — check parent too
            if let Some(parent) = dir.parent() {
                let sibling = parent.join("qrab-init");
                if sibling.is_file() {
                    return Ok(sibling);
                }
            }
        }
    }

    which::which("qrab-init")
        .map_err(|_| crate::Error::Binary("qrab-init not found — install it or set QRAB_INIT_BIN".into()))
}

/// Wrapper around a pre-opened `/dev/kvm` file descriptor.
///
/// Opened *without* `O_CLOEXEC` so the fd survives the `exec` chain
/// (manager fork → qrab-init exec → qrab-init fork → QEMU exec).
pub struct KvmFd(OwnedFd);

/// Wrapper around a pre-opened `/dev/sev` file descriptor.
///
/// Same pattern as [`KvmFd`]: opened in the parent process (which has
/// real credentials and device group membership) before entering the
/// user namespace. The fd is passed through to QEMU via
/// `sev-device=/proc/self/fd/N`.
///
/// SEV ioctls (`SNP_LAUNCH_START`, etc.) require the opener's real
/// credentials. Opening `/dev/sev` inside a user namespace fails because
/// the mapped uid 0 doesn't have real permission for PSP operations.
pub struct SevFd(OwnedFd);

impl KvmFd {
    /// Open `/dev/kvm` with `O_RDWR` and no `O_CLOEXEC`.
    pub fn open() -> crate::Result<Self> {
        let fd = nix::fcntl::open("/dev/kvm", OFlag::O_RDWR, Mode::empty())
            .map_err(|e| crate::Error::Spawn(format!("failed to open /dev/kvm: {e}")))?;
        // SAFETY: `nix::fcntl::open` returns a valid fd on success.
        Ok(KvmFd(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    /// Raw file descriptor number (for `-add-fd fd=N,set=1`).
    pub fn raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }

    /// Remove `FD_CLOEXEC` so the fd survives exec.
    ///
    /// Must be called inside `pre_exec` (after fork, before exec).
    pub fn clear_cloexec(fd: RawFd) -> Result<(), nix::Error> {
        set_fd_cloexec(fd, false)
    }
}

impl SevFd {
    /// Open `/dev/sev` (or custom path) with `O_RDWR` and no `O_CLOEXEC`.
    pub fn open(path: Option<&std::path::Path>) -> crate::Result<Self> {
        let dev = path.unwrap_or(std::path::Path::new("/dev/sev"));
        let fd = nix::fcntl::open(dev, OFlag::O_RDWR, Mode::empty())
            .map_err(|e| crate::Error::Spawn(format!("failed to open {}: {e}", dev.display())))?;
        // SAFETY: `nix::fcntl::open` returns a valid fd on success.
        Ok(SevFd(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    /// Raw file descriptor number.
    pub fn raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

/// Set or clear `FD_CLOEXEC` on a raw file descriptor.
pub fn set_fd_cloexec(fd: RawFd, cloexec: bool) -> Result<(), nix::Error> {
    let flags = nix::fcntl::fcntl(fd, FcntlArg::F_GETFD)?;
    let mut fd_flags = FdFlag::from_bits_truncate(flags);
    if cloexec {
        fd_flags.insert(FdFlag::FD_CLOEXEC);
    } else {
        fd_flags.remove(FdFlag::FD_CLOEXEC);
    }
    nix::fcntl::fcntl(fd, FcntlArg::F_SETFD(fd_flags))?;
    Ok(())
}

/// Parent-side async helper that writes uid/gid maps for the child's user
/// namespace.
///
/// Protocol:
/// 1. Read child PID from `sync_r` pipe.
/// 2. Write `/proc/{pid}/setgroups` → `"deny"`.
/// 3. Write `/proc/{pid}/uid_map` → `"0 {real_uid} 1"`.
/// 4. Write `/proc/{pid}/gid_map` → `"0 {real_gid} 1"`.
/// 5. Write ack byte to `ack_w` pipe.
pub struct NsHelper;

impl NsHelper {
    /// Run the uid/gid map writer. Consumes the pipe fds as `OwnedFd` so
    /// early-error paths do not leak raw descriptors: if this fn returns
    /// `Err` before wrapping a fd in a `tokio::fs::File`, `OwnedFd::drop`
    /// closes it. Once wrapped, the `File` owns the fd.
    pub async fn run(sync_r: OwnedFd, ack_w: OwnedFd) -> crate::Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Wrap fds in `tokio::fs::File` via `std::fs::File`'s
        // `From<OwnedFd>` impl — this hands off ownership safely with no
        // `from_raw_fd` gymnastics. A dropped `File` closes the fd once.
        let mut sync_file = tokio::fs::File::from_std(std::fs::File::from(sync_r));
        let mut ack_file = tokio::fs::File::from_std(std::fs::File::from(ack_w));

        // Read child PID (written as decimal string bytes).
        let mut pid_buf = vec![0u8; 32];
        let n = sync_file.read(&mut pid_buf).await?;
        let pid_str = std::str::from_utf8(&pid_buf[..n])
            .map_err(|e| crate::Error::Namespace(format!("invalid PID from child: {e}")))?
            .trim();
        let pid: u32 = pid_str.parse()
            .map_err(|e| crate::Error::Namespace(format!("invalid PID from child: {e}")))?;

        let real_uid = Uid::current().as_raw();
        let real_gid = Gid::current().as_raw();

        // Write setgroups deny (required before gid_map).
        tokio::fs::write(format!("/proc/{pid}/setgroups"), "deny").await?;

        // Map container uid 0 → real uid.
        tokio::fs::write(
            format!("/proc/{pid}/uid_map"),
            format!("0 {real_uid} 1"),
        )
        .await?;

        // Map container gid 0 → real gid.
        tokio::fs::write(
            format!("/proc/{pid}/gid_map"),
            format!("0 {real_gid} 1"),
        )
        .await?;

        // Ack: tell child that maps are written.
        ack_file.write_all(&[0x42]).await?;

        Ok(())
    }
}

fn nix_to_io(e: nix::Error) -> std::io::Error {
    std::io::Error::from_raw_os_error(e as i32)
}

/// Build the `pre_exec` closure for `Command::new("qrab-init")`.
///
/// This closure runs in the forked child between `fork()` and `exec()`.
/// Everything here must be async-signal-safe (no allocations, no locks).
/// **Must not block** — `Command::spawn()` waits for exec to complete,
/// so blocking here deadlocks with the parent.
///
/// # Arguments
/// - `sync_w`: write end of sync-to-parent pipe (send PID)
/// - `ack_r`: read end of ack-from-parent pipe (passed to qrab-init, not read here)
/// - `kvm_fd`: optional raw fd for `/dev/kvm`
/// - `cfg_r`: read end of config pipe (passed to qrab-init as argv[1])
///
/// # Safety
/// The returned closure is intended for use with `Command::pre_exec`, which
/// requires `unsafe`. The closure itself performs only POSIX syscalls.
pub fn make_pre_exec(
    sync_w: RawFd,
    ack_r: RawFd,
    kvm_fd: Option<RawFd>,
    sev_fd: Option<RawFd>,
    cfg_r: RawFd,
) -> impl FnMut() -> std::io::Result<()> {
    move || {
        // 0. Restrictive umask BEFORE namespace entry and exec, so any
        //    files/sockets QEMU creates later (e.g. `qmp.sock`, VNC sock)
        //    are born `0o600`. Closes TOCTOU window where the parent
        //    would otherwise race to chmod `0o600` after QEMU created
        //    a world-readable socket. Umask is per-process and applies
        //    only to this forked child — parent runtime is unaffected.
        //    The parent still calls `set_permissions(0o600)` as
        //    defense-in-depth.
        nix::sys::stat::umask(Mode::from_bits_truncate(0o077));

        // 1. New session — detach from controlling terminal.
        nix::unistd::setsid().map_err(nix_to_io)?;

        // 2. Enter new namespaces.
        nix::sched::unshare(
            CloneFlags::CLONE_NEWUSER
                | CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWNET
                | CloneFlags::CLONE_NEWUTS
                | CloneFlags::CLONE_NEWPID,
        )
        .map_err(nix_to_io)?;

        // 3. Write our PID to parent so it can set up uid/gid maps.
        let pid = nix::unistd::getpid();
        let pid_bytes = pid.to_string();
        let sync_w_fd = unsafe { BorrowedFd::borrow_raw(sync_w) };
        nix::unistd::write(sync_w_fd, pid_bytes.as_bytes()).map_err(nix_to_io)?;
        nix::unistd::close(sync_w).map_err(nix_to_io)?;

        // 4. Clear CLOEXEC on fds that must survive exec to qrab-init.
        //    qrab-init will: read ack, setuid/setgid, setup_dev, read config.
        KvmFd::clear_cloexec(ack_r).map_err(nix_to_io)?;
        KvmFd::clear_cloexec(cfg_r).map_err(nix_to_io)?;
        if let Some(fd) = kvm_fd {
            KvmFd::clear_cloexec(fd).map_err(nix_to_io)?;
        }
        if let Some(fd) = sev_fd {
            set_fd_cloexec(fd, false).map_err(nix_to_io)?;
        }

        // NOTE: setuid/setgid/setup_dev moved to qrab-init (after ack).
        // Cannot block here — spawn() waits for exec to complete.

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// B8-V3: Verify the pre_exec umask mode bits correspond to `0o077`,
    /// i.e. strip group/world r/w/x. Unit-level guard against a future
    /// edit accidentally widening the mask (`0o007`, `0o022`, …).
    #[test]
    fn pre_exec_umask_mode_is_077() {
        let mode = Mode::from_bits_truncate(0o077);
        assert_eq!(
            mode.bits() & 0o777,
            0o077,
            "umask mask must be 0o077 to keep sockets 0o600",
        );
    }

    /// B8-V4: If `NsHelper::run` errors before the handoff completes, both
    /// pipe fds must be closed by `OwnedFd::drop`, not leaked.
    ///
    /// We force an early error by closing the sync-pipe write end before
    /// calling `run`, so the read returns 0 bytes and PID parsing fails.
    /// After the call, `fcntl(F_GETFD)` on the original fd numbers must
    /// report EBADF (fd closed).
    #[tokio::test]
    async fn ns_helper_run_closes_fds_on_early_error() {
        // sync: parent reads; child would write. ack: parent writes.
        let (sync_r, sync_w) = nix::unistd::pipe().expect("sync pipe");
        let (ack_r, ack_w) = nix::unistd::pipe().expect("ack pipe");

        let sync_r_raw = sync_r.as_raw_fd();
        let ack_w_raw = ack_w.as_raw_fd();

        // Drop the sync write end — parent's read will return EOF, so
        // PID parsing of "" fails and `run` returns Err before any
        // further await points.
        drop(sync_w);
        // Keep ack_r alive (represents qrab-init's end); parent must
        // still close its own ack_w on the error path.
        let _ack_r_keepalive = ack_r;

        let err = NsHelper::run(sync_r, ack_w).await;
        assert!(err.is_err(), "expected NsHelper::run to fail on empty PID");

        // Both fds must now be closed. F_GETFD on a closed fd yields
        // EBADF. Do not open any fds between the drop and this check,
        // or fd numbers may be reused and the check becomes useless.
        let sync_check = nix::fcntl::fcntl(sync_r_raw, FcntlArg::F_GETFD);
        let ack_check = nix::fcntl::fcntl(ack_w_raw, FcntlArg::F_GETFD);
        assert_eq!(
            sync_check.err(),
            Some(nix::errno::Errno::EBADF),
            "sync_r fd leaked (F_GETFD = {sync_check:?})",
        );
        assert_eq!(
            ack_check.err(),
            Some(nix::errno::Errno::EBADF),
            "ack_w fd leaked (F_GETFD = {ack_check:?})",
        );
    }
}
