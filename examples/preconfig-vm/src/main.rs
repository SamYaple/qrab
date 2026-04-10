use anyhow::{bail, Result};
use manager::{Arch, MachineConfig, VmBuilder};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

/// Search for OVMF firmware for x86_64 UEFI boot.
/// Checks environment variables first, then common system paths.
fn find_ovmf() -> Result<PathBuf> {
    // Environment variable override
    for var in ["QEMU_TEST_OVMF_X86_64", "QEMU_TEST_OVMF", "OVMF_CODE"] {
        if let Ok(path) = std::env::var(var) {
            let p = PathBuf::from(&path);
            if p.exists() {
                return Ok(p);
            }
        }
    }

    // Common system paths (Fedora, Debian/Ubuntu, QEMU bundled)
    let candidates = [
        "/usr/share/edk2-ovmf/OVMF_CODE.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
        "/usr/share/qemu/edk2-x86_64-code.fd",
        "/usr/share/edk2/ovmf/OVMF_CODE.fd",
        "/usr/share/edk2/x64/OVMF_CODE.fd",
    ];

    for path in &candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    bail!(
        "OVMF firmware not found. Set OVMF_CODE or QEMU_TEST_OVMF_X86_64 \
         to the path of OVMF_CODE.fd, or install edk2-ovmf / ovmf."
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    let ovmf = find_ovmf()?;
    println!("Using OVMF firmware: {}\n", ovmf.display());

    // ── Spawn UEFI VM in preconfig mode ──────────────────────────────────
    //
    // VmBuilder spawns QEMU with --preconfig, returning a PreconfigVm.
    // The machine is NOT initialized yet -- UEFI firmware has not loaded.
    // PreconfigVm::execute() enforces PreconfigSafe at compile time.

    let mut preconfig = VmBuilder::new(
        Arch::X86_64,
        MachineConfig::Uefi {
            memory: "256M".into(),
        },
    )
    .firmware(ovmf)
    .spawn()
    .await?;

    let version = qapi::qemu_version();
    println!(
        "Connected to QEMU {}.{}.{} (preconfig mode)\n",
        version.0, version.1, version.2
    );

    // ── Introspection queries (all preconfig-safe) ───────────────────────

    let ver: qapi::VersionInfo = preconfig.execute(&qapi::QueryVersion {}).await?;
    println!(
        "query-version: {}.{}.{}",
        ver.qemu.major, ver.qemu.minor, ver.qemu.micro
    );

    let status: qapi::StatusInfo = preconfig.execute(&qapi::QueryStatus {}).await?;
    println!("query-status: running={}", status.running);

    let name: qapi::NameInfo = preconfig.execute(&qapi::QueryName {}).await?;
    println!("query-name: {:?}", name.name);

    let uuid: qapi::UuidInfo = preconfig.execute(&qapi::QueryUuid {}).await?;
    println!("query-uuid: {}", uuid.uuid);

    let cmds: Vec<qapi::CommandInfo> = preconfig.execute(&qapi::QueryCommands {}).await?;
    println!("query-commands: {} commands available", cmds.len());

    let chardevs: Vec<qapi::ChardevInfo> = preconfig.execute(&qapi::QueryChardev {}).await?;
    println!("query-chardev: {} chardevs", chardevs.len());

    let iothreads: Vec<qapi::IoThreadInfo> = preconfig.execute(&qapi::QueryIothreads {}).await?;
    println!("query-iothreads: {} iothreads", iothreads.len());

    let memdevs: Vec<qapi::Memdev> = preconfig.execute(&qapi::QueryMemdev {}).await?;
    println!("query-memdev: {} memdevs", memdevs.len());

    let cpus: Vec<qapi::HotpluggableCpu> =
        preconfig.execute(&qapi::QueryHotpluggableCpus {}).await?;
    println!("query-hotpluggable-cpus: {} cpus", cpus.len());

    let opts: Vec<qapi::CommandLineOptionInfo> = preconfig
        .execute(&qapi::QueryCommandLineOptions { option: None })
        .await?;
    println!("query-command-line-options: {} option groups", opts.len());
    println!();

    // ── QOM introspection (preconfig-safe) ───────────────────────────────

    let types: Vec<qapi::ObjectTypeInfo> = preconfig
        .execute(&qapi::QomListTypes {
            r#abstract: None,
            implements: None,
        })
        .await?;
    println!("qom-list-types: {} types available", types.len());

    let root_props: Vec<qapi::ObjectPropertyInfo> = preconfig
        .execute(&qapi::QomList {
            path: "/".to_string(),
        })
        .await?;
    println!("qom-list /: {} properties", root_props.len());
    for prop in root_props.iter().take(5) {
        println!("  /{} ({})", prop.name, prop.r#type);
    }
    if root_props.len() > 5 {
        println!("  ... and {} more", root_props.len() - 5);
    }

    // List properties of the q35 machine type
    let machine_props: Vec<qapi::ObjectPropertyInfo> = preconfig
        .execute(&qapi::QomListProperties {
            typename: "machine".to_string(),
        })
        .await?;
    println!(
        "qom-list-properties machine: {} properties",
        machine_props.len()
    );

    // Show a few interesting machine properties
    let interesting = ["type", "kernel", "memory-backend", "firmware"];
    for prop in &machine_props {
        if interesting.contains(&prop.name.as_str()) {
            println!("  {} ({})", prop.name, prop.r#type);
        }
    }
    println!();

    // ── Block layer queries (preconfig-safe since 7.1.0) ─────────────────

    let blocks: Vec<qapi::BlockInfo> = preconfig.execute(&qapi::QueryBlock {}).await?;
    println!(
        "query-block: {} devices (empty before blockdev-add)",
        blocks.len()
    );

    let block_jobs: Vec<qapi::BlockJobInfo> = preconfig.execute(&qapi::QueryBlockJobs {}).await?;
    println!("query-block-jobs: {} jobs", block_jobs.len());

    let blockstats: Vec<qapi::BlockStats> = preconfig
        .execute(&qapi::QueryBlockstats { query_nodes: None })
        .await?;
    println!("query-blockstats: {} entries", blockstats.len());

    // query-named-block-nodes: should show the OVMF pflash node
    let named_result: Result<Vec<qapi::BlockDeviceInfo>> = preconfig
        .execute(&qapi::QueryNamedBlockNodes { flat: None })
        .await;
    match named_result {
        Ok(nodes) => {
            println!("query-named-block-nodes: {} nodes", nodes.len());
            for node in &nodes {
                println!(
                    "  {} (drv={})",
                    node.node_name.as_deref().unwrap_or("?"),
                    node.drv
                );
            }
        }
        Err(e) => {
            // May fail on older QEMU or deserialization issues
            println!("query-named-block-nodes: skipped ({})", e);
        }
    }
    println!();

    // ── Drain events during preconfig ────────────────────────────────────

    println!("Checking for events during preconfig...");
    sleep(Duration::from_millis(100)).await;
    let mut event_count = 0;
    while let Ok(Some(evt)) =
        tokio::time::timeout(Duration::from_millis(200), preconfig.events.recv()).await
    {
        println!("  EVENT: {} data={}", evt.event, evt.data);
        event_count += 1;
    }
    if event_count == 0 {
        println!("  (no events -- expected in preconfig)");
    }
    println!();

    // ── Schema introspection ─────────────────────────────────────────────

    let schema: Vec<qapi::SchemaInfo> = preconfig.execute(&qapi::QueryQmpSchema {}).await?;
    let cmd_count = schema
        .iter()
        .filter(|s| matches!(s.meta_type, qapi::SchemaMetaType::Command))
        .count();
    let evt_count = schema
        .iter()
        .filter(|s| matches!(s.meta_type, qapi::SchemaMetaType::Event))
        .count();
    let type_count = schema.len() - cmd_count - evt_count;
    println!(
        "query-qmp-schema: {} entries ({} commands, {} events, {} types)",
        schema.len(),
        cmd_count,
        evt_count,
        type_count
    );
    println!();

    // ── Exit preconfig and start the UEFI VM ─────────────────────────────
    //
    // x-exit-preconfig causes QEMU to initialize the machine and begin
    // executing the UEFI firmware. With no boot device, OVMF will enter
    // its built-in shell or display a "no bootable device" message.

    println!("Exiting preconfig (x-exit-preconfig)...");
    let mut vm = preconfig.start().await?;
    println!("VM started -- UEFI firmware is now executing");

    // ── Post-start verification ──────────────────────────────────────────

    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await?;
    println!("query-status: running={}", status.running);

    // After start, hotpluggable CPUs should be populated
    let cpus: Vec<qapi::HotpluggableCpu> = vm.execute(&qapi::QueryHotpluggableCpus {}).await?;
    println!("query-hotpluggable-cpus: {} cpus", cpus.len());
    for cpu in &cpus {
        println!(
            "  cpu type={} realized={}",
            cpu.r#type,
            cpu.qom_path.is_some()
        );
    }

    // Query PCI devices (available after machine init with q35)
    let pci: Vec<qapi::PciInfo> = vm.execute(&qapi::QueryPci {}).await?;
    println!("query-pci: {} PCI buses", pci.len());
    for bus in &pci {
        println!("  bus {} ({} devices)", bus.bus, bus.devices.len());
    }

    // Drain post-start events
    sleep(Duration::from_millis(200)).await;
    println!("\nPost-start events:");
    let mut post_events = 0;
    while let Ok(Some(evt)) =
        tokio::time::timeout(Duration::from_millis(200), vm.events.recv()).await
    {
        println!("  EVENT: {}", evt.event);
        post_events += 1;
    }
    if post_events == 0 {
        println!("  (no post-start events)");
    }

    // ── Shutdown ─────────────────────────────────────────────────────────

    println!("\nShutting down...");
    vm.shutdown().await;
    println!("Done.");

    Ok(())
}
