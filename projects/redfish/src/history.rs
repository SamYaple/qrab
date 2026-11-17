//! Metric history read back from the cluster Prometheus. Every metric id maps
//! to one fixed PromQL template scoped to this VM's own namespace and Pod,
//! which come from the downward API, never from the request.
use super::*;
use axum::body::Bytes;
use openidconnect::{reqwest, url::Url};
use serde::Deserialize;
use tokio::time::Instant;

pub const PATH: &str = "/redfish/v1/TelemetryService/Oem/Qrab/MetricHistory";
/// kube-prometheus-stack's in-cluster query endpoint.
pub const DEFAULT_PROMETHEUS_URL: &str = "https://qrab-prometheus-history.monitoring.svc:9091";
/// One upstream query, connection and body included.
pub const DEADLINE: Duration = Duration::from_secs(2);
/// Successful answers are reused per (metric, range) for this long.
pub const CACHE_FOR: Duration = Duration::from_secs(15);
const MAX_BODY: usize = 1024 * 1024;
const MAX_SERIES: usize = 16;
/// Points per series the step aims at.
const POINTS: u64 = 240;
/// kube-prometheus-stack and the qrab PodMonitor scrape every 30 s, so a rate
/// window needs at least four scrapes.
const MIN_RATE_WINDOW: u64 = 120;
/// The VM Pod's containers whose cAdvisor usage is shown.
const CONTAINERS: &str = "manager|bmc|redfish";

/// The fixed set of history metrics. vCPU utilisation is absent: the exported
/// families carry the vCPU count but no guest CPU time, and the manager
/// container's CPU also includes QEMU's device emulation and swtpm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    ContainerCpu,
    ContainerMemory,
    BlockRead,
    BlockWrite,
    NicRx,
    NicTx,
}

