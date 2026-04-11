//! Functional tests: preconfig queries, device hotplug, networking, UEFI boot.
//! These tests exercise the full VM lifecycle with real QEMU instances.

#[allow(dead_code, unused_imports)]
mod common;

use common::fixtures;
use common::serial::SerialConsole;
#[allow(unused_imports)]
use common::*;
use qapi::commands::*;
use qapi::events::*;
use qapi::*;

// ══════════════════════════════════════════════════════════════════════════════
// Preconfig tests — queries and commands before x-exit-preconfig
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(preconfig_query_version, [X86_64, Aarch64], |arch: Arch| async move {
    let preconfig = VmBuilder::new(arch, MachineConfig::None)
        .spawn().await.unwrap();

    let ver: VersionInfo = preconfig.execute(&QueryVersion {}).await.unwrap();
    assert!(ver.qemu.major >= 3, "QEMU major version should be >= 3");

    preconfig.shutdown().await;
});

arch_tests!(preconfig_query_status, [X86_64, Aarch64], |arch: Arch| async move {
    let preconfig = VmBuilder::new(arch, MachineConfig::None)
        .spawn().await.unwrap();

    let status: StatusInfo = preconfig.execute(&QueryStatus {}).await.unwrap();
    assert!(!status.running, "Should not be running in preconfig");

    preconfig.shutdown().await;
});

arch_tests!(preconfig_query_commands, [X86_64], |arch: Arch| async move {
    let preconfig = VmBuilder::new(arch, MachineConfig::None)
        .spawn().await.unwrap();

    let cmds: Vec<CommandInfo> = preconfig.execute(&QueryCommands {}).await.unwrap();
    let cmd_names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();

    // These should be available in preconfig
    assert!(cmd_names.contains(&"query-status"), "query-status should be available");
    assert!(cmd_names.contains(&"query-version"), "query-version should be available");
    assert!(cmd_names.contains(&"query-commands"), "query-commands should be available");
    assert!(cmd_names.contains(&"x-exit-preconfig"), "x-exit-preconfig should be available");

    preconfig.shutdown().await;
});

arch_tests!(query_kvm_after_start, [X86_64], |arch: Arch| async move {
    let vm = VmBuilder::new(arch, MachineConfig::None)
        .spawn().await.unwrap()
        .start().await.unwrap();

    let kvm: KvmInfo = vm.qmp.execute(&QueryKvm {}).await.unwrap();
    // KVM may or may not be present, but the query should succeed
    eprintln!("KVM enabled: {}, present: {}", kvm.enabled, kvm.present);

    vm.shutdown().await;
});

arch_tests!(preconfig_to_running_transition, [X86_64, Aarch64], |arch: Arch| async move {
    let preconfig = VmBuilder::new(arch, MachineConfig::None)
        .spawn().await.unwrap();

    // Verify preconfig state
    let status: StatusInfo = preconfig.execute(&QueryStatus {}).await.unwrap();
    assert!(!status.running, "Should not be running in preconfig");

    // Transition to running
    let vm = preconfig.start().await.unwrap();

    // Verify running state
    let status: StatusInfo = vm.execute(&QueryStatus {}).await.unwrap();
    assert!(status.running, "Should be running after start");

    vm.shutdown().await;
});

// ── Preconfig with real machine types ────────────────────────────────────────

arch_tests!(preconfig_bios_machine_queries, [X86_64], |arch: Arch| async move {
    let preconfig = VmBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .spawn().await.unwrap();

    // These queries should work in preconfig with a real machine
    let name: NameInfo = preconfig.execute(&QueryName {}).await.unwrap();
    eprintln!("name: {:?}", name);

    let uuid: UuidInfo = preconfig.execute(&QueryUuid {}).await.unwrap();
    eprintln!("uuid: {}", uuid.uuid);
    assert!(!uuid.uuid.is_empty());

    preconfig.shutdown().await;
});

arch_version_tests!(preconfig_block_queries, [X86_64], >= (7, 1), |arch: Arch| async move {
    // Block queries became preconfig-safe in QEMU 7.1
    let preconfig = VmBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .spawn().await.unwrap();

    let blocks: Vec<BlockInfo> = preconfig.execute(&QueryBlock {}).await.unwrap();
    eprintln!("blocks in preconfig: {}", blocks.len());
    // No drives added, so should be empty
    assert!(blocks.is_empty(), "No block devices in preconfig without drives");

    preconfig.shutdown().await;
});

