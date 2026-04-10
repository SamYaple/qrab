use anyhow::Result;
use manager::{Arch, MachineConfig, VmBuilder};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    let mut vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .no_shutdown()
        .spawn()
        .await?
        .start()
        .await?;

    println!("VM started (QEMU {}.{}.{})\n",
        qapi::qemu_version().0,
        qapi::qemu_version().1,
        qapi::qemu_version().2,
    );

    // Stop the VM -- triggers STOP event
    println!(">>> Sending 'stop'");
    vm.execute(&qapi::Stop {}).await?;
    drain_events(&mut vm, Duration::from_millis(500)).await;

    // Resume -- triggers RESUME event
    println!(">>> Sending 'cont'");
    vm.execute(&qapi::Cont {}).await?;
    drain_events(&mut vm, Duration::from_millis(500)).await;

    // Reset -- triggers RESET event
    println!(">>> Sending 'system_reset'");
    vm.execute(&qapi::SystemReset {}).await?;
    drain_events(&mut vm, Duration::from_millis(500)).await;

    // Powerdown -- triggers POWERDOWN event (may not fire with -machine none)
    println!(">>> Sending 'system_powerdown'");
    vm.execute(&qapi::SystemPowerdown {}).await?;
    drain_events(&mut vm, Duration::from_millis(500)).await;

    // Quit -- triggers SHUTDOWN event, then QEMU exits
    println!(">>> Sending 'quit'");
    let _ = vm.qmp().execute(&qapi::Quit {}).await;
    drain_events(&mut vm, Duration::from_secs(1)).await;

    println!("\nDone.");
    Ok(())
}

async fn drain_events(vm: &mut manager::Vm, timeout: Duration) {
    sleep(Duration::from_millis(50)).await; // let events arrive
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match tokio::time::timeout_at(deadline, vm.events().recv()).await {
            Ok(Some(evt)) => {
                print!("    EVENT: {:<12}", evt.event);
                if !evt.data.is_null() && evt.data != serde_json::json!({}) {
                    print!("  data={}", evt.data);
                }
                println!("  (ts={}.{})", evt.timestamp.seconds, evt.timestamp.microseconds);
            }
            _ => break,
        }
    }
}
