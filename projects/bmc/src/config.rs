use crate::admin_bucket::{self, Feature};
use anyhow::{ensure, Context, Result};
use openidconnect::url::Url;
use std::{
    collections::{BTreeSet, HashSet},
    net::SocketAddr,
    path::PathBuf,
};

/// The browser console: a Kanidm authorization-code login with cookie sessions.
#[derive(Clone)]
pub struct Browser {
    /// Exact public origin, from `BMC_PUBLIC_URL`.
    pub origin: String,
    pub issuer: String,
    pub client_id: String,
}

/// ID tokens another OIDC client (the dashboard) forwards as
/// `Authorization: Bearer` credentials on `/api/*`.
#[derive(Clone)]
pub struct Bearer {
    pub issuer: String,
    pub audience: String,
}

#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    /// Absent without `BMC_PUBLIC_URL`: no login, cookie sessions or UI.
    pub browser: Option<Browser>,
    pub bearer: Option<Bearer>,
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
    /// Admin-bucket features enabled for this VM (`BMC_ADMIN_BUCKET`).
    pub admin_bucket: BTreeSet<Feature>,
    /// Origin of the in-pod Redfish sidecar behind `/redfish/*`, from
    /// `BMC_REDFISH_URL`.
    pub redfish_url: String,
}

/// The default in-pod Redfish listener, reached over the Pod's loopback.
pub const DEFAULT_REDFISH_URL: &str = "http://127.0.0.1:8081";

