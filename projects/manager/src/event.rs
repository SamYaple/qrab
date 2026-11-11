//! Manager-level lifecycle events, separate from the raw QMP event stream.
//!
//! The QMP event channel (`PreconfigVm::events` / `Vm::events`) carries only
//! events QEMU itself emits over its monitor socket. If QEMU crashes
//! (`SIGSEGV`, aborted assertion, OOM kill, etc.) no QMP event is ever
//! dispatched — the monitor socket simply EOFs and the channel closes. That
//! lets long-running subscribers notice the process is gone, but it gives
//! them no way to distinguish a graceful shutdown from a crash and no
//! explicit terminal message to wait on.
//!
//! [`ManagerEvent`] is a parallel, hand-authored stream that the manager
//! itself synthesises. Today the only event is [`ManagerEvent::ProcessExited`],
//! published by the child-reaper task when it observes `child.wait()`
//! returning (or failing). After that one event the sender is dropped, so
//! subscribers get a clean `None` on the next `recv()` and do not block
//! forever waiting for a QMP event that will never come (Bug #7 / #29).
//!
//! The `ManagerEvent` enum is marked `#[non_exhaustive]` to leave room for
//! future variants (e.g. swtpm exit, namespace helper failure) without a
//! breaking change.

use std::process::ExitStatus;

/// A manager-synthesised lifecycle event for a running VM.
///
/// Received via [`PreconfigVm::take_manager_events`] /
/// [`Vm::take_manager_events`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ManagerEvent {
    /// The underlying QEMU/qrab-init process exited.
    ///
    /// - `status` is `Some(..)` when `child.wait()` returned cleanly, or
    ///   `None` when the wait itself failed (e.g. ECHILD from a broken
    ///   reaper — should not happen in normal operation but is surfaced
    ///   rather than silently swallowed).
    /// - `killed_by_signal` is `Some(signo)` when the process was
    ///   terminated by a signal on unix (`ExitStatus::signal()`), else
    ///   `None`. A `SIGSEGV` crash and an `Ok(0)` clean exit are both
    ///   distinguishable by inspecting these two fields.
    ProcessExited {
        /// Raw exit status from `child.wait()`, if available.
        status: Option<ExitStatus>,
        /// Signal number that killed the process, if any.
        killed_by_signal: Option<i32>,
    },
}

impl ManagerEvent {
    /// Build a `ProcessExited` variant from a `child.wait()` result,
    /// extracting the unix signal number when applicable.
    #[must_use]
    pub fn from_wait_result(res: std::io::Result<ExitStatus>) -> Self {
        match res {
            Ok(status) => {
                #[cfg(unix)]
                let killed_by_signal = {
                    use std::os::unix::process::ExitStatusExt;
                    status.signal()
                };
                #[cfg(not(unix))]
                let killed_by_signal = None;
                Self::ProcessExited {
                    status: Some(status),
                    killed_by_signal,
                }
            }
            Err(_) => Self::ProcessExited {
                status: None,
                killed_by_signal: None,
            },
        }
    }

    /// True when the reported exit means the child terminated cleanly
    /// (`Ok(0)` on unix, `ExitStatus::success()` elsewhere). False for
    /// any non-zero exit, signal-kill, or wait failure.
    #[must_use]
    pub fn is_clean_exit(&self) -> bool {
        let Self::ProcessExited {
            status,
            killed_by_signal,
        } = self;
        killed_by_signal.is_none() && status.map(|s| s.success()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::ManagerEvent;

    /// An arbitrary but invalid-for-us errno (`10 = ECHILD` on Linux) is
    /// fine for shape-testing — we only care that `Err(..)` maps to
    /// `status = None` + `killed_by_signal = None`.
    const TEST_ERRNO_ECHILD: i32 = 10;

    #[test]
    fn from_wait_result_wait_error_has_no_status() {
        let err = std::io::Error::from_raw_os_error(TEST_ERRNO_ECHILD);
        let evt = ManagerEvent::from_wait_result(Err(err));
        match evt {
            ManagerEvent::ProcessExited {
                status,
                killed_by_signal,
            } => {
                assert!(status.is_none());
                assert!(killed_by_signal.is_none());
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn from_wait_result_clean_exit_has_no_signal() {
        use std::os::unix::process::ExitStatusExt;
        let status = std::process::ExitStatus::from_raw(0);
        let evt = ManagerEvent::from_wait_result(Ok(status));
        assert!(evt.is_clean_exit());
        match evt {
            ManagerEvent::ProcessExited {
                killed_by_signal, ..
            } => assert!(killed_by_signal.is_none()),
        }
    }

    #[cfg(unix)]
    #[test]
    fn from_wait_result_signal_populates_signo() {
        use std::os::unix::process::ExitStatusExt;
        // `ExitStatus::from_raw` encodes a raw `wstatus`; a bare signal
        // number represents "killed by signal N, no core dump". Use
        // `nix::sys::signal::Signal` to avoid pulling in a direct
        // libc dependency.
        let signo = nix::sys::signal::Signal::SIGSEGV as i32;
        let status = std::process::ExitStatus::from_raw(signo);
        let evt = ManagerEvent::from_wait_result(Ok(status));
        assert!(!evt.is_clean_exit());
        match evt {
            ManagerEvent::ProcessExited {
                killed_by_signal, ..
            } => assert_eq!(killed_by_signal, Some(signo)),
        }
    }
}
