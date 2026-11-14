//! Authorization-code OIDC follows the idm-dashboard Kanidm integration.
//! Optionally, `/api/*` also accepts ID tokens of another OIDC client (the
//! dashboard) as `Authorization: Bearer` credentials, verified per request.
use crate::{AppState, Failure};
use axum::{
    extract::{FromRequestParts, Query, State},
    http::{header::AUTHORIZATION, request::Parts},
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

pub use oidc_bearer::groups_claim;

/// Kanidm scope whose `groups` claim lists group UUIDs only.
pub const GROUPS_SCOPE: &str = "groups_uuid";

/// Session key of the login's verified ID token, which the `/redfish/*`
/// proxy forwards. It lives only in the server-side session record, never in
/// [`User`] (which `/api/session` returns) or a cookie.
pub const ID_TOKEN: &str = "id_token";

pub type Client = CoreClient<
    openidconnect::EndpointSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointMaybeSet,
    openidconnect::EndpointMaybeSet,
>;

/// The browser login client; absent on a bearer-only BMC.
#[derive(Clone)]
pub struct Oidc {
    pub client: Arc<Client>,
    pub http: openidconnect::reqwest::Client,
}

/// This VM's subject and group grants, shared by every credential.
#[derive(Clone, Default)]
pub struct Access {
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

impl Access {
    pub fn from_config(config: &crate::config::Config) -> Self {
        Self {
            admins: config.admin_subjects.clone(),
            viewers: config.viewer_subjects.clone(),
            admin_groups: config.admin_groups.clone(),
            viewer_groups: config.viewer_groups.clone(),
        }
    }

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

    /// The configured groups among `groups`.
    fn keep(&self, groups: impl IntoIterator<Item = String>) -> Vec<String> {
        let mut groups: Vec<String> = groups
            .into_iter()
            .filter(|g| self.admin_groups.contains(g) || self.viewer_groups.contains(g))
            .collect();
        groups.sort();
        groups.dedup();
        groups
    }

    /// The configured groups among a verified ID token's `groups` claim.
    pub fn configured_groups(&self, verified_token: &str) -> Vec<String> {
        self.keep(groups_claim(verified_token))
    }
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
    /// Empty for bearer requests, which need no CSRF token.
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
    pub async fn discover(
        browser: &crate::config::Browser,
        ca_file: Option<&std::path::Path>,
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
        let metadata =
            CoreProviderMetadata::discover_async(IssuerUrl::new(browser.issuer.clone())?, &http)
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
            ClientId::new(browser.client_id.clone()),
            None,
        )
        .set_redirect_uri(RedirectUrl::new(format!(
            "{}/auth/callback",
            browser.origin
        ))?);
        Ok(Self {
            client: Arc::new(client),
            http,
        })
    }
}

fn browser(app: &AppState) -> Result<&Oidc, Failure> {
    app.oidc.as_ref().ok_or_else(Failure::not_found)
}

pub async fn login(State(app): State<AppState>, session: Session) -> Result<Redirect, Failure> {
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let mut request = browser(&app)?
        .client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            || CsrfToken::new_random_len(32),
            || Nonce::new_random_len(32),
        )
        .add_scope(Scope::new("profile".into()));
    if app.access.wants_groups() {
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
    let oidc = browser(&app)?;
    let pending: Pending = session
        .remove("pending")
        .await
        .map_err(|_| Failure::internal())?
        .ok_or_else(Failure::unauthorized)?;
    if pending.expires <= now() || q.state.as_deref() != Some(&pending.state) {
        return Err(Failure::unauthorized());
    }
    let response = oidc
        .client
        .exchange_code(AuthorizationCode::new(
            q.code.ok_or_else(Failure::unauthorized)?,
        ))
        .map_err(|_| Failure::unauthorized())?
        .set_pkce_verifier(PkceCodeVerifier::new(pending.verifier))
        .request_async(&oidc.http)
        .await
        .map_err(|_| Failure::unauthorized())?;
    let id = response.id_token().ok_or_else(Failure::unauthorized)?;
    let token = id.to_string();
    // Redfish refuses larger tokens, and this bounds each session record.
    if token.len() > oidc_bearer::MAX_TOKEN_BYTES {
        tracing::warn!("OIDC ID token exceeds the Redfish token size limit");
        return Err(Failure::unauthorized());
    }
    let verifier = oidc.client.id_token_verifier();
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
        groups: app.access.configured_groups(&token),
        name: claims
            .preferred_username()
            .map(|s| s.as_str())
            .unwrap_or("Operator")
            .into(),
        csrf: uuid::Uuid::new_v4().to_string(),
        expires: (claims.expiration().timestamp().max(0) as u64).min(now() + 900),
    };
    if !authorized(&app.access, &user) {
        return Err(Failure::forbidden());
    }
    session.cycle_id().await.map_err(|_| Failure::internal())?;
    session
        .insert("user", &user)
        .await
        .map_err(|_| Failure::internal())?;
    session
        .insert(ID_TOKEN, token)
        .await
        .map_err(|_| Failure::internal())?;
    tracing::info!(subject=%user.subject, "BMC operator signed in");
    Ok(Redirect::to("/"))
}

