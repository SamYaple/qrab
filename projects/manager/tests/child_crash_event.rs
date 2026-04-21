//! Integration test for Bug #7 / #29: terminal `ManagerEvent` on QEMU
//! child crash. Covers three reaper scenarios without booting QEMU:
//!
//!   1. clean exit(0)         — status.success() + no signal
//!   2. SIGSEGV crash         — killed_by_signal = Some(SIGSEGV)
//!   3. caller-requested stop — terminal event still fires
//!
//! In every case the monitor must also close the channel (drop the
//! sender) after publishing, so subscribers see a clean `None` on the
//! next `recv()` rather than blocking forever.
//!
//! This test lives in `tests/` (its own binary) rather than `src/vm.rs`
//! so the subprocess / pipe churn it triggers cannot race with
//! `namespace::ns_helper_run_closes_fds_on_early_error`, which does a
//! raw `fcntl(F_GETFD)` on fd numbers inside the same test process
//! and is known to be brittle against any concurrent fd work.

use manager::vm::test_support::spawn_monitor_for_test;
use manager::ManagerEvent;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::mpsc;

/// Hard ceiling for "the reaper must have fired" — generous but still
/// well below the default tokio-test timeout.
const REAPER_BUDGET: Duration = Duration::from_secs(5);

/// Spawn a `sh -c <script>` child, attach the reaper, and return
/// its handles.
async fn spawn_probe(
    script: &str,
) -> (
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
    mpsc::Receiver<ManagerEvent>,
) {
    let child = Command::new("sh")
        .arg("-c")
        .arg(script)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn sh");
    let (tx, rx) = mpsc::channel::<ManagerEvent>(1);
    let (shutdown, handle, _exit_watch) = spawn_monitor_for_test(child, tx);
    (shutdown, handle, rx)
}

/// Clean exit: the child returns 0. The monitor still publishes one
/// terminal `ProcessExited`, then closes the channel.
#[tokio::test]
async fn manager_event_on_clean_exit() {
    let (_sd, h, mut rx) = spawn_probe("exit 0").await;

    let evt = tokio::time::timeout(REAPER_BUDGET, rx.recv())
        .await
        .expect("reaper must publish a ManagerEvent within budget")
        .expect("channel must yield Some(ProcessExited) before close");

    match evt {
        ManagerEvent::ProcessExited {
            status,
            killed_by_signal,
        } => {
            assert!(status.is_some(), "status must be captured");
            assert!(
                killed_by_signal.is_none(),
                "clean exit must not report a signal, got {killed_by_signal:?}"
            );
            assert!(
                status.unwrap().success(),
                "exit 0 must be reported as success"
            );
        }
        // `ManagerEvent` is `#[non_exhaustive]` — silently accept
        // future variants so the non_exhaustive contract doesn't
        // require a test edit when a new variant lands.
        other => panic!("unexpected ManagerEvent variant: {other:?}"),
    }

    // After the terminal event the sender is dropped → next recv
    // returns None rather than blocking forever (Bug #7 / #29).
    let closed = tokio::time::timeout(REAPER_BUDGET, rx.recv())
        .await
        .expect("channel close must not block past budget");
    assert!(
        closed.is_none(),
        "channel must be closed after terminal event"
    );

    let _ = h.await;
}

/// SIGSEGV: the reaper must surface the signal number so subscribers
/// can tell a crash apart from a graceful shutdown.
#[cfg(unix)]
#[tokio::test]
async fn manager_event_on_sigsegv_crash() {
    // `kill -SEGV $$` terminates the shell by SIGSEGV without a core
    // dump → `ExitStatus::signal() == Some(SIGSEGV)` on Linux.
    let (_sd, h, mut rx) = spawn_probe("sleep 0.05; kill -SEGV $$").await;

    let evt = tokio::time::timeout(REAPER_BUDGET, rx.recv())
        .await
        .expect("reaper must publish a ManagerEvent within budget")
        .expect("channel must yield Some(ProcessExited) before close");

    match evt {
        ManagerEvent::ProcessExited {
            status,
            killed_by_signal,
        } => {
            let expected = nix::sys::signal::Signal::SIGSEGV as i32;
            assert_eq!(
                killed_by_signal,
                Some(expected),
                "SIGSEGV must be reported distinctly, got {killed_by_signal:?}"
            );
            let st = status.expect("status must be captured");
            assert!(!st.success(), "SIGSEGV exit must not report success");
        }
        other => panic!("unexpected ManagerEvent variant: {other:?}"),
    }

    let closed = tokio::time::timeout(REAPER_BUDGET, rx.recv())
        .await
        .expect("channel close must not block past budget");
    assert!(
        closed.is_none(),
        "channel must be closed after terminal event"
    );

    let _ = h.await;
}

/// Shutdown-path: the caller asks the monitor to kill the child. A
/// terminal event must still fire — consumers should see one
/// notification regardless of who initiated the exit.
#[tokio::test]
async fn manager_event_on_shutdown_request() {
    let (shutdown, h, mut rx) = spawn_probe("sleep 60").await;
    let _ = shutdown.send(());

    let evt = tokio::time::timeout(REAPER_BUDGET, rx.recv())
        .await
        .expect("reaper must publish on shutdown within budget")
        .expect("channel must yield the terminal event");

    match evt {
        ManagerEvent::ProcessExited { status, .. } => {
            assert!(status.is_some(), "status must be captured on shutdown");
        }
        other => panic!("unexpected ManagerEvent variant: {other:?}"),
    }

    let closed = tokio::time::timeout(REAPER_BUDGET, rx.recv())
        .await
        .expect("channel close must not block past budget");
    assert!(
        closed.is_none(),
        "channel must close after shutdown terminal event"
    );

    let _ = h.await;
}
