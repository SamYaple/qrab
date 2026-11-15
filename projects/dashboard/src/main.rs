use anyhow::Context;
use qrab_dashboard::{
    AppState, auth::Oidc, bmc::Bmc, cluster::Kube, config::Config, kanidm::Kanidm, redfish::Redfish,
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
    qrab_spiffe::http::install_tls13_provider()?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        anyhow::ensure!(
            args.len() == 5 && args[0] == "workload-probe",
            "usage: qrab-dashboard [workload-probe NAMESPACE VM-UID IP bmc|redfish]"
        );
        return workload_probe(&args[1], &args[2], &args[3], &args[4]).await;
    }
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
    let identity = qrab_spiffe::Identity::from_env()
        .await
        .context("dashboard SPIFFE identity")?;
    anyhow::ensure!(
        identity.id()?.to_string()
            == format!(
                "spiffe://yapplesauce.com/kubernetes/{}/qrab-dashboard",
                config.namespace
            ),
        "unexpected dashboard SPIFFE identity"
    );
    let state = AppState {
        oidc,
        origin: config.origin.clone(),
        namespace: config.namespace.clone(),
        launcher_group: config.launcher_group.clone(),
        vm_users_group: config.vm_users_group.clone(),
        storage_node: config.storage_node.clone(),
        cluster: Arc::new(Kube::new(client, &config.namespace)),
        kanidm,
        bmc: Bmc::new(identity.clone(), 8443),
        redfish: Redfish::new(identity, 8444),
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

/// Operator diagnostic: fixed unauthenticated GET, no user token or key output.
/// A 401 proves workload TLS admission while preserving application auth.
async fn workload_probe(
    namespace: &str,
    uid: &str,
    ip: &str,
    component: &str,
) -> anyhow::Result<()> {
    let ip: std::net::IpAddr = ip.parse()?;
    let (port, path) = match component {
        "bmc" => (8443, "/api/session"),
        "redfish" => (8444, "/redfish/v1/Systems/1"),
        _ => anyhow::bail!("probe component must be bmc or redfish"),
    };
    let identity = qrab_spiffe::Identity::from_env().await?;
    let own = identity.id()?;
    anyhow::ensure!(
        own.to_string()
            == format!("spiffe://yapplesauce.com/kubernetes/{namespace}/qrab-dashboard"),
        "unexpected probe workload identity"
    );
    let peer = qrab_spiffe::vm_service_id(own.trust_domain().as_ref(), namespace, uid, component)?;
    let client = qrab_spiffe::http::client(&identity, peer.clone())?;
    let response = client
        .get(format!(
            "https://{}{path}",
            std::net::SocketAddr::new(ip, port)
        ))
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await?;
    anyhow::ensure!(
        response.status() == 401,
        "expected application 401 after TLS admission, got {}",
        response.status()
    );
    println!(
        "{}",
        serde_json::json!({"transport":"spiffe-mtls","expected_peer":peer.to_string(),"application_status":401})
    );
    Ok(())
}