/// The role of an unexpired user, if any.
pub fn role(access: &Access, user: &User) -> Option<Role> {
    (user.expires > now())
        .then(|| access.role(&user.subject, &user.groups))
        .flatten()
}
pub fn authorized(access: &Access, user: &User) -> bool {
    role(access, user).is_some()
}

/// How a request authenticated.
#[derive(Clone, Debug)]
pub enum Credential {
    /// A cookie session from the browser login.
    Session(tower_sessions::session::Id),
    /// A verified ID token in this request's `Authorization` header.
    Bearer,
}

/// An authenticated, authorized caller of `/api/*`. As an extractor it uses
/// the request's bearer token when bearer mode is configured and the request
/// carries one, and the cookie session otherwise; the two never mix.
#[derive(Clone)]
pub struct Principal {
    pub user: User,
    pub role: Role,
    pub credential: Credential,
}

impl Principal {
    pub fn is_bearer(&self) -> bool {
        matches!(self.credential, Credential::Bearer)
    }

    /// This principal if it is an admin. Authenticated viewers are 403.
    pub fn admin(self) -> Result<Self, Failure> {
        if self.role != Role::Admin {
            return Err(Failure::forbidden());
        }
        Ok(self)
    }

    /// Whether a long-lived stream may continue: unexpired and still holding
    /// `required`. A session must also still be the same login in the store.
    pub async fn still_current(&self, app: &AppState, required: Role) -> bool {
        match &self.credential {
            Credential::Bearer => {
                role(&app.access, &self.user).is_some_and(|role| role >= required)
            }
            Credential::Session(id) => still_current(app, id, &self.user, required).await,
        }
    }

    /// Resolves when a stream admitted for this principal must end: at
    /// expiry, or at the first failed recheck (made at least once a second).
    pub async fn ended(&self, app: &AppState, required: Role) {
        let expiry = UNIX_EPOCH + Duration::from_secs(self.user.expires);
        let remaining = expiry.duration_since(SystemTime::now()).unwrap_or_default();
        let revoked = async {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                if !self.still_current(app, required).await {
                    break;
                }
            }
        };
        tokio::select! {
            () = tokio::time::sleep(remaining) => {}
            () = revoked => {}
        }
    }
}

impl FromRequestParts<AppState> for Principal {
    type Rejection = Failure;

    async fn from_request_parts(parts: &mut Parts, app: &AppState) -> Result<Self, Failure> {
        if let Some(verifier) = &app.bearer {
            let token = parts
                .headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(oidc_bearer::bearer_token);
            if let Some(token) = token {
                return bearer(app, verifier, token).await;
            }
        }
        let session = Session::from_request_parts(parts, app)
            .await
            .map_err(|_| Failure::internal())?;
        user(app, &session).await
    }
}

/// A bad token is 401; a valid one whose subject and groups grant nothing is 403.
async fn bearer(
    app: &AppState,
    verifier: &oidc_bearer::Verifier,
    token: &str,
) -> Result<Principal, Failure> {
    let verified = verifier
        .verify(token)
        .await
        .ok_or_else(Failure::unauthorized)?;
    let user = User {
        groups: app.access.keep(verified.groups),
        subject: verified.subject,
        name: verified.name.unwrap_or_else(|| "Operator".into()),
        csrf: String::new(),
        expires: verified.expires,
    };
    let role = role(&app.access, &user).ok_or_else(Failure::forbidden)?;
    Ok(Principal {
        user,
        role,
        credential: Credential::Bearer,
    })
}

/// Any authorized cookie-session user. Missing, expired or unlisted sessions
/// are 401.
pub async fn user(app: &AppState, session: &Session) -> Result<Principal, Failure> {
    let user: User = session
        .get("user")
        .await
        .map_err(|_| Failure::internal())?
        .ok_or_else(Failure::unauthorized)?;
    let role = role(&app.access, &user).ok_or_else(Failure::unauthorized)?;
    let id = session.id().ok_or_else(Failure::unauthorized)?;
    Ok(Principal {
        user,
        role,
        credential: Credential::Session(id),
    })
}

/// The verified ID token stored with session `id` at login.
pub async fn session_token(app: &AppState, id: &tower_sessions::session::Id) -> Option<String> {
    use tower_sessions::SessionStore;
    app.sessions
        .load(id)
        .await
        .ok()
        .flatten()
        .and_then(|mut record| record.data.remove(ID_TOKEN))
        .and_then(|value| serde_json::from_value(value).ok())
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
            && role(&app.access, &current).is_some_and(|role| role >= required)
    })
}
