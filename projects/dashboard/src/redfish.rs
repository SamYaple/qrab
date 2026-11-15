//! Server-side calls to one VM's Redfish service over SPIFFE mTLS,
//! carrying the signed-in user's ID token as `Authorization: Bearer`. Redfish
//! trusts the dashboard's token audience and applies the VM's admin/viewer
//! lists, which include the console's while the BMC is enabled, so it
//! enforces the same roles as the dashboard. On a Redfish-only VM (no BMC) only
//! `components.redfish`'s own lists apply, so Redfish may refuse a dashboard
//! admin who is not in them.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

pub const SYSTEM: &str = "/redfish/v1/Systems/1";
pub const RESET: &str = "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset";
pub const RESET_INFO: &str = "/redfish/v1/Systems/1/ResetActionInfo";
pub const EVENT_LOG: &str = "/redfish/v1/Managers/1/LogServices/Events";
pub const EVENT_ENTRIES: &str = "/redfish/v1/Managers/1/LogServices/Events/Entries";
/// The last hour of each Redfish `MetricHistory` metric (fixed server-side
/// PromQL over the cluster Prometheus), in the order the page shows them.
pub const HISTORY: [&str; 6] = [
    "/redfish/v1/TelemetryService/Oem/Qrab/MetricHistory?metric=container-cpu&range=1h",
    "/redfish/v1/TelemetryService/Oem/Qrab/MetricHistory?metric=container-memory&range=1h",
    "/redfish/v1/TelemetryService/Oem/Qrab/MetricHistory?metric=block-read&range=1h",
    "/redfish/v1/TelemetryService/Oem/Qrab/MetricHistory?metric=block-write&range=1h",
    "/redfish/v1/TelemetryService/Oem/Qrab/MetricHistory?metric=nic-rx&range=1h",
    "/redfish/v1/TelemetryService/Oem/Qrab/MetricHistory?metric=nic-tx&range=1h",
];

/// QMP events the detail page shows, newest first.
pub const EVENTS_SHOWN: usize = 20;

/// Largest Redfish resource body the dashboard reads.
pub const MAX_BODY: usize = 1024 * 1024;
/// Largest event-log collection: Redfish has no `$top`/`$skip`, so the
/// dashboard reads the whole bounded log (256 entries with payloads of at most
/// 8 KiB each) and keeps the newest [`EVENTS_SHOWN`].
pub const MAX_LOG_BODY: usize = 4 * 1024 * 1024;

/// The in-pod `ComputerSystem.Reset` types the dashboard sends. `On` is not
/// offered as a reset button of its own: Power on sends it to an
/// existing Pod (see [`crate::power::PowerOn`]), and Resume covers a paused VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ResetType {
    Resume,
    Pause,
    GracefulRestart,
    ForceRestart,
    PowerCycle,
    Nmi,
    GracefulShutdown,
    ForceOff,
    On,
}

impl ResetType {
    /// The offered reset types, in button order.
    pub const ALL: [Self; 8] = [
        Self::Resume,
        Self::Pause,
        Self::GracefulRestart,
        Self::ForceRestart,
        Self::PowerCycle,
        Self::Nmi,
        Self::GracefulShutdown,
        Self::ForceOff,
    ];

    /// ```
    /// use qrab_dashboard::redfish::ResetType;
    /// assert_eq!(ResetType::parse("ForceRestart"), Some(ResetType::ForceRestart));
    /// assert_eq!(ResetType::parse("On"), None);
    /// assert_eq!(ResetType::parse("reset"), None);
    /// ```
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == value)
    }
    /// The DMTF ResetType value.
    pub fn name(self) -> &'static str {
        match self {
            Self::Resume => "Resume",
            Self::Pause => "Pause",
            Self::GracefulRestart => "GracefulRestart",
            Self::ForceRestart => "ForceRestart",
            Self::PowerCycle => "PowerCycle",
            Self::Nmi => "Nmi",
            Self::GracefulShutdown => "GracefulShutdown",
            Self::ForceOff => "ForceOff",
            Self::On => "On",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Resume => "Resume",
            Self::Pause => "Pause",
            Self::GracefulRestart => "Restart guest",
            Self::ForceRestart => "Reset",
            Self::PowerCycle => "Power cycle",
            Self::Nmi => "NMI",
            Self::GracefulShutdown => "Shut down",
            Self::ForceOff => "Force off",
            Self::On => "Power on",
        }
    }
    /// Disruptive actions need an explicit confirmation.
    pub fn needs_confirmation(self) -> bool {
        !matches!(self, Self::Resume | Self::Pause | Self::On)
    }
}

