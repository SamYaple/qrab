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

    let preconfig = VmBuilder::new(
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

    let ver: qapi::VersionInfo = preconfig.execute(&qapi::commands::QueryVersion {}).await?;
    println!("query-version: {ver}");

    let status: qapi::StatusInfo = preconfig.execute(&qapi::commands::QueryStatus {}).await?;
    println!("query-status: {status}");

    let name: qapi::NameInfo = preconfig.execute(&qapi::commands::QueryName {}).await?;
    println!("query-name: {name}");

    let uuid: qapi::UuidInfo = preconfig.execute(&qapi::commands::QueryUuid {}).await?;
    println!("query-uuid: {uuid}");

    let cmds: Vec<qapi::CommandInfo> = preconfig.execute(&qapi::commands::QueryCommands {}).await?;
    println!("query-commands: {} commands available", cmds.len());
    for cmd in cmds.iter() {
        println!("  {cmd}");
    }

    let chardevs: Vec<qapi::ChardevInfo> = preconfig.execute(&qapi::commands::QueryChardev {}).await?;
    println!("query-chardev: {} chardevs", chardevs.len());
    for chardev in &chardevs {
        println!("  {chardev}");
    }

    let iothreads: Vec<qapi::IoThreadInfo> = preconfig.execute(&qapi::commands::QueryIothreads {}).await?;
    println!("query-iothreads: {} iothreads", iothreads.len());
    for iothread in &iothreads {
        println!("  {iothread}");
    }

    let memdevs: Vec<qapi::Memdev> = preconfig.execute(&qapi::commands::QueryMemdev {}).await?;
    println!("query-memdev: {} memdevs", memdevs.len());
    for memdev in &memdevs {
        println!("  {memdev}");
    }

    let cpus: Vec<qapi::HotpluggableCpu> =
        preconfig.execute(&qapi::commands::QueryHotpluggableCpus {}).await?;
    println!("query-hotpluggable-cpus: {} cpus", cpus.len());
    for cpu in &cpus {
        println!("  {cpu}");
    }

    // ── QOM introspection (preconfig-safe) ───────────────────────────────

    let types: Vec<qapi::ObjectTypeInfo> = preconfig
        .execute(&qapi::commands::QomListTypes {
            r#abstract: None,
            implements: None,
        })
        .await?;
    println!("qom-list-types: {} types available", types.len());
    for r#type in types.iter() {
        println!("  {type}");
    }

    let root_props: Vec<qapi::ObjectPropertyInfo> = preconfig
        .execute(&qapi::commands::QomList {
            path: "/".to_string(),
        })
        .await?;
    println!("qom-list /: {} properties", root_props.len());
    for prop in root_props.iter() {
        println!("  {prop}");
    }

    // List properties of the q35 machine type
    let machine_props: Vec<qapi::ObjectPropertyInfo> = preconfig
        .execute(&qapi::commands::QomListProperties {
            typename: "machine".to_string(),
        })
        .await?;
    println!(
        "qom-list-properties machine: {} properties",
        machine_props.len()
    );
    for machine_prop in machine_props.iter() {
        println!("  {machine_prop}");
    }

    // ── Block layer queries (preconfig-safe since 7.1.0) ─────────────────

    let blocks: Vec<qapi::BlockInfo> = preconfig.execute(&qapi::commands::QueryBlock {}).await?;
    println!("query-block: {} devices", blocks.len());
    for block in &blocks {
        println!("  {block}");
    }

    let block_jobs: Vec<qapi::BlockJobInfo> = preconfig.execute(&qapi::commands::QueryBlockJobs {}).await?;
    println!("query-block-jobs: {} jobs", block_jobs.len());
    for job in &block_jobs {
        println!("  {job}");
    }

    let blockstats: Vec<qapi::BlockStats> = preconfig
        .execute(&qapi::commands::QueryBlockstats { query_nodes: None })
        .await?;
    println!("query-blockstats: {} entries", blockstats.len());
    for stat in &blockstats {
        println!("  {stat}");
    }

    // query-named-block-nodes: should show the OVMF pflash node
    let block_nodes: Vec<qapi::BlockDeviceInfo> = preconfig
        .execute(&qapi::commands::QueryNamedBlockNodes { flat: None })
        .await?;
    println!("query-named-block-nodes: {} nodes", block_nodes.len());
    for node in &block_nodes {
        println!("  {node}");
    }

    // ── Exit preconfig and start the UEFI VM ─────────────────────────────
    //
    // x-exit-preconfig causes QEMU to initialize the machine and begin
    // executing the UEFI firmware. With no boot device, OVMF will enter
    // its built-in shell or display a "no bootable device" message.

    println!("Exiting preconfig (x-exit-preconfig)...");
    let mut vm = preconfig.start().await?;
    println!("VM started -- UEFI firmware is now executing");

    // ── Post-start verification ──────────────────────────────────────────

    let status: qapi::StatusInfo = vm.execute(&qapi::commands::QueryStatus {}).await?;
    println!("query-status: {status}");

    // After start, hotpluggable CPUs should be populated
    let cpus: Vec<qapi::HotpluggableCpu> = vm.execute(&qapi::commands::QueryHotpluggableCpus {}).await?;
    println!("query-hotpluggable-cpus: {} cpus", cpus.len());
    for cpu in &cpus {
        println!("  {cpu}");
    }

    // Query PCI devices (available after machine init with q35)
    let pci: Vec<qapi::PciInfo> = vm.execute(&qapi::commands::QueryPci {}).await?;
    println!("query-pci: {} PCI buses", pci.len());
    for bus in &pci {
        println!("  {bus}");
    }

    // Drain post-start events
    sleep(Duration::from_millis(200)).await;
    println!("\nPost-start events:");
    let mut post_events = 0;
    while let Ok(Some(evt)) =
        tokio::time::timeout(Duration::from_millis(200), vm.events.recv()).await
    {
        println!("  {evt:?}");
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
