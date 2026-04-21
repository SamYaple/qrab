//! BMC server auth / bind gating — S7-21.
//!
//! The BMC shipped with an SSE event feed and a set of `#[server]`
//! functions (power on/off, QMP exec) reachable by any HTTP client. This
//! module enforces a coherent default: bind to loopback and require a
//! shared-secret bearer token for every route if the operator opens the
//! listener to the network.
//!
//! Posture (strongest enforced automatically):
//!
//! | Bind             | `QRAB_BMC_AUTH_TOKEN` | Outcome                        |
//! |------------------|-----------------------|--------------------------------|
//! | loopback default | unset                 | allow (dev), loud log line     |
//! | loopback         | set                   | token required on every route  |
//! | non-loopback     | unset                 | startup error, refuse to serve |
//! | non-loopback     | set                   | token required on every route  |
//!
//! The bind address is resolved from three overlapping sources, in
//! priority order:
//!
//! 1. `QRAB_BMC_BIND=host:port` — operator-facing knob.
//! 2. Dioxus `IP` + `PORT` env vars (set by `dx serve`).
//! 3. Default `127.0.0.1:8080`.
//!
//! `QRAB_BMC_BIND` wins because we then forward the parsed pieces back
//! into `IP`/`PORT` so `dioxus::serve` (which reads those directly, see
//! `dioxus_cli_config::fullstack_address_or_localhost`) binds to the
//! same address we validated.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::{bail, Context, Result};
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

/// Operator override for the BMC bind address. If set, takes priority
/// over the Dioxus `IP`/`PORT` pair.
pub const BIND_ENV: &str = "QRAB_BMC_BIND";

/// Shared-secret bearer token. Required when the resolved bind address
/// is not loopback; optional (and a loud dev-mode warning fires) when
/// the bind is loopback.
pub const TOKEN_ENV: &str = "QRAB_BMC_AUTH_TOKEN";

/// Dioxus-reserved env vars; we write them so our resolved address is
/// what `dioxus::serve` actually binds. Kept as constants to match
/// `dioxus_cli_config::SERVER_IP_ENV` / `SERVER_PORT_ENV`.
const DIOXUS_IP_ENV: &str = "IP";
const DIOXUS_PORT_ENV: &str = "PORT";

/// Fully resolved BMC auth + bind configuration, ready for use by
/// `main.rs` and the auth middleware.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// The socket the server will bind on. Printed at startup so the
    /// operator knows where traffic is actually accepted.
    pub bind: SocketAddr,
    /// Optional shared-secret bearer token. Required when `bind` is
    /// non-loopback; optional on loopback (dev mode).
    pub token: Option<String>,
}

impl AuthConfig {
    /// Is this bind locally-only reachable? The middleware relaxes its
    /// requirements for loopback, and startup refuses to serve a
    /// non-loopback bind without a token.
    #[must_use]
    pub fn is_loopback(&self) -> bool {
        self.bind.ip().is_loopback()
    }
}

/// Resolve the BMC auth posture from the environment.
///
/// Errors on any misconfiguration that would leave the BMC open:
/// non-loopback bind without a token, or a malformed `QRAB_BMC_BIND`
/// value. Callers should surface the error to the operator and exit
/// rather than attempting a silent fallback.
pub fn resolve_from_env() -> Result<AuthConfig> {
    resolve_from(|k| std::env::var(k).ok())
}

/// Resolve from an arbitrary env-like closure. Used by tests to avoid
/// polluting the process environment (and to sidestep the
/// test-parallel races that come with it).
pub fn resolve_from<F>(mut env: F) -> Result<AuthConfig>
where
    F: FnMut(&str) -> Option<String>,
{
    let bind = resolve_bind(&mut env).context("resolving BMC bind address")?;
    let token = env(TOKEN_ENV).and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    if !bind.ip().is_loopback() && token.is_none() {
        bail!(
            "BMC bind {bind} is non-loopback but {TOKEN_ENV} is unset; \
             set a shared-secret token or bind to loopback (127.0.0.1)"
        );
    }

    Ok(AuthConfig { bind, token })
}

/// Resolve the bind address in priority order. `QRAB_BMC_BIND` wins;
/// otherwise the Dioxus `IP`+`PORT` pair; otherwise `127.0.0.1:8080`.
fn resolve_bind<F>(env: &mut F) -> Result<SocketAddr>
where
    F: FnMut(&str) -> Option<String>,
{
    if let Some(raw) = env(BIND_ENV) {
        return raw.parse::<SocketAddr>().with_context(|| {
            format!("{BIND_ENV}={raw:?} must parse as host:port (e.g. 127.0.0.1:8080)")
        });
    }

    let ip = match env(DIOXUS_IP_ENV) {
        Some(v) => v
            .parse::<IpAddr>()
            .with_context(|| format!("{DIOXUS_IP_ENV}={v:?} must parse as an IP address"))?,
        None => IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
    };
    let port = match env(DIOXUS_PORT_ENV) {
        Some(v) => v
            .parse::<u16>()
            .with_context(|| format!("{DIOXUS_PORT_ENV}={v:?} must parse as a u16"))?,
        None => 8080,
    };

    Ok(SocketAddr::new(ip, port))
}