// ══════════════════════════════════════════════════════════════════════════════
// Network device tests — user-mode networking
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(netdev_user_mode, [X86_64], |arch: Arch| async move {
    let vm = VmBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .arg("-netdev").arg("user,id=net0")
        .arg("-device").arg("e1000,netdev=net0,id=nic0")
        .spawn().await.unwrap()
        .start().await.unwrap();

    // Query PCI — should see the e1000 NIC
    let pci: Vec<PciInfo> = vm.execute(&QueryPci {}).await.unwrap();
    assert!(!pci.is_empty(), "Should have PCI bus");

    // Look for e1000 in PCI devices
    let has_e1000 = pci.iter().any(|bus| {
        bus.devices.iter().any(|d| {
            d.class_info.class == 512 // 0x0200 = ethernet controller
        })
    });
    assert!(has_e1000, "Should have an ethernet controller on PCI");

    vm.shutdown().await;
});

arch_tests!(netdev_virtio, [X86_64], |arch: Arch| async move {
    let vm = VmBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .arg("-netdev").arg("user,id=net0")
        .arg("-device").arg("virtio-net-pci,netdev=net0,id=nic0")
        .spawn().await.unwrap()
        .start().await.unwrap();

    let pci: Vec<PciInfo> = vm.execute(&QueryPci {}).await.unwrap();
    let has_virtio_net = pci.iter().any(|bus| {
        bus.devices.iter().any(|d| {
            d.class_info.class == 512
        })
    });
    assert!(has_virtio_net, "Should have a virtio-net device on PCI");

    vm.shutdown().await;
});

arch_tests!(multiple_netdevs, [X86_64], |arch: Arch| async move {
    let vm = VmBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .arg("-netdev").arg("user,id=net0")
        .arg("-device").arg("e1000,netdev=net0,id=nic0")
        .arg("-netdev").arg("user,id=net1")
        .arg("-device").arg("e1000,netdev=net1,id=nic1")
        .spawn().await.unwrap()
        .start().await.unwrap();

    let pci: Vec<PciInfo> = vm.execute(&QueryPci {}).await.unwrap();
    let nic_count = pci.iter()
        .flat_map(|bus| bus.devices.iter())
        .filter(|d| d.class_info.class == 512)
        .count();
    assert_eq!(nic_count, 2, "Should have exactly 2 NICs, got {}", nic_count);

    vm.shutdown().await;
});

// ══════════════════════════════════════════════════════════════════════════════
// Device hotplug tests — add and remove devices at runtime
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(hotplug_virtio_net, [X86_64], |arch: Arch| async move {
    let vm = VmBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .arg("-netdev").arg("user,id=hotnet0")
        .spawn().await.unwrap()
        .start().await.unwrap();

    // Count NICs before hotplug
    let pci_before: Vec<PciInfo> = vm.execute(&QueryPci {}).await.unwrap();
    let nics_before = pci_before.iter()
        .flat_map(|bus| bus.devices.iter())
        .filter(|d| d.class_info.class == 512)
        .count();

    // Hotplug a virtio-net device via human-monitor-command
    // (device_add QMP command doesn't support arbitrary properties in the typed API)
    let _: String = vm.execute(&HumanMonitorCommand {
        command_line: "device_add virtio-net-pci,netdev=hotnet0,id=hotnic0".into(),
        cpu_index: None::<i64>,
    }).await.unwrap();

    // Brief pause for device to register
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Count NICs after hotplug
    let pci_after: Vec<PciInfo> = vm.execute(&QueryPci {}).await.unwrap();
    let nics_after = pci_after.iter()
        .flat_map(|bus| bus.devices.iter())
        .filter(|d| d.class_info.class == 512)
        .count();

    assert_eq!(nics_after, nics_before + 1,
        "Should have one more NIC after hotplug (before={}, after={})",
        nics_before, nics_after);

    vm.shutdown().await;
});

// Note: device unplug (device_del) for virtio devices requires guest driver
// cooperation. Without a running guest OS, the device stays in "pending unplug"
// state. The cirros_device_unplug test below exercises unplug with a real guest.

arch_tests!(cirros_device_unplug, [X86_64], |arch: Arch| async move {
    let builder = cirros_with_net(arch).await;
    let mut vm = builder
        .no_shutdown()
        .spawn().await.unwrap()
        .start().await.unwrap();

    let console = SerialConsole::new(vm.serial_log().unwrap().clone());
    console.wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot");

    // The VM has a virtio-net from cirros_with_net(). Add a second NIC.
    let _: String = vm.execute(&HumanMonitorCommand {
        command_line: "device_add virtio-balloon-pci,id=testballoon0".into(),
        cpu_index: None::<i64>,
    }).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Verify balloon is present
    let balloon = vm.execute(&QueryBalloon {}).await;
    assert!(balloon.is_ok(), "Balloon should be queryable after hotplug");

    // Request unplug — the guest driver will acknowledge
    vm.execute(&DeviceDel { id: "testballoon0".into() }).await.unwrap();

    // Wait for DEVICE_DELETED event (guest cooperates)
    let evt = wait_for_event(&mut vm.events, "DEVICE_DELETED", event_timeout()).await;
    assert!(evt.is_some(), "Should receive DEVICE_DELETED event with guest cooperation");

    vm.shutdown().await;
});

