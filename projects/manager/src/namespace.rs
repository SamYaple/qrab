//! Namespace isolation primitives for sandboxed QEMU execution.
//!
//! Provides the pre-fork, pre-exec, and parent-side helpers that establish
//! user/mount/network/UTS/PID namespaces around the `qrab-init` child
//! process. All functions in this module are used by [`super::vm::VmBuilder::spawn`].

use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::path::PathBuf;

use nix::fcntl::{FcntlArg, FdFlag, OFlag};
use nix::mount::{mount, MsFlags};
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

/// Wrapper around a pre-opened `/dev/kvm` file descriptor.
///
/// Opened *without* `O_CLOEXEC` so the fd survives the `exec` chain
/// (manager fork → qrab-init exec → qrab-init fork → QEMU exec).
pub struct KvmFd(OwnedFd);

impl KvmFd {
    /// Open `/dev/kvm` with `O_RDWR` and no `O_CLOEXEC`.
    pub fn open() -> anyhow::Result<Self> {
        let fd = nix::fcntl::open("/dev/kvm", OFlag::O_RDWR, Mode::empty())
            .map_err(|e| anyhow::anyhow!("failed to open /dev/kvm: {e}"))?;
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
        let flags = nix::fcntl::fcntl(fd, FcntlArg::F_GETFD)?;
        let mut fd_flags = FdFlag::from_bits_truncate(flags);
        fd_flags.remove(FdFlag::FD_CLOEXEC);
        nix::fcntl::fcntl(fd, FcntlArg::F_SETFD(fd_flags))?;
        Ok(())
    }
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
    pub async fn run(sync_r: RawFd, ack_w: RawFd) -> anyhow::Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Read child PID (written as decimal string bytes).
        let mut sync_file = unsafe { tokio_fd::tokio_file_from_raw_fd(sync_r) };
        let mut pid_buf = vec![0u8; 32];
        let n = sync_file.read(&mut pid_buf).await?;
        let pid_str = std::str::from_utf8(&pid_buf[..n])?.trim();
        let pid: u32 = pid_str.parse()?;

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

/// Mount a minimal `/dev` inside the mount namespace.
///
/// Called from `pre_exec` after `setuid(0)` in the user namespace, which
/// grants `CAP_SYS_ADMIN` for mount operations. Creates:
/// - tmpfs at `/dev` (replaces host `/dev`)
/// - bind mounts for `/dev/null`, `/dev/zero`, `/dev/urandom`
///
/// No `/dev/kvm` — KVM is accessed exclusively via inherited fd.
pub fn setup_dev() -> Result<(), nix::Error> {
    // Prevent mount propagation to host.
    mount(
        None::<&str>,
        "/",
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None::<&str>,
    )?;

    // Fresh tmpfs for /dev.
    mount(
        Some("tmpfs"),
        "/dev",
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_STRICTATIME,
        Some("mode=755"),
    )?;

    // Bind mount essential device nodes.
    bind_mount_dev("/dev/null")?;
    bind_mount_dev("/dev/zero")?;
    bind_mount_dev("/dev/urandom")?;

    Ok(())
}

/// Create a file at `path` inside the new `/dev` tmpfs and bind-mount
/// the host device node onto it.
fn bind_mount_dev(path: &str) -> Result<(), nix::Error> {
    // The target file must exist for bind mount. Create it in the tmpfs.
    // We're in a mount namespace with a fresh tmpfs /dev, so this creates
    // in the namespace-local tmpfs.
    std::fs::File::create(path).map_err(|_| nix::Error::ENOENT)?;
    mount(
        Some(path),
        path,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    )?;
    Ok(())
}

fn nix_to_io(e: nix::Error) -> std::io::Error {
    std::io::Error::from_raw_os_error(e as i32)
}

/// Build the `pre_exec` closure for `Command::new("qrab-init")`.
///
/// This closure runs in the forked child between `fork()` and `exec()`.
/// Everything here must be async-signal-safe (no allocations, no locks).
///
/// # Arguments
/// - `sync_w`: write end of sync-to-parent pipe (send PID)
/// - `sync_r`: read end of sync-from-parent pipe (receive ack)
/// - `kvm_fd`: optional raw fd for `/dev/kvm`
/// - `cfg_r`: read end of config pipe (passed to qrab-init as argv[1])
///
/// # Safety
/// The returned closure is intended for use with `Command::pre_exec`, which
/// requires `unsafe`. The closure itself performs only POSIX syscalls.
pub fn make_pre_exec(
    sync_w: RawFd,
    sync_r: RawFd,
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
        // SAFETY: sync_w is a valid fd inherited from parent, not owned
        // by any other object in this post-fork context.
        let sync_w_fd = unsafe { BorrowedFd::borrow_raw(sync_w) };
        nix::unistd::write(sync_w_fd, pid_bytes.as_bytes()).map_err(nix_to_io)?;
        nix::unistd::close(sync_w).map_err(nix_to_io)?;

        // 4. Wait for parent ack (uid/gid maps written).
        let mut ack = [0u8; 1];
        nix::unistd::read(sync_r, &mut ack).map_err(nix_to_io)?;
        nix::unistd::close(sync_r).map_err(nix_to_io)?;

        // 5. Now root in user namespace — set uid/gid.
        nix::unistd::setuid(Uid::from_raw(0)).map_err(nix_to_io)?;
        nix::unistd::setgid(Gid::from_raw(0)).map_err(nix_to_io)?;

        // 6. Mount minimal /dev.
        setup_dev().map_err(nix_to_io)?;

        // 7. Clear CLOEXEC on fds that must survive exec to qrab-init.
        if let Some(fd) = kvm_fd {
            KvmFd::clear_cloexec(fd).map_err(nix_to_io)?;
        }
        KvmFd::clear_cloexec(cfg_r).map_err(nix_to_io)?;

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
