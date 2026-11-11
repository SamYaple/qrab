//! Client for the towl storage daemon's `vm-disk-*` verbs.
//!
//! The production client follows the zfs-block-csi identity path: the
//! operator's SPIFFE X509-SVID authenticates to the Kanidm exchange shim over
//! mTLS, the resulting short-lived RFC 8693 bearer calls the host's TLS 1.3
//! storage ingress with a pinned CA, and the root daemon rechecks signature,
//! subject, actor and scope. There is no static token.
use crate::api::BaseImage;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

pub const EXCHANGE_SERVER: &str = "spiffe://yapplesauce.com/kanidm/exchange-shim";
pub const EXCHANGE_POLICY: &str = "qrab-vm-disk-operator";
const RESPONSE_LIMIT: usize = 256 << 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiskPhase {
    Provisioned,
    Preparing,
    Active,
    Fencing,
    Fenced,
    Retired,
}
impl DiskPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Provisioned => "Provisioned",
            Self::Preparing => "Preparing",
            Self::Active => "Active",
            Self::Fencing => "Fencing",
            Self::Fenced => "Fenced",
            Self::Retired => "Retired",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Origin {
    pub image: String,
    pub generation: u32,
}

/// The wrapper's public report. It never carries a credential.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiskReport {
    pub vm_uid: String,
    pub disk: String,
    #[serde(default)]
    pub origin: Option<Origin>,
    pub capacity_bytes: u64,
    pub target_iqn: String,
    pub initiator_iqn: String,
    pub portal: String,
    pub lun: u32,
    pub chap_username: String,
    pub generation: u64,
    pub phase: DiskPhase,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    /// A typed refusal from the daemon or the root wrapper.
    #[error("storage service refused the request: {0}")]
    Refused(String),
    #[error("storage service unavailable: {0}")]
    Unavailable(String),
}
impl StorageError {
    pub fn code(&self) -> &str {
        match self {
            Self::Refused(code) => code,
            Self::Unavailable(_) => "unavailable",
        }
    }
}

/// A CHAP secret. Debug output is redacted; it is sent only in a request body.
#[derive(Clone, PartialEq, Eq)]
pub struct ChapSecret(pub String);
impl std::fmt::Debug for ChapSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ChapSecret(<redacted>)")
    }
}

pub type StorageResult<T> = std::result::Result<T, StorageError>;

#[async_trait::async_trait]
pub trait StorageApi: Send + Sync {
    /// `Ok(None)` when the storage service has no such disk.
    async fn status(&self, vm_uid: &str, disk: &str) -> StorageResult<Option<DiskReport>>;
    async fn create(&self, vm_uid: &str, disk: &str, capacity: u64) -> StorageResult<DiskReport>;
    async fn clone_image(
        &self,
        vm_uid: &str,
        disk: &str,
        image: &BaseImage,
        capacity: u64,
    ) -> StorageResult<DiskReport>;
    async fn expand(&self, vm_uid: &str, disk: &str, capacity: u64) -> StorageResult<DiskReport>;
    async fn export(
        &self,
        vm_uid: &str,
        disk: &str,
        generation: u64,
        secret: &ChapSecret,
    ) -> StorageResult<DiskReport>;
    async fn fence(&self, vm_uid: &str, disk: &str) -> StorageResult<DiskReport>;
}

pub type SharedStorage = Arc<dyn StorageApi>;

pub fn origin(raw: &str) -> anyhow::Result<url::Url> {
    let parsed = url::Url::parse(raw)?;
    anyhow::ensure!(
        raw.starts_with("https://")
            && !raw[8..].contains('/')
            && parsed.host_str().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.query().is_none()
            && parsed.fragment().is_none(),
        "storage endpoints must be bare HTTPS origins"
    );
    Ok(parsed)
}

fn tls13_ring() -> Arc<rustls::crypto::CryptoProvider> {
    let mut provider = rustls::crypto::ring::default_provider();
    provider
        .cipher_suites
        .retain(|suite| suite.version().version == rustls::ProtocolVersion::TLSv1_3);
    Arc::new(provider)
}

fn http_client(tls: rustls::ClientConfig, timeout: Duration) -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .https_only(true)
        .http1_only()
        .pool_max_idle_per_host(0)
        .timeout(timeout)
        .tls_backend_preconfigured(tls)
        .build()?)
}