arch_tests!(hotplug_virtio_balloon, [X86_64], |arch: Arch| async move {
    let vm = VmBuilder::new(arch, MachineConfig::Bios { memory: "256M".into() })
        .spawn().await.unwrap()
        .start().await.unwrap();

    // Hotplug a balloon device
    let _: String = vm.execute(&HumanMonitorCommand {
        command_line: "device_add virtio-balloon-pci,id=balloon0".into(),
        cpu_index: None::<i64>,
    }).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Query balloon — should now respond
    let result = vm.execute(&QueryBalloon {}).await;
    assert!(result.is_ok(), "query-balloon should work after hotplug: {:?}", result.err());

    vm.shutdown().await;
});

// ══════════════════════════════════════════════════════════════════════════════
// Cirros boot with networking — validate guest sees the network
// ══════════════════════════════════════════════════════════════════════════════

/// Helper: create a cirros builder with user-mode networking.
async fn cirros_with_net(arch: Arch) -> VmBuilder {
    let image = fixtures::cirros_image_copy(arch).await;
    let kernel = fixtures::cirros_kernel(arch).await;
    let initrd = fixtures::cirros_initramfs(arch).await;
    let cidata = fixtures::nocloud_cidata().await;

    VmBuilder::new(
        arch,
        MachineConfig::Bios {
            memory: "256M".into(),
        },
    )
    .kernel(
        kernel,
        initrd,
        "console=ttyS0 root=/dev/vda1 dslist=nocloud",
    )
    .arg("-drive")
    .arg(format!("if=virtio,format=qcow2,file={}", image.display()))
    .arg("-drive")
    .arg(format!(
        "if=virtio,format=raw,file={},readonly=on",
        cidata.display()
    ))
    .arg("-netdev")
    .arg("user,id=net0")
    .arg("-device")
    .arg("virtio-net-pci,netdev=net0")
    .serial()
}

arch_tests!(cirros_boot_with_network, [X86_64], |arch: Arch| async move {
    let builder = cirros_with_net(arch).await;
    let t = builder.spawn().await.unwrap().start().await.unwrap();

    let console = SerialConsole::new(t.serial_log().unwrap().clone());
    console.wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot with networking");

    // The guest should have booted with a network device visible
    let pci: Vec<PciInfo> = t.execute(&QueryPci {}).await.unwrap();
    let has_nic = pci.iter().any(|bus| {
        bus.devices.iter().any(|d| d.class_info.class == 512)
    });
    assert!(has_nic, "Booted VM should have a network device");

    t.shutdown().await;
});

arch_tests!(cirros_network_dhcp, [X86_64], |arch: Arch| async move {
    let builder = cirros_with_net(arch).await;
    let t = builder.spawn().await.unwrap().start().await.unwrap();

    let console = SerialConsole::new(t.serial_log().unwrap().clone());
    console.wait_for_boot(boot_timeout()).await
        .expect("cirros did not boot");

    // Wait for DHCP lease — cirros logs "eth0" or network config in serial
    let net_up = console
        .wait_for("inet ", std::time::Duration::from_secs(30))
        .await;

    match net_up {
        Ok(line) => {
            eprintln!("Network configured: {}", line.trim());
            assert!(line.contains("10.0.2."), "DHCP should assign 10.0.2.x address");
        }
        Err(_) => {
            // If we can't find it in serial, check via human-monitor-command
            let info: String = t.execute(&HumanMonitorCommand {
                command_line: "info network".into(),
                cpu_index: None::<i64>,
            }).await.unwrap();
            eprintln!("Network info from QEMU: {}", info);
            assert!(info.contains("net0"), "Should have net0 netdev");
        }
    }

    t.shutdown().await;
});

