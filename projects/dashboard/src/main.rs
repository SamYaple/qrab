use anyhow::Context;
use qrab_dashboard::{
    AppState, auth::Oidc, bmc::Bmc, cluster::Kube, config::Config, kanidm::Kanidm,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "qrab_dashboard=info,kube=warn".into()),
        )
        .init();
    let _ = rustls::crypto::ring::default_provider().install_default();
    let config = Config::from_env()?;
    let kanidm = Kanidm::new(
        &config.kanidm_url,
        config.kanidm_token.clone(),
        config.ca.as_deref(),
        config.allow_loopback_http,
    )?;
    let oidc = Oidc::discover(
        config.issuer.clone(),
        config.client_id.clone(),
        config.client_secret.clone(),
        format!("{}/auth/callback", config.origin),
        config.ca.as_deref(),
    )
    .await
    .context("OIDC discovery")?;
    let client = kube::Client::try_default()
        .await
        .context("in-cluster Kubernetes client")?;
    let state = AppState {
        oidc,
        origin: config.origin.clone(),
        namespace: config.namespace.clone(),
        launcher_group: config.launcher_group.clone(),
        vm_users_group: config.vm_users_group.clone(),
        storage_node: config.storage_node.clone(),
        cluster: Arc::new(Kube::new(client, &config.namespace)),
        kanidm,
        bmc: Bmc::new(8080)?,
        sessions: Default::default(),
        novnc_dir: config.novnc_dir.clone(),
        consoles: Arc::new(tokio::sync::Semaphore::new(16)),
        console_subjects: Default::default(),
    };
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!(origin=%config.origin, namespace=%config.namespace, "qrab-dashboard started");
    axum::serve(listener, qrab_dashboard::router(state))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    if let Ok(mut terminate) =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
        return;
    }
    let _ = tokio::signal::ctrl_c().await;
}
