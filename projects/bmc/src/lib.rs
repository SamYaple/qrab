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
    pub oidc: auth::Oidc,
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
    pub fn internal() -> Self {
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The request could not be completed.",
        )
    }
    pub fn unavailable() -> Self {
        Self(
            StatusCode::SERVICE_UNAVAILABLE,
            "The VM console is unavailable.",
        )
    }
}
impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

pub fn router(app: AppState) -> Router {
    let secure = app.config.origin.starts_with("https://");
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
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../assets/index.html")) }),
        )
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", post(logout))
        .route("/api/session", get(identity))
        .route("/api/status", get(status))
        .route("/api/config", get(config_view))
        .route("/api/events", get(events::page))
        .route("/api/events/stream", get(events::stream))
        .route("/api/action", post(action))
        .route("/api/console", get(console::upgrade))
        .route("/healthz", get(|| async { "ok" }))
        // A paused VM must stay reachable so an operator can resume it.
        .route("/readyz", get(health))
        .nest_service(
            "/assets",
            tower_http::services::ServeDir::new(app.config.assets_dir.clone()),
        )
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
    session: Session,
) -> Result<Json<serde_json::Value>, Failure> {
    let principal = auth::user(&app, &session).await?;
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
    session: Session,
) -> Result<Json<pod::Status>, Failure> {
    auth::user(&app, &session).await?;
    app.control
        .status()
        .await
        .map(Json)
        .map_err(|_| Failure::unavailable())
}
/// Viewers receive the redacted view; only admins see storage endpoints,
/// credential references, identity, paths, node and image.
async fn config_view(
    State(app): State<AppState>,
    session: Session,
) -> Result<Json<pod::ConfigView>, Failure> {
    let principal = auth::user(&app, &session).await?;
    let view = app
        .control
        .config_view()
        .await
        .map_err(|_| Failure::unavailable())?;
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
    headers.get("origin").and_then(|v| v.to_str().ok()) == Some(app.config.origin.as_str())
}
fn mutation(app: &AppState, headers: &HeaderMap, user: &auth::User) -> Result<(), Failure> {
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
    session: Session,
    headers: HeaderMap,
    Json(action): Json<Action>,
) -> Result<StatusCode, Failure> {
    let user = auth::admin(&app, &session).await?;
    mutation(&app, &headers, &user)?;
    let result = app.control.action(action).await;
    tracing::info!(subject=%user.subject, ?action, success=result.is_ok(), "VM control requested");
    result
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| Failure::unavailable())
}
async fn logout(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Result<StatusCode, Failure> {
    let user = auth::user(&app, &session).await?.user;
    mutation(&app, &headers, &user)?;
    session.flush().await.map_err(|_| Failure::internal())?;
    Ok(StatusCode::NO_CONTENT)
}
