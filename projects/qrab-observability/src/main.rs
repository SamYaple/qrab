use anyhow::{ensure, Context, Result};
use qrab_observability::{App, Inventory, HISTORY_ADDRESS, ID, SCRAPE_ADDRESS};
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.as_slice() == ["--probe-ready"] {
        let mut stream =
            TcpStream::connect_timeout(&SCRAPE_ADDRESS.parse()?, Duration::from_secs(1))?;
        stream.set_read_timeout(Some(Duration::from_secs(1)))?;
        stream.set_write_timeout(Some(Duration::from_secs(1)))?;
        stream
            .write_all(b"GET /readyz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
        let mut status = [0; 16];
        stream.read_exact(&mut status)?;
        ensure!(status.starts_with(b"HTTP/1.1 200 "), "telemetry not ready");
        return Ok(());
    }
    ensure!(
        args.as_slice() == ["run"],
        "usage: qrab-observability run|--probe-ready"
    );
    run()
}
#[tokio::main(flavor = "current_thread")]
async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(false)
        .init();
    qrab_spiffe::http::install_tls13_provider()?;
    let namespace = std::env::var("QRAB_NAMESPACE").unwrap_or_else(|_| "qrab".into());
    // Also validates the namespace as a safe SPIFFE/Kubernetes path component.
    qrab_spiffe::vm_service_id("yapplesauce.com", &namespace, "validation", "redfish")?;
    let identity = qrab_spiffe::Identity::from_env().await?;
    ensure!(
        identity.id()? == qrab_spiffe::id(ID)?,
        "unexpected telemetry SPIFFE identity"
    );
    let client = kube::Client::try_default().await?;
    let inventory = Inventory::default();
    inventory
        .refresh(client.clone(), &namespace)
        .await
        .context("initial VM discovery")?;
    let app = App::new(identity.clone(), inventory.clone())?;
    let policy = inventory.clone();
    let history = qrab_spiffe::http::Listener::bind_authorized(
        HISTORY_ADDRESS.parse()?,
        &identity,
        move |id: &qrab_spiffe::SpiffeId| policy.authorizes(id),
    )
    .await?;
    let scrape = tokio::net::TcpListener::bind(SCRAPE_ADDRESS).await?;
    let refresh = async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            if let Err(error) = inventory.refresh(client.clone(), &namespace).await {
                tracing::warn!(%error, "VM discovery refresh failed; old entries expire after 15 seconds");
            }
        }
    };
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let shutdown = |mut stopped: tokio::sync::watch::Receiver<bool>| async move {
        let _ = stopped.wait_for(|v| *v).await;
    };
    let servers = async {
        tokio::try_join!(
            axum::serve(scrape, app.clone().scrape_router())
                .with_graceful_shutdown(shutdown(stopped.clone())),
            axum::serve(
                history,
                app.history_router()
                    .into_make_service_with_connect_info::<qrab_spiffe::http::Peer>()
            )
            .with_graceful_shutdown(shutdown(stopped.clone()))
        )
        .map(|_| ())
    };
    tokio::pin!(servers);
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tracing::info!("qrab workload telemetry bridge started");
    tokio::select! {
        result = &mut servers => return result.context("telemetry listener stopped"),
        _ = refresh => unreachable!(),
        _ = term.recv() => {},
        _ = tokio::signal::ctrl_c() => {},
    }
    stop.send_replace(true);
    let _ = tokio::time::timeout(Duration::from_secs(5), &mut servers).await;
    Ok(())
}
