use manager::pod::{self, Config};
use std::{path::Path, process::ExitCode};

const USAGE: &str = "qrab-manager <run|ready|live|status|check> --config /path/to/vm.json
qrab-manager agent <status|ping|freeze-status|freeze|thaw|update-status|update-trigger> --config /path/to/vm.json [ARG...]
qrab-manager nic --listen 0.0.0.0:7700 --socket /run/nic/passt.sock [--passt /usr/bin/passt]
qrab-manager nic-ready --socket /run/nic/passt.sock

agent talks to the guest agent through the running manager's control socket
(control_socket must be enabled). freeze takes optional mountpoints and
update-trigger optional hook arguments.

nic runs passt for one guest NIC and relays one TCP connection at a time from
the VM's QEMU to it; it exits non-zero when passt exits. nic-ready is its
readiness probe.";

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
    match args.first().map(String::as_str) {
        Some("agent") => return agent(&args[1..]).await,
        Some("nic") => return nic(&args[1..]).await,
        Some("nic-ready") => {
            let options = flags(&args[1..], &["--socket"])?;
            pod::nic::probe(Path::new(&options["--socket"]))?;
            return Ok(());
        }
        _ => {}
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
    use pod::socket::Client;
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
    let known = matches!(
        (args[0].as_str(), extra.is_empty()),
        (
            "status" | "ping" | "freeze-status" | "thaw" | "update-status",
            true
        ) | ("freeze" | "update-trigger", _)
    );
    if !known {
        return Err(manager::Error::Config(USAGE.into()));
    }
    let mut client = Client::connect(&pod::socket::socket_path(&config)).await?;
    // Output is the pretty JSON of the typed result (`null` for ping).
    let result = match args[0].as_str() {
        "status" => serde_json::to_value(client.agent_status().await?)?,
        "ping" => {
            client.agent_ping().await?;
            serde_json::Value::Null
        }
        "freeze-status" => serde_json::to_value(client.agent_freeze_status().await?)?,
        "freeze" => client
            .agent_freeze((!extra.is_empty()).then_some(extra))
            .await?
            .into(),
        "thaw" => client.agent_thaw().await?.into(),
        "update-status" => serde_json::to_value(client.agent_update_status().await?)?,
        "update-trigger" => serde_json::to_value(client.agent_update_trigger(extra).await?)?,
        _ => unreachable!("checked above"),
    };
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

/// `--flag value` pairs; each flag at most once and only from `allowed`.
fn flags(
    args: &[String],
    allowed: &[&str],
) -> manager::Result<std::collections::BTreeMap<String, String>> {
    let usage = || manager::Error::Config(USAGE.into());
    let mut values = std::collections::BTreeMap::new();
    for pair in args.chunks(2) {
        let [flag, value] = pair else {
            return Err(usage());
        };
        if !allowed.contains(&flag.as_str()) || values.insert(flag.clone(), value.clone()).is_some()
        {
            return Err(usage());
        }
    }
    Ok(values)
}

async fn nic(args: &[String]) -> manager::Result<()> {
    let mut values = flags(args, &["--listen", "--socket", "--passt"])?;
    let (Some(listen), Some(socket)) = (values.remove("--listen"), values.remove("--socket"))
    else {
        return Err(manager::Error::Config(USAGE.into()));
    };
    let options = pod::nic::Options {
        listen: listen
            .parse()
            .map_err(|_| manager::Error::Config("--listen must be IP:port".into()))?,
        socket: socket.into(),
        passt: values
            .remove("--passt")
            .unwrap_or_else(|| pod::nic::PASST.into())
            .into(),
    };
    if !options.socket.is_absolute() || !options.passt.is_absolute() {
        return Err(manager::Error::Config(
            "--socket and --passt must be absolute paths".into(),
        ));
    }
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    pod::nic::serve(&options, async {
        tokio::select! {_=term.recv()=>{},_=interrupt.recv()=>{}}
    })
    .await
}
