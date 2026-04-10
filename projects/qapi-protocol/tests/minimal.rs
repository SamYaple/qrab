//! Minimal machine tests: -machine none, all architectures.
//! Tests every query command that works without hardware.

#[allow(unused_imports)]
#[allow(dead_code, unused_imports)]
mod common;

#[allow(unused_imports)]
use common::*;
use qapi::*;

// ══════════════════════════════════════════════════════════════════════════════
// Version and connectivity
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(
    negotiate_and_version,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let version = qapi::qemu_version();
        assert!(
            version.0 > 0 || version == (99, 0, 0),
            "Version should be set"
        );
        let info: VersionInfo = t.execute(&QueryVersion {}).await.unwrap();
        assert!(info.qemu.major > 0, "QEMU major version should be > 0");
        t.shutdown().await;
    }
);

// ══════════════════════════════════════════════════════════════════════════════
// Simple query commands — non-list returns
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(
    query_status,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let status: StatusInfo = t.execute(&QueryStatus {}).await.unwrap();
        assert!(status.running);
        assert_eq!(
            serde_json::to_string(&status.status).unwrap(),
            "\"running\""
        );
        t.shutdown().await;
    }
);

arch_tests!(
    query_name,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let _: NameInfo = t.execute(&QueryName {}).await.unwrap();
        t.shutdown().await;
    }
);

arch_tests!(
    query_kvm,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let kvm: KvmInfo = t.execute(&QueryKvm {}).await.unwrap();
        let _ = kvm.present;
        let _ = kvm.enabled;
        t.shutdown().await;
    }
);

arch_tests!(
    query_uuid,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let uuid: UuidInfo = t.execute(&QueryUuid {}).await.unwrap();
        assert_eq!(uuid.uuid.len(), 36, "UUID should be 36 chars");
        assert!(uuid.uuid.contains('-'), "UUID should contain dashes");
        t.shutdown().await;
    }
);

arch_tests!(
    query_migrate,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let _: MigrationInfo = t.execute(&QueryMigrate {}).await.unwrap();
        t.shutdown().await;
    }
);

arch_tests!(
    query_memory_size_summary,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let mem: MemoryInfo = t.execute(&QueryMemorySizeSummary {}).await.unwrap();
        assert_eq!(mem.base_memory, 0);
        t.shutdown().await;
    }
);

arch_tests!(
    query_display_options,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let opts: DisplayOptions = t.execute(&QueryDisplayOptions {}).await.unwrap();
        assert_eq!(serde_json::to_string(&opts.r#type).unwrap(), "\"none\"");
        t.shutdown().await;
    }
);

arch_version_tests!(query_current_machine, [X86_64, I386, Aarch64, Arm], >= (4, 0), |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let _: CurrentMachineParams = t.execute(&QueryCurrentMachine {}).await.unwrap();
    t.shutdown().await;
});

arch_version_tests!(query_replay, [X86_64, I386, Aarch64, Arm], >= (5, 2), |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let replay: ReplayInfo = t.execute(&QueryReplay {}).await.unwrap();
    assert_eq!(serde_json::to_string(&replay.mode).unwrap(), "\"none\"");
    t.shutdown().await;
});

arch_version_tests!(query_dirty_rate, [X86_64, I386, Aarch64, Arm], >= (5, 2), |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let rate: DirtyRateInfo = t.execute(&QueryDirtyRate { calc_time_unit: None }).await.unwrap();
    assert_eq!(serde_json::to_string(&rate.status).unwrap(), "\"unstarted\"");
    assert_eq!(rate.start_time, 0);
    t.shutdown().await;
});

arch_tests!(
    query_vnc,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let vnc: VncInfo = t.execute(&QueryVnc {}).await.unwrap();
        assert!(!vnc.enabled);
        t.shutdown().await;
    }
);

arch_tests!(
    query_dump_guest_memory_capability,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let caps: DumpGuestMemoryCapability =
            t.execute(&QueryDumpGuestMemoryCapability {}).await.unwrap();
        assert!(
            !caps.formats.is_empty(),
            "Should have at least one dump format"
        );
        t.shutdown().await;
    }
);

