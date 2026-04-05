use anyhow::Result;
use manager::{Arch, MachineConfig, VmBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    // Start QEMU in preconfig mode. No guest code runs until we
    // explicitly exit preconfig. This allows additional machine
    // configuration before boot.
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .preconfig()
        .spawn()
        .await?;

    println!("VM in preconfig mode");

    // Configure the VM here (e.g., add devices, set parameters).
    // Only preconfig-safe QMP commands are available at this point.

    // Exit preconfig to continue boot.
    vm.execute(&qapi::XExitPreconfig {}).await?;
    println!("exited preconfig");

    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await?;
    println!("running={}", status.running);

    vm.shutdown().await;
    Ok(())
}
