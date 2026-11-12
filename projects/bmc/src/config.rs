use anyhow::{ensure, Context, Result};
use openidconnect::url::Url;
use std::{collections::HashSet, net::SocketAddr, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub origin: String,
    pub issuer: String,
    pub client_id: String,
    /// Full access: sensitive configuration, power actions and the console.
    pub admin_subjects: HashSet<String>,
    /// Read-only access: status, events and redacted configuration.
    pub viewer_subjects: HashSet<String>,
    /// Kanidm group UUIDs whose members get admin access.
    pub admin_groups: HashSet<String>,
    /// Kanidm group UUIDs whose members get viewer access.
    pub viewer_groups: HashSet<String>,
    pub assets_dir: PathBuf,
    pub ca_file: Option<PathBuf>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let required =
            |name: &str| std::env::var(name).with_context(|| format!("{name} is required"));
        let listen = std::env::var("BMC_LISTEN")
            .unwrap_or_else(|_| "0.0.0.0:8080".into())
            .parse()?;
        let origin = public_origin(
            &required("BMC_PUBLIC_URL")?,
            std::env::var("BMC_ALLOW_LOOPBACK_HTTP").as_deref() == Ok("true"),
            &listen,
        )?;
        let uuids = |name: &str| -> Result<HashSet<String>> {
            std::env::var(name)
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| {
                    uuid::Uuid::parse_str(s)
                        .map(|id| id.to_string())
                        .with_context(|| format!("{name} must contain UUIDs"))
                })
                .collect()
        };
        // BMC_ALLOWED_SUBJECTS predates roles and keeps granting full access.
        let mut admin_subjects = uuids("BMC_ALLOWED_SUBJECTS")?;
        admin_subjects.extend(uuids("BMC_ADMIN_SUBJECTS")?);
        let viewer_subjects = uuids("BMC_VIEWER_SUBJECTS")?;
        let admin_groups = uuids("BMC_ADMIN_GROUPS")?;
        let viewer_groups = uuids("BMC_VIEWER_GROUPS")?;
        ensure!(
            [&admin_subjects, &viewer_subjects, &admin_groups, &viewer_groups]
                .iter()
                .any(|set| !set.is_empty()),
            "BMC_ALLOWED_SUBJECTS, BMC_ADMIN_SUBJECTS, BMC_VIEWER_SUBJECTS, BMC_ADMIN_GROUPS or BMC_VIEWER_GROUPS must name authorized operator or group UUIDs"
        );
        ensure!(
            admin_subjects.is_disjoint(&viewer_subjects),
            "a subject cannot be both a BMC admin and a viewer"
        );
        ensure!(
            admin_groups.is_disjoint(&viewer_groups),
            "a group cannot grant both BMC admin and viewer access"
        );
        let issuer = required("BMC_OIDC_ISSUER")?;
        ensure!(
            Url::parse(&issuer)?.scheme() == "https",
            "OIDC issuer must use HTTPS"
        );
        let client_id = required("BMC_OIDC_CLIENT_ID")?;
        ensure!(!client_id.is_empty(), "OIDC client ID must not be empty");
        Ok(Self {
            listen,
            origin,
            issuer,
            client_id,
            admin_subjects,
            viewer_subjects,
            admin_groups,
            viewer_groups,
            assets_dir: std::env::var_os("BMC_ASSETS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")),
            ca_file: std::env::var_os("BMC_CA_FILE").map(PathBuf::from),
        })
    }
}

pub fn public_origin(raw: &str, allow_http: bool, listen: &SocketAddr) -> Result<String> {
    let url = Url::parse(raw)?;
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/"
            && url.host_str().is_some(),
        "BMC_PUBLIC_URL must be an origin without credentials, path, query or fragment"
    );
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    ensure!(url.scheme() == "https" || (url.scheme() == "http" && allow_http && loopback && listen.ip().is_loopback()), "BMC_PUBLIC_URL must use HTTPS (explicit HTTP development requires a loopback origin and listener)");
    Ok(url.origin().ascii_serialization())
}
