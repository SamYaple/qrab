//! Prometheus text exposition of viewer-level VM metrics on a separate,
//! unauthenticated listener. Scrapes within [`WINDOW`] share one collection.
use super::*;
use std::fmt::Write as _;
use tokio::time::Instant;

pub const PATH: &str = "/metrics";
pub const CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";
/// Scrapes inside this window reuse the previous collection, so concurrent or
/// HA-paired Prometheus scrapes add no manager or QMP requests.
pub const WINDOW: Duration = Duration::from_secs(5);
const CALL_DEADLINE: Duration = Duration::from_secs(2);
const POWER_STATES: [&str; 5] = ["On", "Off", "PoweringOn", "PoweringOff", "Paused"];

#[derive(Clone)]
pub struct MetricsExport(Arc<Export>);

struct Export {
    cache: tokio::sync::Mutex<Option<(Instant, Arc<str>)>>,
    admission: Semaphore,
}

impl Default for MetricsExport {
    fn default() -> Self {
        Self(Arc::new(Export {
            cache: tokio::sync::Mutex::new(None),
            admission: Semaphore::new(4),
        }))
    }
}

#[derive(Default)]
struct Snapshot {
    status: Option<pod::Status>,
    agent: Option<pod::AgentStatus>,
    metrics: Option<pod::VmMetrics>,
    inventory: Option<pod::HardwareInventory>,
    nics: Vec<pod::NicView>,
    network: Option<pod::GuestNetworkSnapshot>,
    qmp_dropped: u64,
}

async fn bounded<T>(call: impl std::future::Future<Output = manager::Result<T>>) -> Option<T> {
    tokio::time::timeout(CALL_DEADLINE, call)
        .await
        .ok()
        .and_then(Result::ok)
}

async fn collect(app: &AppState) -> Snapshot {
    let control = &app.control;
    let (status, agent, metrics, inventory) = tokio::join!(
        bounded(control.status()),
        bounded(control.agent_status()),
        bounded(control.metrics()),
        bounded(control.inventory()),
    );
    // Guest NIC counters need the agent; skip the guest round trip without it.
    let (nics, network) = if agent.as_ref().is_some_and(|a| a.connected) {
        let (view, network) = tokio::join!(
            bounded(control.config_view()),
            bounded(control.agent_network())
        );
        (view.map(|v| v.redacted().nics).unwrap_or_default(), network)
    } else {
        (Vec::new(), None)
    };
    Snapshot {
        status,
        agent,
        metrics,
        inventory,
        nics,
        network,
        qmp_dropped: control.events(None, 1).qmp_dropped,
    }
}

/// Label values are configured identifiers only. Anything that could carry a
/// host path, IQN, portal, MAC or IP address is refused rather than escaped.
pub fn label_value(value: &str) -> Option<&str> {
    let safe = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    let parts: Vec<_> = value.split(['-', '_']).collect();
    let mac_like = parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()));
    (safe && !mac_like).then_some(value)
}

