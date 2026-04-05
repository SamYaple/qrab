use anyhow::Result;
use manager::vm::VM;

#[tokio::main]
async fn main() -> Result<()> {
    let id = "testid".to_string();
    let mut vm = VM::new(id).await;

    eprintln!("Starting VM");
    let mut events = vm.start().await?;
    eprintln!("VM started");

    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await?;
    println!("query-status: running={}", status.running);

    vm.execute(&qapi::SystemPowerdown {}).await?;
    println!("system_powerdown sent");

    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await?;
    println!("query-status: running={}", status.running);

    eprintln!("Stopping VM");
    vm.stop().await?;
    eprintln!("VM stopped");

    eprintln!("Starting VM again");
    let _events = vm.start().await?;

    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await?;
    println!("query-status: running={}", status.running);

    eprintln!("Restarting VM");
    events = vm.restart().await?;
    eprintln!("VM restarted");

    let status: qapi::StatusInfo = vm.execute(&qapi::QueryStatus {}).await?;
    println!("query-status: running={}", status.running);

    while let Some(e) = events.recv().await {
        println!("Event: {} at {}.{}", e.event, e.timestamp.seconds, e.timestamp.microseconds);
    }
    Ok(())
}