/// Reads at most `limit` bytes; a longer body is refused, not truncated.
async fn read_capped(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, RedfishError> {
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(RedfishError::Invalid);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| RedfishError::Unreachable)?
    {
        if body.len() + chunk.len() > limit {
            return Err(RedfishError::Invalid);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedfishError {
    Status(u16),
    Unreachable,
    Invalid,
}

impl std::fmt::Display for RedfishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Status(401) => write!(f, "Redfish did not accept your sign-in (HTTP 401)"),
            Self::Status(403) => write!(f, "Redfish refused this for your role (HTTP 403)"),
            Self::Status(code) => write!(f, "Redfish answered HTTP {code}"),
            Self::Unreachable => write!(f, "Redfish is unreachable"),
            Self::Invalid => write!(f, "Redfish sent an unexpected response"),
        }
    }
}

#[derive(Clone)]
pub struct Redfish {
    identity: Option<qrab_spiffe::Identity>,
    port: u16,
}

pub struct VmRedfish {
    http: reqwest::Client,
    origin: String,
}

impl Redfish {
    pub fn new(identity: qrab_spiffe::Identity, port: u16) -> Self {
        Self {
            identity: Some(identity),
            port,
        }
    }

    #[cfg(test)]
    pub fn for_test(port: u16) -> anyhow::Result<Self> {
        Ok(Self {
            identity: None,
            port,
        })
    }

    /// Resolve the expected service ID from trusted immutable VM metadata.
    pub fn for_vm(
        &self,
        vm: &qrab_operator::api::VirtualMachine,
        ip: IpAddr,
    ) -> anyhow::Result<VmRedfish> {
        let identity = self.identity.as_ref();
        let (http, scheme) = if let Some(identity) = identity {
            (
                qrab_spiffe::http::client(
                    identity,
                    crate::workload::peer(identity, vm, "redfish")?,
                )?,
                "https",
            )
        } else {
            #[cfg(test)]
            {
                anyhow::ensure!(ip.is_loopback(), "plaintext test fixture must be loopback");
                (
                    reqwest::Client::builder()
                        .redirect(reqwest::redirect::Policy::none())
                        .no_proxy()
                        .connect_timeout(Duration::from_secs(3))
                        .build()?,
                    "http",
                )
            }
            #[cfg(not(test))]
            {
                anyhow::bail!("SPIFFE identity unavailable")
            }
        };
        Ok(VmRedfish {
            http,
            origin: format!("{scheme}://{}", SocketAddr::new(ip, self.port)),
        })
    }
}