struct Family {
    name: &'static str,
    help: &'static str,
    kind: &'static str,
    samples: Vec<(Vec<(&'static str, String)>, u64)>,
}

impl Family {
    fn new(name: &'static str, kind: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            kind,
            samples: Vec::new(),
        }
    }
    fn value(mut self, value: u64) -> Self {
        self.samples.push((Vec::new(), value));
        self
    }
}

fn escape(value: &str) -> String {
    value
        .replace('\\', r"\\")
        .replace('"', "\\\"")
        .replace('\n', r"\n")
}

fn families(snapshot: &Snapshot) -> Vec<Family> {
    let mut out = vec![Family::new(
        "qrab_redfish_build_info",
        "gauge",
        "Redfish sidecar build; always 1.",
    )];
    out[0]
        .samples
        .push((vec![("version", env!("CARGO_PKG_VERSION").into())], 1));
    let (power_state, _, health) = power(snapshot.status.as_ref());
    let mut power_family = Family::new(
        "qrab_vm_power_state",
        "gauge",
        "Redfish PowerState of the VM: 1 for the current state; all 0 when unknown.",
    );
    for state in POWER_STATES {
        power_family.samples.push((
            vec![("state", state.into())],
            u64::from(power_state.as_str() == Some(state)),
        ));
    }
    out.push(power_family);
    out.push(
        Family::new(
            "qrab_vm_health",
            "gauge",
            "Redfish Status.Health of the VM: 0 OK, 1 Warning, 2 Critical.",
        )
        .value(match health {
            "OK" => 0,
            "Warning" => 1,
            _ => 2,
        }),
    );
    if let Some(agent) = &snapshot.agent {
        out.push(
            Family::new(
                "qrab_vm_guest_agent_connected",
                "gauge",
                "1 while the guest agent channel is connected.",
            )
            .value(u64::from(agent.connected)),
        );
    }
    out.push(
        Family::new(
            "qrab_vm_qmp_events_dropped_total",
            "counter",
            "QMP events the manager discarded during the current QEMU connection.",
        )
        .value(snapshot.qmp_dropped),
    );
    if let Some(metrics) = &snapshot.metrics {
        out.push(
            Family::new(
                "qrab_vm_vcpus",
                "gauge",
                "Online guest vCPUs reported by QMP.",
            )
            .value(metrics.logical_processors),
        );
        out.push(
            Family::new(
                "qrab_vm_memory_configured_bytes",
                "gauge",
                "Configured guest RAM, not guest or host memory use.",
            )
            .value(metrics.memory_bytes),
        );
    }
    if let Some(inventory) = &snapshot.inventory {
        type Pick = fn(&pod::DriveCounters) -> u64;
        let block: [(&'static str, &'static str, Pick); 4] = [
            (
                "qrab_vm_block_read_bytes_total",
                "Bytes read by the guest from the drive.",
                |c| c.read_bytes,
            ),
            (
                "qrab_vm_block_write_bytes_total",
                "Bytes written by the guest to the drive.",
                |c| c.write_bytes,
            ),
            (
                "qrab_vm_block_read_ops_total",
                "Read operations completed for the guest.",
                |c| c.read_operations,
            ),
            (
                "qrab_vm_block_write_ops_total",
                "Write operations completed for the guest.",
                |c| c.write_operations,
            ),
        ];
        for (name, help, pick) in block {
            let mut family = Family::new(name, "counter", help);
            for drive in &inventory.drives {
                if let (Some(id), Some(counters)) = (label_value(&drive.id), &drive.counters) {
                    family
                        .samples
                        .push((vec![("drive", id.into())], pick(counters)));
                }
            }
            out.push(family);
        }
    }
    if let Some(network) = &snapshot.network {
        type Pick = fn(&pod::NetworkStatistics) -> u64;
        let nic: [(&'static str, &'static str, Pick); 4] = [
            (
                "qrab_vm_nic_rx_bytes_total",
                "Bytes the guest received on the NIC.",
                |s| s.rx_bytes,
            ),
            (
                "qrab_vm_nic_tx_bytes_total",
                "Bytes the guest transmitted on the NIC.",
                |s| s.tx_bytes,
            ),
            (
                "qrab_vm_nic_rx_packets_total",
                "Packets the guest received on the NIC.",
                |s| s.rx_packets,
            ),
            (
                "qrab_vm_nic_tx_packets_total",
                "Packets the guest transmitted on the NIC.",
                |s| s.tx_packets,
            ),
        ];
        // Configured NIC ids, matched to guest interfaces by MAC; the MAC and
        // guest-chosen interface names never become label values.
        let matched: Vec<_> = snapshot
            .nics
            .iter()
            .filter_map(|nic| {
                let id = label_value(&nic.id)?;
                let stats = network
                    .interfaces
                    .iter()
                    .find(|i| {
                        i.hardware_address
                            .as_deref()
                            .is_some_and(|mac| mac.eq_ignore_ascii_case(&nic.mac))
                    })?
                    .statistics
                    .as_ref()?;
                Some((id, stats))
            })
            .collect();
        for (name, help, pick) in nic {
            let mut family = Family::new(name, "counter", help);
            for (id, stats) in &matched {
                family
                    .samples
                    .push((vec![("nic", (*id).into())], pick(stats)));
            }
            out.push(family);
        }
    }
    out
}

fn render(snapshot: &Snapshot) -> String {
    let mut text = String::new();
    for family in families(snapshot) {
        if family.samples.is_empty() {
            continue;
        }
        let _ = writeln!(text, "# HELP {} {}", family.name, family.help);
        let _ = writeln!(text, "# TYPE {} {}", family.name, family.kind);
        for (labels, value) in &family.samples {
            text.push_str(family.name);
            if !labels.is_empty() {
                let labels: Vec<_> = labels
                    .iter()
                    .map(|(k, v)| format!("{k}=\"{}\"", escape(v)))
                    .collect();
                let _ = write!(text, "{{{}}}", labels.join(","));
            }
            let _ = writeln!(text, " {value}");
        }
    }
    text
}

impl MetricsExport {
    async fn text(&self, app: &AppState) -> Arc<str> {
        // Held across collection: concurrent scrapes wait for one result.
        let mut cache = self.0.cache.lock().await;
        if let Some((at, text)) = cache.as_ref() {
            if at.elapsed() < WINDOW {
                return text.clone();
            }
        }
        let snapshot = collect(app).await;
        if let Some(sample) = &snapshot.metrics {
            telemetry::observe(app, sample);
        }
        let text: Arc<str> = render(&snapshot).into();
        *cache = Some((Instant::now(), text.clone()));
        text
    }
}

async fn scrape(State(app): State<AppState>) -> Response {
    let Ok(_permit) = app.exporter.0.admission.try_acquire() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "scrape limit reached\n").into_response();
    };
    let text = app.exporter.text(&app).await;
    (
        [
            (header::CONTENT_TYPE, CONTENT_TYPE),
            (header::CACHE_CONTROL, "no-store"),
        ],
        text.to_string(),
    )
        .into_response()
}

