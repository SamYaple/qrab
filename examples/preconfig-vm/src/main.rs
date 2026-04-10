use anyhow::Result;
use manager::{Arch, MachineConfig, VmBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    // Spawn QEMU in preconfig mode. No guest code runs until we
    // explicitly call start(). This allows QMP-based configuration
    // before the machine is initialized.
    //
    // Requires QEMU >= 3.0.0 (when --preconfig was introduced).
    let preconfig = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await?;

    let version = qapi::qemu_version();
    println!(
        "connected to QEMU {}.{}.{} (preconfig mode)",
        version.0, version.1, version.2
    );

    // In preconfig state, only PreconfigSafe commands are allowed.
    // The compiler enforces this -- non-preconfig commands won't compile.
    let status: qapi::StatusInfo = preconfig.execute(&qapi::QueryStatus {}).await?;
    println!("preconfig: running={}", status.running);

    // Configure the VM here via preconfig-safe QMP commands:
    //   preconfig.execute(&qapi::ObjectAdd { .. }).await?;
    //   preconfig.execute(&qapi::BlockdevAdd { .. }).await?;
    //   preconfig.execute(&qapi::SetNumaNode { .. }).await?;

    // Exit preconfig -- QEMU initializes the machine and starts the guest.
    let vm = preconfig.start().await?;
    println!("started VM");

    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await?;
    println!("running={}", status.running);

    vm.shutdown().await;
    println!("shut down");
    Ok(())
}
