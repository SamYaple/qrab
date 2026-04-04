//! Boot tests with cirros guest image.
//! Uses direct kernel boot (-kernel/-initrd/-append) to bypass GRUB
//! and passes dslist=nocloud to skip EC2 metadata retries (~10s boot).

#[allow(dead_code, unused_imports)]
mod common;

#[allow(unused_imports)]
use common::*;
use common::fixtures;
use qapi::*;

/// Create a QemuBuilder configured for fast cirros boot (x86, SeaBIOS).
/// Uses direct kernel boot + dslist=nocloud to skip metadata retries (~10s boot).
async fn cirros_builder(arch: Arch) -> QemuBuilder {
    let image = fixtures::cirros_image_copy(arch).await;
    let kernel = fixtures::cirros_kernel(arch).await;
    let initrd = fixtures::cirros_initramfs(arch).await;
    let cidata = fixtures::nocloud_cidata().await;

    QemuBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .kernel(kernel, initrd, "console=ttyS0 root=/dev/vda1 dslist=nocloud")
        .drive(image, "qcow2")
        .raw_drive_ro(cidata)
        .serial()
}

// ══════════════════════════════════════════════════════════════════════════════
// Boot tests
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(boot_cirros, [X86_64], |arch: Arch| async move {
    let builder = cirros_builder(arch).await;
    let t = builder.spawn().await.unwrap();

    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot");

    let status: StatusInfo = t.qmp.execute(&QueryStatus {}).await.unwrap();
    assert!(status.running, "VM should be running after boot");

    t.shutdown().await;
});

// ══════════════════════════════════════════════════════════════════════════════
// Post-boot queries
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(query_cpus_after_boot, [X86_64], |arch: Arch| async move {
    let builder = cirros_builder(arch).await;
    let t = builder.spawn().await.unwrap();
    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await.unwrap();

    let cpus: Vec<CpuInfoFast> = t.qmp.execute(&QueryCpusFast {}).await.unwrap();
    assert!(!cpus.is_empty(), "Should have at least one CPU");

    t.shutdown().await;
});

arch_tests!(query_block_after_boot, [X86_64], |arch: Arch| async move {
    let builder = cirros_builder(arch).await;
    let t = builder.spawn().await.unwrap();
    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await.unwrap();

    let blocks: Vec<BlockInfo> = t.qmp.execute(&QueryBlock {}).await.unwrap();
    assert!(!blocks.is_empty(), "Should have block devices");

    t.shutdown().await;
});

arch_tests!(query_blockstats_after_boot, [X86_64], |arch: Arch| async move {
    let builder = cirros_builder(arch).await;
    let t = builder.spawn().await.unwrap();
    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await.unwrap();

    let stats: Vec<BlockStats> = t.qmp.execute(&QueryBlockstats { query_nodes: None }).await.unwrap();
    assert!(!stats.is_empty(), "Should have block stats");

    t.shutdown().await;
});

arch_tests!(query_memory_after_boot, [X86_64], |arch: Arch| async move {
    let builder = cirros_builder(arch).await;
    let t = builder.spawn().await.unwrap();
    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await.unwrap();

    let mem: MemoryInfo = t.qmp.execute(&QueryMemorySizeSummary {}).await.unwrap();
    assert!(mem.base_memory > 0, "base_memory should be > 0");

    t.shutdown().await;
});

arch_tests!(query_pci_after_boot, [X86_64], |arch: Arch| async move {
    let builder = cirros_builder(arch).await;
    let t = builder.spawn().await.unwrap();
    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await.unwrap();

    let pci: Vec<PciInfo> = t.qmp.execute(&QueryPci {}).await.unwrap();
    assert!(!pci.is_empty(), "Should have PCI devices");

    t.shutdown().await;
});

arch_tests!(query_chardev_after_boot, [X86_64], |arch: Arch| async move {
    let builder = cirros_builder(arch).await;
    let t = builder.spawn().await.unwrap();
    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await.unwrap();

    let devs: Vec<ChardevInfo> = t.qmp.execute(&QueryChardev {}).await.unwrap();
    assert!(devs.len() >= 2, "Expected >= 2 chardevs with serial, got {}", devs.len());

    t.shutdown().await;
});

arch_tests!(query_hotpluggable_cpus_after_boot, [X86_64], |arch: Arch| async move {
    let builder = cirros_builder(arch).await;
    let t = builder.spawn().await.unwrap();
    t.serial.as_ref().unwrap().wait_for_boot(boot_timeout()).await.unwrap();

    let cpus: Vec<HotpluggableCpu> = t.qmp.execute(&QueryHotpluggableCpus {}).await.unwrap();
    assert!(!cpus.is_empty(), "Should have hotpluggable CPU info");

    t.shutdown().await;
});
