use anyhow::{ensure, Context, Result};
use manager::pod;
use std::{path::Path, sync::Arc, time::Duration};
use tokio::sync::{watch, Semaphore};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bmc=info,qrab_bmc=info".into()),
        )
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("qrab-bmc --config /etc/qrab/vm.json\nSee docs/bmc.md for required BMC_* configuration.");
        return Ok(());
    }
    ensure!(
        args.len() == 2 && args[0] == "--config",
        "usage: qrab-bmc --config /etc/qrab/vm.json"
    );
    let vm = pod::Config::read(Path::new(&args[1]))?;
    vm.validate()?;
    ensure!(vm.console, "BMC requires console=true in the VM config");
    let config = bmc::config::Config::from_env()?;
    ensure!(
        config.assets_dir.join("novnc/core/rfb.js").is_file(),
        "BMC noVNC assets are missing"
    );
    let oidc = bmc::auth::Oidc::discover(&config)
        .await
        .context("OIDC discovery")?;
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    let (control, receiver) = pod::control_channel_for(&vm, pod::Deployment::from_env());
    let app = bmc::router(bmc::AppState {
        oidc,
        config: config.clone(),
        vm: vm.clone(),
        control,
        sessions: Default::default(),
        consoles: Arc::new(Semaphore::new(4)),
        event_streams: Arc::new(Semaphore::new(16)),
    });
    let (stop, signal) = watch::channel(false);
    let mut web_signal = signal.clone();
    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = web_signal.wait_for(|v| *v).await;
    });
    let mut server = std::pin::pin!(std::future::IntoFuture::into_future(server));
    let mut vm_signal = signal;
    let runner = pod::run_controlled(
        vm,
        async move {
            let _ = vm_signal.wait_for(|v| *v).await;
        },
        receiver,
    );
    let mut runner = std::pin::pin!(runner);
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    tracing::info!(listen=%config.listen, origin=%config.origin, "BMC started");
    tokio::select! {
        result=&mut runner => { stop.send_replace(true); let _=tokio::time::timeout(Duration::from_secs(2), &mut server).await; result?; },
        result=&mut server => { stop.send_replace(true); runner.await?; result?; },
        _=async { tokio::select! {_=term.recv()=>{}, _=interrupt.recv()=>{}} } => {
            stop.send_replace(true);
            let outcome=runner.await?;
            tracing::info!(reason=%outcome.reason, method=%outcome.shutdown_method, "VM stopped");
            let _=tokio::time::timeout(Duration::from_secs(2), &mut server).await;
        }
    }
    Ok(())
}
