use std::time::Duration;

use bmc::state::BmcState;
use bmc::types::{BmcConfig, VmPhase};
use manager::Arch;

fn require_x86_64() -> bool {
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return false;
    }
    true
}

#[tokio::test]
async fn bmc_power_on_and_phase() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();

    // Should start off
    assert_eq!(state.phase().await, VmPhase::Off);

    // Power on -> preconfig
    state.power_on().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Preconfig);

    // Clean up
    state.force_off().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Off);
}

#[tokio::test]
async fn bmc_full_lifecycle() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();

    // Off -> Preconfig -> Running -> Off
    state.power_on().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Preconfig);

    state.start_vm().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Running);

    state.force_off().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Off);
}

#[tokio::test]
async fn bmc_pause_resume() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();
    state.power_on().await.unwrap();
    state.start_vm().await.unwrap();

    // Pause
    state.pause().await.unwrap();
    // Give QEMU a moment to process
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(state.phase().await, VmPhase::Paused);

    // Resume
    state.resume().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(state.phase().await, VmPhase::Running);

    state.force_off().await.unwrap();
}

#[tokio::test]
async fn bmc_invalid_transitions() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();

    // Can't pause when off
    assert!(state.pause().await.is_err());
    // Can't resume when off
    assert!(state.resume().await.is_err());
    // Can't start_vm when off
    assert!(state.start_vm().await.is_err());

    state.power_on().await.unwrap();

    // Can't power_on again
    assert!(state.power_on().await.is_err());
    // Can't pause in preconfig
    assert!(state.pause().await.is_err());

    state.force_off().await.unwrap();
}

#[tokio::test]
async fn bmc_event_forwarding() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();
    let mut event_rx = state.event_tx.subscribe();

    state.power_on().await.unwrap();
    state.start_vm().await.unwrap();

    // Pause should generate a STOP event
    state.pause().await.unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut got_stop = false;
    loop {
        match tokio::time::timeout_at(deadline, event_rx.recv()).await {
            Ok(Ok(evt)) if evt.event == "STOP" => {
                got_stop = true;
                break;
            }
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }
    assert!(got_stop, "Expected STOP event via broadcast channel");

    // Verify event is also in history
    let history = state.event_history.lock().await;
    assert!(
        history.iter().any(|e| e.event == "STOP"),
        "Expected STOP event in history"
    );
    drop(history);

    state.force_off().await.unwrap();
}

#[tokio::test]
async fn bmc_config_change() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();

    // Default config
    {
        let config = state.config.lock().await;
        assert_eq!(config.arch, "x86_64");
        assert_eq!(config.machine, "none");
    }

    // Change config
    {
        let mut config = state.config.lock().await;
        *config = BmcConfig {
            arch: "x86_64".into(),
            machine: "none".into(),
            memory: "512M".into(),
        };
    }

    // Power on with new config
    state.power_on().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Preconfig);

    state.force_off().await.unwrap();
}
