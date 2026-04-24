//! qrab-next-stage — smoke-test stub for the UKI E2E matrix scenario.
//!
//! `qrab-guest-init` reaches the end of its 22-step gate, seals the
//! secret into a memfd, and execve's this binary with
//! `QRAB_SECRET_FD=<n>` in envp. Production deployments replace this
//! stub with the workload service manager; the smoke test only needs
//! one token on the serial to declare success.
//!
//! Contract:
//!   * Read `QRAB_SECRET_FD` from env; if unset, we were not handed
//!     the sealed secret → emit `QRAB_NEXT_STAGE_NO_FD` and halt.
//!   * Read the fd to EOF and print the byte count.
//!   * Emit `QRAB_NEXT_STAGE_OK len=<n>` and power the VM off via
//!     `reboot(LINUX_REBOOT_CMD_POWER_OFF)`.
//!
//! We deliberately do NOT leak the secret bytes to stderr — only the
//! length. Logging the plaintext would defeat the whole point of the
//! sealed-memfd handoff.

use std::env;
use std::fs::File;
use std::io::Read;
use std::os::fd::FromRawFd;
use std::process;

use nix::sys::reboot::{RebootMode, reboot};

fn main() {
    eprintln!("QRAB_NEXT_STAGE_START pid={}", process::id());

    let fd = match env::var("QRAB_SECRET_FD") {
        Ok(s) => s.parse::<i32>().ok(),
        Err(_) => None,
    };
    match fd {
        Some(n) if n > 2 => {
            // SAFETY: the sequencer seals the memfd before handing us
            // its descriptor; after reading to EOF we drop the File,
            // closing the fd. No other process has it (it was never
            // F_DUPFD'd).
            let mut f = unsafe { File::from_raw_fd(n) };
            let mut buf = Vec::new();
            match f.read_to_end(&mut buf) {
                Ok(_) => eprintln!("QRAB_NEXT_STAGE_OK len={}", buf.len()),
                Err(e) => eprintln!("QRAB_NEXT_STAGE_READ_ERR {}", e),
            }
        }
        Some(n) => eprintln!("QRAB_NEXT_STAGE_BAD_FD {}", n),
        None => eprintln!("QRAB_NEXT_STAGE_NO_FD"),
    }

    // Power off. Kernel prints a shutdown banner after this returns,
    // which QEMU turns into `system_powerdown`.
    let _ = reboot(RebootMode::RB_POWER_OFF);
    // If reboot failed (e.g. CAP_SYS_BOOT revoked in an unprivileged
    // test harness), park to avoid panicking PID 1.
    loop {
        std::thread::park();
    }
}
