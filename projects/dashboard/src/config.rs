//! Environment configuration. Credentials come from files and are never logged.
use anyhow::{Context, Result, ensure};
use std::{net::SocketAddr, path::PathBuf};

pub struct Config {
    pub origin: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub launcher_group: String,
    pub vm_users_group: String,
    /// A group UUID, or a Kanidm group name resolved to its UUID at startup.
    pub image_admin_group: String,
    pub kanidm_url: String,
    pub kanidm_token: String,
    pub namespace: String,
    pub storage_node: Option<String>,
    pub listen: SocketAddr,
    pub ca: Option<Vec<u8>>,
    pub allow_loopback_http: bool,
    pub novnc_dir: PathBuf,
}

fn required(key: &str) -> Result<String> {
    let value = std::env::var(key).with_context(|| format!("{key} is required"))?;
    ensure!(!value.trim().is_empty(), "{key} must not be empty");
    Ok(value.trim().into())
}

/// A credential file's first line, which must be non-empty.
pub fn secret_file(key: &str) -> Result<String> {
    let path = required(key)?;
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {key}"))?;
    let value = raw.lines().next().unwrap_or("").trim().to_owned();
    ensure!(!value.is_empty(), "{key} names an empty file");
    Ok(value)
}

/// A canonical lowercase hyphenated UUID.
pub fn uuid(raw: &str) -> Option<String> {
    uuid::Uuid::parse_str(raw.trim())
        .ok()
        .map(|id| id.to_string())
}

/// A Kubernetes DNS label.
pub fn dns_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

/// The default image-admin group, by name.
pub const IMAGE_ADMIN_GROUP: &str = "qrab_image_admins";

/// `DASHBOARD_IMAGE_ADMIN_GROUP`: a group UUID (canonicalized), or a Kanidm
/// group name to resolve at startup; unset or empty means
/// [`IMAGE_ADMIN_GROUP`].
///
/// ```
/// use qrab_dashboard::config::image_admin_group;
/// assert_eq!(image_admin_group(None).unwrap(), "qrab_image_admins");
/// assert_eq!(image_admin_group(Some(" ")).unwrap(), "qrab_image_admins");
/// assert_eq!(
///     image_admin_group(Some("7617E8D9-74CE-4525-A70B-80235206B06A")).unwrap(),
///     "7617e8d9-74ce-4525-a70b-80235206b06a"
/// );
/// assert!(image_admin_group(Some("Image Admins")).is_err());
/// ```
pub fn image_admin_group(raw: Option<&str>) -> Result<String> {
    let raw = raw
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(IMAGE_ADMIN_GROUP);
    if let Some(id) = uuid(raw) {
        return Ok(id);
    }
    ensure!(
        crate::kanidm::group_name(raw),
        "DASHBOARD_IMAGE_ADMIN_GROUP must be a group UUID or a Kanidm group name"
    );
    Ok(raw.into())
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let allow_loopback_http =
            std::env::var("DASHBOARD_ALLOW_LOOPBACK_HTTP").as_deref() == Ok("true");
        let origin = crate::origin(&required("DASHBOARD_PUBLIC_URL")?, allow_loopback_http)?;
        let group = |key: &str| -> Result<String> {
            uuid(&required(key)?).with_context(|| format!("{key} must be a group UUID"))
        };
        let namespace = std::env::var("DASHBOARD_NAMESPACE").unwrap_or_else(|_| "qrab".into());
        ensure!(
            dns_label(&namespace),
            "DASHBOARD_NAMESPACE must be a DNS label"
        );
        let storage_node = std::env::var("DASHBOARD_STORAGE_NODE")
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        if let Some(node) = &storage_node {
            ensure!(
                node.len() <= 253 && node.split('.').all(dns_label),
                "DASHBOARD_STORAGE_NODE must be a node name"
            );
        }
        Ok(Self {
            origin,
            issuer: required("DASHBOARD_OIDC_ISSUER")?,
            client_id: required("DASHBOARD_OIDC_CLIENT_ID")?,
            client_secret: secret_file("DASHBOARD_OIDC_CLIENT_SECRET_FILE")?,
            launcher_group: group("DASHBOARD_LAUNCHER_GROUP")?,
            vm_users_group: group("DASHBOARD_VM_USERS_GROUP")?,
            image_admin_group: image_admin_group(
                std::env::var("DASHBOARD_IMAGE_ADMIN_GROUP").ok().as_deref(),
            )?,
            kanidm_url: required("DASHBOARD_KANIDM_URL")?,
            kanidm_token: secret_file("DASHBOARD_KANIDM_TOKEN_FILE")?,
            namespace,
            storage_node,
            listen: std::env::var("DASHBOARD_LISTEN")
                .unwrap_or_else(|_| "127.0.0.1:8080".into())
                .parse()
                .context("DASHBOARD_LISTEN must be a socket address")?,
            ca: std::env::var_os("DASHBOARD_CA_FILE")
                .map(std::fs::read)
                .transpose()
                .context("read DASHBOARD_CA_FILE")?,
            allow_loopback_http,
            novnc_dir: std::env::var_os("DASHBOARD_NOVNC_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/novnc")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_and_uuids_are_canonical() {
        assert!(dns_label("vm-1"));
        for bad in ["", "-a", "a-", "A", "a_b", "a.b", &"a".repeat(64)] {
            assert!(!dns_label(bad), "{bad}");
        }
        assert_eq!(
            uuid("7617E8D9-74CE-4525-A70B-80235206B06A").as_deref(),
            Some("7617e8d9-74ce-4525-a70b-80235206b06a")
        );
        assert!(uuid("qrab_vm_launchers").is_none());
    }
}