impl Metric {
    pub const ALL: [Self; 6] = [
        Self::ContainerCpu,
        Self::ContainerMemory,
        Self::BlockRead,
        Self::BlockWrite,
        Self::NicRx,
        Self::NicTx,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::ContainerCpu => "container-cpu",
            Self::ContainerMemory => "container-memory",
            Self::BlockRead => "block-read",
            Self::BlockWrite => "block-write",
            Self::NicRx => "nic-rx",
            Self::NicTx => "nic-tx",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::ContainerCpu => "Container CPU",
            Self::ContainerMemory => "Container memory working set",
            Self::BlockRead => "Disk read",
            Self::BlockWrite => "Disk write",
            Self::NicRx => "Network received",
            Self::NicTx => "Network sent",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::ContainerCpu => "cores",
            Self::ContainerMemory => "bytes",
            _ => "bytes/s",
        }
    }

    /// The one label each series is grouped by.
    pub fn label(self) -> &'static str {
        match self {
            Self::ContainerCpu | Self::ContainerMemory => "container",
            Self::BlockRead | Self::BlockWrite => "drive",
            Self::NicRx | Self::NicTx => "nic",
        }
    }

    /// The fixed query for this Pod incarnation. Names/UID come from validated
    /// Kubernetes metadata; UID also binds retained historical series.
    ///
    /// ```
    /// use qrab_redfish::history::Metric;
    /// assert_eq!(
    ///     Metric::BlockRead.promql("qrab", "vm-a", "pod-uid", 120),
    ///     r#"sum by (drive) (rate(qrab_vm_block_read_bytes_total{namespace="qrab",pod="vm-a",pod_uid="pod-uid"}[120s]))"#
    /// );
    /// ```
    pub fn promql(self, namespace: &str, pod: &str, uid: &str, window: u64) -> String {
        // cAdvisor's id is the kernel cgroup path. Match a whole Pod component
        // for both cgroupfs UUIDs and systemd's underscore-escaped UUIDs.
        let systemd = uid.replace('-', "_");
        let id = serde_json::to_string(&format!(r".*/[^/]*pod({uid}|{systemd})(\.slice)?(/.*)?"))
            .expect("string serializes");
        let cadvisor =
            format!(r#"namespace="{namespace}",pod="{pod}",container=~"{CONTAINERS}",id=~{id}"#);
        let uid = serde_json::to_string(uid).expect("string serializes");
        let vm = format!(r#"namespace="{namespace}",pod="{pod}",pod_uid={uid}"#);
        let rate = |family: &str| {
            format!(
                "sum by ({}) (rate({family}{{{vm}}}[{window}s]))",
                self.label()
            )
        };
        match self {
            Self::ContainerCpu => format!(
                "sum by (container) (rate(container_cpu_usage_seconds_total{{{cadvisor}}}[{window}s]))"
            ),
            Self::ContainerMemory => {
                format!("sum by (container) (container_memory_working_set_bytes{{{cadvisor}}})")
            }
            Self::BlockRead => rate("qrab_vm_block_read_bytes_total"),
            Self::BlockWrite => rate("qrab_vm_block_write_bytes_total"),
            Self::NicRx => rate("qrab_vm_nic_rx_bytes_total"),
            Self::NicTx => rate("qrab_vm_nic_tx_bytes_total"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    M15,
    H1,
    H6,
    H24,
    D7,
}

impl Range {
    pub const ALL: [Self; 5] = [Self::M15, Self::H1, Self::H6, Self::H24, Self::D7];

    pub fn id(self) -> &'static str {
        match self {
            Self::M15 => "15m",
            Self::H1 => "1h",
            Self::H6 => "6h",
            Self::H24 => "24h",
            Self::D7 => "7d",
        }
    }

    pub fn seconds(self) -> u64 {
        match self {
            Self::M15 => 900,
            Self::H1 => 3600,
            Self::H6 => 6 * 3600,
            Self::H24 => 24 * 3600,
            Self::D7 => 7 * 24 * 3600,
        }
    }

    /// Range/240 rounded to whole seconds, at least 15 s.
    ///
    /// ```
    /// use qrab_redfish::history::Range;
    /// let steps: Vec<_> = Range::ALL.iter().map(|r| r.step()).collect();
    /// assert_eq!(steps, [15, 15, 90, 360, 2520]);
    /// ```
    pub fn step(self) -> u64 {
        ((self.seconds() + POINTS / 2) / POINTS).max(15)
    }

    /// The `rate()` window: one step, but never fewer than four scrapes.
    pub fn rate_window(self) -> u64 {
        self.step().max(MIN_RATE_WINDOW)
    }
}

fn out_of_range(parameter: &str, value: &str, allowed: &[&str]) -> Failure {
    let value: String = value.chars().take(64).collect();
    Failure::new(
        StatusCode::BAD_REQUEST,
        "QueryParameterOutOfRange",
        format!(
            "The value {value:?} for the query parameter {parameter} is out of range; use one of {}.",
            allowed.join(", ")
        ),
        "Use a metric and range from the TelemetryService's Oem.Qrab.MetricHistory allowable values.",
    )
}

/// Parses `metric=<id>&range=<id>`; `range` defaults to `1h`. Values are
/// compared verbatim (no percent-decoding), so nothing outside the fixed sets
/// is accepted or forwarded.
pub fn parse_query(query: Option<&str>) -> Result<(Metric, Range), Failure> {
    let (mut metric, mut range) = (None, None);
    for pair in query.unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "metric" if metric.is_none() => {
                metric = Some(
                    Metric::ALL
                        .into_iter()
                        .find(|m| m.id() == value)
                        .ok_or_else(|| {
                            out_of_range("metric", value, &Metric::ALL.map(Metric::id))
                        })?,
                );
            }
            "range" if range.is_none() => {
                range = Some(
                    Range::ALL
                        .into_iter()
                        .find(|r| r.id() == value)
                        .ok_or_else(|| out_of_range("range", value, &Range::ALL.map(Range::id)))?,
                );
            }
            "metric" | "range" => {
                return Err(Failure::new(
                    StatusCode::BAD_REQUEST,
                    "QueryCombinationInvalid",
                    format!("The query parameter {key} is repeated."),
                    "Pass each query parameter once.",
                ))
            }
            other => {
                let other: String = other.chars().take(64).collect();
                return Err(Failure::new(
                    StatusCode::BAD_REQUEST,
                    "QueryParameterUnsupported",
                    format!("The query parameter {other:?} is not supported."),
                    "Use only the metric and range query parameters.",
                ));
            }
        }
    }
    let metric = metric.ok_or_else(|| out_of_range("metric", "", &Metric::ALL.map(Metric::id)))?;
    Ok((metric, range.unwrap_or(Range::H1)))
}

/// A Kubernetes namespace (DNS label) and Pod name (DNS subdomain).
fn kubernetes_name(value: &str, max: usize, dots: bool) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= max
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes.iter().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-' || (dots && *b == b'.')
        })
}

/// This Pod incarnation, from namespace/name/UID downward metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodIdentity {
    pub namespace: String,
    pub pod: String,
    pub uid: String,
}

