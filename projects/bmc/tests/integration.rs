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
        kvm: false,
        disks: Vec::new(),
    };
    state
}

// --- Phase 1 tests ---

#[tokio::test]
async fn bmc_power_on_and_phase() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;
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

    let state = test_state().await;

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

    let state = test_state().await;
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

    let state = test_state().await;

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

    let state = test_state().await;
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

    let state = test_state().await;

    {
        let mut config = state.config.lock().await;
        *config = BmcConfig {
            arch: BmcArch::X86_64,
            machine: BmcMachine::None,
            memory: "512M".into(),
            display: false,
            kvm: false,
            disks: Vec::new(),
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

    let state = test_state().await;
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

    let state = test_state().await;
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

    let state = test_state().await;
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

    let state = test_state().await;
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

// --- Phase 1 correctness tests ---

/// Verify start_vm transitions to Off (not stuck in ShuttingDown) when
/// PreconfigVm::start fails. Failure is induced by shutting down the QMP
/// connection before calling start_vm.
#[tokio::test]
async fn start_vm_error_transitions_to_off() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;
    state.power_on().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Preconfig);

    // Force the QMP connection down so start_vm's QMP command fails.
    {
        let mut vm_guard = state.vm.lock().await;
        if let bmc::state::VmState::Preconfig(pvm) = &mut *vm_guard {
            pvm.qmp.shutdown().await;
        } else {
            panic!("Expected Preconfig state");
        }
    }

    let result = state.start_vm().await;
    assert!(result.is_err(), "start_vm should fail with broken QMP");

    // Critical: must reach Off, not stuck in ShuttingDown
    assert_eq!(state.phase().await, VmPhase::Off);
    assert!(state.vnc_sock().await.is_none());
    assert!(state.serial_log().await.is_none());
    assert!(state.event_history.lock().await.is_empty());
}

#[tokio::test]
async fn force_off_clears_event_history() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;
    state.power_on().await.unwrap();
    state.start_vm().await.unwrap();

    state.pause().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    state.resume().await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        !state.event_history.lock().await.is_empty(),
        "Should have events before force_off"
    );

    state.force_off().await.unwrap();

    assert!(
        state.event_history.lock().await.is_empty(),
        "Event history should be cleared after force_off"
    );
}

#[tokio::test]
async fn power_on_clears_prior_event_history() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;

    // Pre-populate history with a fake event
    {
        let mut history = state.event_history.lock().await;
        history.push_back(qapi_protocol::QmpEvent {
            event: "FAKE".into(),
            timestamp: qapi_protocol::Timestamp {
                seconds: 0,
                microseconds: 0,
            },
            data: serde_json::Value::Null,
        });
    }

    state.power_on().await.unwrap();
    assert!(
        state.event_history.lock().await.is_empty(),
        "Power on should defensively clear stale events"
    );

    state.force_off().await.unwrap();
}

// --- Phase 4 server hardening tests ---

#[tokio::test]
async fn get_console_available_atomic() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    use bmc::server::vm_status::get_console_available;

    let state = test_state().await;
    // Off → not available
    assert_eq!(get_console_available().await.unwrap(), false);

    // Preconfig (machine=none, no display) → not available
    state.power_on().await.unwrap();
    assert_eq!(get_console_available().await.unwrap(), false);

    // Running but no display → not available (vnc_sock is None)
    state.start_vm().await.unwrap();
    assert_eq!(get_console_available().await.unwrap(), false);

    state.force_off().await.unwrap();
    assert_eq!(get_console_available().await.unwrap(), false);
}

#[tokio::test]
async fn get_serial_output_bounded() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;
    state.power_on().await.unwrap();

    // serial_log() reads from VmState now
    let serial_path = state.serial_log().await.expect("serial_log set");

    // Append a 200KB blob to the serial log behind QEMU's back to simulate
    // a long-running VM with lots of output.
    let blob = "X".repeat(200 * 1024);
    tokio::fs::write(&serial_path, &blob).await.unwrap();

    let output = bmc::server::vm_status::get_serial_output().await.unwrap();
    // Bounded to 64 KiB tail
    assert!(
        output.len() <= 64 * 1024,
        "Serial output should be bounded to 64KiB, got {} bytes",
        output.len()
    );

    state.force_off().await.unwrap();
}

