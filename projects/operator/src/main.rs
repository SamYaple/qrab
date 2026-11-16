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
    resources::{pinned_image, BearerSettings, Settings},
};
use std::sync::Arc;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.as_slice() == ["probe-health"] {
        qrab_spiffe::http::probe_health("127.0.0.1:8081".parse()?, "/healthz").await?;
        return Ok(());
    }
    if args.as_slice() == ["crd"] {
        println!(
            "{}",
            serde_json::to_string_pretty(&qrab_operator::api::crd())?
        );
        return Ok(());
    }
    if args.as_slice() == ["migration-crd"] {
        println!(
            "{}",
            serde_json::to_string_pretty(&qrab_operator::api::migration_crd())?
        );
        return Ok(());
    }
    if args.as_slice() == ["image-crd"] {
        println!(
            "{}",
            serde_json::to_string_pretty(&qrab_operator::image::crd())?
        );
        return Ok(());
    }
    if args.as_slice() == ["dashboard-update-policy"] {
        println!(
            "{}",
            serde_json::to_string_pretty(&qrab_operator::api::dashboard_update_policy())?
        );
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("image-job") {
        return qrab_operator::image_job::main(&args[1..]).await;
    }
    if args.as_slice() == ["image-importer"] {
        return image_importer().await;
    }
    ensure!(
        args.as_slice() == ["run"],
        "usage: qrab-operator <run|image-importer|image-job|crd|migration-crd|image-crd|dashboard-update-policy>"
    );
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
    let redfish_issuer = std::env::var("QRAB_REDFISH_OIDC_ISSUER")
        .unwrap_or_else(|_| "https://idm.yaple.net/oauth2/openid/qrab-redfish".into());
    ensure!(
        url::Url::parse(&redfish_issuer)?.scheme() == "https",
        "Redfish OIDC issuer must use HTTPS"
    );
    let bmc_bearer = match (
        std::env::var("QRAB_BMC_BEARER_ISSUER").ok(),
        std::env::var("QRAB_BMC_BEARER_AUDIENCE").ok(),
    ) {
        (None, None) => None,
        (Some(issuer), Some(audience)) => {
            ensure!(
                url::Url::parse(&issuer)?.scheme() == "https",
                "QRAB_BMC_BEARER_ISSUER must use HTTPS"
            );
            ensure!(
                !audience.is_empty(),
                "QRAB_BMC_BEARER_AUDIENCE must not be empty"
            );
            Some(BearerSettings { issuer, audience })
        }
        _ => anyhow::bail!(
            "QRAB_BMC_BEARER_ISSUER and QRAB_BMC_BEARER_AUDIENCE must be set together"
        ),
    };
    let idp_ip = std::env::var("QRAB_IDP_IP").ok();
    if let Some(ip) = &idp_ip {
        ip.parse::<std::net::IpAddr>()
            .context("QRAB_IDP_IP must be an IP address")?;
    }
    // Opt-in stable NIC addresses: needs Cilium multi-pool IPAM and frr-k8s
    // base configurations labelled qrab.yapplesauce.com/nic-route-base.
    let nic_addresses = match std::env::var("QRAB_NIC_ADDRESS_RANGE") {
        Err(_) => None,
        Ok(range) => Some(qrab_operator::address::Addressing {
            range: range
                .parse()
                .map_err(|e| anyhow::anyhow!("QRAB_NIC_ADDRESS_RANGE: {e}"))?,
            frr_namespace: std::env::var("QRAB_FRR_NAMESPACE")
                .unwrap_or_else(|_| qrab_operator::address::DEFAULT_FRR_NAMESPACE.into()),
        }),
    };
    let guest_devid_ca_cn = std::env::var("QRAB_GUEST_DEVID_CA_CN")
        .unwrap_or_else(|_| qrab_operator::guest_identity::DEFAULT_DEVID_CA_CN.into());
    ensure!(
        qrab_operator::guest_identity::valid_devid_ca_cn(&guest_devid_ca_cn),
        "QRAB_GUEST_DEVID_CA_CN must be a CN of 1..64 letters, digits, spaces, '.', '_' or '-'"
    );
    let settings = Settings {
        namespace: std::env::var("QRAB_NAMESPACE").unwrap_or_else(|_| "qrab".into()),
        vm_image,
        oidc_issuer: issuer,
        oidc_client_id: std::env::var("QRAB_OIDC_CLIENT_ID").unwrap_or_else(|_| "qrab-bmc".into()),
        idp_ip,
        redfish_oidc_issuer: redfish_issuer,
        redfish_oidc_audience: std::env::var("QRAB_REDFISH_OIDC_AUDIENCE")
            .unwrap_or_else(|_| "qrab-redfish".into()),
        bmc_bearer,
        nic_addresses,
        guest_devid_ca_cn,
    };
    qrab_spiffe::http::install_tls13_provider()?;
    let identity = qrab_spiffe::Identity::from_env()
        .await
        .context("operator workload identity")?;
    ensure!(
        identity.id()?
            == qrab_spiffe::id(&format!(
                "spiffe://{}/kubernetes/{}/qrab-operator",
                identity.id()?.trust_domain(),
                settings.namespace
            ))?,
        "unexpected operator SPIFFE identity"
    );
    let metrics = Some(qrab_operator::power::Client::new(identity));
    let client = Client::try_default().await?;
    let vms: Api<VirtualMachine> = Api::namespaced(client.clone(), &settings.namespace);
    vms.list(&kube::api::ListParams::default().limit(1))
        .await
        .context("list VirtualMachines (install CRD and RBAC first)")?;
    let pods: Api<Pod> = Api::namespaced(client.clone(), &settings.namespace);
    let configs: Api<ConfigMap> = Api::namespaced(client.clone(), &settings.namespace);
    let migrations: Api<qrab_operator::api::VirtualMachineMigration> =
        Api::namespaced(client.clone(), &settings.namespace);
    let namespace = settings.namespace.clone();
    // Optional: without it, VMs that declare spec.disks report Blocked.
    let storage: Option<qrab_operator::storage::SharedStorage> =
        match std::env::var("QRAB_STORAGE_ORIGIN") {
            Err(_) => None,
            Ok(storage_origin) => {
                let ca_path = std::env::var("QRAB_STORAGE_CA")
                    .unwrap_or_else(|_| "/etc/qrab-storage-tls/ca.pem".into());
                let ca = std::fs::read(&ca_path).context("read QRAB_STORAGE_CA")?;
                let exchange = std::env::var("QRAB_STORAGE_EXCHANGE_ORIGIN")
                    .context("QRAB_STORAGE_EXCHANGE_ORIGIN is required with QRAB_STORAGE_ORIGIN")?;
                let workload = std::env::var("QRAB_SPIFFE_ENDPOINT_SOCKET")
                    .unwrap_or_else(|_| qrab_spiffe::DEFAULT_ENDPOINT.into());
                Some(Arc::new(
                    qrab_operator::storage::HttpStorage::new(
                        &workload,
                        &exchange,
                        &storage_origin,
                        &ca,
                    )
                    .await
                    .context("initialize the storage-service client")?,
                ))
            }
        };
    let ctx = Arc::new(Context {
        client,
        settings,
        storage,
        metrics,
    });
    let controller = Controller::new(vms, watcher::Config::default())
        .owns(
            pods,
            watcher::Config::default().labels("app.kubernetes.io/name=qrab"),
        )
        .owns(
            configs,
            watcher::Config::default().labels("app.kubernetes.io/name=qrab"),
        )
        // A migration is driven by its VM's reconcile.
        .watches(migrations, watcher::Config::default(), move |migration| {
            Some(
                kube::runtime::reflector::ObjectRef::<VirtualMachine>::new(&migration.spec.vm_name)
                    .within(&namespace),
            )
        })
        .run(controller::reconcile, controller::error_policy, ctx.clone())
        .for_each(|result| async move {
            if let Err(error) = result {
                tracing::warn!(%error,"controller event failed");
            }
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8081").await?;
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

/// `qrab-operator image-importer` (docs/images.md): reconciles BaseImages
/// through the storage service under the importer's own SPIFFE identity.
async fn image_importer() -> Result<()> {
    use qrab_operator::{image::BaseImage, image_import};
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "qrab_operator=info,kube=warn".into()),
        )
        .init();
    let namespace = std::env::var("QRAB_NAMESPACE").unwrap_or_else(|_| "qrab".into());
    let job_image =
        std::env::var("QRAB_IMAGE_JOB_IMAGE").context("QRAB_IMAGE_JOB_IMAGE is required")?;
    ensure!(
        job_image
            .strip_prefix("zot.yapplesauce.com/qrab/operator@sha256:")
            .is_some_and(|d| d.len() == 64
                && d.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))),
        "QRAB_IMAGE_JOB_IMAGE must pin the qrab/operator repository by digest"
    );
    let service_account = std::env::var("QRAB_IMAGE_JOB_SERVICE_ACCOUNT")
        .unwrap_or_else(|_| image_import::MANAGER.into());
    let storage_origin =
        std::env::var("QRAB_STORAGE_ORIGIN").context("QRAB_STORAGE_ORIGIN is required")?;
    let ca_path =
        std::env::var("QRAB_STORAGE_CA").unwrap_or_else(|_| "/etc/qrab-storage-tls/ca.pem".into());
    let ca = std::fs::read(&ca_path).context("read QRAB_STORAGE_CA")?;
    let exchange = std::env::var("QRAB_STORAGE_EXCHANGE_ORIGIN")
        .context("QRAB_STORAGE_EXCHANGE_ORIGIN is required")?;
    let workload = std::env::var("QRAB_SPIFFE_ENDPOINT_SOCKET")
        .unwrap_or_else(|_| qrab_spiffe::DEFAULT_ENDPOINT.into());
    let policy = std::env::var("QRAB_IMAGE_EXCHANGE_POLICY")
        .unwrap_or_else(|_| "qrab-image-importer".into());
    let storage = Arc::new(
        qrab_operator::storage::HttpStorage::with_policy(
            &workload,
            &exchange,
            &storage_origin,
            &ca,
            &policy,
        )
        .await
        .context("initialize the storage-service client")?,
    );
    let client = Client::try_default().await?;
    let images: Api<BaseImage> = Api::namespaced(client.clone(), &namespace);
    images
        .list(&kube::api::ListParams::default().limit(1))
        .await
        .context("list BaseImages (install the CRD and RBAC first)")?;
    let jobs: Api<k8s_openapi::api::batch::v1::Job> = Api::namespaced(client.clone(), &namespace);
    let importer = Arc::new(image_import::Importer {
        cluster: Arc::new(image_import::KubeImportCluster::new(client, &namespace)),
        storage,
        namespace: namespace.clone(),
        job_image,
        service_account,
    });
    let controller = Controller::new(images, watcher::Config::default())
        .owns(
            jobs,
            watcher::Config::default().labels("app.kubernetes.io/name=qrab-image-job"),
        )
        .run(
            |image: Arc<BaseImage>, importer: Arc<image_import::Importer>| async move {
                importer
                    .reconcile(&image)
                    .await
                    .map(kube::runtime::controller::Action::requeue)
                    .map_err(ImportError)
            },
            |_image, error: &ImportError, _| {
                tracing::warn!(error = %format!("{:#}", error.0), "BaseImage reconcile failed");
                kube::runtime::controller::Action::requeue(std::time::Duration::from_secs(30))
            },
            importer,
        )
        .for_each(|result| async move {
            if let Err(error) = result {
                tracing::debug!(%error, "image importer event");
            }
        });
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8081").await?;
    let server = axum::serve(
        listener,
        Router::new().route("/healthz", get(|| async { "ok" })),
    );
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tracing::info!(%namespace, "qrab image importer started");
    tokio::select! {
        _ = controller => anyhow::bail!("controller stream ended"),
        result = server => result.map_err(Into::into),
        _ = term.recv() => Ok(()),
    }
}

#[derive(Debug)]
struct ImportError(anyhow::Error);
impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#}", self.0)
    }
}
impl std::error::Error for ImportError {}