impl VmRedfish {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }

    /// Shared transport for the VM-scoped browser bridge, including SSE.
    pub(crate) fn browser_request(
        &self,
        path: &str,
        method: reqwest::Method,
    ) -> reqwest::RequestBuilder {
        self.http
            .request(method, self.url(path))
            .timeout(Duration::from_secs(60))
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &'static str,
        token: &str,
        limit: usize,
    ) -> Result<T, RedfishError> {
        let response = self
            .http
            .get(self.url(path))
            .timeout(Duration::from_secs(8))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| RedfishError::Unreachable)?;
        if !response.status().is_success() {
            return Err(RedfishError::Status(response.status().as_u16()));
        }
        serde_json::from_slice(&read_capped(response, limit).await?)
            .map_err(|_| RedfishError::Invalid)
    }

    /// `ComputerSystem.Reset` with one ResetType.
    pub async fn reset(&self, token: &str, reset: ResetType) -> Result<(), RedfishError> {
        let response = self
            .http
            .post(self.url(RESET))
            .timeout(Duration::from_secs(8))
            .bearer_auth(token)
            .json(&serde_json::json!({"ResetType": reset.name()}))
            .send()
            .await
            .map_err(|_| RedfishError::Unreachable)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(RedfishError::Status(response.status().as_u16()))
        }
    }

    /// The offered reset types Redfish currently allows, in the dashboard's order.
    pub async fn reset_types(&self, token: &str) -> Result<Vec<ResetType>, RedfishError> {
        let info: ActionInfo = self.get(RESET_INFO, token, MAX_BODY).await?;
        let allowed = info
            .parameters
            .into_iter()
            .find(|p| p.name == "ResetType")
            .map(|p| p.allowable_values)
            .unwrap_or_default();
        Ok(offered(&allowed))
    }

    /// The detail page's reads, fetched concurrently.
    pub async fn panel(&self, token: &str) -> Panel {
        let history = futures::future::join_all(HISTORY.map(|path| async move {
            let metric = path
                .split_once("metric=")
                .and_then(|(_, rest)| rest.split('&').next())
                .unwrap_or_default();
            let read = self.get::<History>(path, token, MAX_BODY).await;
            (metric, read.map_err(|e| e.to_string()))
        }));
        let (system, log, entries, history) = tokio::join!(
            self.get::<System>(SYSTEM, token, MAX_BODY),
            self.get::<LogService>(EVENT_LOG, token, MAX_BODY),
            self.get::<Collection<LogEntry>>(EVENT_ENTRIES, token, MAX_LOG_BODY),
            history,
        );
        Panel {
            system: system.map_err(|e| e.to_string()),
            events: entries
                .map(|entries| Events::newest(entries.members, log.ok()))
                .map_err(|e| e.to_string()),
            history,
        }
    }
}

/// The offered reset types among Redfish's allowable values.
pub fn offered(allowed: &[String]) -> Vec<ResetType> {
    ResetType::ALL
        .into_iter()
        .filter(|t| allowed.iter().any(|a| a == t.name()))
        .collect()
}

