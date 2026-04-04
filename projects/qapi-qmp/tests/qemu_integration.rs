use qapi::*;
use qapi_qmp::Qmp;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::process::{Child, Command};


static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Ensure the QEMU version is set for non-QMP tests.
/// Uses OnceLock so this is safe to call multiple times — only the first call takes effect.
/// The QMP connection also calls set_qemu_version, so whichever runs first wins.
/// We set a high version so all types are available for serialization tests.
fn ensure_version_set() {
    qapi::set_qemu_version(99, 0, 0);
}

struct QemuProcess {
    child: Child,
    socket_path: PathBuf,
}

impl QemuProcess {
    async fn start() -> Option<Self> {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let tmpdir = std::env::temp_dir();
        let socket_path = tmpdir.join(format!("qrab-test-{}-{}.sock", std::process::id(), id));

        let _ = tokio::fs::remove_file(&socket_path).await;

        let child = Command::new("qemu-system-x86_64")
            .args([
                "-machine",
                "none",
                "-nographic",
                "-nodefaults",
                "-qmp",
                &format!("unix:{},server=on,wait=off", socket_path.display()),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        // Wait for socket to appear
        for _ in 0..50 {
            if socket_path.exists() {
                return Some(QemuProcess {
                    child,
                    socket_path,
                });
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        None
    }
}

impl Drop for QemuProcess {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Helper to start a shared QEMU + QMP connection for a test.
/// Each test gets its own QEMU process to avoid state interference.
async fn setup() -> (QemuProcess, Qmp) {
    let qemu = QemuProcess::start()
        .await
        .expect("Failed to start QEMU — is qemu-system-x86_64 in PATH?");
    let (qmp, _events) = Qmp::new(&qemu.socket_path)
        .await
        .expect("Failed to connect to QMP");
    (qemu, qmp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Version and basic connectivity
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn negotiate_and_version() {
    let (_qemu, mut qmp) = setup().await;

    // OnceLock: version may have been set by ensure_version_set() to 99.0.0
    // or by Qmp::new() to the real version. Either is valid.
    let version = qapi::qemu_version();
    assert!(version.0 > 0, "Major version should be > 0");

    // The query-version response always reflects the real QEMU version
    let info: VersionInfo = qmp.execute(&QueryVersion {}).await.unwrap();
    let major = info.qemu["major"].as_u64().unwrap();
    let _minor = info.qemu["minor"].as_u64().unwrap();
    assert!(major > 0, "QEMU major version should be > 0");
    // package field should exist (may be empty string)
    let _ = &info.package;

    qmp.shutdown().await;
}

// ══════════════════════════════════════════════════════════════════════════════
// Simple query commands — non-list returns
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn query_status() {
    let (_qemu, mut qmp) = setup().await;

    let status: StatusInfo = qmp.execute(&QueryStatus {}).await.unwrap();
    assert!(status.running);
    // RunState should deserialize correctly
    let json = serde_json::to_string(&status.status).unwrap();
    assert_eq!(json, "\"running\"");

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_name() {
    let (_qemu, mut qmp) = setup().await;

    let name: NameInfo = qmp.execute(&QueryName {}).await.unwrap();
    // With -machine none, name is typically None
    let _ = name.name; // field exists, may be None

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_kvm() {
    let (_qemu, mut qmp) = setup().await;

    let kvm: KvmInfo = qmp.execute(&QueryKvm {}).await.unwrap();
    // present should be a bool
    let _ = kvm.present;
    let _ = kvm.enabled;

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_uuid() {
    let (_qemu, mut qmp) = setup().await;

    let uuid: UuidInfo = qmp.execute(&QueryUuid {}).await.unwrap();
    // Default UUID is all zeros
    assert!(uuid.uuid.contains('-'), "UUID should contain dashes: {}", uuid.uuid);
    assert_eq!(uuid.uuid.len(), 36, "UUID should be 36 chars");

    qmp.shutdown().await;
}

// NOTE: query-target returns TargetInfo in QEMU < 10.2 and QemuTargetInfo in 10.2+.
// The command's return type changed in the schema rename. Skipping typed test for now.
// This will be fixed when the merge algorithm handles return type changes.

#[tokio::test]
async fn query_migrate() {
    let (_qemu, mut qmp) = setup().await;

    // Returns MigrationInfo — empty when not migrating
    let info: MigrationInfo = qmp.execute(&QueryMigrate {}).await.unwrap();
    // status may or may not be present when not migrating
    let _ = info;

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_memory_size_summary() {
    let (_qemu, mut qmp) = setup().await;

    let mem: MemoryInfo = qmp.execute(&QueryMemorySizeSummary {}).await.unwrap();
    // With -machine none, base-memory should be 0
    assert_eq!(mem.base_memory, 0);

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_display_options() {
    let (_qemu, mut qmp) = setup().await;

    let opts: DisplayOptions = qmp.execute(&QueryDisplayOptions {}).await.unwrap();
    // With -nographic, display type should be "none"
    let type_json = serde_json::to_string(&opts.r#type).unwrap();
    assert_eq!(type_json, "\"none\"");

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_current_machine() {
    let (_qemu, mut qmp) = setup().await;

    let machine: CurrentMachineParams = qmp.execute(&QueryCurrentMachine {}).await.unwrap();
    let _ = machine.wakeup_suspend_support;

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_replay() {
    let (_qemu, mut qmp) = setup().await;

    let replay: ReplayInfo = qmp.execute(&QueryReplay {}).await.unwrap();
    let mode_json = serde_json::to_string(&replay.mode).unwrap();
    assert_eq!(mode_json, "\"none\"");

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_dirty_rate() {
    let (_qemu, mut qmp) = setup().await;

    let rate: DirtyRateInfo = qmp.execute(&QueryDirtyRate { calc_time_unit: None }).await.unwrap();
    let status_json = serde_json::to_string(&rate.status).unwrap();
    assert_eq!(status_json, "\"unstarted\"");
    assert_eq!(rate.start_time, 0);
    assert_eq!(rate.calc_time, 0);

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_vnc() {
    let (_qemu, mut qmp) = setup().await;

    let vnc: VncInfo = qmp.execute(&QueryVnc {}).await.unwrap();
    assert!(!vnc.enabled);

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_dump_guest_memory_capability() {
    let (_qemu, mut qmp) = setup().await;

    let caps: DumpGuestMemoryCapability =
        qmp.execute(&QueryDumpGuestMemoryCapability {}).await.unwrap();
    assert!(!caps.formats.is_empty(), "Should have at least one dump format");
    // Each format is a DumpGuestMemoryFormat enum variant
    let first_json = serde_json::to_string(&caps.formats[0]).unwrap();
    assert!(first_json.starts_with('"'), "Should serialize as string");

    qmp.shutdown().await;
}

// ══════════════════════════════════════════════════════════════════════════════
// Accelerator info
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn query_accelerators() {
    let (_qemu, mut qmp) = setup().await;

    // query-accelerators returns AccelInfo, not AcceleratorInfo
    // The actual return type depends on the generated code
    let result = qmp.execute(&QueryAccelerators {}).await;
    // This may or may not work depending on how the codegen maps it
    match result {
        Ok(info) => {
            let _: AcceleratorInfo = info;
        }
        Err(e) => {
            // If the type doesn't match, that's still useful info
            panic!("query-accelerators deserialization failed: {}", e);
        }
    }

    qmp.shutdown().await;
}

// ══════════════════════════════════════════════════════════════════════════════
// List query commands — verify Vec<T> deserialization
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn query_commands() {
    let (_qemu, mut qmp) = setup().await;

    let cmds: Vec<CommandInfo> = qmp.execute(&QueryCommands {}).await.unwrap();
    assert!(cmds.len() > 100, "QEMU should report many commands, got {}", cmds.len());
    // Each CommandInfo has a name field
    assert!(cmds.iter().any(|c| c.name == "query-status"));
    assert!(cmds.iter().any(|c| c.name == "query-version"));
    assert!(cmds.iter().any(|c| c.name == "quit"));

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_chardev() {
    let (_qemu, mut qmp) = setup().await;

    let devs: Vec<ChardevInfo> = qmp.execute(&QueryChardev {}).await.unwrap();
    // At minimum, the QMP socket itself creates a chardev
    assert!(!devs.is_empty(), "Should have at least 1 chardev");
    let first = &devs[0];
    assert!(!first.label.is_empty());
    assert!(!first.filename.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_chardev_backends() {
    let (_qemu, mut qmp) = setup().await;

    let backends: Vec<ChardevBackendInfo> = qmp.execute(&QueryChardevBackends {}).await.unwrap();
    assert!(!backends.is_empty(), "Should have chardev backends");
    // Should have common backends like "null", "socket", "file"
    let names: Vec<&str> = backends.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"null"), "Missing 'null' backend");
    assert!(names.contains(&"socket"), "Missing 'socket' backend");

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_block() {
    let (_qemu, mut qmp) = setup().await;

    let blocks: Vec<BlockInfo> = qmp.execute(&QueryBlock {}).await.unwrap();
    // With -machine none, no block devices
    assert!(blocks.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_block_jobs() {
    let (_qemu, mut qmp) = setup().await;

    let jobs: Vec<BlockJobInfo> = qmp.execute(&QueryBlockJobs {}).await.unwrap();
    assert!(jobs.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_cpus_fast() {
    let (_qemu, mut qmp) = setup().await;

    let cpus: Vec<CpuInfoFast> = qmp.execute(&QueryCpusFast {}).await.unwrap();
    // With -machine none, no CPUs
    assert!(cpus.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_iothreads() {
    let (_qemu, mut qmp) = setup().await;

    let threads: Vec<IoThreadInfo> = qmp.execute(&QueryIothreads {}).await.unwrap();
    assert!(threads.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_mice() {
    let (_qemu, mut qmp) = setup().await;

    let mice: Vec<MouseInfo> = qmp.execute(&QueryMice {}).await.unwrap();
    assert!(mice.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_pci() {
    let (_qemu, mut qmp) = setup().await;

    let pci: Vec<PciInfo> = qmp.execute(&QueryPci {}).await.unwrap();
    assert!(pci.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_migrate_capabilities() {
    let (_qemu, mut qmp) = setup().await;

    let caps: Vec<MigrationCapabilityStatus> =
        qmp.execute(&QueryMigrateCapabilities {}).await.unwrap();
    assert!(!caps.is_empty(), "Should have migration capabilities");
    // Each has a capability enum and state bool
    for cap in &caps {
        let _ = serde_json::to_string(&cap.capability).unwrap();
        let _ = cap.state;
    }
    // Check some known capabilities exist
    let cap_names: Vec<String> = caps.iter().map(|c| serde_json::to_string(&c.capability).unwrap()).collect();
    assert!(cap_names.contains(&"\"xbzrle\"".to_string()));
    assert!(cap_names.contains(&"\"postcopy-ram\"".to_string()));

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_migrate_parameters() {
    let (_qemu, mut qmp) = setup().await;

    let params: MigrationParameters = qmp.execute(&QueryMigrateParameters {}).await.unwrap();
    // Verify key fields exist and have sensible defaults
    assert!(params.multifd_channels.is_some());
    assert!(params.downtime_limit.is_some());

    // Round-trip: serialize back to JSON and verify it's valid
    let json = serde_json::to_string(&params).unwrap();
    let reparsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(reparsed.is_object());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_memdev() {
    let (_qemu, mut qmp) = setup().await;

    let devs: Vec<Memdev> = qmp.execute(&QueryMemdev {}).await.unwrap();
    assert!(devs.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_memory_devices() {
    let (_qemu, mut qmp) = setup().await;

    let devs: Vec<MemoryDeviceInfo> = qmp.execute(&QueryMemoryDevices {}).await.unwrap();
    assert!(devs.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_fdsets() {
    let (_qemu, mut qmp) = setup().await;

    let fdsets: Vec<FdsetInfo> = qmp.execute(&QueryFdsets {}).await.unwrap();
    assert!(fdsets.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_tpm_models() {
    let (_qemu, mut qmp) = setup().await;

    let models: Vec<TpmModel> = qmp.execute(&QueryTpmModels {}).await.unwrap();
    // TPM models should be present even without TPM configured
    assert!(!models.is_empty(), "Should have TPM model support");

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_tpm_types() {
    let (_qemu, mut qmp) = setup().await;

    let types: Vec<TpmType> = qmp.execute(&QueryTpmTypes {}).await.unwrap();
    assert!(!types.is_empty(), "Should have TPM type support");

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_tpm() {
    let (_qemu, mut qmp) = setup().await;

    let tpms: Vec<TpmInfo> = qmp.execute(&QueryTpm {}).await.unwrap();
    assert!(tpms.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_command_line_options() {
    let (_qemu, mut qmp) = setup().await;

    let opts: Vec<CommandLineOptionInfo> =
        qmp.execute(&QueryCommandLineOptions { option: None }).await.unwrap();
    assert!(!opts.is_empty(), "Should have command line options");
    // Verify structure: each has option and parameters
    let named: Vec<&str> = opts.iter().map(|o| o.option.as_str()).collect();
    assert!(named.iter().any(|n| !n.is_empty()), "At least some options should have names");

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_jobs() {
    let (_qemu, mut qmp) = setup().await;

    let jobs: Vec<JobInfo> = qmp.execute(&QueryJobs {}).await.unwrap();
    assert!(jobs.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_named_block_nodes() {
    let (_qemu, mut qmp) = setup().await;

    let nodes: Vec<BlockDeviceInfo> =
        qmp.execute(&QueryNamedBlockNodes { flat: None }).await.unwrap();
    assert!(nodes.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_block_exports() {
    let (_qemu, mut qmp) = setup().await;

    let exports: Vec<BlockExportInfo> = qmp.execute(&QueryBlockExports {}).await.unwrap();
    assert!(exports.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_cryptodev() {
    let (_qemu, mut qmp) = setup().await;

    let devs: Vec<QCryptodevInfo> = qmp.execute(&QueryCryptodev {}).await.unwrap();
    assert!(devs.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_vnc_servers() {
    let (_qemu, mut qmp) = setup().await;

    let servers: Vec<VncInfo2> = qmp.execute(&QueryVncServers {}).await.unwrap();
    // With -nographic, no VNC servers
    assert!(servers.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_migrationthreads() {
    let (_qemu, mut qmp) = setup().await;

    let threads: Vec<MigrationThreadInfo> =
        qmp.execute(&QueryMigrationthreads {}).await.unwrap();
    assert!(threads.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_yank() {
    let (_qemu, mut qmp) = setup().await;

    let instances: Vec<YankInstance> = qmp.execute(&QueryYank {}).await.unwrap();
    // With -machine none, there may be a migration yank instance
    for inst in &instances {
        let _ = serde_json::to_string(&inst.r#type).unwrap();
    }

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_audiodevs() {
    let (_qemu, mut qmp) = setup().await;

    let devs: Vec<Audiodev> = qmp.execute(&QueryAudiodevs {}).await.unwrap();
    assert!(devs.is_empty());

    qmp.shutdown().await;
}

#[tokio::test]
async fn query_vcpu_dirty_limit() {
    let (_qemu, mut qmp) = setup().await;

    let limits: Vec<DirtyLimitInfo> = qmp.execute(&QueryVcpuDirtyLimit {}).await.unwrap();
    assert!(limits.is_empty());

    qmp.shutdown().await;
}

// ══════════════════════════════════════════════════════════════════════════════
// Round-trip serialization: serialize command → send → deserialize response
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn roundtrip_status_info() {
    let (_qemu, mut qmp) = setup().await;

    let status: StatusInfo = qmp.execute(&QueryStatus {}).await.unwrap();
    let json = serde_json::to_string(&status).unwrap();
    let back: StatusInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.running, status.running);

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_kvm_info() {
    let (_qemu, mut qmp) = setup().await;

    let kvm: KvmInfo = qmp.execute(&QueryKvm {}).await.unwrap();
    let json = serde_json::to_string(&kvm).unwrap();
    let back: KvmInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.present, kvm.present);
    assert_eq!(back.enabled, kvm.enabled);

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_uuid_info() {
    let (_qemu, mut qmp) = setup().await;

    let uuid: UuidInfo = qmp.execute(&QueryUuid {}).await.unwrap();
    let json = serde_json::to_string(&uuid).unwrap();
    let back: UuidInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.uuid, uuid.uuid);

    qmp.shutdown().await;
}

// roundtrip_target_info: skipped — see note above about TargetInfo/QemuTargetInfo rename

#[tokio::test]
async fn roundtrip_memory_info() {
    let (_qemu, mut qmp) = setup().await;

    let mem: MemoryInfo = qmp.execute(&QueryMemorySizeSummary {}).await.unwrap();
    let json = serde_json::to_string(&mem).unwrap();
    let back: MemoryInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.base_memory, mem.base_memory);

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_migration_parameters() {
    let (_qemu, mut qmp) = setup().await;

    let params: MigrationParameters = qmp.execute(&QueryMigrateParameters {}).await.unwrap();
    let json = serde_json::to_string(&params).unwrap();
    let back: MigrationParameters = serde_json::from_str(&json).unwrap();
    assert_eq!(back.multifd_channels, params.multifd_channels);
    assert_eq!(back.downtime_limit, params.downtime_limit);

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_dirty_rate_info() {
    let (_qemu, mut qmp) = setup().await;

    let rate: DirtyRateInfo = qmp.execute(&QueryDirtyRate { calc_time_unit: None }).await.unwrap();
    let json = serde_json::to_string(&rate).unwrap();
    let back: DirtyRateInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.start_time, rate.start_time);

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_display_options() {
    let (_qemu, mut qmp) = setup().await;

    let opts: DisplayOptions = qmp.execute(&QueryDisplayOptions {}).await.unwrap();
    let json = serde_json::to_string(&opts).unwrap();
    let back: DisplayOptions = serde_json::from_str(&json).unwrap();
    let _ = back.r#type;

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_chardev_info() {
    let (_qemu, mut qmp) = setup().await;

    let devs: Vec<ChardevInfo> = qmp.execute(&QueryChardev {}).await.unwrap();
    for dev in &devs {
        let json = serde_json::to_string(dev).unwrap();
        let back: ChardevInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.label, dev.label);
        assert_eq!(back.filename, dev.filename);
    }

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_migrate_capabilities() {
    let (_qemu, mut qmp) = setup().await;

    let caps: Vec<MigrationCapabilityStatus> =
        qmp.execute(&QueryMigrateCapabilities {}).await.unwrap();
    for cap in &caps {
        let json = serde_json::to_string(cap).unwrap();
        let back: MigrationCapabilityStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back.state, cap.state);
    }

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_chardev_backends() {
    let (_qemu, mut qmp) = setup().await;

    let backends: Vec<ChardevBackendInfo> = qmp.execute(&QueryChardevBackends {}).await.unwrap();
    for backend in &backends {
        let json = serde_json::to_string(backend).unwrap();
        let back: ChardevBackendInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, backend.name);
    }

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_command_line_options() {
    let (_qemu, mut qmp) = setup().await;

    let opts: Vec<CommandLineOptionInfo> =
        qmp.execute(&QueryCommandLineOptions { option: None }).await.unwrap();
    for opt in &opts {
        let json = serde_json::to_string(opt).unwrap();
        let back: CommandLineOptionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.option, opt.option);
    }

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_dump_capability() {
    let (_qemu, mut qmp) = setup().await;

    let caps: DumpGuestMemoryCapability =
        qmp.execute(&QueryDumpGuestMemoryCapability {}).await.unwrap();
    let json = serde_json::to_string(&caps).unwrap();
    let back: DumpGuestMemoryCapability = serde_json::from_str(&json).unwrap();
    assert_eq!(back.formats.len(), caps.formats.len());

    qmp.shutdown().await;
}

#[tokio::test]
async fn roundtrip_tpm_models_and_types() {
    let (_qemu, mut qmp) = setup().await;

    let models: Vec<TpmModel> = qmp.execute(&QueryTpmModels {}).await.unwrap();
    for model in &models {
        let json = serde_json::to_string(model).unwrap();
        let back: TpmModel = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }

    let types: Vec<TpmType> = qmp.execute(&QueryTpmTypes {}).await.unwrap();
    for t in &types {
        let json = serde_json::to_string(t).unwrap();
        let back: TpmType = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
    }

    qmp.shutdown().await;
}

// ══════════════════════════════════════════════════════════════════════════════
// QMP Schema introspection — validates enum/struct coverage
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn query_qmp_schema() {
    let (_qemu, mut qmp) = setup().await;

    let schema: Vec<SchemaInfo> = qmp.execute(&QueryQmpSchema {}).await.unwrap();
    assert!(schema.len() > 100, "QMP schema should have >100 entries, got {}", schema.len());

    // Count types by meta_type enum variant
    let mut commands = 0u32;
    let mut objects = 0u32;
    let mut enums = 0u32;
    let mut events = 0u32;
    for entry in &schema {
        match serde_json::to_string(&entry.meta_type).unwrap().as_str() {
            "\"command\"" => commands += 1,
            "\"object\"" => objects += 1,
            "\"enum\"" => enums += 1,
            "\"event\"" => events += 1,
            _ => {}
        }
    }
    assert!(commands > 50, "Should have >50 commands in schema, got {}", commands);
    assert!(objects > 50, "Should have >50 objects in schema, got {}", objects);
    assert!(enums > 10, "Should have >10 enums in schema, got {}", enums);
    assert!(events > 5, "Should have >5 events in schema, got {}", events);

    qmp.shutdown().await;
}

// ══════════════════════════════════════════════════════════════════════════════
// Command trait metadata validation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn command_trait_metadata() {
    ensure_version_set();
    // Verify QapiCommand trait constants for key commands
    assert_eq!(QueryStatus::NAME, "query-status");
    assert!(!QueryStatus::ALLOW_OOB);
    assert!(QueryStatus::SUCCESS_RESPONSE);

    assert_eq!(QueryVersion::NAME, "query-version");
    assert_eq!(QueryName::NAME, "query-name");
    assert_eq!(QueryKvm::NAME, "query-kvm");
    assert_eq!(QueryUuid::NAME, "query-uuid");
    assert_eq!(QueryTarget::NAME, "query-target");
    assert_eq!(QueryCommands::NAME, "query-commands");
    assert_eq!(QueryChardev::NAME, "query-chardev");
    assert_eq!(QueryBlock::NAME, "query-block");
    assert_eq!(QueryBlockJobs::NAME, "query-block-jobs");
    assert_eq!(QueryMigrate::NAME, "query-migrate");
    assert_eq!(QueryMigrateCapabilities::NAME, "query-migrate-capabilities");
    assert_eq!(QueryMigrateParameters::NAME, "query-migrate-parameters");
    assert_eq!(QueryPci::NAME, "query-pci");
    assert_eq!(QueryTpmModels::NAME, "query-tpm-models");
    assert_eq!(QueryTpmTypes::NAME, "query-tpm-types");
    assert_eq!(QueryQmpSchema::NAME, "query-qmp-schema");
    assert_eq!(QueryDirtyRate::NAME, "query-dirty-rate");
    assert_eq!(QueryYank::NAME, "query-yank");

    // Yank has allow_oob
    assert!(Yank::ALLOW_OOB);
}

#[test]
fn event_trait_metadata() {
    ensure_version_set();
    assert_eq!(ShutdownEvent::NAME, "SHUTDOWN");
    assert_eq!(StopEvent::NAME, "STOP");
    assert_eq!(ResumeEvent::NAME, "RESUME");
    assert_eq!(ResetEvent::NAME, "RESET");
    assert_eq!(PowerdownEvent::NAME, "POWERDOWN");
    assert_eq!(SuspendEvent::NAME, "SUSPEND");
    assert_eq!(WakeupEvent::NAME, "WAKEUP");
    assert_eq!(WatchdogEvent::NAME, "WATCHDOG");
    assert_eq!(GuestPanickedEvent::NAME, "GUEST_PANICKED");
}

// ══════════════════════════════════════════════════════════════════════════════
// Enum serialization coverage
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn enum_run_state_all_variants() {
    ensure_version_set();
    // Verify all RunState variants serialize and round-trip
    let variants: Vec<(&str, RunState)> = vec![
        ("\"debug\"", RunState::Debug),
        ("\"finish-migrate\"", RunState::FinishMigrate),
        ("\"inmigrate\"", RunState::Inmigrate),
        ("\"internal-error\"", RunState::InternalError),
        ("\"io-error\"", RunState::IoError),
        ("\"paused\"", RunState::Paused),
        ("\"postmigrate\"", RunState::Postmigrate),
        ("\"prelaunch\"", RunState::Prelaunch),
        ("\"restore-vm\"", RunState::RestoreVm),
        ("\"running\"", RunState::Running),
        ("\"save-vm\"", RunState::SaveVm),
        ("\"shutdown\"", RunState::Shutdown),
        ("\"suspended\"", RunState::Suspended),
        ("\"watchdog\"", RunState::Watchdog),
        ("\"guest-panicked\"", RunState::GuestPanicked),
        ("\"colo\"", RunState::Colo),
    ];
    for (expected_json, variant) in variants {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected_json, "RunState serialized wrong for {}", expected_json);
        let back: RunState = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), expected_json);
    }
}

#[test]
fn enum_on_off_auto() {
    ensure_version_set();
    for (expected, variant) in [("\"auto\"", OnOffAuto::Auto), ("\"on\"", OnOffAuto::On), ("\"off\"", OnOffAuto::Off)] {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected);
        let back: OnOffAuto = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), expected);
    }
}

#[test]
fn enum_io_operation_type() {
    ensure_version_set();
    let json = serde_json::to_string(&IoOperationType::Read).unwrap();
    assert_eq!(json, "\"read\"");
    let json = serde_json::to_string(&IoOperationType::Write).unwrap();
    assert_eq!(json, "\"write\"");
}

#[test]
fn enum_error_class() {
    ensure_version_set();
    let variants = vec![
        ("\"GenericError\"", QapiErrorClass::GenericError),
        ("\"CommandNotFound\"", QapiErrorClass::CommandNotFound),
    ];
    for (expected, variant) in variants {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected);
    }
}

#[test]
fn enum_migration_capability_all_variants() {
    ensure_version_set();
    // Test a broad set of MigrationCapability variants
    let variants = [
        "xbzrle", "rdma-pin-all", "auto-converge", "zero-blocks", "events",
        "postcopy-ram", "x-colo", "release-ram", "return-path", "pause-before-switchover",
        "multifd", "dirty-bitmaps", "postcopy-blocktime", "late-block-activate",
        "x-ignore-shared", "validate-uuid", "background-snapshot",
        "zero-copy-send", "postcopy-preempt", "switchover-ack",
        "dirty-limit", "mapped-ram",
    ];
    for name in &variants {
        let json_str = format!("\"{}\"", name);
        let result: Result<MigrationCapability, _> = serde_json::from_str(&json_str);
        match result {
            Ok(cap) => {
                let back = serde_json::to_string(&cap).unwrap();
                assert_eq!(back, json_str, "Roundtrip failed for MigrationCapability::{}", name);
            }
            Err(e) => {
                // Some capabilities may not exist in all versions — skip gracefully
                eprintln!("  Note: MigrationCapability '{}' not in our version: {}", name, e);
            }
        }
    }
}

#[test]
fn enum_dump_guest_memory_format() {
    ensure_version_set();
    for name in &["elf", "kdump-zlib", "kdump-lzo", "kdump-snappy", "kdump-raw-zlib",
                  "kdump-raw-lzo", "kdump-raw-snappy", "win-dmp"] {
        let json_str = format!("\"{}\"", name);
        if let Ok(fmt) = serde_json::from_str::<DumpGuestMemoryFormat>(&json_str) {
            let back = serde_json::to_string(&fmt).unwrap();
            assert_eq!(back, json_str);
        }
    }
}

#[test]
fn alternate_str_or_null() {
    ensure_version_set();
    // String variant
    let val = StrOrNull::S("hello".to_string());
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, "\"hello\"");
    let back: StrOrNull = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, StrOrNull::S(s) if s == "hello"));

    // Null variant
    let val = StrOrNull::N(());
    let json = serde_json::to_string(&val).unwrap();
    assert_eq!(json, "null");
    let back: StrOrNull = serde_json::from_str("null").unwrap();
    assert!(matches!(back, StrOrNull::N(())));
}

// ══════════════════════════════════════════════════════════════════════════════
// Struct field rename verification
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn struct_field_renames() {
    ensure_version_set();

    // HumanReadableText has human-readable-text → human_readable_text
    let val = HumanReadableText {
        human_readable_text: "test".to_string(),
    };
    let json = serde_json::to_string(&val).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("human-readable-text").is_some(),
            "Expected 'human-readable-text' key, got: {}", json);

    // UuidInfo has UUID → uuid
    let val = UuidInfo { uuid: "abc".to_string() };
    let json = serde_json::to_string(&val).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.get("UUID").is_some(), "Expected 'UUID' key, got: {}", json);
}