fn https(name: &str, raw: &str) -> Result<()> {
    ensure!(
        Url::parse(raw)
            .with_context(|| format!("{name} must be a URL"))?
            .scheme()
            == "https",
        "{name} must use HTTPS"
    );
    Ok(())
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// Configuration from `var`, which returns a variable's value if set.
    /// Empty values count as unset.
    ///
    /// ```
    /// use bmc::config::Config;
    /// let vars = |pairs: &'static [(&str, &str)]| {
    ///     move |name: &str| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string())
    /// };
    /// const ADMIN: (&str, &str) = ("BMC_ADMIN_SUBJECTS", "6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b");
    /// const ISSUER: (&str, &str) = ("BMC_BEARER_ISSUER", "https://idm.example/oauth2/openid/dash");
    /// const AUDIENCE: (&str, &str) = ("BMC_BEARER_AUDIENCE", "dash");
    /// // Bearer-only: no public URL, so no browser login.
    /// let config = Config::from_vars(vars(&[ADMIN, ISSUER, AUDIENCE])).unwrap();
    /// assert!(config.browser.is_none());
    /// assert_eq!(config.bearer.unwrap().audience, "dash");
    /// // Some way in is required, and the bearer settings come as a pair.
    /// assert!(Config::from_vars(vars(&[ADMIN])).is_err());
    /// assert!(Config::from_vars(vars(&[ADMIN, ISSUER])).is_err());
    /// const HTTP: (&str, &str) = ("BMC_BEARER_ISSUER", "http://idm.example/oauth2/openid/dash");
    /// assert!(Config::from_vars(vars(&[ADMIN, HTTP, AUDIENCE])).is_err());
    /// // The browser console keeps requiring its own OIDC client.
    /// const PUBLIC: (&str, &str) = ("BMC_PUBLIC_URL", "https://vm.example");
    /// assert!(Config::from_vars(vars(&[ADMIN, PUBLIC])).is_err());
    /// let both = Config::from_vars(vars(&[
    ///     ADMIN, PUBLIC, ISSUER, AUDIENCE,
    ///     ("BMC_OIDC_ISSUER", "https://idm.example/oauth2/openid/qrab-bmc"),
    ///     ("BMC_OIDC_CLIENT_ID", "qrab-bmc"),
    /// ]))
    /// .unwrap();
    /// assert_eq!(both.browser.unwrap().origin, "https://vm.example");
    /// assert!(both.bearer.is_some());
    /// // The admin bucket is empty unless named, and names must be known.
    /// assert!(config.admin_bucket.is_empty());
    /// let audio = Config::from_vars(vars(&[ADMIN, ISSUER, AUDIENCE, ("BMC_ADMIN_BUCKET", "audio")]));
    /// assert_eq!(audio.unwrap().admin_bucket.into_iter().collect::<Vec<_>>(), [bmc::admin_bucket::Feature::Audio]);
    /// assert!(Config::from_vars(vars(&[ADMIN, ISSUER, AUDIENCE, ("BMC_ADMIN_BUCKET", "mic")])).is_err());
    /// // `/redfish/*` goes to the in-pod Redfish sidecar unless overridden.
    /// assert_eq!(both.redfish_url, "http://127.0.0.1:8081");
    /// const PATHED: (&str, &str) = ("BMC_REDFISH_URL", "http://127.0.0.1:8081/redfish");
    /// assert!(Config::from_vars(vars(&[ADMIN, ISSUER, AUDIENCE, PATHED])).is_err());
    /// ```
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let var = |name: &str| var(name).filter(|value| !value.is_empty());
        let required = |name: &str| var(name).with_context(|| format!("{name} is required"));
        let listen = var("BMC_LISTEN")
            .unwrap_or_else(|| "127.0.0.1:8080".into())
            .parse()?;
        let uuids = |name: &str| -> Result<HashSet<String>> {
            var(name)
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
        let browser = match var("BMC_PUBLIC_URL") {
            None => None,
            Some(public_url) => {
                let origin = public_origin(
                    &public_url,
                    var("BMC_ALLOW_LOOPBACK_HTTP").as_deref() == Some("true"),
                    &listen,
                )?;
                let issuer = required("BMC_OIDC_ISSUER")?;
                https("BMC_OIDC_ISSUER", &issuer)?;
                Some(Browser {
                    origin,
                    issuer,
                    client_id: required("BMC_OIDC_CLIENT_ID")?,
                })
            }
        };
        let bearer = match (var("BMC_BEARER_ISSUER"), var("BMC_BEARER_AUDIENCE")) {
            (None, None) => None,
            (Some(issuer), Some(audience)) => {
                https("BMC_BEARER_ISSUER", &issuer)?;
                Some(Bearer { issuer, audience })
            }
            _ => anyhow::bail!("BMC_BEARER_ISSUER and BMC_BEARER_AUDIENCE must be set together"),
        };
        ensure!(
            browser.is_some() || bearer.is_some(),
            "BMC_PUBLIC_URL (browser console) or BMC_BEARER_ISSUER and BMC_BEARER_AUDIENCE (dashboard bearer tokens) are required"
        );
        Ok(Self {
            listen,
            browser,
            bearer,
            admin_subjects,
            viewer_subjects,
            admin_groups,
            viewer_groups,
            assets_dir: var("BMC_ASSETS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")),
            ca_file: var("BMC_CA_FILE").map(PathBuf::from),
            admin_bucket: admin_bucket::parse_list(&var("BMC_ADMIN_BUCKET").unwrap_or_default())
                .map_err(|name| {
                    anyhow::anyhow!("BMC_ADMIN_BUCKET names an unknown feature: {name}")
                })?,
            redfish_url: redfish_origin(
                &var("BMC_REDFISH_URL").unwrap_or_else(|| DEFAULT_REDFISH_URL.into()),
            )?,
        })
    }
}

/// An HTTP(S) origin without credentials, path, query or fragment.
fn redfish_origin(raw: &str) -> Result<String> {
    let url = Url::parse(raw).context("BMC_REDFISH_URL must be a URL")?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && url.path() == "/"
            && url.host_str().is_some(),
        "BMC_REDFISH_URL must be an HTTP(S) origin without credentials, path, query or fragment"
    );
    Ok(url.origin().ascii_serialization())
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
