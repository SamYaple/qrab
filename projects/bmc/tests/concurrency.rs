//! Concurrent-operation tests for BmcState.
//!
//! Verifies that interleaved calls from multiple tasks don't leave the VM
//! stuck in a transient state (e.g. `ShuttingDown`) and that the state
//! machine remains consistent under races.

use std::time::Duration;

use bmc::state::BmcState;
use bmc::types::{BmcArch, BmcConfig, BmcMachine, VmPhase};
use manager::Arch;

fn require_x86_64() -> bool {
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return false;
    }
    true
}

/// Create BmcState with machine=none config for lightweight tests.
async fn test_state() -> std::sync::Arc<BmcState> {
    let state = BmcState::new();
    // Override default (bios+display) with lightweight none machine for tests
    *state.config.lock().await = BmcConfig {
        arch: BmcArch::X86_64,
        machine: BmcMachine::None,
        memory: "256M".into(),
        display: false,
        disks: Vec::new(),
    };
    state
}

/// Two concurrent `power_on()` futures: exactly one succeeds.
#[tokio::test]
async fn double_power_on_rejects_second() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;

    let s1 = state.clone();
    let s2 = state.clone();
    let h1 = tokio::spawn(async move { s1.power_on().await });
    let h2 = tokio::spawn(async move { s2.power_on().await });

    let r1 = h1.await.unwrap();
    let r2 = h2.await.unwrap();

    let successes = [&r1, &r2].iter().filter(|r| r.is_ok()).count();
    let errors = [&r1, &r2].iter().filter(|r| r.is_err()).count();
    assert_eq!(
        successes, 1,
        "exactly one power_on should succeed, got r1={r1:?} r2={r2:?}"
    );
    assert_eq!(
        errors, 1,
        "exactly one power_on should fail, got r1={r1:?} r2={r2:?}"
    );

    state.force_off().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Off);
}

/// Racing `force_off` against `power_on` should still settle cleanly in `Off`.
#[tokio::test]
async fn force_off_during_power_on() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    for iter in 0..5 {
        let state = test_state().await;

        let s1 = state.clone();
        let power_on_handle = tokio::spawn(async move { s1.power_on().await });

        // Race: fire force_off while power_on is still in flight. force_off
        // may legitimately error with "VM is already shutting down" or
        // "not off" depending on ordering — that's fine. The invariant is
        // that once both operations drain, the state is `Off`.
        let _ = state.force_off().await;

        // Let power_on finish so we can observe the final state.
        let _ = power_on_handle.await.unwrap();

        // If power_on won the race and created a preconfig VM, it is now
        // leaked unless we explicitly tear it down. Do so.
        let _ = state.force_off().await;

        assert_eq!(
            state.phase().await,
            VmPhase::Off,
            "iter {iter}: final phase must be Off, not stuck in ShuttingDown"
        );
    }
}

/// `force_off` racing against `start_vm` must settle cleanly in `Off`.
#[tokio::test]
async fn force_off_during_start_vm() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    for iter in 0..5 {
        let state = test_state().await;
        state.power_on().await.unwrap();

        let s1 = state.clone();
        let start_handle = tokio::spawn(async move { s1.start_vm().await });

        // Race force_off against the in-flight start_vm. Either ordering is
        // allowed; the final state must be Off.
        let _ = state.force_off().await;

        // Drain start_vm so we don't leak a Running VM.
        let _ = start_handle.await.unwrap();

        // If start_vm beat force_off and left a Running VM behind, tear it down.
        let _ = state.force_off().await;

        assert_eq!(
            state.phase().await,
            VmPhase::Off,
            "iter {iter}: final phase must be Off"
        );
    }
}

/// `pause` racing against `start_vm` should either succeed or error cleanly
/// (no deadlock, no panic).
#[tokio::test]
async fn pause_during_start_vm() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;
    state.power_on().await.unwrap();

    let s1 = state.clone();
    let start_handle = tokio::spawn(async move { s1.start_vm().await });

    // Pause may succeed (if start_vm already finished) or fail (VM not
    // yet Running). Either is fine — the invariant is no deadlock.
    let pause_result = tokio::time::timeout(Duration::from_secs(10), state.pause()).await;
    assert!(
        pause_result.is_ok(),
        "pause deadlocked waiting for vm lock"
    );

    let start_result = tokio::time::timeout(Duration::from_secs(10), start_handle)
        .await
        .expect("start_vm deadlocked")
        .unwrap();
    let _ = start_result;

    state.force_off().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Off);
}

/// 100 concurrent `phase()` reads must not panic and must each return a
/// valid `VmPhase` variant.
#[tokio::test]
async fn concurrent_status_reads() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;
    state.power_on().await.unwrap();
    state.start_vm().await.unwrap();

    let mut handles = Vec::with_capacity(100);
    for _ in 0..100 {
        let s = state.clone();
        handles.push(tokio::spawn(async move { s.phase().await }));
    }

    for h in handles {
        let phase = h.await.expect("phase() task panicked");
        // Any valid VmPhase variant is acceptable — this just asserts the
        // returned value is one of the known enum discriminants. The match
        // is exhaustive, so compilation enforces the variant set.
        match phase {
            VmPhase::Off
            | VmPhase::Preconfig
            | VmPhase::Running
            | VmPhase::Paused
            | VmPhase::ShuttingDown => {}
        }
    }

    state.force_off().await.unwrap();
}
