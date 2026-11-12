use anyhow::{ensure, Context, Result};
use std::{collections::HashSet, net::SocketAddr, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub issuer: String,
    pub audience: String,
    pub admin_subjects: HashSet<String>,
    pub viewer_subjects: HashSet<String>,
    pub admin_groups: HashSet<String>,
    pub viewer_groups: HashSet<String>,
    pub ca_file: Option<PathBuf>,
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
        let issuer = required("REDFISH_OIDC_ISSUER")?;
        ensure!(
            openidconnect::url::Url::parse(&issuer)?.scheme() == "https",
            "REDFISH_OIDC_ISSUER must use HTTPS"
        );
        let audience = required("REDFISH_OIDC_AUDIENCE")?;
        ensure!(
            !audience.is_empty(),
            "REDFISH_OIDC_AUDIENCE must not be empty"
        );
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
        Ok(Self {
            listen: std::env::var("REDFISH_LISTEN")
                .unwrap_or_else(|_| "0.0.0.0:8081".into())
                .parse()?,
            issuer,
            audience,
            admin_subjects,
            viewer_subjects,
            admin_groups,
            viewer_groups,
            ca_file: std::env::var_os("REDFISH_CA_FILE").map(PathBuf::from),
        })
    }
}
