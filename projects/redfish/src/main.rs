use anyhow::{ensure, Context, Result};
use manager::pod;
use std::{sync::Arc, time::Duration};
use tokio::sync::{watch, Semaphore};

const USAGE: &str = "usage: qrab-redfish --socket /run/qrab-control/manager/control.sock";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "qrab_redfish=info".into()),
        )
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}\nSee docs/redfish.md for required REDFISH_* configuration.");
        return Ok(());
    }
    ensure!(args.len() == 2 && args[0] == "--socket", USAGE);
    let config = qrab_redfish::config::Config::from_env()?;
    let auth = qrab_redfish::auth::Authenticator::discover(&config)
        .await
        .context("OIDC discovery")?;
    let remote = pod::Remote::new(args[1].clone().into());
    let state = qrab_redfish::AppState {
        auth: Arc::new(auth),
        control: pod::Backend::Socket(remote.clone()),
        streams: Arc::new(Semaphore::new(8)),
        requests: Arc::new(Semaphore::new(32)),
        tasks: Default::default(),
        telemetry: qrab_redfish::TelemetryStore::from_env().map_err(anyhow::Error::msg)?,
        triggers: Default::default(),
        subscriptions: qrab_redfish::SubscriptionStore::from_env().map_err(anyhow::Error::msg)?,
        exporter: Default::default(),
    };
    let sampler = qrab_redfish::spawn_sampler(state.clone());
    let subscriptions = qrab_redfish::spawn_subscriptions(state.clone());
    let metrics = qrab_redfish::metrics_router(state.clone());
    let app = qrab_redfish::router(state);
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    let metrics_listener = tokio::net::TcpListener::bind(config.metrics_listen).await?;
    let (stop, stopped) = watch::channel(false);
    let shutdown = |mut stopped: watch::Receiver<bool>| async move {
        let _ = stopped.wait_for(|v| *v).await;
    };
    let api = axum::serve(listener, app).with_graceful_shutdown(shutdown(stopped.clone()));
    let exporter = axum::serve(metrics_listener, metrics).with_graceful_shutdown(shutdown(stopped));
    // Either listener failing ends the sidecar; both drain on SIGTERM.
    let server = async move {
        tokio::try_join!(
            std::future::IntoFuture::into_future(api),
            std::future::IntoFuture::into_future(exporter)
        )
        .map(|_| ())
    };
    let mut server = std::pin::pin!(server);
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    tracing::info!(listen=%config.listen, metrics=%config.metrics_listen, socket=%args[1], issuers=config.sources.len(), "Redfish sidecar started");
    tokio::select! {
        result=&mut server => result?,
        () = remote.mirror() => unreachable!("the event mirror never returns"),
        _ = async { tokio::select! {_=term.recv()=>{}, _=interrupt.recv()=>{}} } => {
            // Exit 0 promptly so the pod's clean-release check sees an orderly stop.
            stop.send_replace(true);
            let _ = tokio::time::timeout(Duration::from_secs(2), &mut server).await;
            tracing::info!("Redfish sidecar stopped");
        }
    }
    sampler.abort();
    subscriptions.abort();
    Ok(())
}