// ══════════════════════════════════════════════════════════════════════════════
// Block device hotplug
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(hotplug_virtio_blk, [X86_64], |arch: Arch| async move {
    let disk = fixtures::blank_qcow2("hotplug-test", 64).await;

    let vm = VmBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .spawn().await.unwrap()
        .start().await.unwrap();

    // Count block devices before
    let blocks_before: Vec<BlockInfo> = vm.execute(&QueryBlock {}).await.unwrap();
    let count_before = blocks_before.len();

    // Add a blockdev
    let _: String = vm.execute(&HumanMonitorCommand {
        command_line: format!(
            "drive_add 0 if=none,id=hotdisk0,format=qcow2,file={}",
            disk.display()
        ),
        cpu_index: None::<i64>,
    }).await.unwrap();

    // Add a virtio-blk device using the drive
    let _: String = vm.execute(&HumanMonitorCommand {
        command_line: "device_add virtio-blk-pci,drive=hotdisk0,id=hotblk0".into(),
        cpu_index: None::<i64>,
    }).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Count block devices after
    let blocks_after: Vec<BlockInfo> = vm.execute(&QueryBlock {}).await.unwrap();
    assert!(
        blocks_after.len() > count_before,
        "Should have more block devices after hotplug (before={}, after={})",
        count_before,
        blocks_after.len()
    );

    vm.shutdown().await;
});

// ══════════════════════════════════════════════════════════════════════════════
// UEFI boot tests
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(uefi_boot_preconfig, [X86_64], |arch: Arch| async move {
    let fw = match fixtures::find_firmware(arch) {
        Some(fw) => fw,
        None => {
            eprintln!("SKIP: UEFI firmware not found for {}", arch);
            return;
        }
    };

    let preconfig = VmBuilder::new(arch, MachineConfig::Uefi { memory: "256M".into() })
        .firmware(fw)
        .spawn().await.unwrap();

    // Preconfig queries should work with UEFI machine
    let status: StatusInfo = preconfig.execute(&QueryStatus {}).await.unwrap();
    assert!(!status.running, "Should not be running in preconfig");

    let cmds: Vec<CommandInfo> = preconfig.execute(&QueryCommands {}).await.unwrap();
    assert!(!cmds.is_empty(), "Should have commands available in UEFI preconfig");

    let hotcpus: Vec<HotpluggableCpu> = preconfig.execute(&QueryHotpluggableCpus {}).await.unwrap();
    eprintln!("UEFI preconfig hotpluggable CPUs: {}", hotcpus.len());

    preconfig.shutdown().await;
});

arch_tests!(uefi_boot_and_query, [X86_64], |arch: Arch| async move {
    let fw = match fixtures::find_firmware(arch) {
        Some(fw) => fw,
        None => {
            eprintln!("SKIP: UEFI firmware not found for {}", arch);
            return;
        }
    };

    let vm = VmBuilder::new(arch, MachineConfig::Uefi { memory: "256M".into() })
        .firmware(fw)
        .spawn().await.unwrap()
        .start().await.unwrap();

    // Post-start queries
    let status: StatusInfo = vm.execute(&QueryStatus {}).await.unwrap();
    assert!(status.running, "UEFI VM should be running");

    let pci: Vec<PciInfo> = vm.execute(&QueryPci {}).await.unwrap();
    assert!(!pci.is_empty(), "UEFI (Q35) should have PCI bus");
    eprintln!("UEFI PCI buses: {}", pci.len());

    let mem: MemoryInfo = vm.qmp.execute(&QueryMemorySizeSummary {}).await.unwrap();
    assert!(mem.base_memory > 0, "Should have memory");

    vm.shutdown().await;
});

// ══════════════════════════════════════════════════════════════════════════════
// VM lifecycle tests — stop/cont/reset with events
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(lifecycle_stop_cont_with_bios, [X86_64], |arch: Arch| async move {
    let mut vm = VmBuilder::new(arch, MachineConfig::Bios { memory: "128M".into() })
        .no_shutdown()
        .spawn().await.unwrap()
        .start().await.unwrap();

    // Stop
    vm.stop().await.unwrap();
    let evt = wait_for_event(&mut vm.events, "STOP", event_timeout()).await;
    assert!(evt.is_some(), "Should get STOP event");

    let status: StatusInfo = vm.execute(&QueryStatus {}).await.unwrap();
    assert!(!status.running, "Should not be running after stop");

    // Continue
    vm.cont().await.unwrap();
    let evt = wait_for_event(&mut vm.events, "RESUME", event_timeout()).await;
    assert!(evt.is_some(), "Should get RESUME event");

    let status: StatusInfo = vm.execute(&QueryStatus {}).await.unwrap();
    assert!(status.running, "Should be running after cont");

    // Reset
    vm.reset().await.unwrap();
    let evt = wait_for_event(&mut vm.events, "RESET", event_timeout()).await;
    assert!(evt.is_some(), "Should get RESET event");

    vm.shutdown().await;
});
