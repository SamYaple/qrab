//! Kanidm OIDC: authorization code with S256 PKCE, state and nonce, for a
//! confidential client (client_secret_basic). The verified ID token stays in
//! the server-side session and is forwarded only to VM BMCs.
use crate::{AppState, Failure};
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Redirect,
};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tower_sessions::{Session, session::Expiry};

/// Kanidm scope whose `groups` claim lists group UUIDs only.
pub const GROUPS_SCOPE: &str = "groups_uuid";
/// Maximum session length, whatever the token lifetime.
pub const MAX_SESSION_SECS: u64 = 900;
/// Bound on the group UUIDs kept per session.
pub const MAX_GROUPS: usize = 1024;

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
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Pending {
    pub(crate) state: String,
    verifier: String,
    nonce: String,
    pub(crate) expires: u64,
    /// Where the callback lands; only a path accepted by [`return_path`].
    #[serde(default)]
    next: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginQuery {
    next: Option<String>,
}

/// A same-origin VM page to return to after sign-in, or `None`. Anything else
/// (absolute URLs, `//host`, traversal, query strings) is refused, so the
/// callback can never redirect off-site.
///
/// ```
/// use qrab_dashboard::auth::return_path;
/// assert_eq!(return_path("/vms/qrab/test").as_deref(), Some("/vms/qrab/test"));
/// assert_eq!(return_path("/vms/qrab/web.a"), Some("/vms/qrab/web.a".into()));
/// for bad in ["//evil.example/vms/a/b", "https://evil.example/", "/vms/qrab", "/vms/qrab/a/../b",
///             "/vms/qrab/a?x=1", "/vms/qrab/A", "/launch", "/vms//a"] {
///     assert_eq!(return_path(bad), None, "{bad}");
/// }
/// ```
pub fn return_path(next: &str) -> Option<String> {
    let rest = next.strip_prefix("/vms/")?;
    let (namespace, name) = rest.split_once('/')?;
    let label = |s: &str, dots: bool| {
        !s.is_empty()
            && s.len() <= 63
            && s.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || (dots && b == b'.')
            })
            && !s.starts_with(['-', '.'])
            && !s.ends_with(['-', '.'])
            && !s.contains("..")
    };
    (label(namespace, false) && label(name, true)).then(|| next.to_owned())
}

/// A signed-in user. No `Debug`: it holds the raw ID token.
#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub subject: String,
    pub name: String,
    /// Group UUIDs from the verified token's `groups` claim. Per-VM groups
    /// are created at run time, so every UUID is kept (bounded).
    pub groups: Vec<String>,
    pub csrf: String,
    /// The earlier of the token `exp` and 15 minutes after sign-in.
    pub expires: u64,
    /// The verified compact ID token, sent only as a BMC bearer.
    pub id_token: String,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Sessions end at the earlier of the token expiry and 15 minutes.
///
/// ```
/// assert_eq!(qrab_dashboard::auth::session_expiry(1_000, 500), 1_000);
/// assert_eq!(qrab_dashboard::auth::session_expiry(9_999, 500), 1_400);
/// ```
pub fn session_expiry(token_exp: u64, instant: u64) -> u64 {
    token_exp.min(instant + MAX_SESSION_SECS)
}

/// The canonical UUID entries of a verified compact JWS's `groups` claim,
/// sorted, deduplicated and bounded. Names and SPNs are ignored.
///
/// ```
/// use base64::Engine;
/// let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
///     .encode(br#"{"groups":["7E1F5D44-0B67-4C8B-9C4A-2B6A9E3C1D00","admins@idm.example"]}"#);
/// assert_eq!(
///     qrab_dashboard::auth::groups_claim(&format!("h.{payload}.s")),
///     ["7e1f5d44-0b67-4c8b-9c4a-2b6a9e3c1d00"]
/// );
/// assert!(qrab_dashboard::auth::groups_claim("not-a-jwt").is_empty());
/// ```
pub fn groups_claim(token: &str) -> Vec<String> {
    use base64::Engine;
    let Some(payload) = token.split('.').nth(1) else {
        return Vec::new();
    };
    let mut groups: Vec<String> = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|claims| claims.get("groups").and_then(|g| g.as_array()).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|g| g.as_str())
        .filter_map(crate::config::uuid)
        .collect();
    groups.sort();
    groups.dedup();
    groups.truncate(MAX_GROUPS);
    groups
}

