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
    pub allowed: HashSet<String>,
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
            allowed: config.allowed_subjects.clone(),
        })
    }
}

pub async fn login(State(app): State<AppState>, session: Session) -> Result<Redirect, Failure> {
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
        .set_pkce_challenge(challenge)
        .url();
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

pub fn authorized(oidc: &Oidc, user: &User) -> bool {
    user.expires > now() && oidc.allowed.contains(&user.subject)
}
pub async fn user(app: &AppState, session: &Session) -> Result<User, Failure> {
    let user: User = session
        .get("user")
        .await
        .map_err(|_| Failure::internal())?
        .ok_or_else(Failure::unauthorized)?;
    if !authorized(&app.oidc, &user) {
        return Err(Failure::unauthorized());
    }
    Ok(user)
}
