use manager::{Arch, MachineConfig, VmBuilder};
use manager::arch::detect_version;

fn require_x86_64() -> bool {
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return false;
    }
    true
}

#[tokio::test]
async fn start_and_query_status() {
    if !require_x86_64() { return; }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap();
    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await.unwrap();
    assert!(status.running);
    vm.shutdown().await;
}

#[tokio::test]
async fn start_and_shutdown() {
    if !require_x86_64() { return; }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap();
    vm.shutdown().await;
    // If we get here without hanging, the shutdown was clean.
}

#[tokio::test]
async fn preconfig_flow() {
    if !require_x86_64() { return; }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .preconfig()
        .spawn()
        .await
        .unwrap();

    // In preconfig mode, x-exit-preconfig is needed to continue boot.
    vm.execute(&qapi::XExitPreconfig {}).await.unwrap();

    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await.unwrap();
    assert!(status.running);
    vm.shutdown().await;
}

#[tokio::test]
async fn execute_multiple_commands() {
    if !require_x86_64() { return; }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap();

    let _status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await.unwrap();
    let _name: qapi::NameInfo = vm.execute(&qapi::QueryName {}).await.unwrap();
    let _kvm: qapi::KvmInfo = vm.execute(&qapi::QueryKvm {}).await.unwrap();
    let _uuid: qapi::UuidInfo = vm.execute(&qapi::QueryUuid {}).await.unwrap();

    vm.shutdown().await;
}

#[tokio::test]
async fn events_received_on_stop() {
    if !require_x86_64() { return; }
    let mut vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .no_shutdown()
        .spawn()
        .await
        .unwrap();

    // Send stop command
    vm.execute(&qapi::Stop {}).await.unwrap();

    // Wait for the STOP event
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut got_stop = false;
    loop {
        match tokio::time::timeout_at(deadline, vm.events.recv()).await {
            Ok(Some(evt)) if evt.event == "STOP" => {
                got_stop = true;
                break;
            }
            Ok(Some(_)) => continue,
            _ => break,
        }
    }
    assert!(got_stop, "Expected STOP event");

    vm.shutdown().await;
}

#[tokio::test]
async fn detect_qemu_version() {
    if !require_x86_64() { return; }
    let version = detect_version(Arch::X86_64);
    assert!(version.is_some(), "Should detect QEMU version");
    let (major, minor, _patch) = version.unwrap();
    assert!(major >= 5, "Expected QEMU >= 5.0, got {}.{}", major, minor);
}

#[tokio::test]
async fn arch_availability() {
    // X86_64 should be available in our test environment
    assert!(Arch::X86_64.is_available());
    // Verify binary path is reasonable
    assert!(Arch::X86_64.binary().to_str().unwrap().contains("qemu-system-x86_64"));
}
