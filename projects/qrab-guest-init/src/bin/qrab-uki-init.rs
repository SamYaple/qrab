//! qrab-uki-init — minimal PID-1 that prepares the UKI initramfs
//! before exec'ing `qrab-guest-init`.
//!
//! Named `qrab-uki-init` to avoid a binary collision with
//! `projects/qrab-init/` (the manager-spawned namespace-isolation
//! PID 1). Inside the UKI it is exec'd as `/init` by the kernel —
//! the binary name only matters to the dev build tree.
//!
//! Layout when linked into `target/qrab-uki/qrab.efi`:
//!
//!   /init              → this binary (kernel-default init path)
//!   /sbin/qrab-guest-init
//!   /sbin/next-stage   (smoke stub; real deployments replace this)
//!
//! Responsibilities, in order:
//!   1. Mount procfs, sysfs, devtmpfs, tmpfs on `/run`, and efivarfs.
//!   2. Stage `QrabReport` / `QrabSession` / `QrabSecret` from
//!      `/sys/firmware/efi/efivars/` into `/run/qrab/`.
//!   3. `execve("/sbin/qrab-guest-init", ...)` with
//!      `QRAB_GUEST_INIT_LIVE=1`. After this point PID 1 is the
//!      sequencer itself.
//!
//! Every step emits one stderr token so the serial console reveals
//! which mount / stage failed. Fatal errors `reboot(LINUX_REBOOT_CMD_
//! POWER_OFF)` rather than returning from PID 1 (which would panic
//! the kernel).
//!
//! Non-goals: dev hotplug, udev, module loading, kernel cmdline
//! parsing. The UKI already bakes in the modules it needs; anything
//! else is a policy decision that belongs in `qrab-guest-init`.

use std::env;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process;

use nix::mount::{MsFlags, mount};
use nix::sys::reboot::{RebootMode, reboot};
use nix::unistd::execve;
use qrab_guest_init::stash::{stage_public, stage_secret};

/// EFI global namespace GUID — must match the layout `qrab-boot`
/// uses when publishing the variables. See
/// `projects/qrab-boot/src/uki.rs::QRAB_REPORT_GUID`.
const QRAB_GUID: &str = "6e6f1cba-b5ba-4e54-a3b3-2a7a8f3d9b00";

const EFIVARFS_DIR: &str = "/sys/firmware/efi/efivars";
const RUN_DIR: &str = "/run/qrab";
const NEXT_BINARY: &str = "/sbin/qrab-guest-init";

fn main() {
    eprintln!("QRAB_INIT_START pid={}", process::id());

    if let Err(code) = run() {
        eprintln!("QRAB_INIT_FATAL code={code}");
        // Give the kernel time to flush the message, then power off.
        // Returning from PID 1 panics the kernel; sysrq-friendly
        // shutdown is less scary for anyone reading the serial.
        let _ = reboot(RebootMode::RB_POWER_OFF);
        // If reboot(2) returns (it shouldn't), loop so the kernel
        // attributes the panic to us, not to kthread exits.
        loop {
            std::thread::park();
        }
    }
}

fn run() -> Result<(), i32> {
    mount_pseudo_fs().map_err(|_| 1)?;
    stage_efivars();
    exec_guest_init().map_err(|_| 2)?;
    Ok(())
}

/// Mount the five pseudo-filesystems the rest of the boot path needs.
/// Each failure is non-fatal *individually* — e.g. on a dev host the
/// initramfs may inherit an already-mounted `/proc`; EBUSY there is
/// fine. efivarfs being missing is tolerated because the staging
/// step already reports `QRAB_INIT_EFIVARFS_ABSENT`.
fn mount_pseudo_fs() -> Result<(), nix::Error> {
    struct Spec {
        src: &'static str,
        target: &'static str,
        fstype: Option<&'static str>,
        flags: MsFlags,
    }

    let specs = [
        Spec {
            src: "proc",
            target: "/proc",
            fstype: Some("proc"),
            flags: MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
        },
        Spec {
            src: "sys",
            target: "/sys",
            fstype: Some("sysfs"),
            flags: MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
        },
        Spec {
            src: "dev",
            target: "/dev",
            fstype: Some("devtmpfs"),
            flags: MsFlags::MS_NOSUID,
        },
        Spec {
            src: "run",
            target: "/run",
            fstype: Some("tmpfs"),
            flags: MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
        },
        Spec {
            src: "efivarfs",
            target: "/sys/firmware/efi/efivars",
            fstype: Some("efivarfs"),
            flags: MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV,
        },
    ];

    for spec in specs {
        let _ = std::fs::create_dir_all(spec.target);
        let data: Option<&[u8]> = None;
        match mount(Some(spec.src), spec.target, spec.fstype, spec.flags, data) {
            Ok(()) => eprintln!("QRAB_INIT_MOUNT_OK {}", spec.target),
            Err(nix::Error::EBUSY) => {
                eprintln!("QRAB_INIT_MOUNT_ALREADY {}", spec.target);
            }
            Err(e) => {
                eprintln!("QRAB_INIT_MOUNT_ERR {} errno={}", spec.target, e);
            }
        }
    }
    Ok(())
}

