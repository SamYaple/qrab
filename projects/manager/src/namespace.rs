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
    /// Run the uid/gid map writer. Consumes the pipe fds.
    pub async fn run(sync_r: RawFd, ack_w: RawFd) -> crate::Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Read child PID (written as decimal string bytes).
        let mut sync_file = unsafe { tokio_fd::tokio_file_from_raw_fd(sync_r) };
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
        let mut ack_file = unsafe { tokio_fd::tokio_file_from_raw_fd(ack_w) };
        ack_file.write_all(&[0x42]).await?;

        // Don't let OwnedFd in tokio::fs::File double-close — we consumed
        // them via from_raw_fd. Drop handles close.

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
    cfg_r: RawFd,
) -> impl FnMut() -> std::io::Result<()> {
    move || {
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

        // NOTE: setuid/setgid/setup_dev moved to qrab-init (after ack).
        // Cannot block here — spawn() waits for exec to complete.

        Ok(())
    }
}

/// Use `tokio::fs::File::from_raw_fd` — re-export the conversion for
/// NsHelper. This is a workaround because `tokio::fs::File` doesn't
/// impl `FromRawFd` directly; we go through `std::fs::File` first.
mod tokio_fd {
    use std::os::fd::{FromRawFd, RawFd};

    /// # Safety
    /// The caller must ensure `fd` is a valid, open file descriptor that
    /// is not owned by another object.
    pub unsafe fn tokio_file_from_raw_fd(fd: RawFd) -> tokio::fs::File {
        let std_file = std::fs::File::from_raw_fd(fd);
        tokio::fs::File::from_std(std_file)
    }
}