/// The metrics listener's whole surface: `GET /metrics`, nothing else.
pub fn router(app: AppState) -> Router {
    Router::new()
        .route(PATH, get(scrape))
        .fallback(|| async { StatusCode::NOT_FOUND })
        .method_not_allowed_fallback(|| async { StatusCode::METHOD_NOT_ALLOWED })
        .with_state(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_values_refuse_addresses_paths_and_identifiers_with_separators() {
        for good in ["disk-root", "boot", "device-3", "lan_0", "eth0"] {
            assert_eq!(label_value(good), Some(good));
        }
        for bad in [
            "",
            "/dev/sda",
            "iqn.2026-11.com.example:disk",
            "192.0.2.10",
            "192.0.2.10:3260",
            "fe80::1",
            "02:aa:bb:cc:dd:ee",
            "02-aa-bb-cc-dd-ee",
            "02_AA_BB_CC_DD_EE",
            "a\"b",
            &"x".repeat(65),
        ] {
            assert_eq!(label_value(bad), None, "{bad}");
        }
    }

    #[test]
    fn unknown_status_renders_no_current_power_state() {
        let text = render(&Snapshot::default());
        assert!(text.contains("qrab_vm_power_state{state=\"On\"} 0\n"));
        assert!(!text
            .lines()
            .any(|l| l.starts_with("qrab_vm_power_state") && l.ends_with(" 1")));
        assert!(text.contains("qrab_vm_health 1\n"));
        assert!(!text.contains("qrab_vm_vcpus"));
        assert!(!text.contains("qrab_vm_guest_agent_connected"));
    }
}
