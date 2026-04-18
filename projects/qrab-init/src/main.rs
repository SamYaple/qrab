mod config;

use config::InitConfig;
use nix::libc;
use nix::mount::{mount, MsFlags};
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{close, execve, fork, ForkResult, Gid, Pid, Uid};
use std::ffi::{CString, OsString};
use std::fs::File;
use std::io::Read;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::io::FromRawFd;
use std::process;
use std::sync::atomic::{AtomicI32, Ordering};

/// Child PID stored for signal handler to forward signals.
static CHILD_PID: AtomicI32 = AtomicI32::new(-1);

extern "C" fn forward_signal(sig: libc::c_int) {
    let pid = CHILD_PID.load(Ordering::Relaxed);
    if pid > 0 {
        unsafe {
            libc::kill(pid, sig);
        }
    }
}

/// Wait for parent to finish writing uid/gid maps, then close the pipe.
fn wait_for_ack(ack_fd: i32) -> Result<(), String> {
    let mut file = unsafe { File::from_raw_fd(ack_fd) };
    let mut buf = [0u8; 1];
    file.read_exact(&mut buf)
        .map_err(|e| format!("failed to read ack from parent: {e}"))?;
    // File::drop closes the fd
    Ok(())
}

fn read_config(fd: i32) -> Result<InitConfig, String> {
    let mut file = unsafe { File::from_raw_fd(fd) };
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("failed to read config pipe: {e}"))?;
    // File::drop closes the fd
    serde_json::from_slice(&buf).map_err(|e| format!("failed to deserialize config: {e}"))
}