arch_tests!(
    query_migrate_parameters,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let params: MigrationParameters = t.execute(&QueryMigrateParameters {}).await.unwrap();
        let json = serde_json::to_string(&params).unwrap();
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        t.shutdown().await;
    }
);

// ══════════════════════════════════════════════════════════════════════════════
// List query commands
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(
    query_commands,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let cmds: Vec<CommandInfo> = t.execute(&QueryCommands {}).await.unwrap();
        assert!(
            cmds.len() > 100,
            "Expected >100 commands, got {}",
            cmds.len()
        );
        assert!(cmds.iter().any(|c| c.name == "query-status"));
        assert!(cmds.iter().any(|c| c.name == "quit"));
        t.shutdown().await;
    }
);

arch_tests!(
    query_chardev,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let devs: Vec<ChardevInfo> = t.execute(&QueryChardev {}).await.unwrap();
        assert!(
            !devs.is_empty(),
            "Should have at least 1 chardev (QMP socket)"
        );
        assert!(!devs[0].label.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_chardev_backends,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let backends: Vec<ChardevBackendInfo> = t.execute(&QueryChardevBackends {}).await.unwrap();
        assert!(!backends.is_empty());
        let names: Vec<&str> = backends.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"null"), "Missing 'null' backend");
        assert!(names.contains(&"socket"), "Missing 'socket' backend");
        t.shutdown().await;
    }
);

