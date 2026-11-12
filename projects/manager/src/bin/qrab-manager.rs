use manager::pod::{self, Config};
use std::{path::Path, process::ExitCode};

const USAGE: &str = "qrab-manager <run|ready|live|status|check> --config /path/to/vm.json
qrab-manager agent <status|ping|freeze-status|freeze|thaw|update-status|update-trigger> --config /path/to/vm.json [ARG...]

agent talks to the guest agent through the running manager's control socket
(control_socket must be enabled). freeze takes optional mountpoints and
update-trigger optional hook arguments.";

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
    if args.first().map(String::as_str) == Some("agent") {
        return agent(&args[1..]).await;
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

async fn agent(args: &[String]) -> manager::Result<()> {
    use pod::socket::{AgentCommand, Client};
    if args.len() < 3 || args[1] != "--config" {
        return Err(manager::Error::Config(USAGE.into()));
    }
    let config = Config::read(Path::new(&args[2]))?;
    if !config.control_socket {
        return Err(manager::Error::Config(
            "agent commands need control_socket: true".into(),
        ));
    }
    let extra = args[3..].to_vec();
    let command = match (args[0].as_str(), extra.is_empty()) {
        ("status", true) => AgentCommand::Status,
        ("ping", true) => AgentCommand::Ping,
        ("freeze-status", true) => AgentCommand::FreezeStatus,
        ("freeze", _) => AgentCommand::Freeze {
            mountpoints: (!extra.is_empty()).then_some(extra),
        },
        ("thaw", true) => AgentCommand::Thaw,
        ("update-status", true) => AgentCommand::UpdateStatus,
        ("update-trigger", _) => AgentCommand::UpdateTrigger { args: extra },
        _ => return Err(manager::Error::Config(USAGE.into())),
    };
    let mut client = Client::connect(&pod::socket::socket_path(&config)).await?;
    let result = client.agent(command).await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