/// Stage the three efivars into `/run/qrab/`. Logs a token for each.
fn stage_efivars() {
    let efi = Path::new(EFIVARFS_DIR);
    if !efi.exists() {
        eprintln!("QRAB_INIT_EFIVARFS_ABSENT");
        return;
    }
    let _ = std::fs::create_dir_all(RUN_DIR);
    let _ = std::fs::set_permissions(
        RUN_DIR,
        std::fs::Permissions::from_mode_local(0o700),
    );

    stage_one_public("QrabReport", "report");
    stage_one_public("QrabSession", "session");
    stage_one_secret();
}

fn stage_one_public(var_name: &str, dst_basename: &str) {
    let src = PathBuf::from(EFIVARFS_DIR).join(format!("{var_name}-{QRAB_GUID}"));
    let dst = PathBuf::from(RUN_DIR).join(dst_basename);
    if !src.exists() {
        eprintln!("QRAB_INIT_{var_name}_ABSENT");
        return;
    }
    match stage_public(&src, &dst) {
        Ok(n) => eprintln!("QRAB_INIT_{var_name}_OK len={n}"),
        Err(e) => eprintln!("QRAB_INIT_{var_name}_ERR {:?}", e),
    }
}

fn stage_one_secret() {
    let src = PathBuf::from(EFIVARFS_DIR).join(format!("QrabSecret-{QRAB_GUID}"));
    let dst = PathBuf::from(RUN_DIR).join("secret");
    if !src.exists() {
        eprintln!("QRAB_INIT_QrabSecret_ABSENT");
        return;
    }
    match stage_secret(&src, &dst) {
        Ok(n) => eprintln!("QRAB_INIT_QrabSecret_OK len={n}"),
        Err(e) => eprintln!("QRAB_INIT_QrabSecret_ERR {:?}", e),
    }
}

/// execve into `qrab-guest-init`. On success this function does NOT
/// return; on failure we bubble up and PID 1 powers off.
fn exec_guest_init() -> Result<(), nix::Error> {
    let bin = CString::new(NEXT_BINARY).unwrap();
    let argv = vec![CString::new("qrab-guest-init").unwrap()];

    let mut envp: Vec<CString> = env::vars_os()
        .filter_map(|(k, v)| {
            let mut buf = Vec::with_capacity(k.len() + 1 + v.len());
            buf.extend_from_slice(k.as_bytes());
            buf.push(b'=');
            buf.extend_from_slice(v.as_bytes());
            CString::new(buf).ok()
        })
        .collect();
    // Flip the sequencer into live mode. The real-probe runs only
    // under this flag; dry-run remains the default for `cargo run
    // --bin qrab-guest-init` on a dev box.
    envp.push(CString::new("QRAB_GUEST_INIT_LIVE=1").unwrap());

    eprintln!("QRAB_INIT_EXEC {}", NEXT_BINARY);
    let _ = execve(&bin, &argv, &envp)?;
    // execve(2) returns only on error, but the type system requires
    // a return value; on success this line is unreachable.
    Ok(())
}

// Small local helper trait because `std::fs::Permissions::from_mode`
// lives in `PermissionsExt` which would otherwise need a second
// `use` at the call site above — keep the bin file lean.
trait PermissionsFromMode {
    fn from_mode_local(mode: u32) -> Self;
}

impl PermissionsFromMode for std::fs::Permissions {
    fn from_mode_local(mode: u32) -> Self {
        use std::os::unix::fs::PermissionsExt;
        std::fs::Permissions::from_mode(mode)
    }
}
