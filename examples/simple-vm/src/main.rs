use anyhow::Result;
use manager::qmp::qapi::QMPCommand;
use manager::vm::VM;

#[tokio::main]
async fn main() -> Result<()> {
    let id = "testid".to_string();
    let mut vm = VM::new(id).await;

    eprintln!("Starting VM");
    let _events = vm.start().await?;
    eprintln!("VM started");

    let reply = vm.qmp(QMPCommand::QueryStatus).await?;
    println!("query-status -- {reply}");

    let reply = vm.qmp(QMPCommand::SystemPowerdown).await?;
    println!("system_powerdown -- {reply}");

    let reply = vm.qmp(QMPCommand::QueryStatus).await?;
    println!("query-status -- {reply}");

    eprintln!("Stopping VM");
    vm.stop().await.unwrap();
    eprintln!("VM process is stopped and namespaces removed");
    eprintln!("Starting VM again");
    let _events = vm.start().await?;

    let reply = vm.qmp(QMPCommand::QueryStatus).await?;
    println!("query-status -- {reply}");

    eprintln!("Resetting VM");
    let mut events = vm.restart().await?;
    eprintln!("VM reset");

    let reply = vm.qmp(QMPCommand::QueryStatus).await?;
    println!("query-status -- {reply}");

    while let Some(e) = events.recv().await {
        println!("Event: {:#?}", e);
    }
    Ok(())
}
