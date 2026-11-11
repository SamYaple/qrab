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

/// Tokens larger than this are refused before parsing.
pub const MAX_TOKEN_BYTES: usize = 8 * 1024;
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
        }
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
        );
        authenticator.fetch = Some(Fetch {
            http,
            url: metadata.jwks_uri().clone(),
            last_attempt: Mutex::new(None),
        });
        Ok(authenticator)
    }

    fn role(&self, subject: &str) -> Option<Role> {
        if self.admins.contains(subject) {
            Some(Role::Admin)
        } else if self.viewers.contains(subject) {
            Some(Role::Viewer)
        } else {
            None
        }
    }

    fn check(&self, token: &CoreIdToken) -> Option<(String, u64)> {
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
        Some((claims.subject().as_str().to_owned(), expires))
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
        let (subject, expires) = verified.ok_or(Rejection::Unauthenticated)?;
        if expires <= now() {
            return Err(Rejection::Unauthenticated);
        }
        let role = self.role(&subject).ok_or(Rejection::Forbidden)?;
        Ok(Principal {
            subject,
            role,
            expires,
        })
    }
}
