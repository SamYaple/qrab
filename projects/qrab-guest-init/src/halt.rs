//! Permanent-halt helper. L5 spec §Boot sequence step-22 says every
//! failure and every unexpected `execve` return must land here — no
//! reboot, no poweroff, no retry. Keeps forensics attached.

/// Call `reboot(LINUX_REBOOT_CMD_HALT)`. Never returns on a working
/// kernel; on the off-chance the syscall fails (e.g. we don't have
/// CAP_SYS_BOOT at the moment of the call), loop so that no control
/// flow ever re-enters userspace from this function.
#[cfg(not(test))]
pub fn halt() -> ! {
    // nix::sys::reboot::reboot gives us the typed wrapper around the
    // glibc `reboot(2)` call; we choose `RB_HALT_SYSTEM` since
    // `LINUX_REBOOT_CMD_HALT` (0xCDEF0123) maps to it.
    let _ = nix::sys::reboot::reboot(nix::sys::reboot::RebootMode::RB_HALT_SYSTEM);
    // If we ever get here, the kernel rejected the request. A tight
    // spin is the safest remaining option — we absolutely do not
    // want to return to a caller that might then invoke a shell.
    loop {
        std::thread::park();
    }
}

/// Test-only stub so the sequencer's "halt after emit" path is
/// reachable without actually halting the test runner. Unit tests
/// never call this; integration tests that drive the sequencer
/// swap in a `CaptureSink` and observe tokens before the sequencer
/// returns.
#[cfg(test)]
pub fn halt() -> ! {
    panic!("halt() called in test build — the sequencer should return Err(Token) instead");
}
