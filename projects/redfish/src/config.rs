use anyhow::{ensure, Context, Result};
use std::{collections::HashSet, net::SocketAddr, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    /// Unauthenticated Prometheus listener; serves only `/metrics`.
    pub metrics_listen: SocketAddr,
    /// Trusted ID-token sources, one per issuer, `REDFISH_OIDC_ISSUER` first.
    pub sources: Vec<TokenSource>,
    pub admin_subjects: HashSet<String>,
    pub viewer_subjects: HashSet<String>,
    pub admin_groups: HashSet<String>,
    pub viewer_groups: HashSet<String>,
    pub ca_file: Option<PathBuf>,
    pub local_accounts_file: Option<PathBuf>,
    pub public_discovery: bool,
}

/// An issuer and the audiences (OAuth2 client IDs) whose ID tokens it may
/// present. A trusted source grants no role by itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenSource {
    pub issuer: String,
    pub audiences: Vec<String>,
}

/// The primary pair followed by the comma-separated `issuer|audience` pairs
/// of `REDFISH_TRUSTED_TOKENS`, grouped by issuer in first-seen order.
/// Repeated pairs collapse; a shared issuer keeps each audience as its own pair.
///
/// ```
/// use qrab_redfish::config::{token_sources, TokenSource};
/// let sources = token_sources(
///     "https://idm.example/oauth2/openid/qrab-redfish",
///     "qrab-redfish",
///     "https://idm.example/oauth2/openid/qrab-bmc|qrab-bmc, https://idm.example/oauth2/openid/qrab-redfish|qrab-redfish",
/// )
/// .unwrap();
/// assert_eq!(sources, [
///     TokenSource {
///         issuer: "https://idm.example/oauth2/openid/qrab-redfish".into(),
///         audiences: vec!["qrab-redfish".into()],
///     },
///     TokenSource {
///         issuer: "https://idm.example/oauth2/openid/qrab-bmc".into(),
///         audiences: vec!["qrab-bmc".into()],
///     },
/// ]);
/// let shared = token_sources("https://idm.example", "a", "https://idm.example|b").unwrap();
/// assert_eq!(shared[0].audiences, ["a", "b"]);
/// assert!(token_sources("https://idm.example", "a", "https://idm.example").is_err());
/// assert!(token_sources("https://idm.example", "a", "http://idm.example|b").is_err());
/// assert!(token_sources("https://idm.example", "a", "https://idm.example|").is_err());
/// assert!(token_sources("https://idm.example", "", "").is_err());
/// ```
pub fn token_sources(issuer: &str, audience: &str, trusted: &str) -> Result<Vec<TokenSource>> {
    let mut pairs = vec![(
        "REDFISH_OIDC_ISSUER/REDFISH_OIDC_AUDIENCE",
        issuer.to_owned(),
        audience.to_owned(),
    )];
    for pair in trusted.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (issuer, audience) = pair
            .split_once('|')
            .context("REDFISH_TRUSTED_TOKENS entries must be issuer|audience")?;
        pairs.push((
            "REDFISH_TRUSTED_TOKENS",
            issuer.trim().to_owned(),
            audience.trim().to_owned(),
        ));
    }
    let mut sources: Vec<TokenSource> = Vec::new();
    for (name, issuer, audience) in pairs {
        ensure!(
            openidconnect::url::Url::parse(&issuer)
                .with_context(|| format!("{name} issuer"))?
                .scheme()
                == "https",
            "{name} issuers must use HTTPS"
        );
        ensure!(
            !audience.is_empty() && !audience.contains(char::is_whitespace),
            "{name} audiences must be non-empty client IDs"
        );
        match sources.iter_mut().find(|s| s.issuer == issuer) {
            Some(source) if source.audiences.contains(&audience) => {}
            Some(source) => source.audiences.push(audience),
            None => sources.push(TokenSource {
                issuer,
                audiences: vec![audience],
            }),
        }
    }
    Ok(sources)
}

pub fn subjects(name: &str, raw: &str) -> Result<HashSet<String>> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            uuid::Uuid::parse_str(s)
                .map(|id| id.to_string())
                .with_context(|| format!("{name} must contain UUIDs"))
        })
        .collect()
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let required =
            |name: &str| std::env::var(name).with_context(|| format!("{name} is required"));
        let optional = |name: &str| std::env::var(name).unwrap_or_default();
        let sources = token_sources(
            &required("REDFISH_OIDC_ISSUER")?,
            &required("REDFISH_OIDC_AUDIENCE")?,
            &optional("REDFISH_TRUSTED_TOKENS"),
        )?;
        let admin_subjects = subjects(
            "REDFISH_ADMIN_SUBJECTS",
            &optional("REDFISH_ADMIN_SUBJECTS"),
        )?;
        let viewer_subjects = subjects(
            "REDFISH_VIEWER_SUBJECTS",
            &optional("REDFISH_VIEWER_SUBJECTS"),
        )?;
        let admin_groups = subjects("REDFISH_ADMIN_GROUPS", &optional("REDFISH_ADMIN_GROUPS"))?;
        let viewer_groups = subjects("REDFISH_VIEWER_GROUPS", &optional("REDFISH_VIEWER_GROUPS"))?;
        ensure!(
            [&admin_subjects, &viewer_subjects, &admin_groups, &viewer_groups]
                .iter()
                .any(|set| !set.is_empty()),
            "REDFISH_ADMIN_SUBJECTS, REDFISH_VIEWER_SUBJECTS, REDFISH_ADMIN_GROUPS or REDFISH_VIEWER_GROUPS must name subject or group UUIDs"
        );
        ensure!(
            admin_subjects.is_disjoint(&viewer_subjects),
            "a subject cannot be both a Redfish admin and a viewer"
        );
        ensure!(
            admin_groups.is_disjoint(&viewer_groups),
            "a group cannot grant both Redfish admin and viewer access"
        );
        let listen: SocketAddr = std::env::var("REDFISH_LISTEN")
            .unwrap_or_else(|_| "0.0.0.0:8081".into())
            .parse()
            .context("REDFISH_LISTEN must be a socket address")?;
        let metrics_listen: SocketAddr = std::env::var("REDFISH_METRICS_LISTEN")
            .unwrap_or_else(|_| "0.0.0.0:9181".into())
            .parse()
            .context("REDFISH_METRICS_LISTEN must be a socket address")?;
        ensure!(
            listen.port() != metrics_listen.port(),
            "REDFISH_METRICS_LISTEN must use a different port from REDFISH_LISTEN"
        );
        Ok(Self {
            listen,
            metrics_listen,
            sources,
            admin_subjects,
            viewer_subjects,
            admin_groups,
            viewer_groups,
            ca_file: std::env::var_os("REDFISH_CA_FILE").map(PathBuf::from),
            local_accounts_file: std::env::var_os("REDFISH_LOCAL_ACCOUNTS_FILE").map(PathBuf::from),
            public_discovery: std::env::var("REDFISH_PUBLIC_DISCOVERY")
                .unwrap_or_else(|_| "false".into())
                .parse()
                .context("REDFISH_PUBLIC_DISCOVERY must be true or false")?,
        })
    }
}