/// Storage ingress client trusting only the configured public CA.
pub fn storage_client(mut ca: &[u8]) -> anyhow::Result<reqwest::Client> {
    let mut roots = rustls::RootCertStore::empty();
    let certificates = rustls_pemfile::certs(&mut ca).collect::<std::io::Result<Vec<_>>>()?;
    anyhow::ensure!(
        !certificates.is_empty(),
        "storage CA contains no certificate"
    );
    for certificate in certificates {
        roots.add(certificate)?;
    }
    let tls = rustls::ClientConfig::builder_with_provider(tls13_ring())
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_root_certificates(roots)
        .with_no_client_auth();
    http_client(tls, Duration::from_secs(75))
}

async fn bounded_json(mut response: reqwest::Response) -> anyhow::Result<Value> {
    anyhow::ensure!(
        response
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| h.to_ascii_lowercase().starts_with("application/json")),
        "non-JSON response"
    );
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            chunk.len() <= RESPONSE_LIMIT.saturating_sub(body.len()),
            "response exceeds its bound"
        );
        body.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&body)?)
}

/// Reads the last `{"error": code}` line the root wrapper wrote to stderr.
pub fn wrapper_refusal(stderr: &str) -> Option<String> {
    stderr.lines().rev().find_map(|line| {
        serde_json::from_str::<Value>(line.trim())
            .ok()?
            .get("error")?
            .as_str()
            .filter(|code| {
                !code.is_empty()
                    && code.len() <= 64
                    && code.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
            })
            .map(str::to_owned)
    })
}

/// Interprets a daemon `/v1/execute` response for `operation`/`request_id`.
pub fn parse_execution(
    status: u16,
    body: &Value,
    operation: &str,
    request_id: &str,
) -> StorageResult<Value> {
    if status == 401 || status == 403 {
        return Err(StorageError::Unavailable(format!(
            "storage authorization rejected ({})",
            body["error"].as_str().unwrap_or("forbidden")
        )));
    }
    if status == 422 {
        if let Some(code) = body["result"]["stderr"].as_str().and_then(wrapper_refusal) {
            return Err(StorageError::Refused(code));
        }
    }
    if status != 200 {
        return Err(match body["error"].as_str() {
            Some(code) if status == 400 => StorageError::Refused(code.to_owned()),
            Some(code) => StorageError::Unavailable(code.to_owned()),
            None => StorageError::Unavailable(format!("HTTP {status}")),
        });
    }
    if body["ok"] != true
        || body["result"]["success"] != true
        || body["request_id"] != request_id
        || body["operation"] != operation
    {
        return Err(StorageError::Unavailable("invalid storage result".into()));
    }
    Ok(body["result"]["output"].clone())
}

pub fn parse_report(output: &Value, vm_uid: &str, disk: &str) -> StorageResult<DiskReport> {
    let report: DiskReport = serde_json::from_value(output["disk"].clone())
        .map_err(|_| StorageError::Unavailable("invalid disk report".into()))?;
    if report.vm_uid != vm_uid || report.disk != disk || report.lun != 0 {
        return Err(StorageError::Unavailable(
            "storage report names another disk".into(),
        ));
    }
    Ok(report)
}

pub struct HttpStorage {
    exchange: reqwest::Client,
    storage: reqwest::Client,
    exchange_origin: String,
    storage_origin: String,
    source: spiffe::X509Source,
}

#[derive(Deserialize)]
struct Token {
    access_token: String,
    token_type: String,
    expires_in: i64,
}

impl HttpStorage {
    pub async fn new(
        workload_api: &str,
        exchange_origin: &str,
        storage_origin: &str,
        ca: &[u8],
    ) -> anyhow::Result<Self> {
        origin(exchange_origin)?;
        origin(storage_origin)?;
        anyhow::ensure!(
            workload_api.starts_with("unix:"),
            "the SPIFFE Workload API must be a Unix socket"
        );
        let storage = storage_client(ca)?;
        let source = spiffe::X509Source::builder()
            .endpoint(workload_api)
            .initial_sync_timeout(Duration::from_secs(30))
            .shutdown_timeout(Some(Duration::from_secs(5)))
            .build()
            .await
            .map_err(|_| anyhow::anyhow!("SPIFFE Workload API identity unavailable"))?;
        let id: spiffe::SpiffeId = EXCHANGE_SERVER.parse()?;
        let tls = spiffe_rustls::mtls_client(source.clone())
            .authorize(spiffe_rustls::exact([EXCHANGE_SERVER])?)
            .trust_domain_policy(spiffe_rustls::LocalOnly(id.trust_domain().clone()))
            .with_alpn_protocols([b"http/1.1"])
            .build()?;
        Ok(Self {
            exchange: http_client(tls, Duration::from_secs(20))?,
            storage,
            exchange_origin: exchange_origin.into(),
            storage_origin: storage_origin.into(),
            source,
        })
    }