arch_tests!(
    query_block_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let blocks: Vec<BlockInfo> = t.execute(&QueryBlock {}).await.unwrap();
        assert!(blocks.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_block_jobs_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let jobs: Vec<BlockJobInfo> = t.execute(&QueryBlockJobs {}).await.unwrap();
        assert!(jobs.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_cpus_fast_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let cpus: Vec<CpuInfoFast> = t.execute(&QueryCpusFast {}).await.unwrap();
        assert!(cpus.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_iothreads_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let threads: Vec<IoThreadInfo> = t.execute(&QueryIothreads {}).await.unwrap();
        assert!(threads.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_mice_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let mice: Vec<MouseInfo> = t.execute(&QueryMice {}).await.unwrap();
        assert!(mice.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_pci_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let pci: Vec<PciInfo> = t.execute(&QueryPci {}).await.unwrap();
        assert!(pci.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_migrate_capabilities,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let caps: Vec<MigrationCapabilityStatus> =
            t.execute(&QueryMigrateCapabilities {}).await.unwrap();
        assert!(!caps.is_empty());
        for cap in &caps {
            let _ = serde_json::to_string(&cap.capability).unwrap();
        }
        t.shutdown().await;
    }
);

arch_tests!(
    query_memdev_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let devs: Vec<Memdev> = t.execute(&QueryMemdev {}).await.unwrap();
        assert!(devs.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_memory_devices_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let devs: Vec<MemoryDeviceInfo> = t.execute(&QueryMemoryDevices {}).await.unwrap();
        assert!(devs.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_fdsets_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let fdsets: Vec<FdsetInfo> = t.execute(&QueryFdsets {}).await.unwrap();
        assert!(fdsets.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_tpm_models,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let models: Vec<TpmModel> = t.execute(&QueryTpmModels {}).await.unwrap();
        assert!(!models.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_tpm_types,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let types: Vec<TpmType> = t.execute(&QueryTpmTypes {}).await.unwrap();
        assert!(!types.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_tpm_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let tpms: Vec<TpmInfo> = t.execute(&QueryTpm {}).await.unwrap();
        assert!(tpms.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_command_line_options,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let opts: Vec<CommandLineOptionInfo> = t
            .execute(&QueryCommandLineOptions { option: None })
            .await
            .unwrap();
        assert!(!opts.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_jobs_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let jobs: Vec<JobInfo> = t.execute(&QueryJobs {}).await.unwrap();
        assert!(jobs.is_empty());
        t.shutdown().await;
    }
);

arch_tests!(
    query_named_block_nodes_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let nodes: Vec<BlockDeviceInfo> = t
            .execute(&QueryNamedBlockNodes { flat: None })
            .await
            .unwrap();
        assert!(nodes.is_empty());
        t.shutdown().await;
    }
);

arch_version_tests!(query_block_exports_empty, [X86_64, I386, Aarch64, Arm], >= (5, 2), |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let exports: Vec<BlockExportInfo> = t.execute(&QueryBlockExports {}).await.unwrap();
    assert!(exports.is_empty());
    t.shutdown().await;
});

arch_version_tests!(query_cryptodev_empty, [X86_64, I386, Aarch64, Arm], >= (8, 0), |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let devs: Vec<QCryptodevInfo> = t.execute(&QueryCryptodev {}).await.unwrap();
    assert!(devs.is_empty());
    t.shutdown().await;
});

arch_tests!(
    query_vnc_servers_empty,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let servers: Vec<VncInfo2> = t.execute(&QueryVncServers {}).await.unwrap();
        assert!(servers.is_empty());
        t.shutdown().await;
    }
);

arch_version_tests!(query_migrationthreads_empty, [X86_64, I386, Aarch64, Arm], >= (8, 0), |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let threads: Vec<MigrationThreadInfo> = t.execute(&QueryMigrationthreads {}).await.unwrap();
    assert!(threads.is_empty());
    t.shutdown().await;
});

arch_version_tests!(query_yank, [X86_64, I386, Aarch64, Arm], >= (6, 0), |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let instances: Vec<YankInstance> = t.execute(&QueryYank {}).await.unwrap();
    for inst in &instances {
        let _ = serde_json::to_string(&inst.r#type).unwrap();
    }
    t.shutdown().await;
});

arch_version_tests!(query_audiodevs_empty, [X86_64, I386, Aarch64, Arm], >= (8, 0), |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let devs: Vec<Audiodev> = t.execute(&QueryAudiodevs {}).await.unwrap();
    assert!(devs.is_empty());
    t.shutdown().await;
});

arch_version_tests!(query_vcpu_dirty_limit_empty, [X86_64, I386, Aarch64, Arm], >= (7, 1), |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let limits: Vec<DirtyLimitInfo> = t.execute(&QueryVcpuDirtyLimit {}).await.unwrap();
    assert!(limits.is_empty());
    t.shutdown().await;
});

// ══════════════════════════════════════════════════════════════════════════════
// Round-trip serialization: query → serialize → deserialize → compare
// ══════════════════════════════════════════════════════════════════════════════

arch_tests!(
    roundtrip_status,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let status: StatusInfo = t.execute(&QueryStatus {}).await.unwrap();
        let json = serde_json::to_string(&status).unwrap();
        let back: StatusInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.running, status.running);
        t.shutdown().await;
    }
);

arch_tests!(
    roundtrip_kvm,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let kvm: KvmInfo = t.execute(&QueryKvm {}).await.unwrap();
        let json = serde_json::to_string(&kvm).unwrap();
        let back: KvmInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.present, kvm.present);
        assert_eq!(back.enabled, kvm.enabled);
        t.shutdown().await;
    }
);

arch_tests!(
    roundtrip_uuid,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let uuid: UuidInfo = t.execute(&QueryUuid {}).await.unwrap();
        let json = serde_json::to_string(&uuid).unwrap();
        let back: UuidInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.uuid, uuid.uuid);
        t.shutdown().await;
    }
);

arch_tests!(
    roundtrip_memory,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let mem: MemoryInfo = t.execute(&QueryMemorySizeSummary {}).await.unwrap();
        let json = serde_json::to_string(&mem).unwrap();
        let back: MemoryInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.base_memory, mem.base_memory);
        t.shutdown().await;
    }
);