/// Mount minimal /dev inside the mount namespace.
///
/// Creates a fresh tmpfs at /dev with only null, zero, urandom.
/// No /dev/kvm — KVM is accessed via inherited fd only.
#[allow(dead_code)] // TODO: enable when pivot_root-based /dev isolation lands
fn setup_dev() -> Result<(), String> {
    // In user namespaces, the kernel blocks mount propagation back to the
    // initial namespace regardless of propagation type. No need to change
    // propagation on inherited mounts (MS_PRIVATE/MS_SLAVE both EPERM on
    // locked mounts in user namespaces). Just mount directly.

    // Fresh tmpfs for /dev.
    mount(
        Some("tmpfs"),
        "/dev",
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_STRICTATIME,
        Some("mode=755"),
    )
    .map_err(|e| format!("mount tmpfs /dev: {e}"))?;

    // Bind mount essential device nodes.
    for dev in &["null", "zero", "urandom"] {
        let target = format!("/dev/{dev}");
        let source = format!("/dev/{dev}");
        // Create target file for bind mount.
        std::fs::File::create(&target).map_err(|e| format!("create {target}: {e}"))?;
        mount(
            Some(source.as_str()),
            target.as_str(),
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .map_err(|e| format!("bind mount {source} -> {target}: {e}"))?;
    }

    Ok(())
}

#[allow(dead_code)] // TODO: enable when pivot_root-based /proc isolation lands
fn mount_proc() -> Result<(), String> {
    let flags = MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC;
    mount(Some("proc"), "/proc", Some("proc"), flags, None::<&str>)
        .map_err(|e| format!("failed to mount /proc: {e}"))
}

fn build_cstring(s: &[u8]) -> Result<CString, String> {
    CString::new(s.to_vec()).map_err(|e| format!("invalid string for execve: {e}"))
}

fn collect_environ() -> Result<Vec<CString>, String> {
    std::env::vars_os()
        .map(|(k, v)| {
            let mut entry = OsString::new();
            entry.push(&k);
            entry.push("=");
            entry.push(&v);
            build_cstring(&entry.into_vec())
        })
        .collect()
}

fn run() -> Result<(), String> {
    // Parse argv: <config-pipe-fd> <ack-pipe-fd>
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        return Err(format!("usage: {} <config-pipe-fd> <ack-pipe-fd>", args[0]));
    }
    let config_fd: i32 = args[1]
        .parse()
        .map_err(|e| format!("invalid config pipe fd '{}': {e}", args[1]))?;
    let ack_fd: i32 = args[2]
        .parse()
        .map_err(|e| format!("invalid ack pipe fd '{}': {e}", args[2]))?;

    // 1. Wait for parent to write uid/gid maps and send ack.
    //    At this point we're in a user namespace but haven't called
    //    setuid(0) yet — the parent needs to write the maps first.
    wait_for_ack(ack_fd)?;

    // 2. Now uid/gid maps are written — become root in user namespace.
    nix::unistd::setuid(Uid::from_raw(0)).map_err(|e| format!("setuid(0): {e}"))?;
    nix::unistd::setgid(Gid::from_raw(0)).map_err(|e| format!("setgid(0): {e}"))?;

    // 3. /dev isolation (tmpfs + bind mounts) deferred — requires pivot_root
    //    approach for user namespaces where inherited mounts are locked.
    //    QEMU uses host /dev for now. TODO: implement pivot_root-based
    //    /dev isolation in a follow-up.

    // 4. Read and deserialize config.
    let config = read_config(config_fd)?;

    // 5. Fork: child becomes pid 1 in PID namespace, parent is init.
    match unsafe { fork() } {
        Ok(ForkResult::Child) => {
            // /proc remount deferred — same locked-mount EPERM as /dev in
            // user namespaces. QEMU works with inherited /proc (shows parent
            // PID ns, but user ns prevents access to other processes).
            // TODO: implement via pivot_root in follow-up.

            // Dup SEV fd to a low number before execve. QEMU closes all
            // high-numbered fds during startup (close-all-open-fd). Only
            // low fds and -add-fd registered fds survive. The SEV fd gets
            // a high number (269+) when the parent has many open fds
            // (Dioxus server). sev-device=/proc/self/fd/N won't work if
            // QEMU already closed fd N. Duping to a low fd (4) ensures
            // it survives. We also rewrite the QEMU arg that references it.
            const SEV_DUP_FD: i32 = 4;
            let mut args_rewritten = config.qemu_args.clone();
            if let Some(sev_fd) = config.sev_fd {
                if sev_fd != SEV_DUP_FD {
                    nix::unistd::dup2(sev_fd, SEV_DUP_FD)
                        .map_err(|e| format!("dup2 sev_fd {sev_fd} -> {SEV_DUP_FD}: {e}"))?;
                    let _ = close(sev_fd);
                }
                // Rewrite /proc/self/fd/{old} -> /proc/self/fd/4 in args
                let old_ref = format!("/proc/self/fd/{sev_fd}");
                let new_ref = format!("/proc/self/fd/{SEV_DUP_FD}");
                for arg in &mut args_rewritten {
                    if arg.contains(&old_ref) {
                        *arg = arg.replace(&old_ref, &new_ref);
                    }
                }
            }

            // Build argv for execve.
            let binary = build_cstring(config.qemu_binary.as_os_str().as_encoded_bytes())?;

            let mut argv = vec![binary.clone()];
            for arg in &args_rewritten {
                argv.push(
                    CString::new(arg.as_bytes()).map_err(|e| format!("invalid qemu arg: {e}"))?,
                );
            }

            // Collect current environment.
            let environ = collect_environ()?;

            // Debug: dump dup2 result
            eprintln!(
                "qrab-init: CHILD pid={} sev_fd={:?} after dup2",
                std::process::id(),
                config.sev_fd
            );

            // Debug: dump open fds before execve
            if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
                let mut fds: Vec<i32> = entries
                    .filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().to_str().and_then(|s| s.parse().ok()))
                    .collect();
                fds.sort();
                eprintln!("qrab-init: open fds before execve: {:?}", fds);
                if let Some(sev_fd) = config.sev_fd {
                    let dup_fd = 4;
                    let orig_exists =
                        std::path::Path::new(&format!("/proc/self/fd/{sev_fd}")).exists();
                    let dup_exists =
                        std::path::Path::new(&format!("/proc/self/fd/{dup_fd}")).exists();
                    eprintln!("qrab-init: sev_fd={sev_fd} exists={orig_exists} dup_fd={dup_fd} exists={dup_exists}");
                }
            }

            // execve replaces this process with QEMU.
            execve(&binary, &argv, &environ).map_err(|e| format!("execve failed: {e}"))?;

            unreachable!()
        }
        Ok(ForkResult::Parent { child }) => {
            // Close device fds if present — only QEMU child needs them.
            if let Some(kvm_fd) = config.kvm_fd {
                let _ = close(kvm_fd);
            }
            if let Some(sev_fd) = config.sev_fd {
                let _ = close(sev_fd);
            }

            // Store child pid for signal handler.
            CHILD_PID.store(child.as_raw(), Ordering::Relaxed);

            // Install signal handlers that forward to child.
            let sa = SigAction::new(
                SigHandler::Handler(forward_signal),
                SaFlags::SA_RESTART,
                SigSet::empty(),
            );
            unsafe {
                sigaction(Signal::SIGTERM, &sa).map_err(|e| format!("sigaction SIGTERM: {e}"))?;
                sigaction(Signal::SIGINT, &sa).map_err(|e| format!("sigaction SIGINT: {e}"))?;
            }

            // Reap loop — wait for all children.
            loop {
                match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::empty())) {
                    Ok(WaitStatus::Exited(pid, code)) if pid == child => {
                        process::exit(code);
                    }
                    Ok(WaitStatus::Signaled(pid, sig, _)) if pid == child => {
                        process::exit(128 + sig as i32);
                    }
                    Ok(_) => {
                        // Other child reaped (zombie), continue.
                    }
                    Err(nix::errno::Errno::ECHILD) => {
                        eprintln!("qrab-init: no children remaining, exiting");
                        process::exit(1);
                    }
                    Err(nix::errno::Errno::EINTR) => {
                        // Interrupted by signal, continue.
                    }
                    Err(e) => {
                        eprintln!("qrab-init: waitpid error: {e}");
                        process::exit(1);
                    }
                }
            }
        }
        Err(e) => Err(format!("fork failed: {e}")),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("qrab-init: {e}");
        process::exit(1);
    }
}