/// Push the resolved bind back into `IP`/`PORT` so `dioxus::serve` —
/// which reads those directly via `dioxus_cli_config` — honours our
/// resolution. Safe to call even when the values came from `IP`/`PORT`
/// originally (write-back is a no-op in that case).
///
/// `std::env::set_var` is marked `unsafe` in the 2024 edition because
/// concurrent `getenv` is UB on POSIX. This call site runs from `main`
/// BEFORE `dioxus::serve` spawns the tokio runtime — no other threads
/// exist yet — so the scoped `allow(unsafe_code)` is genuinely safe.
#[allow(unsafe_code)]
pub fn export_bind_env(cfg: &AuthConfig) {
    // SAFETY: single-threaded startup context; nothing else reads env.
    unsafe {
        std::env::set_var(DIOXUS_IP_ENV, cfg.bind.ip().to_string());
        std::env::set_var(DIOXUS_PORT_ENV, cfg.bind.port().to_string());
    }
}

/// Emit a one-line startup summary so the operator can see at a glance
/// what posture the BMC actually landed in.
pub fn log_startup(cfg: &AuthConfig) {
    match (cfg.is_loopback(), cfg.token.is_some()) {
        (true, false) => tracing::warn!(
            bind = %cfg.bind,
            "BMC: DEV MODE — loopback bind, no auth token; set {TOKEN_ENV} to require bearer auth"
        ),
        (true, true) => tracing::info!(bind = %cfg.bind, "BMC: loopback bind, bearer token required"),
        (false, true) => tracing::warn!(
            bind = %cfg.bind,
            "BMC: NON-LOOPBACK BIND — exposed to the network; bearer token required"
        ),
        // Unreachable: `resolve_from_env` bails for (non-loopback, no token).
        (false, false) => tracing::error!(
            bind = %cfg.bind,
            "BMC: NON-LOOPBACK BIND WITH NO TOKEN — this should have been rejected at resolve time"
        ),
    }
}

/// Axum middleware wrapping the full BMC route set. Enforces bearer
/// auth whenever `cfg.token` is set; a `None` token means the operator
/// opted into dev mode (possible only on a loopback bind).
pub async fn require_token(
    State(cfg): State<AuthConfig>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let Some(expected) = cfg.token.as_deref() else {
        return next.run(req).await;
    };

    match extract_bearer(&req) {
        Some(tok) if constant_time_eq(tok.as_bytes(), expected.as_bytes()) => next.run(req).await,
        _ => unauthorized(),
    }
}

/// Pull the bearer value out of the `Authorization` header, if any.
/// Case-insensitive scheme match per RFC 7235.
fn extract_bearer<B>(req: &Request<B>) -> Option<&str> {
    let raw = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, value) = raw.split_once(' ')?;
    if scheme.eq_ignore_ascii_case("Bearer") {
        Some(value.trim())
    } else {
        None
    }
}

