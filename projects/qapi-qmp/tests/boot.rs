//! Boot tests with cirros guest image.
//! Tests SeaBIOS boot (x86_64) and post-boot queries with a running guest.
//!
//! NOTE: UEFI boot tests are not yet implemented — cirros GRUB menu requires
//! keyboard input when booted via UEFI. TODO: send keypress via QMP or use
//! a custom GRUB config.

mod common;

use common::*;
use common::fixtures;
use qapi::*;

// ══════════════════════════════════════════════════════════════════════════════
// SeaBIOS boot
// ═══════════════════════════════════════════════════════════════��══════════════

arch_tests!(boot_seabios, [X86_64], |arch: Arch| async move {
    let image = fixtures::cirros_image_copy(arch).await;
    let mut t = QemuBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .drive(image, "qcow2")
        .serial()
        .spawn().await.unwrap();

    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot (SeaBIOS)");

    let status: StatusInfo = t.qmp.execute(&QueryStatus {}).await.unwrap();
    assert!(status.running, "VM should be running after boot");

    t.shutdown().await;
});

// ══════════════════════════════════════════════════════════════════════════════
// Post-boot queries — CPUs, block, memory, PCI, chardev
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(query_cpus_after_boot, [X86_64], |arch: Arch| async move {
    let image = fixtures::cirros_image_copy(arch).await;
    let mut t = QemuBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .drive(image, "qcow2")
        .serial()
        .spawn().await.unwrap();

    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot");

    let cpus: Vec<CpuInfoFast> = t.qmp.execute(&QueryCpusFast {}).await.unwrap();
    assert!(!cpus.is_empty(), "Should have at least one CPU");

    t.shutdown().await;
});

arch_tests!(query_block_after_boot, [X86_64], |arch: Arch| async move {
    let image = fixtures::cirros_image_copy(arch).await;
    let mut t = QemuBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .drive(image, "qcow2")
        .serial()
        .spawn().await.unwrap();

    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot");

    let blocks: Vec<BlockInfo> = t.qmp.execute(&QueryBlock {}).await.unwrap();
    assert!(!blocks.is_empty(), "Should have block devices");

    t.shutdown().await;
});

arch_tests!(query_blockstats_after_boot, [X86_64], |arch: Arch| async move {
    let image = fixtures::cirros_image_copy(arch).await;
    let mut t = QemuBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .drive(image, "qcow2")
        .serial()
        .spawn().await.unwrap();

    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot");

    let stats: Vec<BlockStats> = t.qmp.execute(&QueryBlockstats { query_nodes: None }).await.unwrap();
    assert!(!stats.is_empty(), "Should have block stats");

    t.shutdown().await;
});

arch_tests!(query_memory_after_boot, [X86_64], |arch: Arch| async move {
    let image = fixtures::cirros_image_copy(arch).await;
    let mut t = QemuBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .drive(image, "qcow2")
        .serial()
        .spawn().await.unwrap();

    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot");

    let mem: MemoryInfo = t.qmp.execute(&QueryMemorySizeSummary {}).await.unwrap();
    assert!(mem.base_memory > 0, "base_memory should be > 0");

    t.shutdown().await;
});

arch_tests!(query_pci_after_boot, [X86_64], |arch: Arch| async move {
    let image = fixtures::cirros_image_copy(arch).await;
    let mut t = QemuBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .drive(image, "qcow2")
        .serial()
        .spawn().await.unwrap();

    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot");

    let pci: Vec<PciInfo> = t.qmp.execute(&QueryPci {}).await.unwrap();
    assert!(!pci.is_empty(), "Should have PCI devices");

    t.shutdown().await;
});

arch_tests!(query_chardev_after_boot, [X86_64], |arch: Arch| async move {
    let image = fixtures::cirros_image_copy(arch).await;
    let mut t = QemuBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .drive(image, "qcow2")
        .serial()
        .spawn().await.unwrap();

    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot");

    let devs: Vec<ChardevInfo> = t.qmp.execute(&QueryChardev {}).await.unwrap();
    assert!(devs.len() >= 2, "Expected >= 2 chardevs with serial, got {}", devs.len());

    t.shutdown().await;
});

arch_tests!(query_hotpluggable_cpus_after_boot, [X86_64], |arch: Arch| async move {
    let image = fixtures::cirros_image_copy(arch).await;
    let mut t = QemuBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .drive(image, "qcow2")
        .serial()
        .spawn().await.unwrap();

    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot");

    let cpus: Vec<HotpluggableCpu> = t.qmp.execute(&QueryHotpluggableCpus {}).await.unwrap();
    assert!(!cpus.is_empty(), "Should have hotpluggable CPU info");

    t.shutdown().await;
});
