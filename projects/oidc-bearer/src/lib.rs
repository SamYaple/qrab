//! OIDC ID tokens (JWTs) presented as bearer credentials and verified against
//! the issuer's JWKS, like Kubernetes API-server OIDC authentication. Callers
//! decide what the verified subject and groups may do.
use openidconnect::{
    core::{
        CoreIdToken, CoreIdTokenVerifier, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
        CoreProviderMetadata,
    },
    ClientId, IssuerUrl, JsonWebKeySetUrl, Nonce,
};
use std::{
    path::Path,
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

/// The claims of a token whose signature, issuer, audience and expiry passed.
#[derive(Debug, Clone)]
pub struct Verified {
    pub subject: String,
    pub name: Option<String>,
    /// UUID entries of the `groups` claim (Kanidm scope `groups_uuid`).
    pub groups: Vec<String>,
    /// Unix seconds; always in the future when returned.
    pub expires: u64,
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

pub struct Verifier {
    issuer: IssuerUrl,
    audience: ClientId,
    keys: RwLock<Keys>,
    fetch: Option<Fetch>,
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

/// The credential of an `Authorization` header value whose scheme is
/// `Bearer` (case-insensitive), possibly empty; `None` for other schemes.
///
/// ```
/// assert_eq!(oidc_bearer::bearer_token("Bearer a.b.c"), Some("a.b.c"));
/// assert_eq!(oidc_bearer::bearer_token("bearer  a.b.c "), Some("a.b.c"));
/// assert_eq!(oidc_bearer::bearer_token("Bearer "), Some(""));
/// assert_eq!(oidc_bearer::bearer_token("Basic YTpi"), None);
/// assert_eq!(oidc_bearer::bearer_token("Bearerx"), None);
/// ```
pub fn bearer_token(header: &str) -> Option<&str> {
    let scheme = header.get(..6)?;
    let rest = &header[6..];
    (scheme.eq_ignore_ascii_case("bearer") && (rest.is_empty() || rest.starts_with(' ')))
        .then(|| rest.trim())
}

impl Verifier {
    /// Fixed keys, for tests and offline use; never refreshed.
    pub fn new(issuer: IssuerUrl, audience: ClientId, keys: CoreJsonWebKeySet) -> Self {
        Self {
            issuer,
            audience,
            keys: RwLock::new(Keys {
                set: keys,
                fetched: Instant::now(),
            }),
            fetch: None,
        }
    }

    /// OIDC discovery over HTTPS with redirects disabled. `ca_file` adds a
    /// PEM root for an isolated test IdP.
    pub async fn discover(
        issuer: &str,
        audience: &str,
        ca_file: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let mut builder = openidconnect::reqwest::Client::builder()
            .redirect(openidconnect::reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15));
        if let Some(path) = ca_file {
            builder = builder.add_root_certificate(openidconnect::reqwest::Certificate::from_pem(
                &std::fs::read(path)?,
            )?);
        }
        let http = builder.build()?;
        let issuer = IssuerUrl::new(issuer.to_owned())?;
        anyhow::ensure!(
            issuer.url().scheme() == "https",
            "OIDC issuer must use HTTPS"
        );
        let metadata = CoreProviderMetadata::discover_async(issuer.clone(), &http).await?;
        anyhow::ensure!(
            metadata.jwks_uri().url().scheme() == "https",
            "OIDC JWKS endpoint must use HTTPS"
        );
        let mut verifier = Self::new(
            issuer,
            ClientId::new(audience.to_owned()),
            metadata.jwks().clone(),
        );
        verifier.fetch = Some(Fetch {
            http,
            url: metadata.jwks_uri().clone(),
            last_attempt: Mutex::new(None),
        });
        Ok(verifier)
    }

    fn check(&self, token: &CoreIdToken) -> Option<Verified> {
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
        Some(Verified {
            subject: claims.subject().as_str().to_owned(),
            name: claims.preferred_username().map(|name| name.to_string()),
            groups: groups_claim(&token.to_string()),
            expires: u64::try_from(claims.expiration().timestamp()).ok()?,
        })
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

    /// Verify a compact JWS ID token: ES256 or RS256 signature against the
    /// issuer's keys, issuer, audience (and `azp` with several audiences) and
    /// expiry. `None` means unauthenticated.
    pub async fn verify(&self, token: &str) -> Option<Verified> {
        if token.is_empty() || token.len() > MAX_TOKEN_BYTES {
            return None;
        }
        let token = CoreIdToken::from_str(token).ok()?;
        self.refresh(false).await;
        let verified = match self.check(&token) {
            Some(verified) => verified,
            None => {
                // A rotated signing key is fetched once per backoff window.
                self.refresh(true).await;
                self.check(&token)?
            }
        };
        (verified.expires > now()).then_some(verified)
    }
}

/// The UUID entries of the `groups` claim of a compact JWS whose signature has
/// already been verified. Kanidm's `groups_uuid` scope emits hyphenated UUIDs;
/// anything else (SPNs, names) is ignored.
///
/// ```
/// use base64::Engine;
/// let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
///     .encode(br#"{"groups":["7E1F5D44-0B67-4C8B-9C4A-2B6A9E3C1D00","admins@idm.example",3]}"#);
/// assert_eq!(
///     oidc_bearer::groups_claim(&format!("h.{payload}.s")),
///     ["7e1f5d44-0b67-4c8b-9c4a-2b6a9e3c1d00"]
/// );
/// assert!(oidc_bearer::groups_claim("not-a-jwt").is_empty());
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
