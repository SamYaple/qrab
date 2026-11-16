use anyhow::{anyhow, Context, Result};
use qrab_tpm_issuer::{Config, Issuer, USAGE};
use std::sync::Arc;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("qrab-tpm-issuer: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let config = Config::parse(args, |name| std::env::var(name).ok()).map_err(|e| anyhow!(e))?;
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    qrab_spiffe::http::install_tls13_provider()?;
    let issuer = Arc::new(
        Issuer::load(&config.ca_dir, &config.state_dir, config.allow_rekey)
            .context("loading CAs and state")?,
    );
    let identity = qrab_spiffe::Identity::from_env().await?;
    let listener = qrab_tpm_issuer::bind(config.listen, &identity).await?;
    tracing::info!(
        id = %identity.id()?,
        listen = %config.listen,
        allow_rekey = config.allow_rekey,
        "qrab-tpm-issuer started"
    );
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    axum::serve(
        listener,
        qrab_tpm_issuer::router(issuer)
            .into_make_service_with_connect_info::<qrab_spiffe::http::Peer>(),
    )
    .with_graceful_shutdown(async move {
        term.recv().await;
    })
    .await
    .context("listener stopped")
}
