//! Authorization-code OIDC follows the idm-dashboard Kanidm integration.
use crate::{AppState, Failure};
use axum::{
    extract::{Query, State},
    response::Redirect,
};
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    AccessTokenHash, AuthorizationCode, ClientId, CsrfToken, IssuerUrl, Nonce, OAuth2TokenResponse,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tower_sessions::Session;

/// Kanidm scope whose `groups` claim lists group UUIDs only.
pub const GROUPS_SCOPE: &str = "groups_uuid";

pub type Client = CoreClient<
    openidconnect::EndpointSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointMaybeSet,
    openidconnect::EndpointMaybeSet,
>;
#[derive(Clone)]
pub struct Oidc {
    pub client: Arc<Client>,
    pub http: openidconnect::reqwest::Client,
    pub admins: HashSet<String>,
    pub viewers: HashSet<String>,
    pub admin_groups: HashSet<String>,
    pub viewer_groups: HashSet<String>,
}

/// Roles are resolved from configuration on every request, never stored in
/// the session, so a session cannot carry a stale privilege.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Viewer,
    Admin,
}

impl Oidc {
    /// The strongest role granted to `subject` directly or through `groups`.
    pub fn role(&self, subject: &str, groups: &[String]) -> Option<Role> {
        let member = |set: &HashSet<String>| groups.iter().any(|g| set.contains(g));
        if self.admins.contains(subject) || member(&self.admin_groups) {
            Some(Role::Admin)
        } else if self.viewers.contains(subject) || member(&self.viewer_groups) {
            Some(Role::Viewer)
        } else {
            None
        }
    }

    fn wants_groups(&self) -> bool {
        !self.admin_groups.is_empty() || !self.viewer_groups.is_empty()
    }

    /// The configured groups among a verified ID token's `groups` claim.
    pub fn configured_groups(&self, verified_token: &str) -> Vec<String> {
        let mut groups: Vec<String> = groups_claim(verified_token)
            .into_iter()
            .filter(|g| self.admin_groups.contains(g) || self.viewer_groups.contains(g))
            .collect();
        groups.sort();
        groups.dedup();
        groups
    }
}

/// The UUID entries of the `groups` claim of a compact JWS whose signature has
/// already been verified. Kanidm's `groups_uuid` scope emits hyphenated UUIDs;
/// anything else (SPNs, names) is ignored.
///
/// ```
/// use base64::Engine;
/// let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
///     .encode(br#"{"groups":["7E1F5D44-0B67-4C8B-9C4A-2B6A9E3C1D00","admins@idm.example"]}"#);
/// assert_eq!(
///     bmc::auth::groups_claim(&format!("h.{payload}.s")),
///     ["7e1f5d44-0b67-4c8b-9c4a-2b6a9e3c1d00"]
/// );
/// assert!(bmc::auth::groups_claim("not-a-jwt").is_empty());
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
#[derive(Serialize, Deserialize)]
struct Pending {
    state: String,
    verifier: String,
    nonce: String,
    expires: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub subject: String,
    /// Configured role groups the ID token listed at login. Sessions end
    /// within 15 minutes, which bounds how long a removed membership lasts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    pub name: String,
    pub csrf: String,
    pub expires: u64,
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Oidc {
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
        let metadata =
            CoreProviderMetadata::discover_async(IssuerUrl::new(config.issuer.clone())?, &http)
                .await?;
        anyhow::ensure!(
            metadata.authorization_endpoint().url().scheme() == "https"
                && metadata
                    .token_endpoint()
                    .is_some_and(|u| u.url().scheme() == "https")
                && metadata.jwks_uri().url().scheme() == "https",
            "OIDC endpoints must use HTTPS"
        );
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(config.client_id.clone()),
            None,
        )
        .set_redirect_uri(RedirectUrl::new(format!(
            "{}/auth/callback",
            config.origin
        ))?);
        Ok(Self {
            client: Arc::new(client),
            http,
            admins: config.admin_subjects.clone(),
            viewers: config.viewer_subjects.clone(),
            admin_groups: config.admin_groups.clone(),
            viewer_groups: config.viewer_groups.clone(),
        })
    }
}

pub async fn login(State(app): State<AppState>, session: Session) -> Result<Redirect, Failure> {
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let mut request = app
        .oidc
        .client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            || CsrfToken::new_random_len(32),
            || Nonce::new_random_len(32),
        )
        .add_scope(Scope::new("profile".into()));
    if app.oidc.wants_groups() {
        request = request.add_scope(Scope::new(GROUPS_SCOPE.into()));
    }
    let (url, state, nonce) = request.set_pkce_challenge(challenge).url();
    session
        .insert(
            "pending",
            Pending {
                state: state.secret().clone(),
                verifier: verifier.secret().clone(),
                nonce: nonce.secret().clone(),
                expires: now() + 300,
            },
        )
        .await
        .map_err(|_| Failure::internal())?;
    Ok(Redirect::temporary(url.as_str()))
}