impl Oidc {
    pub async fn discover(
        issuer: String,
        client_id: String,
        client_secret: String,
        callback: String,
        ca: Option<&[u8]>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            url::Url::parse(&issuer)?.scheme() == "https",
            "OIDC issuer must use HTTPS"
        );
        let mut builder = openidconnect::reqwest::Client::builder()
            .redirect(openidconnect::reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20));
        if let Some(pem) = ca {
            builder =
                builder.add_root_certificate(openidconnect::reqwest::Certificate::from_pem(pem)?);
        }
        let http = builder.build()?;
        let metadata = CoreProviderMetadata::discover_async(IssuerUrl::new(issuer)?, &http).await?;
        anyhow::ensure!(
            metadata.authorization_endpoint().url().scheme() == "https"
                && metadata
                    .token_endpoint()
                    .is_some_and(|u| u.url().scheme() == "https")
                && metadata.jwks_uri().url().scheme() == "https",
            "OIDC endpoints must use HTTPS"
        );
        Self::from_metadata(metadata, client_id, client_secret, callback, http)
    }

    /// The default auth type is client_secret_basic at the token endpoint.
    pub fn from_metadata(
        metadata: CoreProviderMetadata,
        client_id: String,
        client_secret: String,
        callback: String,
        http: openidconnect::reqwest::Client,
    ) -> anyhow::Result<Self> {
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
        )
        .set_redirect_uri(RedirectUrl::new(callback)?);
        Ok(Self {
            client: Arc::new(client),
            http,
        })
    }
}

