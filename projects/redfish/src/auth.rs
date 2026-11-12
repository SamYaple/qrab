//! Per-request bearer authentication: an OIDC ID token (JWT) verified against
//! the issuer's JWKS, like Kubernetes API-server OIDC authentication. There
//! are no Redfish sessions, static tokens, Basic credentials or anonymous
//! access.
use openidconnect::{
    core::{
        CoreIdToken, CoreIdTokenVerifier, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
        CoreProviderMetadata,
    },
    ClientId, IssuerUrl, JsonWebKeySetUrl, Nonce,
};
use serde::Serialize;
use std::{
    collections::HashSet,
    str::FromStr,
    sync::{Mutex, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Tokens larger than this are refused before parsing. A `groups_uuid` claim
/// costs about 50 bytes per group.
pub const MAX_TOKEN_BYTES: usize = 16 * 1024;
/// Keys are refetched at most this often after an unknown key or bad signature.
const REFRESH_BACKOFF: Duration = Duration::from_secs(30);
/// Keys are refreshed before use when older than this.
const KEY_MAX_AGE: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer,
    Admin,
}

#[derive(Debug, Clone)]
pub struct Principal {
    pub subject: String,
    pub role: Role,
    /// Unix seconds; live streams end here.
    pub expires: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Rejection {
    /// Missing, malformed, unverifiable or expired credentials: 401.
    Unauthenticated,
    /// A valid token whose subject is not listed: 403.
    Forbidden,
}

struct Keys {
    set: CoreJsonWebKeySet,
    fetched: Instant,
}

struct Fetch {
    http: openidconnect::reqwest::Client,
    url: JsonWebKeySetUrl,
    last_attempt: Mutex<Option<Instant>>,
}

pub struct Authenticator {
    issuer: IssuerUrl,
    audience: ClientId,
    keys: RwLock<Keys>,
    fetch: Option<Fetch>,
    admins: HashSet<String>,
    viewers: HashSet<String>,
    admin_groups: HashSet<String>,
    viewer_groups: HashSet<String>,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn any_nonce(_: Option<&Nonce>) -> Result<(), String> {
    // ID tokens here are presented as bearer credentials, not login replies.
    Ok(())
}

impl Authenticator {
    /// Fixed keys, for tests and offline use; never refreshed.
    pub fn new(
        issuer: IssuerUrl,
        audience: ClientId,
        keys: CoreJsonWebKeySet,
        admins: HashSet<String>,
        viewers: HashSet<String>,
    ) -> Self {
        Self {
            issuer,
            audience,
            keys: RwLock::new(Keys {
                set: keys,
                fetched: Instant::now(),
            }),
            fetch: None,
            admins,
            viewers,
            admin_groups: HashSet::new(),
            viewer_groups: HashSet::new(),
        }
    }

    /// Also grant roles to members of these Kanidm group UUIDs, read from the
    /// token's `groups` claim.
    pub fn with_groups(mut self, admins: HashSet<String>, viewers: HashSet<String>) -> Self {
        self.admin_groups = admins;
        self.viewer_groups = viewers;
        self
    }

    /// OIDC discovery over HTTPS with redirects disabled.
    pub async fn discover(config: &crate::config::Config) -> anyhow::Result<Self> {
        let mut builder = openidconnect::reqwest::Client::builder()
            .redirect(openidconnect::reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15));
        if let Some(path) = &config.ca_file {
            builder = builder.add_root_certificate(openidconnect::reqwest::Certificate::from_pem(
                &std::fs::read(path)?,
            )?);
        }
        let http = builder.build()?;
        let issuer = IssuerUrl::new(config.issuer.clone())?;
        let metadata = CoreProviderMetadata::discover_async(issuer.clone(), &http).await?;
        anyhow::ensure!(
            metadata.jwks_uri().url().scheme() == "https",
            "OIDC JWKS endpoint must use HTTPS"
        );
        let mut authenticator = Self::new(
            issuer,
            ClientId::new(config.audience.clone()),
            metadata.jwks().clone(),
            config.admin_subjects.clone(),
            config.viewer_subjects.clone(),
        )
        .with_groups(config.admin_groups.clone(), config.viewer_groups.clone());
        authenticator.fetch = Some(Fetch {
            http,
            url: metadata.jwks_uri().clone(),
            last_attempt: Mutex::new(None),
        });
        Ok(authenticator)
    }

    fn role(&self, subject: &str, groups: &[String]) -> Option<Role> {
        let member = |set: &HashSet<String>| groups.iter().any(|g| set.contains(g));
        if self.admins.contains(subject) || member(&self.admin_groups) {
            Some(Role::Admin)
        } else if self.viewers.contains(subject) || member(&self.viewer_groups) {
            Some(Role::Viewer)
        } else {
            None
        }
    }

    fn check(&self, token: &CoreIdToken) -> Option<(String, u64, Vec<String>)> {
        let keys = self
            .keys
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .set
            .clone();
        let verifier = CoreIdTokenVerifier::new_public_client(
            self.audience.clone(),
            self.issuer.clone(),
            keys,
        )
        .set_allowed_algs([
            CoreJwsSigningAlgorithm::EcdsaP256Sha256,
            CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
        ]);
        let claims = token.claims(&verifier, any_nonce).ok()?;
        let expires = u64::try_from(claims.expiration().timestamp()).ok()?;
        Some((
            claims.subject().as_str().to_owned(),
            expires,
            groups_claim(&token.to_string()),
        ))
    }

    async fn refresh(&self, force: bool) {
        let Some(fetch) = &self.fetch else { return };
        let stale = self
            .keys
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .fetched
            .elapsed()
            > KEY_MAX_AGE;
        if !force && !stale {
            return;
        }
        {
            let mut last = fetch
                .last_attempt
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if last.is_some_and(|at| at.elapsed() < REFRESH_BACKOFF) {
                return;
            }
            *last = Some(Instant::now());
        }
        match CoreJsonWebKeySet::fetch_async(&fetch.url, &fetch.http).await {
            Ok(set) => {
                *self
                    .keys
                    .write()
                    .unwrap_or_else(|poison| poison.into_inner()) = Keys {
                    set,
                    fetched: Instant::now(),
                };
            }
            Err(error) => tracing::warn!(%error, "JWKS refresh failed; keeping current keys"),
        }
    }

    /// Authenticate an `Authorization` header value.
    pub async fn authenticate(&self, header: Option<&str>) -> Result<Principal, Rejection> {
        let token = header
            .and_then(|value| value.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|token| !token.is_empty() && token.len() <= MAX_TOKEN_BYTES)
            .ok_or(Rejection::Unauthenticated)?;
        let token = CoreIdToken::from_str(token).map_err(|_| Rejection::Unauthenticated)?;
        self.refresh(false).await;
        let verified = match self.check(&token) {
            Some(verified) => Some(verified),
            None => {
                // A rotated signing key is fetched once per backoff window.
                self.refresh(true).await;
                self.check(&token)
            }
        };
        let (subject, expires, groups) = verified.ok_or(Rejection::Unauthenticated)?;
        if expires <= now() {
            return Err(Rejection::Unauthenticated);
        }
        let role = self.role(&subject, &groups).ok_or(Rejection::Forbidden)?;
        Ok(Principal {
            subject,
            role,
            expires,
        })
    }
}

/// The UUID entries of the `groups` claim of a compact JWS whose signature has
/// already been verified; SPNs and names are ignored.
///
/// ```
/// use base64::Engine;
/// let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
///     .encode(br#"{"groups":["7e1f5d44-0b67-4c8b-9c4a-2b6a9e3c1d00","admins@idm.example",3]}"#);
/// assert_eq!(
///     qrab_redfish::auth::groups_claim(&format!("h.{payload}.s")),
///     ["7e1f5d44-0b67-4c8b-9c4a-2b6a9e3c1d00"]
/// );
/// ```
pub fn groups_claim(token: &str) -> Vec<String> {
    use base64::Engine;
    let Some(payload) = token.split('.').nth(1) else {
        return Vec::new();
    };
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|claims| claims.get("groups").and_then(|g| g.as_array()).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|g| g.as_str())
        .filter_map(|g| uuid::Uuid::parse_str(g).ok())
        .map(|id| id.to_string())
        .collect()
}
