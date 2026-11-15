//! Browser-only `/bmc/redfish/*` bridge to the in-pod Redfish sidecar, the VM's single
//! management plane. A browser session's request is authorized here (the
//! exact Origin and CSRF token for mutations) and forwarded with the login's
//! own ID token. Explicit Redfish credentials are forwarded unchanged;
//! Redfish verifies them and decides the role. No cookie or identity header
//! is sent upstream, and invalid explicit credentials never select a login.
use crate::{auth, AppState, Failure};
use axum::{
    body::{Body, Bytes},
    extract::{Request, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::Response,
};
use futures::StreamExt;
use openidconnect::reqwest;
use std::time::Duration;
use tower_sessions::Session;

/// Redfish's own request-body bounds.
const BODY_LIMIT: usize = 4096;
const SIGNED_UPDATE_LIMIT: usize = 192 * 1024;
const SIGNED_UPDATE: &str =
    "/redfish/v1/Systems/1/SecureBoot/Actions/Oem/QrabSecureBoot.UpdateAuthenticatedDatabase";
const SSE: &str = "/redfish/v1/EventService/SSE";
/// Bound on a non-streaming Redfish exchange.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

const REQUEST_HEADERS: [HeaderName; 5] = [
    header::CONTENT_TYPE,
    header::ACCEPT,
    HeaderName::from_static("last-event-id"),
    HeaderName::from_static("odata-version"),
    HeaderName::from_static("odata-maxversion"),
];
const RESPONSE_HEADERS: [HeaderName; 9] = [
    header::CONTENT_TYPE,
    HeaderName::from_static("odata-version"),
    header::LOCATION,
    header::ETAG,
    header::ALLOW,
    header::RETRY_AFTER,
    header::LINK,
    header::WWW_AUTHENTICATE,
    HeaderName::from_static("x-auth-token"),
];

/// The Redfish sidecar: no redirects, no environment proxy.
#[derive(Clone)]
pub struct Upstream {
    origin: String,
    client: reqwest::Client,
}

impl Upstream {
    /// `origin` is an HTTP(S) origin such as `http://127.0.0.1:8081`.
    pub fn new(origin: &str) -> anyhow::Result<Self> {
        let url = reqwest::Url::parse(origin)?;
        anyhow::ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
                && url.path() == "/",
            "Redfish transport requires a bare origin"
        );
        let loopback = match url.host() {
            Some(openidconnect::url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(openidconnect::url::Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
        anyhow::ensure!(
            url.scheme() == "http" && loopback,
            "plain Redfish transport is local loopback only; HTTPS requires an exact SPIFFE peer"
        );
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .build()?;
        Ok(Self {
            origin: origin.trim_end_matches('/').into(),
            client,
        })
    }

    pub fn mtls(
        origin: &str,
        identity: &qrab_spiffe::Identity,
        peer: qrab_spiffe::SpiffeId,
    ) -> anyhow::Result<Self> {
        let url = reqwest::Url::parse(origin)?;
        anyhow::ensure!(
            url.scheme() == "https"
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
                && url.path() == "/",
            "Redfish mTLS requires a bare HTTPS origin"
        );
        Ok(Self {
            origin: origin.trim_end_matches('/').into(),
            client: qrab_spiffe::http::client(identity, peer)?,
        })
    }
}

/// How the forwarded request authenticates to Redfish.
enum Forward {
    /// All supplied authentication values, including malformed or ambiguous
    /// ones, reach Redfish without browser-session fallback.
    Explicit,
    /// A signed-in browser session, forwarded with its login's ID token.
    Session(auth::Principal, Option<String>),
    /// No credential: Redfish answers 401 itself.
    Anonymous,
}

fn dot_segment(path: &str) -> bool {
    path.split('/').any(|segment| {
        segment == "." || segment == ".." || segment.to_ascii_lowercase().contains("%2e")
    })
}

pub async fn forward(
    State(app): State<AppState>,
    session: Session,
    request: Request,
) -> Result<Response, Failure> {
    let (parts, body) = request.into_parts();
    if dot_segment(parts.uri.path()) {
        return Err(Failure::bad_request("Invalid Redfish path."));
    }
    // Redfish decides method support and returns its own Allow/error response.
    // Browser login fallback remains conservative for every non-GET/HEAD.
    let read_only = matches!(parts.method, Method::GET | Method::HEAD);
    let path = parts
        .uri
        .path()
        .strip_prefix("/bmc")
        .filter(|path| *path == "/redfish" || path.starts_with("/redfish/"))
        .ok_or_else(|| Failure::bad_request("Invalid browser Redfish path."))?;
    let explicit = parts.headers.contains_key(header::AUTHORIZATION)
        || parts.headers.contains_key("x-auth-token");
    let password_bootstrap = parts.method == Method::POST
        && matches!(
            path,
            "/redfish/v1/SessionService/Sessions" | "/redfish/v1/SessionService/Sessions/Members"
        );
    let forward = match explicit {
        true => Forward::Explicit,
        false => match auth::user(&app, &session).await {
            Ok(principal) => {
                let auth::Credential::Session(id) = &principal.credential else {
                    return Err(Failure::internal());
                };
                // Checked before a single byte goes upstream.
                if read_only {
                    if parts.headers.contains_key(header::ORIGIN)
                        && !crate::same_origin(&app, &parts.headers)
                    {
                        return Err(Failure::forbidden());
                    }
                } else {
                    crate::mutation(&app, &parts.headers, &principal)?;
                }
                // Password session bootstrap is authenticated by its JSON
                // credentials in Redfish. An ambient browser login still
                // requires Origin/CSRF, but must not inject another credential.
                let token = if password_bootstrap {
                    None
                } else {
                    Some(
                        auth::session_token(&app, id)
                            .await
                            .ok_or_else(Failure::unauthorized)?,
                    )
                };
                Forward::Session(principal, token)
            }
            Err(_) => Forward::Anonymous,
        },
    };
    let limit = if path == SIGNED_UPDATE {
        SIGNED_UPDATE_LIMIT
    } else {
        BODY_LIMIT
    };
    let too_large = || {
        Failure(
            StatusCode::PAYLOAD_TOO_LARGE,
            "The request body is too large.",
        )
    };
    if parts
        .headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
        .is_some_and(|length| length > limit)
    {
        return Err(too_large());
    }
    let body: Bytes = axum::body::to_bytes(body, limit)
        .await
        .map_err(|_| too_large())?;
    let stream = path == SSE;
    let permit = if stream {
        Some(
            app.event_streams
                .clone()
                .try_acquire_owned()
                .map_err(|_| Failure::unavailable("Too many live event streams are open."))?,
        )
    } else {
        None
    };
    let target = format!(
        "{}{}",
        app.redfish.origin,
        parts
            .uri
            .path_and_query()
            .map_or(path, |p| p.as_str().strip_prefix("/bmc").unwrap_or(path))
    );
    let mut upstream = app.redfish.client.request(parts.method.clone(), target);
    if !stream {
        upstream = upstream.timeout(REQUEST_TIMEOUT);
    }
    let mut headers = HeaderMap::new();
    for name in REQUEST_HEADERS {
        for value in parts.headers.get_all(&name) {
            headers.append(name.clone(), value.clone());
        }
    }
    match &forward {
        Forward::Explicit => {
            for name in [
                header::AUTHORIZATION,
                HeaderName::from_static("x-auth-token"),
            ] {
                for value in parts.headers.get_all(&name) {
                    let mut value = value.clone();
                    value.set_sensitive(true);
                    headers.append(name.clone(), value);
                }
            }
        }
        Forward::Session(_, Some(token)) => {
            let mut value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| Failure::unauthorized())?;
            value.set_sensitive(true);
            headers.insert(header::AUTHORIZATION, value);
        }
        Forward::Session(_, None) | Forward::Anonymous => {}
    }
    let reply = upstream
        .headers(headers)
        .body(body)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                Failure(
                    StatusCode::GATEWAY_TIMEOUT,
                    "The VM's Redfish service did not answer in time.",
                )
            } else {
                Failure::unavailable("The VM's Redfish service is unavailable.")
            }
        })?;
    let status = reply.status();
    if let Forward::Session(principal, _) = &forward {
        if !parts.method.is_safe() {
            tracing::info!(subject=%principal.user.subject, method=%parts.method, path, status=status.as_u16(), "Redfish request proxied");
        }
        // The BMC accepted this login; Redfish refusing its token is a
        // configuration fault, not a signed-out browser.
        if status == StatusCode::UNAUTHORIZED && matches!(&forward, Forward::Session(_, Some(_))) {
            return Err(Failure(
                StatusCode::BAD_GATEWAY,
                "The VM's Redfish service did not accept this sign-in.",
            ));
        }
    }
    let mut response = Response::builder().status(status);
    for name in RESPONSE_HEADERS {
        for value in reply.headers().get_all(&name) {
            let mut value = value.clone();
            if name == "x-auth-token" {
                value.set_sensitive(true);
            }
            response = response.header(name.clone(), value);
        }
    }
    let chunks = futures::stream::unfold(Some((reply, permit)), |state| async move {
        let (mut reply, permit) = state?;
        match reply.chunk().await {
            Ok(Some(bytes)) => Some((Ok(bytes), Some((reply, permit)))),
            Ok(None) => None,
            Err(error) => Some((Err(std::io::Error::other(error)), None)),
        }
    });
    let body = match forward {
        // Logout, expiry or role loss ends the response, rechecked at least
        // once a second; bearer streams end at the token's expiry in Redfish.
        Forward::Session(principal, _) => Body::from_stream(
            chunks.take_until(async move { principal.ended(&app, auth::Role::Viewer).await }),
        ),
        Forward::Explicit | Forward::Anonymous => Body::from_stream(chunks),
    };
    response.body(body).map_err(|_| Failure::internal())
}