pub async fn login(
    State(app): State<AppState>,
    session: Session,
    Query(query): Query<LoginQuery>,
) -> Result<Redirect, Failure> {
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, state, nonce) = app
        .oidc
        .client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            || CsrfToken::new_random_len(32),
            || Nonce::new_random_len(32),
        )
        .add_scope(Scope::new("profile".into()))
        .add_scope(Scope::new(GROUPS_SCOPE.into()))
        .set_pkce_challenge(challenge)
        .url();
    let expires = now() + 300;
    if session
        .get::<serde_json::Value>("user")
        .await
        .map_err(|_| Failure::internal())?
        .is_none()
    {
        session.set_expiry(Some(Expiry::AtDateTime(
            time::OffsetDateTime::from_unix_timestamp(expires as i64)
                .map_err(|_| Failure::internal())?,
        )));
    }
    session
        .insert(
            "pending",
            Pending {
                state: state.secret().clone(),
                verifier: verifier.secret().clone(),
                nonce: nonce.secret().clone(),
                expires,
                next: query.next.as_deref().and_then(return_path),
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

/// The pending login is consumed before anything else, so a state can be
/// tried at most once.
pub(crate) fn check_state(pending: &Pending, state: Option<&str>, instant: u64) -> bool {
    pending.expires > instant && !pending.state.is_empty() && state == Some(&pending.state)
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
    if !check_state(&pending, q.state.as_deref(), now()) {
        return Err(Failure::unauthorized());
    }
    let code = q.code.ok_or_else(Failure::unauthorized)?;
    let response = app
        .oidc
        .client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|_| Failure::unauthorized())?
        .set_pkce_verifier(PkceCodeVerifier::new(pending.verifier))
        .request_async(&app.oidc.http)
        .await
        .map_err(|error| {
            tracing::warn!(error=%error, "OIDC code exchange failed");
            Failure::unauthorized()
        })?;
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
    // Kanidm subjects are UUIDs; launch adds them to groups by UUID.
    let subject =
        crate::config::uuid(claims.subject().as_str()).ok_or_else(Failure::unauthorized)?;
    let raw = id.to_string();
    let user = User {
        subject,
        name: claims
            .preferred_username()
            .map(|v| v.as_str())
            .unwrap_or("user")
            .into(),
        groups: groups_claim(&raw),
        csrf: uuid::Uuid::new_v4().to_string(),
        expires: session_expiry(claims.expiration().timestamp().max(0) as u64, now()),
        id_token: raw,
    };
    establish_session(&session, user).await?;
    Ok(Redirect::to(pending.next.as_deref().unwrap_or("/")))
}

pub(crate) async fn establish_session(session: &Session, user: User) -> Result<(), Failure> {
    session.cycle_id().await.map_err(|_| Failure::internal())?;
    session.set_expiry(Some(Expiry::AtDateTime(
        time::OffsetDateTime::from_unix_timestamp(user.expires as i64)
            .map_err(|_| Failure::internal())?,
    )));
    tracing::info!(subject=%user.subject, "dashboard user signed in");
    session
        .insert("user", user)
        .await
        .map_err(|_| Failure::internal())?;
    Ok(())
}

/// An unexpired signed-in user; anything else is 401.
pub async fn user(session: &Session) -> Result<User, Failure> {
    let user: User = session
        .get("user")
        .await
        .map_err(|_| Failure::internal())?
        .ok_or_else(Failure::unauthorized)?;
    if user.expires <= now() {
        return Err(Failure::unauthorized());
    }
    Ok(user)
}

pub fn origin_matches(headers: &HeaderMap, expected: &str) -> bool {
    headers.get("origin").and_then(|h| h.to_str().ok()) == Some(expected)
}

/// Every mutation needs the session's CSRF token and the exact Origin.
pub fn mutation_allowed(user: &User, csrf: &str, headers: &HeaderMap, expected: &str) -> bool {
    !user.csrf.is_empty() && csrf == user.csrf && origin_matches(headers, expected)
}

/// A long-lived stream's session must still be the same, unexpired login.
pub async fn still_current(
    store: &crate::session_store::BoundedStore,
    id: &tower_sessions::session::Id,
    user: &User,
) -> bool {
    use tower_sessions::SessionStore;
    let current: Option<User> = store
        .load(id)
        .await
        .ok()
        .flatten()
        .and_then(|record| record.data.get("user").cloned())
        .and_then(|value| serde_json::from_value(value).ok());
    current.is_some_and(|current| {
        current.subject == user.subject && current.csrf == user.csrf && current.expires > now()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn user() -> User {
        User {
            subject: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
            name: "alice".into(),
            groups: vec![],
            csrf: "nonce".into(),
            expires: now() + 60,
            id_token: "token".into(),
        }
    }
    #[test]
    fn mutation_guard_requires_csrf_and_exact_origin() {
        let u = user();
        let mut ok = HeaderMap::new();
        ok.insert("origin", "https://vms.example".parse().unwrap());
        assert!(mutation_allowed(&u, "nonce", &ok, "https://vms.example"));
        assert!(!mutation_allowed(&u, "wrong", &ok, "https://vms.example"));
        assert!(!mutation_allowed(&u, "", &ok, "https://vms.example"));
        assert!(!mutation_allowed(
            &u,
            "nonce",
            &HeaderMap::new(),
            "https://vms.example"
        ));
        let mut evil = HeaderMap::new();
        evil.insert("origin", "https://vms.example.evil".parse().unwrap());
        assert!(!mutation_allowed(&u, "nonce", &evil, "https://vms.example"));
        let empty = User {
            csrf: String::new(),
            ..user()
        };
        assert!(!mutation_allowed(&empty, "", &ok, "https://vms.example"));
    }
    #[test]
    fn callback_state_must_match_and_be_fresh() {
        let pending = Pending {
            state: "s".into(),
            verifier: "v".into(),
            nonce: "n".into(),
            expires: 100,
            next: None,
        };
        assert!(check_state(&pending, Some("s"), 99));
        assert!(!check_state(&pending, Some("s"), 100));
        assert!(!check_state(&pending, Some("t"), 99));
        assert!(!check_state(&pending, None, 99));
    }
    #[test]
    fn groups_are_bounded() {
        use base64::Engine;
        let groups: Vec<String> = (0..MAX_GROUPS + 10)
            .map(|i| format!("00000000-0000-4000-8000-{i:012}"))
            .collect();
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&serde_json::json!({ "groups": groups })).unwrap());
        assert_eq!(groups_claim(&format!("h.{payload}.s")).len(), MAX_GROUPS);
    }
}
