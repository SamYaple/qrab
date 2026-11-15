pub mod admin_bucket;
pub mod auth;
pub mod config;
mod console;
pub mod proxy;
pub mod rfb_filter;
pub mod session_store;

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{any, get, post},
    Json, Router,
};
use manager::pod;
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
    /// Live Redfish event streams proxied at once.
    pub event_streams: Arc<Semaphore>,
    /// The in-pod Redfish sidecar behind the browser-only `/bmc/redfish/*` bridge.
    pub redfish: proxy::Upstream,
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
            manager::Error::PoweredOff => Self(StatusCode::CONFLICT, "The VM is powered off."),
            _ => Self::unavailable(message),
        }
    }
}
impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

/// The BMC serves login, UI assets, `/api/session`, `/api/console`, the
/// browser-session `/bmc/redfish/*` bridge and probes. Native Redfish clients
/// reach the sidecar directly through Ingress. Never add duplicate `/api/*`
/// management routes. Without `BMC_PUBLIC_URL` there is no login or UI.
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
        .route("/api/console", get(console::upgrade))
        .route("/bmc/redfish", any(proxy::forward))
        .route("/bmc/redfish/", any(proxy::forward))
        .route("/bmc/redfish/{*path}", any(proxy::forward))
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
    let mut body =
        serde_json::json!({"user":principal.user,"role":principal.role,"vm_name":vm_name});
    // Only admins learn which risky features this VM enables.
    if principal.role == auth::Role::Admin {
        body["admin_bucket"] = serde_json::json!(app.config.admin_bucket);
    }
    Ok(Json(body))
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
