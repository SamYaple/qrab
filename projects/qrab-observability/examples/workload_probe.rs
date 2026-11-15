//! Read-only live oracle. Execute in the attested container; do not export keys.
//! The executable can be staged by the manager in its shared control directory,
//! shared with Redfish, then removed after the disposable VM test.
use anyhow::{ensure, Context, Result};
use qrab_observability::ID;
use qrab_redfish::history::{Metric, PodIdentity};
use std::{
    net::{IpAddr, SocketAddr},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 4,
        "usage: workload_probe metrics NAMESPACE VM-UID POD-IP | history NAMESPACE VM-UID POD-NAME"
    );
    qrab_spiffe::http::install_tls13_provider()?;
    let identity = qrab_spiffe::Identity::from_env().await?;
    let own = identity.id()?;
    let rf = qrab_spiffe::vm_service_id("yapplesauce.com", &args[1], &args[2], "redfish")?;
    let (peer, url) = match args[0].as_str() {
        "metrics" => {
            let ip: IpAddr = args[3].parse()?;
            (
                rf,
                format!("https://{}/metrics", SocketAddr::new(ip, 9181)).parse()?,
            )
        }
        "history" => {
            ensure!(
                own == rf,
                "history probe must run in the specified VM's Redfish container"
            );
            let pod = PodIdentity::new(&args[1], &args[3]).map_err(anyhow::Error::msg)?;
            let end = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            let mut url: url::Url =
                "https://qrab-prometheus-history.monitoring.svc:9091/api/v1/query_range".parse()?;
            url.query_pairs_mut()
                .append_pair(
                    "query",
                    &Metric::ContainerMemory.promql(&pod.namespace, &pod.pod, 120),
                )
                .append_pair("start", &end.saturating_sub(900).to_string())
                .append_pair("end", &end.to_string())
                .append_pair("step", "30")
                .append_pair("timeout", "2s");
            (qrab_spiffe::id(ID)?, url)
        }
        _ => anyhow::bail!("only metrics/history read probes are supported"),
    };
    let client = qrab_spiffe::http::client(&identity, peer.clone())?;
    let mut response = client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .context("workload TLS read")?;
    let status = response.status();
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            chunk.len() <= (1024usize * 1024).saturating_sub(body.len()),
            "probe answer exceeds bound"
        );
        body.extend_from_slice(&chunk);
    }
    // Only identity/status/counts leave the process, never tokens, keys or samples.
    let series = if args[0] == "history" && status.is_success() {
        let value: serde_json::Value = serde_json::from_slice(&body)?;
        ensure!(
            value["status"] == "success" && value["data"]["resultType"] == "matrix",
            "invalid history response"
        );
        Some(
            value["data"]["result"]
                .as_array()
                .context("history matrix")?
                .len(),
        )
    } else {
        None
    };
    println!(
        "{}",
        serde_json::json!({"ownIdentity":own.to_string(),"expectedPeer":peer.to_string(),
        "status":status.as_u16(),"bytes":body.len(),"series":series})
    );
    ensure!(status.is_success(), "workload read returned {status}");
    Ok(())
}