arch_tests!(
    roundtrip_migrate_params,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let params: MigrationParameters = t.execute(&QueryMigrateParameters {}).await.unwrap();
        let json = serde_json::to_string(&params).unwrap();
        let back: MigrationParameters = serde_json::from_str(&json).unwrap();
        assert_eq!(back.multifd_channels, params.multifd_channels);
        t.shutdown().await;
    }
);

arch_version_tests!(roundtrip_dirty_rate, [X86_64, I386, Aarch64, Arm], >= (5, 2), |arch: Arch| async move {
    let t = VmBuilder::new(arch, MachineConfig::None).spawn().await.unwrap().start().await.unwrap();
    let rate: DirtyRateInfo = t.execute(&QueryDirtyRate { calc_time_unit: None }).await.unwrap();
    let json = serde_json::to_string(&rate).unwrap();
    let back: DirtyRateInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.start_time, rate.start_time);
    t.shutdown().await;
});

arch_tests!(
    roundtrip_display_options,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let opts: DisplayOptions = t.execute(&QueryDisplayOptions {}).await.unwrap();
        let json = serde_json::to_string(&opts).unwrap();
        let _: DisplayOptions = serde_json::from_str(&json).unwrap();
        t.shutdown().await;
    }
);

arch_tests!(
    roundtrip_chardev,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let devs: Vec<ChardevInfo> = t.execute(&QueryChardev {}).await.unwrap();
        for dev in &devs {
            let json = serde_json::to_string(dev).unwrap();
            let back: ChardevInfo = serde_json::from_str(&json).unwrap();
            assert_eq!(back.label, dev.label);
        }
        t.shutdown().await;
    }
);

arch_tests!(
    roundtrip_migrate_caps,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let caps: Vec<MigrationCapabilityStatus> =
            t.execute(&QueryMigrateCapabilities {}).await.unwrap();
        for cap in &caps {
            let json = serde_json::to_string(cap).unwrap();
            let back: MigrationCapabilityStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back.state, cap.state);
        }
        t.shutdown().await;
    }
);

arch_tests!(
    roundtrip_chardev_backends,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let backends: Vec<ChardevBackendInfo> = t.execute(&QueryChardevBackends {}).await.unwrap();
        for b in &backends {
            let json = serde_json::to_string(b).unwrap();
            let back: ChardevBackendInfo = serde_json::from_str(&json).unwrap();
            assert_eq!(back.name, b.name);
        }
        t.shutdown().await;
    }
);

arch_tests!(
    roundtrip_command_line_options,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let opts: Vec<CommandLineOptionInfo> = t
            .execute(&QueryCommandLineOptions { option: None })
            .await
            .unwrap();
        for opt in &opts {
            let json = serde_json::to_string(opt).unwrap();
            let back: CommandLineOptionInfo = serde_json::from_str(&json).unwrap();
            assert_eq!(back.option, opt.option);
        }
        t.shutdown().await;
    }
);

arch_tests!(
    roundtrip_dump_capability,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let caps: DumpGuestMemoryCapability =
            t.execute(&QueryDumpGuestMemoryCapability {}).await.unwrap();
        let json = serde_json::to_string(&caps).unwrap();
        let back: DumpGuestMemoryCapability = serde_json::from_str(&json).unwrap();
        assert_eq!(back.formats.len(), caps.formats.len());
        t.shutdown().await;
    }
);

arch_tests!(
    roundtrip_tpm,
    [X86_64, I386, Aarch64, Arm],
    |arch: Arch| async move {
        let t = VmBuilder::new(arch, MachineConfig::None)
            .spawn()
            .await
            .unwrap()
            .start()
            .await
            .unwrap();
        let models: Vec<TpmModel> = t.execute(&QueryTpmModels {}).await.unwrap();
        for m in &models {
            let json = serde_json::to_string(m).unwrap();
            let back: TpmModel = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&back).unwrap(), json);
        }
        let types: Vec<TpmType> = t.execute(&QueryTpmTypes {}).await.unwrap();
        for ty in &types {
            let json = serde_json::to_string(ty).unwrap();
            let back: TpmType = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&back).unwrap(), json);
        }
        t.shutdown().await;
    }
);
