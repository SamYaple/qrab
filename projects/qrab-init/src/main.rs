mod config;

use config::InitConfig;
use nix::libc;
use nix::mount::{mount, MsFlags};
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{close, execve, fork, ForkResult, Pid};
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

fn read_config(fd: i32) -> Result<InitConfig, String> {
    let mut file = unsafe { File::from_raw_fd(fd) };
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("failed to read config pipe: {e}"))?;
    // File::drop closes the fd
    serde_json::from_slice(&buf).map_err(|e| format!("failed to deserialize config: {e}"))
}

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
    // Parse argv[1] as config pipe fd
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        return Err(format!("usage: {} <config-pipe-fd>", args[0]));
    }
    let config_fd: i32 = args[1]
        .parse()
        .map_err(|e| format!("invalid config pipe fd '{}': {e}", args[1]))?;

    // Read and deserialize config (File::drop closes fd)
    let config = read_config(config_fd)?;

    // Fork: child becomes pid 1 in PID namespace, parent is init
    match unsafe { fork() } {
        Ok(ForkResult::Child) => {
            // Mount /proc inside PID namespace
            mount_proc()?;

            // Build argv for execve
            let binary = build_cstring(config.qemu_binary.as_os_str().as_encoded_bytes())?;

            let mut argv = vec![binary.clone()];
            for arg in &config.qemu_args {
                argv.push(
                    CString::new(arg.as_bytes()).map_err(|e| format!("invalid qemu arg: {e}"))?,
                );
            }

            // Collect current environment
            let environ = collect_environ()?;

            // execve replaces this process with QEMU
            execve(&binary, &argv, &environ).map_err(|e| format!("execve failed: {e}"))?;

            unreachable!()
        }
        Ok(ForkResult::Parent { child }) => {
            // Close KVM fd if present — only QEMU child needs it
            if let Some(kvm_fd) = config.kvm_fd {
                let _ = close(kvm_fd);
            }

            // Store child pid for signal handler
            CHILD_PID.store(child.as_raw(), Ordering::Relaxed);

            // Install signal handlers that forward to child
            let sa = SigAction::new(
                SigHandler::Handler(forward_signal),
                SaFlags::SA_RESTART,
                SigSet::empty(),
            );
            unsafe {
                sigaction(Signal::SIGTERM, &sa).map_err(|e| format!("sigaction SIGTERM: {e}"))?;
                sigaction(Signal::SIGINT, &sa).map_err(|e| format!("sigaction SIGINT: {e}"))?;
            }

            // Reap loop — wait for all children
            loop {
                match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::empty())) {
                    Ok(WaitStatus::Exited(pid, code)) if pid == child => {
                        process::exit(code);
                    }
                    Ok(WaitStatus::Signaled(pid, sig, _)) if pid == child => {
                        process::exit(128 + sig as i32);
                    }
                    Ok(_) => {
                        // Other child reaped (zombie), continue
                    }
                    Err(nix::errno::Errno::ECHILD) => {
                        // No more children — shouldn't happen if QEMU is running,
                        // but exit cleanly
                        eprintln!("qrab-init: no children remaining, exiting");
                        process::exit(1);
                    }
                    Err(nix::errno::Errno::EINTR) => {
                        // Interrupted by signal, continue
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
