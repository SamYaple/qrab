use anyhow::Result;
use manager::{Arch, MachineConfig, VmBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    // Spawn returns a PreconfigVm; call start() to begin execution.
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await?
        .start()
        .await?;

    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await?;
    println!("status: {status:?}");

    let version = qapi::qemu_version();
    println!(
        "connected to QEMU {}.{}.{}",
        version.0, version.1, version.2
    );

    vm.shutdown().await;
    Ok(())
}
