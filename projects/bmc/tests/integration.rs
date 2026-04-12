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

// --- Phase 1 tests ---

#[tokio::test]
async fn bmc_power_on_and_phase() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();
    assert_eq!(state.phase().await, VmPhase::Off);

    state.power_on().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Preconfig);

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

    state.pause().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(state.phase().await, VmPhase::Paused);

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

    assert!(state.pause().await.is_err());
    assert!(state.resume().await.is_err());
    assert!(state.start_vm().await.is_err());

    state.power_on().await.unwrap();
    assert!(state.power_on().await.is_err());
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

    {
        let config = state.config.lock().await;
        assert_eq!(config.arch, "x86_64");
        assert_eq!(config.machine, "none");
    }

    {
        let mut config = state.config.lock().await;
        *config = BmcConfig {
            arch: "x86_64".into(),
            machine: "none".into(),
            memory: "512M".into(),
        };
    }

    state.power_on().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Preconfig);
    state.force_off().await.unwrap();
}

// --- Phase 2 tests ---

#[tokio::test]
async fn bmc_query_status_running() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();
    state.power_on().await.unwrap();
    state.start_vm().await.unwrap();

    // Query status directly on the VM
    {
        let vm_guard = state.vm.lock().await;
        if let bmc::state::VmState::Running(vm) = &*vm_guard {
            let status: qapi::StatusInfo =
                vm.execute(&qapi::commands::QueryStatus {}).await.unwrap();
            assert!(status.running, "VM should be running");
        } else {
            panic!("Expected Running state");
        }
    }

    state.force_off().await.unwrap();
}

#[tokio::test]
async fn bmc_query_cpu_info() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();
    state.power_on().await.unwrap();
    state.start_vm().await.unwrap();

    // Query CPUs (machine=none has no vCPUs, just verify the command succeeds)
    {
        let vm_guard = state.vm.lock().await;
        if let bmc::state::VmState::Running(vm) = &*vm_guard {
            let cpus: Vec<qapi::CpuInfoFast> =
                vm.execute(&qapi::commands::QueryCpusFast {}).await.unwrap();
            // machine=none typically has no vCPUs, but the command should succeed
            let _ = cpus;
        } else {
            panic!("Expected Running state");
        }
    }

    state.force_off().await.unwrap();
}

#[tokio::test]
async fn bmc_pause_resume_with_events() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();
    let mut event_rx = state.event_tx.subscribe();

    state.power_on().await.unwrap();
    state.start_vm().await.unwrap();

    // Pause and wait for STOP event
    state.pause().await.unwrap();
    let mut got_stop = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
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
    assert!(got_stop, "Expected STOP event after pause");

    // Resume and wait for RESUME event
    state.resume().await.unwrap();
    let mut got_resume = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        match tokio::time::timeout_at(deadline, event_rx.recv()).await {
            Ok(Ok(evt)) if evt.event == "RESUME" => {
                got_resume = true;
                break;
            }
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }
    assert!(got_resume, "Expected RESUME event after resume");

    state.force_off().await.unwrap();
}

#[tokio::test]
async fn bmc_event_history_accumulates() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();
    state.power_on().await.unwrap();
    state.start_vm().await.unwrap();

    // Pause/resume to generate events
    state.pause().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    state.resume().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let history = state.event_history.lock().await;
    assert!(
        history.len() >= 2,
        "Expected at least 2 events in history, got {}",
        history.len()
    );

    let event_names: Vec<&str> = history.iter().map(|e| e.event.as_str()).collect();
    assert!(
        event_names.contains(&"STOP"),
        "Expected STOP in history, got: {event_names:?}"
    );
    assert!(
        event_names.contains(&"RESUME"),
        "Expected RESUME in history, got: {event_names:?}"
    );
    drop(history);

    state.force_off().await.unwrap();
}