// --- Phase 3 QMP dynamic config tests ---

#[tokio::test]
async fn bmc_power_on_with_disk_via_qmp() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = BmcState::new();
    // Default config has bios+display+empty disks → cirros fallback kicks in
    state.power_on().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Preconfig);
    state.start_vm().await.unwrap();
    assert_eq!(state.phase().await, VmPhase::Running);

    // Verify the disk was actually wired by querying block info
    {
        let vm_guard = state.vm.lock().await;
        if let bmc::state::VmState::Running(vm) = &*vm_guard {
            let blocks: Vec<qapi::BlockInfo> =
                vm.execute(&qapi::commands::QueryBlock {}).await.unwrap();
            // virtio-blk-pci device should appear in the block list
            assert!(
                blocks.iter().any(|b| b.device.starts_with("blk")
                    || b.qdev.as_deref().map(|q| q.contains("blk")).unwrap_or(false)),
                "Expected blk device, got: {blocks:?}"
            );
        } else {
            panic!("Expected Running");
        }
    }

    state.force_off().await.unwrap();
}

#[tokio::test]
async fn bmc_config_enum_serde_roundtrip() {
    let config = BmcConfig {
        arch: BmcArch::X86_64,
        machine: BmcMachine::Bios,
        memory: "512M".into(),
        display: true,
        kvm: false,
        disks: vec![bmc::types::DiskConfig {
            file: "/tmp/test.qcow2".into(),
            format: bmc::types::DiskFormat::Qcow2,
            interface: bmc::types::DiskInterface::Virtio,
            read_only: false,
        }],
    };
    let json = serde_json::to_string(&config).unwrap();
    let parsed: BmcConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.arch, BmcArch::X86_64);
    assert_eq!(parsed.machine, BmcMachine::Bios);
    assert_eq!(parsed.memory, "512M");
    assert!(parsed.display);
    assert_eq!(parsed.disks.len(), 1);
    assert_eq!(parsed.disks[0].file, std::path::PathBuf::from("/tmp/test.qcow2"));
    assert_eq!(parsed.disks[0].format, bmc::types::DiskFormat::Qcow2);
}

#[tokio::test]
async fn bmc_machine_uefi_serializes_lowercase() {
    let json = serde_json::to_string(&BmcMachine::Uefi).unwrap();
    assert_eq!(json, "\"uefi\"");
    let json = serde_json::to_string(&BmcMachine::None).unwrap();
    assert_eq!(json, "\"none\"");
    let json = serde_json::to_string(&BmcArch::Aarch64).unwrap();
    assert_eq!(json, "\"aarch64\"");
}

#[tokio::test]
async fn forwarder_joins_cleanly_on_force_off() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;
    state.power_on().await.unwrap();
    state.start_vm().await.unwrap();

    state.pause().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    state.resume().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let start = std::time::Instant::now();
    state.force_off().await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "force_off should complete quickly, took {elapsed:?}"
    );
    assert_eq!(state.phase().await, VmPhase::Off);
}

// --- Phase 6 event forwarder lifecycle tests ---

/// After `force_off`, the forwarder task has exited: no new events arrive
/// on the broadcast channel. (The broadcast sender itself stays alive
/// because it lives in `BmcState`, so we can't rely on a `Closed` error —
/// instead we verify silence on the channel after teardown.)
#[tokio::test]
async fn forwarder_exits_on_force_off() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;
    let mut event_rx = state.event_tx.subscribe();

    state.power_on().await.unwrap();
    state.start_vm().await.unwrap();

    // Generate some events so we know the forwarder is alive.
    state.pause().await.unwrap();
    state.resume().await.unwrap();

    // Drain any pending events delivered before force_off.
    while tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .is_ok()
    {}

    state.force_off().await.unwrap();

    // After force_off: any events still queued from the kill should drain
    // quickly. Give it a short grace, then verify no new events arrive
    // within 500ms (forwarder exited, QEMU is gone).
    while tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .is_ok()
    {}

    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    let res = tokio::time::timeout_at(deadline, event_rx.recv()).await;
    assert!(
        res.is_err(),
        "no new events should arrive after force_off; got {res:?}"
    );
}

