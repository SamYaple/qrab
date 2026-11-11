use anyhow::{ensure, Context as _, Result};
use axum::{routing::get, Router};
use futures::StreamExt;
use k8s_openapi::api::core::v1::{ConfigMap, Pod};
use kube::{
    runtime::{watcher, Controller},
    Api, Client,
};
use qrab_operator::{
    api::VirtualMachine,
    controller::{self, Context},
    resources::{pinned_image, Settings},
};
use std::sync::Arc;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.as_slice() == ["crd"] {
        println!(
            "{}",
            serde_json::to_string_pretty(&qrab_operator::api::crd())?
        );
        return Ok(());
    }
    ensure!(args.as_slice() == ["run"], "usage: qrab-operator <run|crd>");
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "qrab_operator=info,kube=warn".into()),
        )
        .init();
    let vm_image = std::env::var("QRAB_VM_IMAGE").context("QRAB_VM_IMAGE is required")?;
    ensure!(
        pinned_image(&vm_image),
        "QRAB_VM_IMAGE must pin the qrab/manager repository by digest"
    );
    let issuer = std::env::var("QRAB_OIDC_ISSUER")
        .unwrap_or_else(|_| "https://idm.yaple.net/oauth2/openid/qrab-bmc".into());
    ensure!(
        url::Url::parse(&issuer)?.scheme() == "https",
        "OIDC issuer must use HTTPS"
    );
    let idp_ip = std::env::var("QRAB_IDP_IP").ok();
    if let Some(ip) = &idp_ip {
        ip.parse::<std::net::IpAddr>()
            .context("QRAB_IDP_IP must be an IP address")?;
    }
    let settings = Settings {
        namespace: std::env::var("QRAB_NAMESPACE").unwrap_or_else(|_| "qrab".into()),
        vm_image,
        oidc_issuer: issuer,
        oidc_client_id: std::env::var("QRAB_OIDC_CLIENT_ID").unwrap_or_else(|_| "qrab-bmc".into()),
        idp_ip,
    };
    let client = Client::try_default().await?;
    let vms: Api<VirtualMachine> = Api::namespaced(client.clone(), &settings.namespace);
    vms.list(&kube::api::ListParams::default().limit(1))
        .await
        .context("list VirtualMachines (install CRD and RBAC first)")?;
    let pods: Api<Pod> = Api::namespaced(client.clone(), &settings.namespace);
    let configs: Api<ConfigMap> = Api::namespaced(client.clone(), &settings.namespace);
    let ctx = Arc::new(Context { client, settings });
    let controller = Controller::new(vms, watcher::Config::default())
        .owns(
            pods,
            watcher::Config::default().labels("app.kubernetes.io/name=qrab"),
        )
        .owns(
            configs,
            watcher::Config::default().labels("app.kubernetes.io/name=qrab"),
        )
        .run(controller::reconcile, controller::error_policy, ctx.clone())
        .for_each(|result| async move {
            if let Err(error) = result {
                tracing::warn!(%error,"controller event failed");
            }
        });
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081").await?;
    let server = axum::serve(
        listener,
        Router::new().route("/healthz", get(|| async { "ok" })),
    );
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let orphan_context = ctx.clone();
    let orphan_cleanup = async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            tick.tick().await;
            if let Err(error) = controller::sweep_orphans(orphan_context.clone()).await {
                tracing::warn!(%error,"iSCSI orphan drain inspection failed");
            }
        }
    };
    tracing::info!(namespace=%ctx.settings.namespace,"qrab operator started");
    tokio::select! {
        _=controller=>anyhow::bail!("controller stream ended"),
        result=server=>result?,
        _=orphan_cleanup=>anyhow::bail!("orphan cleanup loop ended"),
        _=term.recv()=>{},
        _=tokio::signal::ctrl_c()=>{},
    }
    Ok(())
}
