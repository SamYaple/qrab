mod agent;
pub mod auth;
pub mod config;
mod console;
mod events;
pub mod session_store;

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use manager::pod::{self, Action};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tower_sessions::{cookie::SameSite, Expiry, Session, SessionManagerLayer};

#[derive(Clone)]
pub struct AppState {
    /// The browser login; `None` on a bearer-only BMC.
    pub oidc: Option<auth::Oidc>,
    pub access: auth::Access,
    /// Verifies dashboard ID tokens when bearer mode is configured.
    pub bearer: Option<Arc<oidc_bearer::Verifier>>,
    pub config: config::Config,
    /// The VM, in-process or behind the manager's control socket.
    pub control: pod::Backend,
    pub sessions: session_store::BoundedStore,
    pub consoles: Arc<Semaphore>,
    pub event_streams: Arc<Semaphore>,
}

pub struct Failure(StatusCode, &'static str);
impl Failure {
    pub fn unauthorized() -> Self {
        Self(StatusCode::UNAUTHORIZED, "Sign in to access this VM.")
    }
    pub fn forbidden() -> Self {
        Self(StatusCode::FORBIDDEN, "This request is not authorized.")
    }
    pub fn not_found() -> Self {
        Self(StatusCode::NOT_FOUND, "Not found.")
    }
    pub fn internal() -> Self {
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The request could not be completed.",
        )
    }
    /// 503 naming what is unavailable.
    pub fn unavailable(message: &'static str) -> Self {
        Self(StatusCode::SERVICE_UNAVAILABLE, message)
    }
    pub fn bad_request(message: &'static str) -> Self {
        Self(StatusCode::BAD_REQUEST, message)
    }
    /// A failed VM request; shutdown is reported as such.
    pub fn vm(error: &manager::Error, message: &'static str) -> Self {
        match error {
            manager::Error::ShuttingDown => Self::unavailable("The VM is shutting down."),
            _ => Self::unavailable(message),
        }
    }
}
impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

/// Without a browser login (`BMC_PUBLIC_URL`) there is no UI, `/auth/*` or
/// `/assets`: only the JSON API, for bearer tokens, and the probes.
pub fn router(app: AppState) -> Router {
    let secure = app
        .config
        .browser
        .as_ref()
        .is_none_or(|browser| browser.origin.starts_with("https://"));
    let sessions = SessionManagerLayer::new(app.sessions.clone())
        .with_name(if secure {
            "__Host-qrab-session"
        } else {
            "qrab-dev-session"
        })
        .with_secure(secure)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::minutes(15)));
    let mut router = Router::new();
    if app.oidc.is_some() {
        router = router
            .route(
                "/",
                get(|| async { Html(include_str!("../assets/index.html")) }),
            )
            .route("/auth/login", get(auth::login))
            .route("/auth/callback", get(auth::callback))
            .route("/auth/logout", post(logout))
            .nest_service(
                "/assets",
                tower_http::services::ServeDir::new(app.config.assets_dir.clone()),
            );
    }
    router
        .route("/api/session", get(identity))
        .route("/api/status", get(status))
        .route("/api/resources", get(resource_usage))
        .route("/api/config", get(config_view))
        .route("/api/events", get(events::page))
        .route("/api/events/stream", get(events::stream))
        .route("/api/action", post(action))
        .route("/api/agent", get(agent::view))
        .route("/api/agent/freeze", post(agent::freeze))
        .route("/api/agent/thaw", post(agent::thaw))
        .route("/api/agent/update", post(agent::update))
        .route("/api/console", get(console::upgrade))
        .route("/healthz", get(|| async { "ok" }))
        // A paused VM must stay reachable so an operator can resume it.
        .route("/readyz", get(health))
        .layer(DefaultBodyLimit::max(4096))
        .layer(axum::middleware::from_fn(headers))
        .layer(sessions)
        .with_state(app)
}

async fn headers(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let mut response = next.run(request).await;
    for (key, value) in [
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("content-security-policy", "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data: blob:; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"),
        ("strict-transport-security", "max-age=31536000"),
    ] { response.headers_mut().insert(axum::http::HeaderName::from_static(key), axum::http::HeaderValue::from_static(value)); }
    response
}

async fn identity(
    State(app): State<AppState>,
    principal: auth::Principal,
) -> Result<Json<serde_json::Value>, Failure> {
    let vm_name = app
        .control
        .config_view()
        .await
        .map_or_else(|_| "VM".into(), |view| view.name);
    Ok(Json(
        serde_json::json!({"user":principal.user,"role":principal.role,"vm_name":vm_name}),
    ))
}
async fn status(
    State(app): State<AppState>,
    _: auth::Principal,
) -> Result<Json<pod::Status>, Failure> {
    app.control
        .status()
        .await
        .map(Json)
        .map_err(|_| Failure::unavailable("The VM status is unavailable."))
}
/// Viewers receive the redacted view; only admins see storage endpoints,
/// credential references, identity, paths, node and image.
async fn config_view(
    State(app): State<AppState>,
    principal: auth::Principal,
) -> Result<Json<pod::ConfigView>, Failure> {
    let view = app
        .control
        .config_view()
        .await
        .map_err(|_| Failure::unavailable("The VM configuration is unavailable."))?;
    Ok(Json(match principal.role {
        auth::Role::Admin => view,
        auth::Role::Viewer => view.redacted(),
    }))
}
async fn health(State(app): State<AppState>) -> StatusCode {
    if app.control.probe(false).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

fn same_origin(app: &AppState, headers: &HeaderMap) -> bool {
    let origin = app.config.browser.as_ref().map(|b| b.origin.as_str());
    origin.is_some() && headers.get("origin").and_then(|v| v.to_str().ok()) == origin
}
/// Cookie-session mutations need the exact Origin and the session's CSRF
/// token. Browsers never attach bearer tokens by themselves, so bearer
/// requests need neither.
fn mutation(
    app: &AppState,
    headers: &HeaderMap,
    principal: &auth::Principal,
) -> Result<(), Failure> {
    if principal.is_bearer() {
        return Ok(());
    }
    let user = &principal.user;
    if !same_origin(app, headers)
        || headers.get("x-csrf-token").and_then(|v| v.to_str().ok()) != Some(user.csrf.as_str())
        || user.csrf.is_empty()
    {
        return Err(Failure::forbidden());
    }
    Ok(())
}
async fn action(
    State(app): State<AppState>,
    principal: auth::Principal,
    headers: HeaderMap,
    Json(action): Json<Action>,
) -> Result<StatusCode, Failure> {
    let principal = principal.admin()?;
    mutation(&app, &headers, &principal)?;
    let result = app.control.action(action).await;
    tracing::info!(subject=%principal.user.subject, bearer=principal.is_bearer(), ?action, success=result.is_ok(), "VM control requested");
    result
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|error| Failure::vm(&error, "The VM did not complete the power action."))
}
async fn logout(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<StatusCode, Failure> {
    let principal = auth::user(&app, &session).await?;
    mutation(&app, &headers, &principal)?;
    session.flush().await.map_err(|_| Failure::internal())?;
    Ok(StatusCode::NO_CONTENT)
}

async fn resource_usage(
    State(app): State<AppState>,
    _: auth::Principal,
) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({"bmc":pod::ResourceUsage::sample(),"manager":app.control.resource_usage().await.ok()}),
    )
}
