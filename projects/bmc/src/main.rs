use anyhow::{ensure, Context, Result};
use manager::pod;
use std::{path::Path, sync::Arc, time::Duration};
use tokio::sync::{watch, Semaphore};

const USAGE: &str = "usage: qrab-bmc --config /etc/qrab/vm.json | qrab-bmc --socket /run/qrab-control/manager/control.sock";

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
    if args.as_slice() == ["probe-health"] || args.as_slice() == ["probe-ready"] {
        let path = if args[0] == "probe-ready" {
            "/readyz"
        } else {
            "/healthz"
        };
        let address = std::env::var("BMC_LISTEN")
            .unwrap_or_else(|_| "127.0.0.1:8080".into())
            .parse()?;
        qrab_spiffe::http::probe_health(address, path).await?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}\n--config runs the VM in this process; --socket is the sidecar mode for a separate qrab-manager.\nSee docs/bmc.md for required BMC_* configuration.");
        return Ok(());
    }
    ensure!(
        args.len() == 2 && (args[0] == "--config" || args[0] == "--socket"),
        USAGE
    );
    qrab_spiffe::http::install_tls13_provider()?;
    let config = bmc::config::Config::from_env()?;
    qrab_spiffe::http::require_loopback(config.listen)?;
    let oidc = match &config.browser {
        None => None,
        Some(browser) => {
            ensure!(
                config.assets_dir.join("novnc/core/rfb.js").is_file(),
                "BMC noVNC assets are missing"
            );
            Some(
                bmc::auth::Oidc::discover(browser, config.ca_file.as_deref())
                    .await
                    .context("OIDC discovery")?,
            )
        }
    };
    let bearer = match &config.bearer {
        None => None,
        Some(bearer) => Some(Arc::new(
            oidc_bearer::Verifier::discover(
                &bearer.issuer,
                &bearer.audience,
                config.ca_file.as_deref(),
            )
            .await
            .context("bearer token issuer discovery")?,
        )),
    };
    let origin = config.browser.as_ref().map(|b| b.origin.clone());
    let workload_config = qrab_spiffe::http::Config::from_env("BMC", &[config.listen.port()])?;
    let tls_identity = if workload_config.is_some() || config.redfish_url.starts_with("https:") {
        let identity = qrab_spiffe::Identity::from_env()
            .await
            .context("BMC SPIFFE identity")?;
        if let Ok(expected) = std::env::var("QRAB_EXPECTED_SPIFFE_ID") {
            ensure!(
                identity.id()? == qrab_spiffe::id(&expected)?,
                "unexpected BMC SPIFFE identity"
            );
        }
        Some(identity)
    } else {
        None
    };
    let workload = match workload_config {
        Some(config) => Some(
            config
                .bind(tls_identity.as_ref().expect("configured identity"))
                .await?,
        ),
        None => None,
    };
    let redfish = if config.redfish_url.starts_with("https:") {
        let peer = std::env::var("BMC_REDFISH_SPIFFE_ID")
            .context("BMC_REDFISH_SPIFFE_ID is required for HTTPS Redfish")?;
        bmc::proxy::Upstream::mtls(
            &config.redfish_url,
            tls_identity.as_ref().expect("configured identity"),
            qrab_spiffe::id(&peer)?,
        )?
    } else {
        ensure!(
            workload.is_none(),
            "workload-enabled BMC requires Redfish mTLS"
        );
        bmc::proxy::Upstream::new(&config.redfish_url)?
    };
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    let state = |control| bmc::AppState {
        oidc,
        access: bmc::auth::Access::from_config(&config),
        bearer,
        config: config.clone(),
        control,
        sessions: Default::default(),
        consoles: Arc::new(Semaphore::new(4)),
        event_streams: Arc::new(Semaphore::new(16)),
        redfish,
    };
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let signal = async move {
        tokio::select! {_=term.recv()=>{}, _=interrupt.recv()=>{}}
    };
    if args[0] == "--socket" {
        let remote = pod::Remote::new(args[1].clone().into());
        let app = bmc::router(state(pod::Backend::Socket(remote.clone())));
        tracing::info!(listen=%config.listen, origin=?origin, bearer=config.bearer.is_some(), redfish=%config.redfish_url, socket=%args[1], "BMC sidecar started");
        return sidecar(listener, workload, app, remote, signal).await;
    }
    let vm = pod::Config::read(Path::new(&args[1]))?;
    vm.validate()?;
    ensure!(vm.console, "BMC requires console=true in the VM config");
    let (control, receiver) = pod::control_channel_for(&vm, pod::Deployment::from_env());
    let app = bmc::router(state(pod::Backend::Local(control)));
    let (stop, signal_rx) = watch::channel(false);
    let server = serve_http(listener, workload, app, signal_rx.clone());
    let mut server = std::pin::pin!(std::future::IntoFuture::into_future(server));
    let mut vm_signal = signal_rx;
    let runner = pod::run_controlled(
        vm,
        async move {
            let _ = vm_signal.wait_for(|v| *v).await;
        },
        receiver,
    );
    let mut runner = std::pin::pin!(runner);
    tracing::info!(listen=%config.listen, origin=?origin, bearer=config.bearer.is_some(), "BMC started");
    tokio::select! {
        result=&mut runner => { stop.send_replace(true); let _=tokio::time::timeout(Duration::from_secs(2), &mut server).await; result?; },
        result=&mut server => { stop.send_replace(true); runner.await?; result?; },
        _=signal => {
            stop.send_replace(true);
            let outcome=runner.await?;
            tracing::info!(reason=%outcome.reason, method=%outcome.shutdown_method, "VM stopped");
            let _=tokio::time::timeout(Duration::from_secs(2), &mut server).await;
        }
    }
    Ok(())
}

