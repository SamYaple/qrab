use manager::pod::{self, Config};
use std::{path::Path, process::ExitCode};

const USAGE: &str = "qrab-manager <run|ready|live|status|check> --config /path/to/vm.json";

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match execute().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

async fn execute() -> manager::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    if args.len() != 3 || args[1] != "--config" {
        return Err(manager::Error::Config(USAGE.into()));
    }
    let config = Config::read(Path::new(&args[2]))?;
    match args[0].as_str() {
        "check" => config.validate(),
        "ready" => pod::probe(&config, true),
        "live" => pod::probe(&config, false),
        "status" => {
            println!("{}", serde_json::to_string_pretty(&pod::status(&config)?)?);
            Ok(())
        }
        "run" => {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
            let mut interrupt =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
            let (_control, receiver) =
                pod::control_channel_for(&config, pod::Deployment::from_env());
            let outcome = pod::run_controlled(
                config,
                async {
                    tokio::select! {_=term.recv()=>{},_=interrupt.recv()=>{}}
                },
                receiver,
            )
            .await?;
            eprintln!(
                "{}",
                serde_json::json!({"event":"manager_exit","outcome":outcome})
            );
            Ok(())
        }
        _ => Err(manager::Error::Config(USAGE.into())),
    }
}
