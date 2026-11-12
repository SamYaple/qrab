//! Server-side calls to one VM's BMC at `http://<pod IP>:8080`, carrying the
//! signed-in user's ID token as `Authorization: Bearer`. The BMC enforces
//! its own admin/viewer roles on every request.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

pub const STATUS: &str = "/api/status";
pub const AGENT: &str = "/api/agent";
pub const EVENTS: &str = "/api/events?limit=20";
pub const CONFIG: &str = "/api/config";
pub const ACTION: &str = "/api/action";
pub const CONSOLE: &str = "/api/console";

/// The BMC's power actions, sent as a JSON string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Pause,
    Resume,
    Reset,
    Powerdown,
}

impl Action {
    pub const ALL: [Self; 4] = [Self::Pause, Self::Resume, Self::Reset, Self::Powerdown];
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.key() == value)
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Reset => "reset",
            Self::Powerdown => "powerdown",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Pause => "Pause",
            Self::Resume => "Resume",
            Self::Reset => "Reset",
            Self::Powerdown => "Shut down",
        }
    }
    /// Disruptive actions need an explicit confirmation.
    pub fn needs_confirmation(self) -> bool {
        matches!(self, Self::Reset | Self::Powerdown)
    }
}

/// Largest BMC response body the dashboard reads.
pub const MAX_BODY: usize = 1024 * 1024;

/// Reads at most `limit` bytes; a longer body is refused, not truncated.
async fn read_capped(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, BmcError> {
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(BmcError::Invalid);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| BmcError::Unreachable)? {
        if body.len() + chunk.len() > limit {
            return Err(BmcError::Invalid);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BmcError {
    Status(u16),
    Unreachable,
    Invalid,
}

impl std::fmt::Display for BmcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Status(401) => write!(f, "the BMC did not accept your sign-in (HTTP 401)"),
            Self::Status(403) => write!(f, "the BMC refused this for your role (HTTP 403)"),
            Self::Status(code) => write!(f, "the BMC answered HTTP {code}"),
            Self::Unreachable => write!(f, "the BMC is unreachable"),
            Self::Invalid => write!(f, "the BMC sent an unexpected response"),
        }
    }
}

#[derive(Clone)]
pub struct Bmc {
    http: reqwest::Client,
    port: u16,
}

impl Bmc {
    pub fn new(port: u16) -> anyhow::Result<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(8))
                .build()?,
            port,
        })
    }

    pub fn address(&self, ip: IpAddr) -> SocketAddr {
        SocketAddr::new(ip, self.port)
    }

    fn url(&self, ip: IpAddr, path: &str) -> String {
        format!("http://{}{path}", self.address(ip))
    }

    pub async fn get<T: DeserializeOwned>(
        &self,
        ip: IpAddr,
        path: &'static str,
        token: &str,
    ) -> Result<T, BmcError> {
        let response = self
            .http
            .get(self.url(ip, path))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| BmcError::Unreachable)?;
        if !response.status().is_success() {
            return Err(BmcError::Status(response.status().as_u16()));
        }
        serde_json::from_slice(&read_capped(response, MAX_BODY).await?)
            .map_err(|_| BmcError::Invalid)
    }

    pub async fn action(&self, ip: IpAddr, token: &str, action: Action) -> Result<(), BmcError> {
        let response = self
            .http
            .post(self.url(ip, ACTION))
            .bearer_auth(token)
            .json(&action)
            .send()
            .await
            .map_err(|_| BmcError::Unreachable)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(BmcError::Status(response.status().as_u16()))
        }
    }

    /// The upstream console WebSocket request. No Origin or cookie is sent.
    pub fn console_request(
        &self,
        ip: IpAddr,
        token: &str,
    ) -> anyhow::Result<tokio_tungstenite::tungstenite::handshake::client::Request> {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut request = format!("ws://{}{CONSOLE}", self.address(ip)).into_client_request()?;
        let mut bearer: axum::http::HeaderValue = format!("Bearer {token}").parse()?;
        bearer.set_sensitive(true);
        request.headers_mut().insert("authorization", bearer);
        Ok(request)
    }

    /// The four read panels, fetched concurrently.
    pub async fn panel(&self, ip: IpAddr, token: &str) -> Panel {
        let (status, agent, events, config) = tokio::join!(
            self.get(ip, STATUS, token),
            self.get(ip, AGENT, token),
            self.get(ip, EVENTS, token),
            self.get(ip, CONFIG, token),
        );
        Panel {
            status: status.map_err(|e: BmcError| e.to_string()),
            agent: agent.map_err(|e: BmcError| e.to_string()),
            events: events.map_err(|e: BmcError| e.to_string()),
            config: config.map_err(|e: BmcError| e.to_string()),
        }
    }
}

/// Lenient copies of the BMC's JSON. Every string here may be guest- or
/// VM-controlled; the UI renders them only as escaped text.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Status {
    pub phase: String,
    pub ready: bool,
    pub run_state: Option<String>,
    pub qemu_version: Option<Vec<u32>>,
    pub exit_code: Option<i32>,
    pub detail: Option<String>,
    pub observed_unix_ms: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentStatus {
    pub enabled: bool,
    pub connected: bool,
    pub version: Option<String>,
    pub connected_since_unix_ms: Option<u64>,
    pub last_seen_unix_ms: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct UpdateStatus {
    pub state: String,
    pub run_id: u64,
    pub exit_code: Option<i32>,
    pub hook_installed: bool,
    pub output: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Agent {
    pub status: AgentStatus,
    pub freeze: Option<String>,
    pub update: Option<UpdateStatus>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Event {
    pub seq: u64,
    pub received_unix_ms: u64,
    pub event: String,
    pub data: serde_json::Value,
    pub truncated: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Events {
    pub events: Vec<Event>,
    pub latest: u64,
    pub missed: u64,
    pub qmp_dropped: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Hotplug {
    pub max_cpus: u32,
    pub max_memory_mib: u64,
    pub phase: Option<String>,
    pub last_error: Option<String>,
}

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

#[derive(Debug, Clone)]
pub struct Panel {
    pub status: Result<Status, String>,
    pub agent: Result<Agent, String>,
    pub events: Result<Events, String>,
    pub config: Result<Config, String>,
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
pub fn event_data(event: &Event) -> String {
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
    use super::*;
    #[test]
    fn actions_serialize_as_the_bmc_expects() {
        assert_eq!(
            serde_json::to_string(&Action::Powerdown).unwrap(),
            "\"powerdown\""
        );
        for action in Action::ALL {
            assert_eq!(Action::parse(action.key()), Some(action));
        }
        assert_eq!(Action::parse("raw-qmp"), None);
        assert!(Action::Reset.needs_confirmation() && !Action::Pause.needs_confirmation());
    }
    #[test]
    fn bmc_shapes_parse_leniently() {
        let agent: Agent = serde_json::from_value(serde_json::json!({
            "status": {"enabled": true, "connected": true, "version": "0.1.0"},
            "freeze": "thawed",
            "update": {"state": "idle", "run-id": 2, "hook-installed": true, "output": "", "args": []}
        }))
        .unwrap();
        assert_eq!(agent.update.unwrap().run_id, 2);
        let status: Status = serde_json::from_value(serde_json::json!({
            "phase": "running", "ready": true, "qemu_version": [11, 1, 3], "manager_pid": 1
        }))
        .unwrap();
        assert_eq!(status.qemu_version, Some(vec![11, 1, 3]));
        let event = Event {
            data: serde_json::json!({"x": "y".repeat(500)}),
            ..Default::default()
        };
        assert!(event_data(&event).chars().count() <= 241);
    }
}