/// The VM belongs to the manager container. On SIGTERM (sent after the
/// manager exits) live streams get two seconds, then the process exits 0 so
/// the pod's clean-release check sees an orderly sidecar stop.
async fn sidecar(
    listener: tokio::net::TcpListener,
    workload: Option<qrab_spiffe::http::Listener>,
    app: axum::Router,
    remote: pod::Remote,
    signal: impl std::future::Future<Output = ()>,
) -> Result<()> {
    let (stop, stopped) = watch::channel(false);
    let server = serve_http(listener, workload, app, stopped);
    let mut server = std::pin::pin!(std::future::IntoFuture::into_future(server));
    tokio::select! {
        result=&mut server => result?,
        () = remote.mirror() => unreachable!("the event mirror never returns"),
        () = signal => {
            stop.send_replace(true);
            let _ = tokio::time::timeout(Duration::from_secs(2), &mut server).await;
            tracing::info!("BMC sidecar stopped");
        }
    }
    Ok(())
}

/// Both listeners share app authorization, admission budgets and shutdown.
async fn serve_http(
    listener: tokio::net::TcpListener,
    workload: Option<qrab_spiffe::http::Listener>,
    app: axum::Router,
    stopped: watch::Receiver<bool>,
) -> std::io::Result<()> {
    let shutdown = |mut signal: watch::Receiver<bool>| async move {
        let _ = signal.wait_for(|v| *v).await;
    };
    let ordinary_app = if workload.is_some() {
        app.clone()
            .layer(axum::middleware::from_fn(qrab_spiffe::http::health_only))
    } else {
        app.clone()
    };
    let ordinary =
        axum::serve(listener, ordinary_app).with_graceful_shutdown(shutdown(stopped.clone()));
    let protected = async move {
        match workload {
            Some(listener) => {
                axum::serve(listener, app)
                    .with_graceful_shutdown(shutdown(stopped))
                    .await
            }
            None => Ok(()),
        }
    };
    tokio::try_join!(std::future::IntoFuture::into_future(ordinary), protected).map(|_| ())
}
