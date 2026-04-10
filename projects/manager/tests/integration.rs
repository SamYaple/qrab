use manager::{Arch, MachineConfig, VmBuilder, PRECONFIG_MIN_VERSION};
use manager::arch::detect_version;

fn require_x86_64() -> bool {
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return false;
    }
    true
}

#[tokio::test]
async fn spawn_preconfig_and_start() {
    if !require_x86_64() { return; }
    let preconfig = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap();

    // In preconfig state, query-status is allowed (PreconfigSafe)
    let status: qapi::StatusInfo = preconfig.execute(&qapi::QueryStatus {}).await.unwrap();
    assert!(!status.running, "VM should not be running in preconfig state");

    // Transition to running
    let vm = preconfig.start().await.unwrap();
    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await.unwrap();
    assert!(status.running, "VM should be running after start()");

    vm.shutdown().await;
}

#[tokio::test]
async fn preconfig_shutdown_without_start() {
    if !require_x86_64() { return; }
    let preconfig = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap();

    // Abort without starting -- should clean up without hanging
    preconfig.shutdown().await;
}

#[tokio::test]
async fn preconfig_query_commands() {
    if !require_x86_64() { return; }
    let preconfig = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap();

    // These are all preconfig-safe commands
    let _status: qapi::StatusInfo = preconfig.execute(&qapi::QueryStatus {}).await.unwrap();
    let _name: qapi::NameInfo = preconfig.execute(&qapi::QueryName {}).await.unwrap();

    let vm = preconfig.start().await.unwrap();
    vm.shutdown().await;
}

#[tokio::test]
async fn execute_multiple_commands_after_start() {
    if !require_x86_64() { return; }
    let preconfig = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap();
    let vm = preconfig.start().await.unwrap();

    let _status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await.unwrap();
    let _name: qapi::NameInfo = vm.execute(&qapi::QueryName {}).await.unwrap();
    let _kvm: qapi::KvmInfo = vm.execute(&qapi::QueryKvm {}).await.unwrap();
    let _uuid: qapi::UuidInfo = vm.execute(&qapi::QueryUuid {}).await.unwrap();

    vm.shutdown().await;
}

#[tokio::test]
async fn events_received_on_stop() {
    if !require_x86_64() { return; }
    let preconfig = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .no_shutdown()
        .spawn()
        .await
        .unwrap();
    let mut vm = preconfig.start().await.unwrap();

    // Send stop command
    vm.execute(&qapi::Stop {}).await.unwrap();

    // Wait for the STOP event
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut got_stop = false;
    loop {
        match tokio::time::timeout_at(deadline, vm.events().recv()).await {
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
    assert!(major >= 2, "Expected QEMU >= 2.0, got {}.{}", major, minor);
}

#[tokio::test]
async fn arch_availability() {
    // X86_64 should be available in our test environment
    assert!(Arch::X86_64.is_available());
    let binary = Arch::X86_64.binary();
    assert!(!binary.as_os_str().is_empty(), "Binary path should not be empty");
}

#[tokio::test]
async fn preconfig_min_version_constant() {
    // Verify the constant matches expected QEMU 3.0.0
    assert_eq!(PRECONFIG_MIN_VERSION, (3, 0, 0));
}