/// Lenient copies of the Redfish resources. Every string here may be guest-
/// or VM-controlled; the UI renders them only as escaped text.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct System {
    pub power_state: Option<String>,
    pub status: Status,
    pub actions: SystemActions,
    pub oem: SystemOem,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Status {
    pub state: Option<String>,
    pub health: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SystemActions {
    #[serde(rename = "#ComputerSystem.Reset")]
    pub reset: ResetAction,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ResetAction {
    #[serde(rename = "ResetType@Redfish.AllowableValues")]
    pub allowable_values: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct SystemOem {
    pub qrab: QrabSystem,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct QrabSystem {
    pub configuration: Option<Config>,
    pub guest_agent: Option<AgentStatus>,
}

/// A known guest-agent prerequisite; absence does not grant action permission.
pub fn guest_restart_unavailable(system: &System) -> Option<&'static str> {
    let agent = system.oem.qrab.guest_agent.as_ref()?;
    if !agent.enabled || !agent.connected {
        Some("Guest restart needs a connected guest agent.")
    } else if agent.freeze_state.as_deref() == Some("Frozen") {
        Some("Thaw guest filesystems before restarting the guest.")
    } else {
        None
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct ActionInfo {
    parameters: Vec<ActionParameter>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct ActionParameter {
    name: String,
    allowable_values: Vec<String>,
}

/// The manager's guest-agent status, as `Oem.Qrab.GuestAgent`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentStatus {
    pub enabled: bool,
    pub connected: bool,
    pub version: Option<String>,
    pub connected_since_unix_ms: Option<u64>,
    pub last_seen_unix_ms: Option<u64>,
    pub last_error: Option<String>,
    /// `Frozen` or `Thawed`; null while the agent is disconnected or slow.
    #[serde(rename = "FreezeState")]
    pub freeze_state: Option<String>,
    #[serde(rename = "LastUpdate")]
    pub last_update: Option<LastUpdate>,
}

/// The guest's last update-hook run. Redfish sends `Output` and `Error` to
/// admins only.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct LastUpdate {
    pub state: String,
    pub run_id: u64,
    pub hook_installed: bool,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub end_time: Option<String>,
    pub output: Option<String>,
    pub output_truncated: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Hotplug {
    pub max_cpus: u32,
    pub max_memory_mib: u64,
    pub phase: Option<String>,
    pub last_error: Option<String>,
}

/// The manager's configuration view, as `Oem.Qrab.Configuration` (redacted
/// for viewers by Redfish).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub name: String,
    pub memory_mib: u64,
    pub cpus: u32,
    pub accelerator: String,
    pub boot: String,
    pub console: bool,
    pub tpm: bool,
    pub guest_network: bool,
    pub disks: Vec<serde_json::Value>,
    pub startup_timeout_secs: u64,
    pub shutdown_grace_secs: u64,
    pub redacted: bool,
    pub state_id: Option<String>,
    pub hotplug: Option<Hotplug>,
    pub deployment: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Collection<T> {
    pub members: Vec<T>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct LogService {
    pub oem: LogServiceOem,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct LogServiceOem {
    pub qrab: QrabLogService,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct QrabLogService {
    pub qmp_dropped: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct LogEntry {
    pub created: String,
    pub severity: String,
    pub message: String,
    pub oem: LogEntryOem,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct LogEntryOem {
    pub qrab: QrabEvent,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct QrabEvent {
    pub sequence: u64,
    pub event: String,
    pub data: serde_json::Value,
    pub truncated: bool,
}

/// The newest QMP events, newest first.
#[derive(Debug, Clone, Default)]
pub struct Events {
    pub entries: Vec<LogEntry>,
    /// Events QEMU emitted that the manager had to discard; `None` when the
    /// log service could not be read.
    pub qmp_dropped: Option<u64>,
}

impl Events {
    fn newest(mut entries: Vec<LogEntry>, log: Option<LogService>) -> Self {
        entries.sort_by_key(|e| std::cmp::Reverse(e.oem.qrab.sequence));
        entries.truncate(EVENTS_SHOWN);
        Self {
            entries,
            qmp_dropped: log.map(|log| log.oem.qrab.qmp_dropped),
        }
    }
}

/// One Redfish `MetricHistory` answer: series of `[unix seconds, value]`
/// points in `unit`. Labels are configured drive/NIC ids or container names,
/// still rendered only as escaped text.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct History {
    #[serde(rename = "Name")]
    pub name: String,
    pub unit: String,
    pub step: u64,
    pub start: i64,
    pub end: i64,
    pub series: Vec<HistorySeries>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HistorySeries {
    pub labels: BTreeMap<String, String>,
    pub points: Vec<(f64, f64)>,
}

impl HistorySeries {
    /// The series' one label value, e.g. `manager` or a drive id.
    pub fn label(&self) -> String {
        self.labels.values().next().cloned().unwrap_or_default()
    }
}

/// A [`HISTORY`] metric id and its answer, or the error text.
pub type HistoryRead = (&'static str, Result<History, String>);

#[derive(Debug, Clone)]
pub struct Panel {
    pub system: Result<System, String>,
    pub events: Result<Events, String>,
    /// Each [`HISTORY`] metric id with its answer, in order.
    pub history: Vec<HistoryRead>,
}

/// A short description of a config-view disk.
pub fn describe_disk(disk: &serde_json::Value) -> String {
    let text = |key: &str| disk[key].as_str().unwrap_or("").to_owned();
    let mut parts = vec![match text("kind").as_str() {
        "iscsi" => format!("iSCSI {} ({})", text("id"), text("bus")),
        "file" => format!("file ({})", text("format")),
        other => other.to_owned(),
    }];
    parts.push(if disk["read_only"].as_bool().unwrap_or(false) {
        "read-only".into()
    } else {
        "read-write".into()
    });
    if let Some(index) = disk["boot_index"].as_u64() {
        parts.push(format!("boot {index}"));
    }
    parts.join(" · ")
}

/// Event payloads are shown as compact, bounded JSON.
pub fn event_data(event: &QrabEvent) -> String {
    if event.truncated {
        return "(payload too large; omitted)".into();
    }
    let mut data = match &event.data {
        serde_json::Value::Null => String::new(),
        value => value.to_string(),
    };
    if data == "{}" {
        data.clear();
    }
    if data.chars().count() > 240 {
        data = data.chars().take(240).collect::<String>() + "…";
    }
    data
}

#[cfg(test)]
mod tests {
    #[test]
    fn restart_prerequisites_report_only_known_agent_obstructions() {
        let mut system = super::System::default();
        assert_eq!(super::guest_restart_unavailable(&system), None);
        system.oem.qrab.guest_agent = Some(super::AgentStatus::default());
        assert!(
            super::guest_restart_unavailable(&system)
                .unwrap()
                .contains("connected")
        );
        let agent = system.oem.qrab.guest_agent.as_mut().unwrap();
        agent.enabled = true;
        agent.connected = true;
        agent.freeze_state = Some("Frozen".into());
        assert!(
            super::guest_restart_unavailable(&system)
                .unwrap()
                .contains("Thaw")
        );
        system.oem.qrab.guest_agent.as_mut().unwrap().freeze_state = Some("Thawed".into());
        assert_eq!(super::guest_restart_unavailable(&system), None);
        system.oem.qrab.guest_agent.as_mut().unwrap().freeze_state = None;
        assert_eq!(super::guest_restart_unavailable(&system), None);
    }
    use super::*;
    #[test]
    fn reset_types_round_trip_and_only_known_allowed_values_are_offered() {
        for reset in ResetType::ALL {
            assert_eq!(ResetType::parse(reset.name()), Some(reset));
            assert_eq!(
                serde_json::to_value(reset).unwrap(),
                serde_json::json!(reset.name())
            );
        }
        assert!(ResetType::Nmi.needs_confirmation() && !ResetType::Pause.needs_confirmation());
        assert!(ResetType::ForceOff.needs_confirmation() && !ResetType::On.needs_confirmation());
        let allowed: Vec<String> = ["On", "Nmi", "ForceOff", "Pause", "ForceOn"]
            .map(String::from)
            .to_vec();
        // On is sent by Power on, never offered as a reset button.
        assert_eq!(
            offered(&allowed),
            [ResetType::Pause, ResetType::Nmi, ResetType::ForceOff]
        );
        assert_eq!(ResetType::parse("On"), None);
        assert_eq!(ResetType::On.name(), "On");
    }
    #[test]
    fn redfish_shapes_parse_leniently() {
        let system: System = serde_json::from_value(serde_json::json!({
            "PowerState": "Paused", "Status": {"State": "Quiesced", "Health": "OK"},
            "Actions": {"#ComputerSystem.Reset": {"ResetType@Redfish.AllowableValues": ["Pause"]}},
            "Oem": {"Qrab": {"Configuration": {"name": "vm", "cpus": 2, "unknown": 1},
                "GuestAgent": {"enabled": true, "connected": false}}}
        }))
        .unwrap();
        assert_eq!(system.power_state.as_deref(), Some("Paused"));
        assert_eq!(system.actions.reset.allowable_values, ["Pause"]);
        assert_eq!(system.oem.qrab.configuration.unwrap().cpus, 2);
        // A viewer's agent: no output or error; a null freeze state when slow.
        let agent: AgentStatus = serde_json::from_value(serde_json::json!({
            "enabled": true, "connected": true, "FreezeState": null,
            "LastUpdate": {"State": "Succeeded", "RunId": 3, "ExitCode": 0,
                "HookInstalled": true, "Signal": null, "Unknown": 1}
        }))
        .unwrap();
        assert!(agent.freeze_state.is_none());
        let update = agent.last_update.unwrap();
        assert_eq!((update.state.as_str(), update.run_id), ("Succeeded", 3));
        assert!(update.output.is_none() && update.error.is_none());
        let offline: System = serde_json::from_value(serde_json::json!({
            "PowerState": null, "Status": {"State": "UnavailableOffline"}
        }))
        .unwrap();
        assert!(offline.power_state.is_none() && offline.oem.qrab.guest_agent.is_none());
        let entries = (1..=30)
            .map(|sequence| LogEntry {
                oem: LogEntryOem {
                    qrab: QrabEvent {
                        sequence,
                        ..Default::default()
                    },
                },
                ..Default::default()
            })
            .collect();
        let events = Events::newest(entries, None);
        assert_eq!(events.entries.len(), EVENTS_SHOWN);
        assert_eq!(events.entries[0].oem.qrab.sequence, 30);
        let event = QrabEvent {
            data: serde_json::json!({"x": "y".repeat(500)}),
            ..Default::default()
        };
        assert!(event_data(&event).chars().count() <= 241);
    }
}