impl PodIdentity {
    /// ```
    /// use qrab_redfish::history::PodIdentity;
    /// assert!(PodIdentity::new("qrab", "vm-a.1", "pod-uid").is_ok());
    /// assert!(PodIdentity::new("qrab", "vm\"}", "pod-uid").is_err());
    /// assert!(PodIdentity::new("Qrab", "vm", "pod-uid").is_err());
    /// assert!(PodIdentity::new("q.rab", "vm", "pod-uid").is_err());
    /// assert!(PodIdentity::new("qrab", "vm", "").is_err());
    /// assert!(PodIdentity::new("qrab", "vm", "uid|another").is_err());
    /// ```
    pub fn new(namespace: &str, pod: &str, uid: &str) -> Result<Self, String> {
        if !kubernetes_name(namespace, 63, false) {
            return Err("QRAB_POD_NAMESPACE must be a Kubernetes namespace name".into());
        }
        if !kubernetes_name(pod, 253, true) {
            return Err("QRAB_POD_NAME must be a Kubernetes Pod name".into());
        }
        if !kubernetes_name(uid, 128, false) {
            return Err("QRAB_POD_UID must be a nonempty Kubernetes Pod UID".into());
        }
        Ok(Self {
            namespace: namespace.into(),
            pod: pod.into(),
            uid: uid.into(),
        })
    }
}

/// The Prometheus base URL: plain HTTP(S), no credentials, query or fragment.
///
/// ```
/// use qrab_redfish::history::prometheus_url;
/// assert!(prometheus_url("http://kube-prometheus-stack-prometheus.monitoring.svc:9090").is_ok());
/// assert!(prometheus_url("http://127.0.0.1:9090/prometheus/").is_ok());
/// assert!(prometheus_url("http://user:pw@prometheus:9090").is_err());
/// assert!(prometheus_url("http://prometheus:9090/?x=1").is_err());
/// assert!(prometheus_url("file:///etc/passwd").is_err());
/// ```
pub fn prometheus_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|_| "REDFISH_PROMETHEUS_URL must be a URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "REDFISH_PROMETHEUS_URL must be an http(s) URL without credentials, query or fragment"
                .into(),
        );
    }
    Ok(url)
}

type Slot = tokio::sync::Mutex<Option<(Instant, Bytes)>>;

struct Upstream {
    endpoint: Url,
    pod: PodIdentity,
    client: reqwest::Client,
    /// One slot per (metric, range): concurrent identical reads share a query.
    slots: Vec<Slot>,
    admission: Semaphore,
}

/// History queries for one VM Pod; `Default` is unconfigured (503).
#[derive(Clone, Default)]
pub struct HistoryStore(Option<Arc<Upstream>>);

impl HistoryStore {
    pub fn new(prometheus: Url, pod: Option<PodIdentity>) -> Result<Self, String> {
        if pod.is_none() {
            return Ok(Self(None));
        }
        // Standalone compatibility is restricted to the same host's literal
        // loopback. Cluster startup uses mtls(), with no WebPKI/plain fallback.
        if !matches!(prometheus.host(), Some(openidconnect::url::Host::Ipv4(ip)) if ip.is_loopback())
            && !matches!(prometheus.host(), Some(openidconnect::url::Host::Ipv6(ip)) if ip.is_loopback())
        {
            return Err("nonlocal Prometheus history requires SPIFFE mTLS".into());
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(DEADLINE)
            .timeout(DEADLINE)
            .pool_max_idle_per_host(2)
            .build()
            .map_err(|e| e.to_string())?;
        Self::with_client(prometheus, pod, client)
    }

    pub fn mtls(
        prometheus: Url,
        pod: Option<PodIdentity>,
        identity: &qrab_spiffe::Identity,
        peer: qrab_spiffe::SpiffeId,
    ) -> Result<Self, String> {
        if prometheus.scheme() != "https" || prometheus.path() != "/" {
            return Err("workload Prometheus history requires a bare HTTPS origin".into());
        }
        let client = qrab_spiffe::http::client(identity, peer).map_err(|e| e.to_string())?;
        Self::with_client(prometheus, pod, client)
    }

    fn with_client(
        prometheus: Url,
        pod: Option<PodIdentity>,
        client: reqwest::Client,
    ) -> Result<Self, String> {
        let Some(pod) = pod else {
            return Ok(Self(None));
        };
        let mut endpoint = prometheus;
        endpoint
            .path_segments_mut()
            .map_err(|_| "invalid Prometheus origin")?
            .pop_if_empty()
            .extend(["api", "v1", "query_range"]);
        Ok(Self(Some(Arc::new(Upstream {
            endpoint,
            pod,
            client,
            slots: (0..Metric::ALL.len() * Range::ALL.len())
                .map(|_| Slot::new(None))
                .collect(),
            admission: Semaphore::new(4),
        }))))
    }
}

#[derive(Deserialize)]
struct Answer {
    status: String,
    #[serde(default)]
    data: Option<Matrix>,
}

#[derive(Deserialize)]
struct Matrix {
    #[serde(rename = "resultType")]
    result_type: String,
    result: Vec<PromSeries>,
}

#[derive(Deserialize)]
struct PromSeries {
    #[serde(default)]
    metric: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    values: Vec<(f64, String)>,
}

enum UpstreamError {
    Unreachable,
    Timeout,
    Invalid,
}

impl From<reqwest::Error> for UpstreamError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout
        } else {
            Self::Unreachable
        }
    }
}

