mod common;

use common::*;
use qapi::*;

#[test]
fn infra_compiles() {
    ensure_version_set();
    assert!(Arch::X86_64.is_available() || !Arch::X86_64.is_available());
}

arch_tests!(arch_probe, [X86_64, I386, Aarch64, Arm], |arch: Arch| async move {
    let mut t = QemuBuilder::new(arch, MachineConfig::None).spawn().await.unwrap();
    let status: StatusInfo = t.qmp.execute(&QueryStatus {}).await.unwrap();
    assert!(status.running);
    t.shutdown().await;
});