/// Avoid timing-oracle on token compare. Inputs can differ in length so
/// we fold that into the result rather than short-circuiting.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn unauthorized() -> Response {
    // `WWW-Authenticate` is required by RFC 7235 on any 401. The realm
    // label is purely advisory.
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::WWW_AUTHENTICATE, "Bearer realm=\"qrab-bmc\"")
        .body(Body::empty())
        .expect("static 401 response must build")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::Request;
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use axum::Router;
    use std::collections::HashMap;
    use tower::ServiceExt;

    /// Build a closure over an owned HashMap so each test gets an
    /// isolated env snapshot — critical because `cargo test` runs
    /// tests in parallel and `std::env::set_var` is process-global.
    fn env_from(pairs: &[(&str, &str)]) -> impl FnMut(&str) -> Option<String> {
        let map: HashMap<String, String> =
            pairs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect();
        move |k| map.get(k).cloned()
    }

    // ----- resolve_from -----

    #[test]
    fn loopback_default_no_token_is_dev_mode() {
        let cfg = resolve_from(env_from(&[])).expect("loopback + no token is allowed");
        assert!(cfg.is_loopback());
        assert_eq!(cfg.bind, SocketAddr::from(([127, 0, 0, 1], 8080)));
        assert!(cfg.token.is_none());
    }

    #[test]
    fn loopback_with_token_carries_token() {
        let cfg = resolve_from(env_from(&[("QRAB_BMC_AUTH_TOKEN", "s3cret")]))
            .expect("loopback + token is allowed");
        assert!(cfg.is_loopback());
        assert_eq!(cfg.token.as_deref(), Some("s3cret"));
    }

    #[test]
    fn non_loopback_without_token_fails_startup() {
        let err = resolve_from(env_from(&[("QRAB_BMC_BIND", "0.0.0.0:8080")]))
            .expect_err("non-loopback without token must be rejected");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("non-loopback") && msg.contains("QRAB_BMC_AUTH_TOKEN"),
            "error should explain why: {msg}"
        );
    }

    #[test]
    fn non_loopback_with_token_ok() {
        let cfg = resolve_from(env_from(&[
            ("QRAB_BMC_BIND", "10.0.0.5:7100"),
            ("QRAB_BMC_AUTH_TOKEN", "hunter2"),
        ]))
        .expect("non-loopback + token is allowed");
        assert!(!cfg.is_loopback());
        assert_eq!(cfg.bind, SocketAddr::from(([10, 0, 0, 5], 7100)));
        assert_eq!(cfg.token.as_deref(), Some("hunter2"));
    }

    #[test]
    fn empty_token_env_is_treated_as_unset() {
        // Operators might export an empty value from a shell template
        // and expect dev mode. Otherwise a blank token would satisfy the
        // non-loopback gate and leave the BMC open.
        let cfg = resolve_from(env_from(&[("QRAB_BMC_AUTH_TOKEN", "   ")]))
            .expect("loopback + blank token is allowed");
        assert!(cfg.token.is_none());

        let err = resolve_from(env_from(&[
            ("QRAB_BMC_BIND", "0.0.0.0:8080"),
            ("QRAB_BMC_AUTH_TOKEN", "   "),
        ]))
        .expect_err("blank token must not satisfy non-loopback gate");
        assert!(format!("{err:#}").contains("non-loopback"));
    }

    #[test]
    fn bind_precedence_over_ip_port() {
        let cfg = resolve_from(env_from(&[
            ("QRAB_BMC_BIND", "127.0.0.1:9999"),
            ("IP", "0.0.0.0"),
            ("PORT", "80"),
        ]))
        .expect("QRAB_BMC_BIND wins");
        assert_eq!(cfg.bind, SocketAddr::from(([127, 0, 0, 1], 9999)));
    }

    #[test]
    fn dioxus_ip_port_respected_when_bind_unset() {
        let cfg = resolve_from(env_from(&[("IP", "127.0.0.1"), ("PORT", "9100")]))
            .expect("dioxus IP+PORT pair is a fine bind");
        assert_eq!(cfg.bind, SocketAddr::from(([127, 0, 0, 1], 9100)));
    }

    #[test]
    fn malformed_bind_is_startup_error() {
        let err = resolve_from(env_from(&[("QRAB_BMC_BIND", "not-a-socket")]))
            .expect_err("garbage QRAB_BMC_BIND must not silently default");
        assert!(format!("{err:#}").contains("QRAB_BMC_BIND"));
    }

    // ----- middleware -----

    async fn ok_handler() -> &'static str {
        "ok"
    }

    fn router_with(cfg: AuthConfig) -> Router {
        Router::new().route("/api/events", get(ok_handler)).layer(
            from_fn_with_state(cfg, require_token),
        )
    }

    fn get_req(path: &str, auth: Option<&str>) -> Request<Body> {
        let mut b = Request::builder().uri(path);
        if let Some(tok) = auth {
            b = b.header(header::AUTHORIZATION, format!("Bearer {tok}"));
        }
        b.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn loopback_no_token_permits_all() {
        let cfg = AuthConfig {
            bind: SocketAddr::from(([127, 0, 0, 1], 8080)),
            token: None,
        };
        let router = router_with(cfg);
        let resp = router.oneshot(get_req("/api/events", None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_token_is_401() {
        let cfg = AuthConfig {
            bind: SocketAddr::from(([127, 0, 0, 1], 8080)),
            token: Some("topsecret".into()),
        };
        let router = router_with(cfg);
        let resp = router.oneshot(get_req("/api/events", None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            resp.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "Bearer realm=\"qrab-bmc\""
        );
    }

    #[tokio::test]
    async fn wrong_token_is_401() {
        let cfg = AuthConfig {
            bind: SocketAddr::from(([127, 0, 0, 1], 8080)),
            token: Some("topsecret".into()),
        };
        let router = router_with(cfg);
        let resp = router
            .oneshot(get_req("/api/events", Some("bogus")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn correct_token_passes() {
        let cfg = AuthConfig {
            bind: SocketAddr::from(([127, 0, 0, 1], 8080)),
            token: Some("topsecret".into()),
        };
        let router = router_with(cfg);
        let resp = router
            .oneshot(get_req("/api/events", Some("topsecret")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 64).await.unwrap();
        assert_eq!(&body[..], b"ok");
    }

    #[tokio::test]
    async fn non_bearer_auth_scheme_is_401() {
        // Someone sending `Authorization: Basic ...` shouldn't squeak
        // through even if the Base64 body happens to match the token.
        let cfg = AuthConfig {
            bind: SocketAddr::from(([127, 0, 0, 1], 8080)),
            token: Some("topsecret".into()),
        };
        let router = router_with(cfg);
        let req = Request::builder()
            .uri("/api/events")
            .header(header::AUTHORIZATION, "Basic topsecret")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn constant_time_eq_basic() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }
}