impl Upstream {
    async fn query(&self, metric: Metric, range: Range, end: u64) -> Result<Value, UpstreamError> {
        let step = range.step();
        let start = end.saturating_sub(range.seconds());
        let mut url = self.endpoint.clone();
        url.query_pairs_mut()
            .append_pair(
                "query",
                &metric.promql(
                    &self.pod.namespace,
                    &self.pod.pod,
                    &self.pod.uid,
                    range.rate_window(),
                ),
            )
            .append_pair("start", &start.to_string())
            .append_pair("end", &end.to_string())
            .append_pair("step", &step.to_string())
            .append_pair("timeout", "2s");
        let mut response = self.client.get(url).send().await?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|n| n > MAX_BODY as u64)
        {
            return Err(UpstreamError::Invalid);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > MAX_BODY {
                return Err(UpstreamError::Invalid);
            }
            body.extend_from_slice(&chunk);
        }
        let answer: Answer = serde_json::from_slice(&body).map_err(|_| UpstreamError::Invalid)?;
        let matrix = answer
            .data
            .filter(|d| answer.status == "success" && d.result_type == "matrix")
            .ok_or(UpstreamError::Invalid)?;
        let label = metric.label();
        let mut series: Vec<Value> = matrix
            .result
            .into_iter()
            .take(MAX_SERIES)
            .map(|s| {
                let name = s.metric.get(label).cloned().unwrap_or_default();
                // NaN and infinities have no JSON number; they are gaps.
                let points: Vec<Value> = s
                    .values
                    .iter()
                    .filter_map(|(t, v)| {
                        let v: f64 = v.parse().ok().filter(|v: &f64| v.is_finite())?;
                        Some(json!([t.round() as i64, v]))
                    })
                    .collect();
                json!({"labels": {label: name}, "points": points})
            })
            .collect();
        series.sort_by(|a, b| {
            a["labels"][label]
                .as_str()
                .cmp(&b["labels"][label].as_str())
        });
        Ok(json!({
            "@odata.id": PATH,
            "@odata.type": "#QrabMetricHistory.v1_0_0.QrabMetricHistory",
            "Id": "MetricHistory",
            "Name": metric.name(),
            "metric": metric.id(),
            "range": range.id(),
            "unit": metric.unit(),
            "step": step,
            "start": start,
            "end": end,
            "series": series,
        }))
    }
}

/// The TelemetryService's `Oem.Qrab.MetricHistory` discovery object.
pub(super) fn discovery() -> Value {
    json!({
        "@odata.id": PATH,
        "Metric@Redfish.AllowableValues": Metric::ALL.map(Metric::id),
        "Range@Redfish.AllowableValues": Range::ALL.map(Range::id),
    })
}

pub(super) async fn history(
    State(app): State<AppState>,
    headers: HeaderMap,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> Result<Response, Failure> {
    principal(&app, &headers).await?;
    let (metric, range) = parse_query(query.as_deref())?;
    let upstream = app.history.0.as_ref().ok_or_else(|| {
        Failure::unavailable(
            "Metric history requires QRAB_POD_NAMESPACE, QRAB_POD_NAME and QRAB_POD_UID.",
        )
    })?;
    let index = Metric::ALL.iter().position(|m| *m == metric).unwrap() * Range::ALL.len()
        + Range::ALL.iter().position(|r| *r == range).unwrap();
    let answer = tokio::time::timeout(DEADLINE, async {
        let mut slot = upstream.slots[index].lock().await;
        if let Some((_, body)) = slot.as_ref().filter(|(at, _)| at.elapsed() < CACHE_FOR) {
            return Ok(body.clone());
        }
        let _permit = upstream
            .admission
            .acquire()
            .await
            .map_err(|_| UpstreamError::Unreachable)?;
        let value = upstream.query(metric, range, auth::now()).await?;
        let body = Bytes::from(serde_json::to_vec(&value).map_err(|_| UpstreamError::Invalid)?);
        *slot = Some((Instant::now(), body.clone()));
        Ok(body)
    })
    .await;
    match answer {
        Ok(Ok(body)) => Ok(([(header::CONTENT_TYPE, "application/json")], body).into_response()),
        Ok(Err(UpstreamError::Unreachable)) => Err(Failure::unavailable(
            "Prometheus is unreachable, so metric history is unavailable.",
        )),
        Ok(Err(UpstreamError::Invalid)) => Err(Failure::unavailable(
            "Prometheus returned an unusable answer to the metric history query.",
        )),
        Ok(Err(UpstreamError::Timeout)) | Err(_) => Err(Failure::unavailable(
            "Prometheus did not answer the metric history query within 2 seconds.",
        )),
    }
}