    /// One fresh short-lived bearer per RPC; never cached or logged.
    async fn token(&self) -> StorageResult<String> {
        if self.source.svid().is_err() {
            return Err(StorageError::Unavailable(
                "SPIFFE identity unavailable".into(),
            ));
        }
        let unavailable = |_| StorageError::Unavailable("identity exchange unavailable".into());
        let response = self
            .exchange
            .post(format!("{}/exchange", self.exchange_origin))
            .json(&json!({"policy": EXCHANGE_POLICY}))
            .send()
            .await
            .map_err(unavailable)?;
        let status = response.status().as_u16();
        let token: Token = bounded_json(response)
            .await
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .ok_or_else(|| StorageError::Unavailable("identity exchange rejected".into()))?;
        if status != 200
            || token.token_type != "Bearer"
            || !(10..=3600).contains(&token.expires_in)
            || token.access_token.is_empty()
            || token.access_token.len() > 8192
            || token.access_token.contains([' ', '\r', '\n', '\t'])
        {
            return Err(StorageError::Unavailable(
                "identity exchange rejected".into(),
            ));
        }
        Ok(token.access_token)
    }

    async fn rpc(&self, operation: Value, secret: Option<&ChapSecret>) -> StorageResult<Value> {
        let name = operation["name"].as_str().unwrap_or_default().to_owned();
        let token = self.token().await?;
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce)
            .map_err(|_| StorageError::Unavailable("request identity unavailable".into()))?;
        let request_id: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        let mut body = json!({"request_id": request_id, "operation": operation});
        if let Some(secret) = secret {
            body["credential"] = json!({"chap_password": secret.0});
        }
        let response = self
            .storage
            .post(format!("{}/v1/execute", self.storage_origin))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|_| StorageError::Unavailable("storage API unavailable".into()))?;
        let status = response.status().as_u16();
        let value = bounded_json(response)
            .await
            .map_err(|_| StorageError::Unavailable("invalid storage response".into()))?;
        parse_execution(status, &value, &name, &request_id)
    }

    async fn disk(
        &self,
        mut operation: Value,
        vm_uid: &str,
        disk: &str,
        secret: Option<&ChapSecret>,
    ) -> StorageResult<DiskReport> {
        operation["vm_uid"] = json!(vm_uid);
        operation["disk"] = json!(disk);
        let output = self.rpc(operation, secret).await?;
        parse_report(&output, vm_uid, disk)
    }
}