#[derive(Deserialize)]
pub struct Callback {
    code: Option<String>,
    state: Option<String>,
}
pub async fn callback(
    State(app): State<AppState>,
    session: Session,
    Query(q): Query<Callback>,
) -> Result<Redirect, Failure> {
    let pending: Pending = session
        .remove("pending")
        .await
        .map_err(|_| Failure::internal())?
        .ok_or_else(Failure::unauthorized)?;
    if pending.expires <= now() || q.state.as_deref() != Some(&pending.state) {
        return Err(Failure::unauthorized());
    }
    let response = app
        .oidc
        .client
        .exchange_code(AuthorizationCode::new(
            q.code.ok_or_else(Failure::unauthorized)?,
        ))
        .map_err(|_| Failure::unauthorized())?
        .set_pkce_verifier(PkceCodeVerifier::new(pending.verifier))
        .request_async(&app.oidc.http)
        .await
        .map_err(|_| Failure::unauthorized())?;
    let id = response.id_token().ok_or_else(Failure::unauthorized)?;
    let verifier = app.oidc.client.id_token_verifier();
    let claims = id
        .claims(&verifier, &Nonce::new(pending.nonce))
        .map_err(|error| {
            tracing::warn!(error=%error, "OIDC ID token rejected");
            Failure::unauthorized()
        })?;
    if let Some(expected) = claims.access_token_hash() {
        let actual = AccessTokenHash::from_token(
            response.access_token(),
            id.signing_alg().map_err(|_| Failure::unauthorized())?,
            id.signing_key(&verifier)
                .map_err(|_| Failure::unauthorized())?,
        )
        .map_err(|_| Failure::unauthorized())?;
        if actual != *expected {
            return Err(Failure::unauthorized());
        }
    }
    let user = User {
        subject: claims.subject().as_str().into(),
        groups: app.oidc.configured_groups(&id.to_string()),
        name: claims
            .preferred_username()
            .map(|s| s.as_str())
            .unwrap_or("Operator")
            .into(),
        csrf: uuid::Uuid::new_v4().to_string(),
        expires: (claims.expiration().timestamp().max(0) as u64).min(now() + 900),
    };
    if !authorized(&app.oidc, &user) {
        return Err(Failure::forbidden());
    }
    session.cycle_id().await.map_err(|_| Failure::internal())?;
    session
        .insert("user", &user)
        .await
        .map_err(|_| Failure::internal())?;
    tracing::info!(subject=%user.subject, "BMC operator signed in");
    Ok(Redirect::to("/"))
}

/// The role of an unexpired session user, if any.
pub fn role(oidc: &Oidc, user: &User) -> Option<Role> {
    (user.expires > now())
        .then(|| oidc.role(&user.subject, &user.groups))
        .flatten()
}
pub fn authorized(oidc: &Oidc, user: &User) -> bool {
    role(oidc, user).is_some()
}

pub struct Principal {
    pub user: User,
    pub role: Role,
}

/// Any authorized user. Missing, expired or unlisted sessions are 401.
pub async fn user(app: &AppState, session: &Session) -> Result<Principal, Failure> {
    let user: User = session
        .get("user")
        .await
        .map_err(|_| Failure::internal())?
        .ok_or_else(Failure::unauthorized)?;
    let role = role(&app.oidc, &user).ok_or_else(Failure::unauthorized)?;
    Ok(Principal { user, role })
}

/// An admin user. Authenticated viewers are 403.
pub async fn admin(app: &AppState, session: &Session) -> Result<User, Failure> {
    let principal = user(app, session).await?;
    if principal.role != Role::Admin {
        return Err(Failure::forbidden());
    }
    Ok(principal.user)
}

/// Re-read a long-lived stream's session from the store: it must still be the
/// same login, unexpired and hold at least `required`.
pub async fn still_current(
    app: &AppState,
    id: &tower_sessions::session::Id,
    user: &User,
    required: Role,
) -> bool {
    use tower_sessions::SessionStore;
    let current: Option<User> = app
        .sessions
        .load(id)
        .await
        .ok()
        .flatten()
        .and_then(|record| record.data.get("user").cloned())
        .and_then(|value| serde_json::from_value(value).ok());
    current.is_some_and(|current| {
        current.subject == user.subject
            && current.groups == user.groups
            && current.csrf == user.csrf
            && role(&app.oidc, &current).is_some_and(|role| role >= required)
    })
}
