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
    if args.as_slice() == ["probe-health"] || args.as_slice() == ["probe-ready"] {
        let path = if args[0] == "probe-ready" {
            "/readyz"
        } else {
            "/healthz"
        };
        let address = std::env::var("REDFISH_LISTEN")
            .unwrap_or_else(|_| "127.0.0.1:8081".into())
            .parse()?;
        qrab_spiffe::http::probe_health(address, path).await?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}\nSee docs/redfish.md for required REDFISH_* configuration.");
        return Ok(());
    }
    ensure!(args.len() == 2 && args[0] == "--socket", USAGE);
    qrab_spiffe::http::install_tls13_provider()?;
    let config = qrab_redfish::config::Config::from_env()?;
    qrab_spiffe::http::require_loopback(config.listen)?;
    let workload = qrab_redfish::workload::Config::from_env(config.listen, config.metrics_listen)?;
    let identity = match &workload {
        Some(c) => Some(c.identity().await?),
        None => None,
    };
    let metrics_config = qrab_spiffe::http::Config::parse(
        Some(&config.metrics_listen.to_string()),
        std::env::var("REDFISH_METRICS_PEERS").ok().as_deref(),
        &[config.listen.port()],
    );
    let metrics_tls = match (&identity, metrics_config) {
        (Some(id), Ok(Some(c))) => Some(c.bind(id).await?),
        (_, _)
            if config.metrics_listen.ip().is_loopback()
                && std::env::var_os("REDFISH_METRICS_PEERS").is_none() =>
        {
            None
        }
        _ => anyhow::bail!(
            "remote metrics requires workload identity and exact REDFISH_METRICS_PEERS"
        ),
    };
    let history = match (&identity, config.pod.is_some()) {
        (_, false) => Ok(qrab_redfish::HistoryStore::default()),
        (Some(id), true) => qrab_redfish::HistoryStore::mtls(
            config.prometheus_url.clone(),
            config.pod.clone(),
            id,
            qrab_spiffe::id(
                &std::env::var("REDFISH_PROMETHEUS_SPIFFE_ID")
                    .context("REDFISH_PROMETHEUS_SPIFFE_ID is required with workload TLS")?,
            )?,
        ),
        (None, true) => {
            qrab_redfish::HistoryStore::new(config.prometheus_url.clone(), config.pod.clone())
        }
    }
    .map_err(anyhow::Error::msg)?;
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
        history,
    };
    let sampler = qrab_redfish::spawn_sampler(state.clone());
    let subscriptions = qrab_redfish::spawn_subscriptions(state.clone());
    let metrics = qrab_redfish::metrics_router(state.clone());
    let app = qrab_redfish::router(state);
    let workload = match (workload, identity.as_ref()) {
        (Some(config), Some(id)) => Some(config.bind_identity(id).await?),
        _ => None,
    };
    let workload_app = app.clone();
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    let metrics_listener = if metrics_tls.is_none() {
        Some(tokio::net::TcpListener::bind(config.metrics_listen).await?)
    } else {
        None
    };
    let (stop, stopped) = watch::channel(false);
    let shutdown = |mut stopped: watch::Receiver<bool>| async move {
        let _ = stopped.wait_for(|v| *v).await;
    };
    let ordinary_app = if workload.is_some() {
        app.layer(axum::middleware::from_fn(qrab_spiffe::http::health_only))
    } else {
        app
    };
    let api = axum::serve(listener, ordinary_app).with_graceful_shutdown(shutdown(stopped.clone()));
    let metrics_stopped = stopped.clone();
    let exporter = async move {
        match metrics_tls {
            Some(tls) => {
                axum::serve(
                    tls,
                    metrics
                        .layer(axum::middleware::from_fn(
                            qrab_spiffe::http::require_valid_peer,
                        ))
                        .into_make_service_with_connect_info::<qrab_spiffe::http::Peer>(),
                )
                .with_graceful_shutdown(shutdown(metrics_stopped))
                .await
            }
            None => {
                axum::serve(metrics_listener.expect("local metrics listener"), metrics)
                    .with_graceful_shutdown(shutdown(metrics_stopped))
                    .await
            }
        }
    };
    let workloads = async move {
        match workload {
            Some(listener) => {
                axum::serve(listener, workload_app)
                    .with_graceful_shutdown(shutdown(stopped))
                    .await
            }
            None => Ok(()),
        }
    };
    // Any listener failing ends the sidecar; all drain on SIGTERM.
    let server = async move {
        tokio::try_join!(
            std::future::IntoFuture::into_future(api),
            std::future::IntoFuture::into_future(exporter),
            workloads
        )
        .map(|_| ())
    };
    let mut server = std::pin::pin!(server);
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    tracing::info!(listen=%config.listen, metrics=%config.metrics_listen, socket=%args[1], issuers=config.sources.len(), history=config.pod.is_some(), "Redfish sidecar started");
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