#[async_trait::async_trait]
impl StorageApi for HttpStorage {
    async fn status(&self, vm_uid: &str, disk: &str) -> StorageResult<Option<DiskReport>> {
        match self
            .disk(json!({"name":"vm-disk-status"}), vm_uid, disk, None)
            .await
        {
            Ok(report) => Ok(Some(report)),
            Err(StorageError::Refused(code)) if code == "disk-not-found" => Ok(None),
            Err(error) => Err(error),
        }
    }
    async fn create(&self, vm_uid: &str, disk: &str, capacity: u64) -> StorageResult<DiskReport> {
        self.disk(
            json!({"name":"vm-disk-create","capacity_bytes":capacity}),
            vm_uid,
            disk,
            None,
        )
        .await
    }
    async fn clone_image(
        &self,
        vm_uid: &str,
        disk: &str,
        image: &BaseImage,
        capacity: u64,
    ) -> StorageResult<DiskReport> {
        self.disk(
            json!({"name":"vm-disk-clone","image":image.name,"image_generation":image.generation,"capacity_bytes":capacity}),
            vm_uid,
            disk,
            None,
        )
        .await
    }
    async fn expand(&self, vm_uid: &str, disk: &str, capacity: u64) -> StorageResult<DiskReport> {
        self.disk(
            json!({"name":"vm-disk-expand","capacity_bytes":capacity}),
            vm_uid,
            disk,
            None,
        )
        .await
    }
    async fn export(
        &self,
        vm_uid: &str,
        disk: &str,
        generation: u64,
        secret: &ChapSecret,
    ) -> StorageResult<DiskReport> {
        self.disk(
            json!({"name":"vm-disk-export","generation":generation}),
            vm_uid,
            disk,
            Some(secret),
        )
        .await
    }
    async fn fence(&self, vm_uid: &str, disk: &str) -> StorageResult<DiskReport> {
        self.disk(json!({"name":"vm-disk-fence"}), vm_uid, disk, None)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UID: &str = "e309f751-5dda-4271-a0f4-4454c9527a59";

    fn report() -> Value {
        json!({"disk":{"schema":2,"vm_uid":UID,"disk":"root","capacity_bytes":67108864,"dataset":"tank/qrab-iscsi/disk-x","target_iqn":"iqn.2026-11.com.yapplesauce:qrab.disk.x.root","initiator_iqn":"iqn.2026-11.com.yapplesauce:qrab.vm.x","portal":"10.203.0.1:3260","lun":0,"chap_username":"qrab-x-root","generation":1,"phase":"active","parent_quota_bytes":1,"root_state":"/var/lib/qrab-iscsi-target","dedicated_hba":"iblock_47"}})
    }

    #[test]
    fn daemon_envelopes_map_to_typed_refusals_and_reports() {
        let ok = json!({"ok":true,"request_id":"r1","operation":"vm-disk-status","result":{"success":true,"exit_code":0,"output":report(),"stderr":""}});
        let output = parse_execution(200, &ok, "vm-disk-status", "r1").unwrap();
        let parsed = parse_report(&output, UID, "root").unwrap();
        assert_eq!(parsed.phase, DiskPhase::Active);
        assert_eq!(parsed.origin, None);
        assert!(parse_report(&output, UID, "other").is_err());
        assert!(parse_execution(200, &ok, "vm-disk-status", "r2").is_err());
        assert!(parse_execution(200, &ok, "vm-disk-fence", "r1").is_err());
        let refused = json!({"ok":false,"request_id":"r1","operation":"vm-disk-status","result":{"success":false,"exit_code":1,"output":null,"stderr":"{\"error\":\"disk-not-found\",\"message\":\"disk-not-found\"}"}});
        assert_eq!(
            parse_execution(422, &refused, "vm-disk-status", "r1"),
            Err(StorageError::Refused("disk-not-found".into()))
        );
        assert_eq!(
            parse_execution(
                400,
                &json!({"ok":false,"error":"invalid-credential"}),
                "vm-disk-export",
                "r1"
            ),
            Err(StorageError::Refused("invalid-credential".into()))
        );
        assert!(matches!(
            parse_execution(
                503,
                &json!({"ok":false,"error":"vm-disk-wrapper-disabled"}),
                "vm-disk-status",
                "r1"
            ),
            Err(StorageError::Unavailable(_))
        ));
        assert!(matches!(
            parse_execution(
                403,
                &json!({"ok":false,"error":"forbidden-operation"}),
                "vm-disk-status",
                "r1"
            ),
            Err(StorageError::Unavailable(_))
        ));
    }

    #[test]
    fn wrapper_refusal_codes_are_bounded_tokens() {
        assert_eq!(
            wrapper_refusal("noise\n{\"error\":\"fence-required\"}\n").as_deref(),
            Some("fence-required")
        );
        assert_eq!(wrapper_refusal("{\"error\":\"Bad Code!\"}"), None);
        assert_eq!(wrapper_refusal("plain text"), None);
    }

    #[test]
    fn endpoints_are_bare_https_origins_and_secrets_are_redacted() {
        assert!(origin("https://10.203.0.1:9477").is_ok());
        for bad in [
            "http://10.203.0.1:9477",
            "https://10.203.0.1:9477/v1",
            "https://user@10.203.0.1:9477",
            "https://10.203.0.1:9477?x=1",
        ] {
            assert!(origin(bad).is_err(), "{bad}");
        }
        let secret = ChapSecret("0123456789abcdef0123456789abcdef".into());
        assert!(!format!("{secret:?}").contains("0123"));
    }
}