/// A second `power_on` after `force_off` must spin up a fresh forwarder
/// task so that events from the second QEMU instance are observable.
#[tokio::test]
async fn forwarder_restarted_on_second_power_on() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;

    // First lifecycle.
    state.power_on().await.unwrap();
    state.force_off().await.unwrap();

    // Second lifecycle. Subscribe *after* force_off so we don't see stale
    // events from round one.
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
    assert!(
        got_stop,
        "second power_on did not restart the forwarder; no STOP event observed"
    );

    state.force_off().await.unwrap();
}

/// Repeated power_on/start_vm/force_off cycles must not leak tasks or slow
/// down over time.
///
/// Note: `tokio::runtime::Handle::metrics()` is unstable and behind the
/// `tokio_unstable` cfg flag, so we can't query active_tasks_count directly
/// here. Instead we verify via a wall-clock heuristic: ten full cycles must
/// complete in well under the per-cycle worst-case budget — a task leak
/// would accumulate polling cost and eventually blow this budget.
/// Lacking direct task metrics; verifying via wall-clock heuristic.
#[tokio::test]
async fn forwarder_does_not_leak_tasks() {
    if !require_x86_64() {
        return;
    }
    qapi::set_qemu_version(9, 0, 0);

    let state = test_state().await;

    for iter in 0..10 {
        let start = std::time::Instant::now();
        state.power_on().await.unwrap();
        state.start_vm().await.unwrap();
        state.force_off().await.unwrap();
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(5),
            "iter {iter}: cycle took {elapsed:?}; possible task leak"
        );
        assert_eq!(state.phase().await, VmPhase::Off);
    }
}

// --- Frontend use_reactive! regression guards ------------------------------
//
// Round 2 cleanup consolidated polling into Dashboard and made child
// components props-driven. `console_available` (Console) and `serial_tail`
// (SerialConsole) are non-reactive props — a plain `use_effect(move || {})`
// that reads them captures the value once and *never re-runs* on prop
// updates. The fix is to wrap the closure in `use_reactive!(|prop| ...)`
// which explicitly registers the prop as a dependency.
//
// The VNC console went completely blank on 2026-04-13 because this wrapper
// was missing: the noVNC loader only ran with the initial `false` value, so
// the canvas never attached. These text-level regression guards make sure
// the wrapper can't be silently removed by a future refactor.
//
// They are deliberately text-based because:
//   1. Dioxus component effects only run under a VirtualDom runtime, and
//      setting one up in a unit test is substantial plumbing for a single
//      regression class.
//   2. The unit tests in `components::console::tests` already cover the
//      pure transition logic. These guards cover the wiring.

#[test]
fn console_effect_wires_use_reactive() {
    let src = include_str!("../src/components/console.rs");
    assert!(
        src.contains("use_effect(use_reactive!(|console_available|"),
        "REGRESSION: Console component's use_effect must wrap the closure \
         in `use_reactive!(|console_available| ...)`. Without it, the \
         non-reactive `console_available` prop does not trigger effect \
         re-runs, the noVNC loader never fires on the Running transition, \
         and the VNC panel renders completely blank. See the 2026-04-13 \
         regression and Dioxus 0.7 non-reactive-dependencies docs."
    );
}

#[test]
fn serial_effect_wires_use_reactive() {
    let src = include_str!("../src/components/serial.rs");
    assert!(
        src.contains("use_effect(use_reactive!(|serial_tail|"),
        "REGRESSION: SerialConsole's auto-scroll use_effect must wrap the \
         closure in `use_reactive!(|serial_tail| ...)`. Without it, the \
         non-reactive `serial_tail` prop does not trigger re-runs, \
         auto-scroll-to-bottom stops working as new output arrives."
    );
}
