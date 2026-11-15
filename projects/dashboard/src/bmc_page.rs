//! The shared BMC interface, scoped to a dashboard VM and login.
use crate::{AppState, Failure, Role, auth, pod_ip, visible_vm};
use axum::{
    Json,
    body::{Body, Bytes},
    extract::{Path, Request, State},
    http::{Method, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use tower_sessions::Session;

pub async fn page(
    State(app): State<AppState>,
    session: Session,
    Path((ns, name)): Path<(String, String)>,
) -> Result<Response, Failure> {
    let user = auth::user(&session).await?;
    let (vm, _) = visible_vm(&app, &user, &ns, &name).await?;
    if !vm.spec.components.bmc.enabled {
        return Err(Failure::not_found());
    }
    let html = include_str!("../../bmc/assets/index.html")
        .replace("/assets/style.css", "/assets/bmc.css")
        .replace("/assets/console.js", "/assets/bmc.js")
        .replace(
            "<body>",
            &format!("<body data-vm-base=\"/vms/{ns}/{name}\">"),
        );
    Ok((
        [(header::CONTENT_SECURITY_POLICY, super::CONSOLE_CSP)],
        Html(html),
    )
        .into_response())
}

pub async fn script() -> impl IntoResponse {
    let js = include_str!("../../bmc/assets/console.js");
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        js,
    )
}

/// The BMC's fetch routes return a small JSON error, while navigation errors
/// on `page` remain normal dashboard HTML. Never send a whole error page to a
/// control that expects a Redfish-shaped response.
fn api_response<T: IntoResponse>(result: Result<T, Failure>) -> Response {
    match result {
        Ok(reply) => reply.into_response(),
        Err(Failure(status, message)) => (
            status,
            Json(serde_json::json!({"error": {"message": message}})),
        )
            .into_response(),
    }
}

pub async fn identity(
    app: State<AppState>,
    session: Session,
    path: Path<(String, String)>,
) -> Response {
    api_response(identity_inner(app, session, path).await)
}

async fn identity_inner(
    State(app): State<AppState>,
    session: Session,
    Path((ns, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, Failure> {
    let user = auth::user(&session).await?;
    let (vm, role) = visible_vm(&app, &user, &ns, &name).await?;
    let mut features = Vec::new();
    if role == Role::Admin
        && vm
            .spec
            .console
            .as_ref()
            .and_then(|c| c.admin_bucket)
            .is_some_and(|b| b.audio)
    {
        features.push("audio");
    }
    Ok(Json(
        serde_json::json!({"user":{"name":user.name,"subject":user.subject,"csrf":user.csrf},"role":if role == Role::Admin {"admin"} else {"viewer"},"vm_name":name,"admin_bucket":features}),
    ))
}

pub async fn logout(
    app: State<AppState>,
    session: Session,
    path: Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Response {
    api_response(logout_inner(app, session, path, headers).await)
}

async fn logout_inner(
    State(app): State<AppState>,
    session: Session,
    Path((ns, name)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<StatusCode, Failure> {
    let user = auth::user(&session).await?;
    visible_vm(&app, &user, &ns, &name).await?;
    guard(&app, &user, &headers)?;
    session.flush().await.map_err(|_| Failure::internal())?;
    Ok(StatusCode::NO_CONTENT)
}
fn guard(
    app: &AppState,
    user: &auth::User,
    headers: &axum::http::HeaderMap,
) -> Result<(), Failure> {
    let csrf = headers
        .get("x-csrf-token")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if !auth::mutation_allowed(user, csrf, headers, &app.origin) {
        return Err(Failure::forbidden());
    }
    Ok(())
}

pub async fn forward(
    app: State<AppState>,
    session: Session,
    path: Path<(String, String, String)>,
    request: Request,
) -> Response {
    api_response(forward_inner(app, session, path, request).await)
}

async fn forward_inner(
    State(app): State<AppState>,
    session: Session,
    Path((ns, name, path)): Path<(String, String, String)>,
    request: Request,
) -> Result<Response, Failure> {
    let user = auth::user(&session).await?;
    let (vm, role) = visible_vm(&app, &user, &ns, &name).await?;
    if !vm.spec.components.redfish_enabled() {
        return Err(Failure::not_found());
    }
    if !path.starts_with("v1/")
        || path
            .split('/')
            .any(|p| p == "." || p == ".." || p.to_ascii_lowercase().contains("%2e"))
    {
        return Err(Failure::invalid("Invalid Redfish path."));
    }
    let (parts, body) = request.into_parts();
    if !matches!(parts.method, Method::GET | Method::HEAD) {
        if role != Role::Admin {
            return Err(Failure::forbidden());
        }
        guard(&app, &user, &parts.headers)?;
    }
    let ip = pod_ip(&app, &vm)
        .await?
        .ok_or_else(|| Failure::unavailable("The VM has no running Pod."))?;
    let body = axum::body::to_bytes(body, 192 * 1024)
        .await
        .map_err(|_| Failure::invalid("Request body too large."))?;
    let suffix = parts
        .uri
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let mut upstream = app
        .redfish
        .browser_request(ip, &format!("/redfish/{path}{suffix}"), parts.method)
        .bearer_auth(&user.id_token)
        .body(body);
    for key in [
        "content-type",
        "accept",
        "last-event-id",
        "odata-version",
        "if-match",
    ] {
        if let Some(value) = parts.headers.get(key) {
            upstream = upstream.header(key, value);
        }
    }
    let mut upstream = upstream
        .send()
        .await
        .map_err(|_| Failure::unavailable("Redfish is unavailable."))?;
    let mut response = Response::builder().status(upstream.status());
    for key in [
        "content-type",
        "odata-version",
        "etag",
        "allow",
        "retry-after",
    ] {
        if let Some(value) = upstream.headers().get(key) {
            response = response.header(key, value);
        }
    }
    if path == "v1/EventService/SSE" && upstream.status().is_success() {
        let permit = app
            .consoles
            .clone()
            .try_acquire_owned()
            .map_err(|_| Failure::unavailable("Too many live streams."))?;
        let id = session.id().ok_or_else(Failure::unauthorized)?;
        let stream = futures::stream::unfold(
            (upstream, app, user, id, permit),
            |(mut upstream, app, user, id, permit)| async move {
                loop {
                    if !auth::still_current(&app.sessions, &id, &user).await {
                        return None;
                    }
                    match tokio::time::timeout(std::time::Duration::from_secs(1), upstream.chunk())
                        .await
                    {
                        Ok(Ok(Some(chunk))) => {
                            return Some((
                                Ok::<Bytes, std::io::Error>(chunk),
                                (upstream, app, user, id, permit),
                            ));
                        }
                        Ok(_) => return None,
                        Err(_) => continue,
                    }
                }
            },
        );
        return response
            .body(Body::from_stream(stream))
            .map_err(|_| Failure::internal());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = upstream
        .chunk()
        .await
        .map_err(|_| Failure::unavailable("Redfish response failed."))?
    {
        if bytes.len() + chunk.len() > crate::redfish::MAX_LOG_BODY {
            return Err(Failure::unavailable("Redfish response too large."));
        }
        bytes.extend_from_slice(&chunk);
    }
    response
        .body(Body::from(bytes))
        .map_err(|_| Failure::internal())
}
