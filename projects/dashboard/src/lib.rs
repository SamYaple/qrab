//! qrab-dashboard: launch, inspect and open the consoles of VMs through one
//! Kanidm OIDC client. Each VM's Redfish service (status, events, power) and
//! BMC console are called server-side with the user's ID token as a bearer;
//! the token never reaches the browser.
pub mod access;
pub mod auth;
pub mod bmc;
mod bmc_page;
pub mod cluster;
pub mod config;
pub mod console;
pub mod images;
pub mod kanidm;
pub mod launch;
pub mod model;
pub mod power;
pub mod redfish;
pub mod session_store;
#[cfg(test)]
mod tests;
pub mod ui;

use access::Role;
use axum::{
    Router,
    extract::{DefaultBodyLimit, Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use qrab_operator::api::VirtualMachine;
use std::{net::IpAddr, path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;
use tower_sessions::{Expiry, Session, SessionManagerLayer, cookie::SameSite};

#[derive(Clone)]
pub struct AppState {
    pub oidc: auth::Oidc,
    pub origin: String,
    pub namespace: String,
    pub launcher_group: String,
    pub vm_users_group: String,
    pub storage_node: Option<String>,
    pub cluster: Arc<dyn cluster::Cluster>,
    pub kanidm: kanidm::Kanidm,
    /// Each VM's console WebSocket (port 8080).
    pub bmc: bmc::Bmc,
    /// Each VM's Redfish service (port 8081).
    pub redfish: redfish::Redfish,
    pub sessions: session_store::BoundedStore,
    pub novnc_dir: PathBuf,
    /// Global cap on open consoles.
    pub consoles: Arc<Semaphore>,
    /// Open consoles per subject, capped at `console::PER_SUBJECT`.
    pub console_subjects: console::SubjectCounts,
}

pub struct Failure(pub StatusCode, pub String);
impl Failure {
    pub fn unauthorized() -> Self {
        Self(
            StatusCode::UNAUTHORIZED,
            "Sign in again to continue.".into(),
        )
    }
    pub fn forbidden() -> Self {
        Self(
            StatusCode::FORBIDDEN,
            "This request is not authorized.".into(),
        )
    }
    pub fn not_found() -> Self {
        Self(
            StatusCode::NOT_FOUND,
            "This VM does not exist or is not shared with you.".into(),
        )
    }
    pub fn internal() -> Self {
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "The request could not be completed.".into(),
        )
    }
    pub fn invalid(message: &str) -> Self {
        Self(StatusCode::BAD_REQUEST, message.into())
    }
    pub fn unavailable(message: &str) -> Self {
        Self(StatusCode::SERVICE_UNAVAILABLE, message.into())
    }
    pub fn upstream(error: &anyhow::Error) -> Self {
        tracing::warn!(error=%cluster::describe(error), "Kubernetes request failed");
        Self(
            StatusCode::BAD_GATEWAY,
            "Kubernetes could not complete this request.".into(),
        )
    }
}
impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let error = ui::ErrorPage {
            status: self.0.as_u16(),
            message: self.1,
            return_to: None,
        };
        let mut response = (
            self.0,
            Html(ui::render(ui::Page {
                body: ui::Body::Error(error.clone()),
                ..Default::default()
            })),
        )
            .into_response();
        response.extensions_mut().insert(error);
        response
    }
}

/// The dashboard's public origin: HTTPS, or loopback HTTP when explicitly allowed.
pub fn origin(raw: &str, allow_local_http: bool) -> anyhow::Result<String> {
    let url = url::Url::parse(raw)?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    anyhow::ensure!(
        url.scheme() == "https" || (allow_local_http && local && url.scheme() == "http"),
        "Dashboard origin must use HTTPS, except explicitly allowed loopback development"
    );
    anyhow::ensure!(
        url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none(),
        "Dashboard URL must be an origin"
    );
    Ok(url.origin().ascii_serialization())
}

const PAGE_CSP: &str = "default-src 'none'; style-src 'self'; img-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'";
/// noVNC needs scripts, a same-origin WebSocket and canvas images.
const CONSOLE_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data: blob:; form-action 'self'; base-uri 'none'; frame-ancestors 'none'";

pub fn router(state: AppState) -> Router {
    let secure = state.origin.starts_with("https://");
    let sessions = SessionManagerLayer::new(state.sessions.clone())
        .with_name(if secure {
            "__Host-qrab-dashboard"
        } else {
            "qrab-dashboard-dev"
        })
        .with_secure(secure)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::minutes(15)));
    let novnc = tower_http::services::ServeDir::new(state.novnc_dir.clone());
    Router::new()
        .route("/", get(home))
        .route(
            "/launch",
            get(launch_form)
                .post(launch)
                .layer(DefaultBodyLimit::max(launch::LAUNCH_BODY_LIMIT)),
        )
        .route("/images", get(images_page))
        .route("/vms/{namespace}/{name}", get(detail))
        .route("/vms/{namespace}/{name}/action", post(action))
        .route("/vms/{namespace}/{name}/delete", post(delete))
        .route(
            "/vms/{namespace}/{name}/power",
            get(power::status).post(power::act),
        )
        .route("/vms/{namespace}/{name}/bmc", get(bmc_page::page))
        .route(
            "/vms/{namespace}/{name}/bmc/session",
            get(bmc_page::identity),
        )
        .route("/vms/{namespace}/{name}/bmc/logout", post(bmc_page::logout))
        .route(
            "/vms/{namespace}/{name}/bmc/redfish/{*path}",
            axum::routing::any(bmc_page::forward).layer(DefaultBodyLimit::max(192 * 1024)),
        )
        .route("/assets/bmc.js", get(bmc_page::script))
        .route(
            "/assets/favicon.svg",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
                    include_str!("../../bmc/assets/favicon.svg"),
                )
            }),
        )
        .route(
            "/assets/bmc.css",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../../bmc/assets/style.css"),
                )
            }),
        )
        .route("/vms/{namespace}/{name}/console", get(console_page))
        .route("/vms/{namespace}/{name}/console/ws", get(console::upgrade))
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", post(logout))
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(|| async { "ok" }))
        .route(
            "/assets/style.css",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../assets/style.css"),
                )
            }),
        )
        .route(
            "/assets/console.js",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("../assets/console.js"),
                )
            }),
        )
        .nest_service("/assets/novnc", novnc)
        .fallback(|| async {
            Failure(
                StatusCode::NOT_FOUND,
                "This page could not be found.".into(),
            )
        })
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(axum::middleware::from_fn(headers))
        .layer(sessions)
        .with_state(state)
}

async fn headers(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    // Request context is used only for safe GET recovery links. Never retry a
    // submitted action or replace its status with a sign-in redirect.
    let path = request.uri().path();
    let return_to = auth::return_path(path).or_else(|| {
        ["/action", "/delete"]
            .iter()
            .find_map(|suffix| path.strip_suffix(suffix).and_then(auth::return_path))
    });
    let mut response = next.run(request).await;
    if let Some(mut error) = response.extensions_mut().remove::<ui::ErrorPage>()
        && let Some(path) = return_to
    {
        error.return_to = Some(path);
        *response.body_mut() = axum::body::Body::from(ui::render(ui::Page {
            body: ui::Body::Error(error),
            ..Default::default()
        }));
        response
            .headers_mut()
            .remove(axum::http::header::CONTENT_LENGTH);
    }
    let h = response.headers_mut();
    if !h.contains_key("content-security-policy") {
        h.insert(
            "content-security-policy",
            axum::http::HeaderValue::from_static(PAGE_CSP),
        );
    }
    for (key, value) in [
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "same-origin"),
        ("strict-transport-security", "max-age=31536000"),
    ] {
        h.insert(
            axum::http::HeaderName::from_static(key),
            axum::http::HeaderValue::from_static(value),
        );
    }
    response
}

fn user_view(app: &AppState, user: &auth::User) -> ui::UserView {
    ui::UserView {
        name: user.name.clone(),
        csrf: user.csrf.clone(),
        launcher: access::can_launch(&user.groups, &app.launcher_group),
    }
}

async fn page(
    app: &AppState,
    session: &Session,
    user: &auth::User,
    body: ui::Body,
) -> Result<Html<String>, Failure> {
    let notice: Option<String> = session
        .remove("flash")
        .await
        .map_err(|_| Failure::internal())?;
    Ok(Html(ui::render(ui::Page {
        user: Some(user_view(app, user)),
        notice,
        body,
    })))
}

async fn flash(session: &Session, message: String) -> Result<(), Failure> {
    session
        .insert("flash", message)
        .await
        .map_err(|_| Failure::internal())
}

/// The VM and the user's role, or 404 for absent and invisible VMs alike.
pub(crate) async fn visible_vm(
    app: &AppState,
    user: &auth::User,
    namespace: &str,
    name: &str,
) -> Result<(VirtualMachine, Role), Failure> {
    if namespace != app.namespace || !config::dns_label(name) {
        return Err(Failure::not_found());
    }
    let vm = app
        .cluster
        .get_vm(name)
        .await
        .map_err(|e| Failure::upstream(&e))?
        .ok_or_else(Failure::not_found)?;
    let role = access::role(&vm, &user.subject, &user.groups).ok_or_else(Failure::not_found)?;
    Ok((vm, role))
}

/// The pod IP of this VM's own Pod, if it is running.
pub(crate) async fn pod_ip(app: &AppState, vm: &VirtualMachine) -> Result<Option<IpAddr>, Failure> {
    let Some(name) = vm.status.as_ref().and_then(|s| s.pod_name.as_deref()) else {
        return Ok(None);
    };
    if !config::dns_label(name) {
        return Ok(None);
    }
    let pod = app
        .cluster
        .get_pod(name)
        .await
        .map_err(|e| Failure::upstream(&e))?;
    Ok(pod.and_then(|pod| cluster::vm_pod_ip(vm, &pod)))
}

async fn home(State(app): State<AppState>, session: Session) -> Result<Html<String>, Failure> {
    let user = match auth::user(&session).await {
        Ok(u) => u,
        Err(e) if e.0 == StatusCode::UNAUTHORIZED => {
            return Ok(Html(ui::render(ui::Page::default())));
        }
        Err(e) => return Err(e),
    };
    let vms = app
        .cluster
        .list_vms()
        .await
        .map_err(|e| Failure::upstream(&e))?;
    let usage = app.cluster.pod_usage().await.unwrap_or_else(|error| {
        tracing::warn!(error=%cluster::describe(&error), "pod metrics unavailable");
        Default::default()
    });
    let rows = access::visible(&vms, &user.subject, &user.groups)
        .into_iter()
        .map(|(vm, role)| model::row(vm, role, &usage))
        .collect();
    page(&app, &session, &user, ui::Body::List(rows)).await
}

async fn detail(
    State(app): State<AppState>,
    session: Session,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<Html<String>, Failure> {
    let user = auth::user(&session).await?;
    let (vm, role) = visible_vm(&app, &user, &namespace, &name).await?;
    let pod = match vm.status.as_ref().and_then(|s| s.pod_name.as_deref()) {
        Some(pod) if config::dns_label(pod) => app
            .cluster
            .get_pod(pod)
            .await
            .map_err(|e| Failure::upstream(&e))?,
        _ => None,
    };
    let ip = pod.as_ref().and_then(|p| cluster::vm_pod_ip(&vm, p));
    let usage = app.cluster.pod_usage().await.unwrap_or_default();
    let panel = match (vm.spec.components.redfish_enabled(), ip) {
        (false, _) => ui::PanelState::Disabled,
        (true, None) => ui::PanelState::NoPod,
        (true, Some(ip)) => {
            ui::PanelState::Loaded(Box::new(app.redfish.panel(ip, &user.id_token).await))
        }
    };
    let catalog = base_images(&app).await.unwrap_or_default();
    let detail = model::detail(&vm, role, &usage, pod.as_ref(), ip, &catalog);
    let deletable = access::can_delete(&vm, role);
    page(
        &app,
        &session,
        &user,
        ui::Body::Detail(Box::new(ui::DetailPage {
            detail,
            panel,
            deletable,
        })),
    )
    .await
}

type Fields = Form<Vec<(String, String)>>;

/// Session, then CSRF token and exact Origin, before anything else.
async fn mutation(
    app: &AppState,
    session: &Session,
    headers: &HeaderMap,
    fields: &[(String, String)],
) -> Result<auth::User, Failure> {
    let user = auth::user(session).await?;
    if !auth::mutation_allowed(&user, launch::field(fields, "csrf"), headers, &app.origin) {
        return Err(Failure::forbidden());
    }
    Ok(user)
}

/// The detail page's power form: one power action (a Redfish reset, or power
/// on/off), reported as a flash.
async fn action(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((namespace, name)): Path<(String, String)>,
    Form(fields): Fields,
) -> Result<Redirect, Failure> {
    let user = mutation(&app, &session, &headers, &fields).await?;
    let action = power::Power::parse(launch::field(&fields, "action"))
        .ok_or_else(|| Failure::invalid("Unknown power action."))?;
    if action.needs_confirmation() && launch::field(&fields, "confirm") != "yes" {
        return Err(Failure::invalid("Confirm the power action."));
    }
    let (vm, role) = visible_vm(&app, &user, &namespace, &name).await?;
    if role != Role::Admin {
        return Err(Failure::forbidden());
    }
    let result = power::apply(&app, &user, &vm, action).await;
    tracing::info!(subject=%user.subject, vm=%name, action=action.key(), success=result.is_ok(), "power action");
    flash(
        &session,
        match result {
            Ok(()) => format!("{} sent to {name}.", action.label()),
            Err(failure) => failure.1,
        },
    )
    .await?;
    Ok(Redirect::to(&format!("/vms/{namespace}/{name}")))
}

async fn delete(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((namespace, name)): Path<(String, String)>,
    Form(fields): Fields,
) -> Result<Redirect, Failure> {
    let user = mutation(&app, &session, &headers, &fields).await?;
    if launch::field(&fields, "confirm_name") != name {
        return Err(Failure::invalid("Type the VM name to confirm deletion."));
    }
    let (vm, role) = visible_vm(&app, &user, &namespace, &name).await?;
    if !access::can_delete(&vm, role) {
        return Err(Failure(
            StatusCode::FORBIDDEN,
            "Only console admins can delete, and only VMs launched by this dashboard.".into(),
        ));
    }
    let uid = vm.metadata.uid.clone().ok_or_else(Failure::internal)?;
    app.cluster
        .delete_vm(&name, &uid)
        .await
        .map_err(|e| Failure::upstream(&e))?;
    tracing::info!(subject=%user.subject, vm=%name, %uid, "VM deleted");
    flash(
        &session,
        format!(
            "Deleted VM {name}. Its state PVC {} was kept: it holds the TPM identity and firmware variables. An administrator deletes it deliberately.",
            vm.spec.state_volume_claim
        ),
    )
    .await?;
    Ok(Redirect::to("/"))
}

async fn console_page(
    State(app): State<AppState>,
    session: Session,
    Path((namespace, name)): Path<(String, String)>,
) -> Result<Response, Failure> {
    let user = auth::user(&session).await?;
    let (vm, role) = visible_vm(&app, &user, &namespace, &name).await?;
    if role != Role::Admin || !vm.spec.components.bmc.enabled {
        return Err(Failure(
            StatusCode::FORBIDDEN,
            "The console requires the admin role on this VM.".into(),
        ));
    }
    let html = page(
        &app,
        &session,
        &user,
        ui::Body::Console(ui::ConsolePage {
            namespace,
            name,
            csrf: user.csrf.clone(),
            audio: vm
                .spec
                .console
                .as_ref()
                .and_then(|console| console.admin_bucket)
                .is_some_and(|bucket| bucket.audio),
        }),
    )
    .await?;
    Ok((
        [(
            axum::http::header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_static(CONSOLE_CSP),
        )],
        html,
    )
        .into_response())
}

/// The base-image catalog, read on each request (it is small).
pub(crate) async fn base_images(app: &AppState) -> anyhow::Result<Vec<images::BaseImage>> {
    let raw = app
        .cluster
        .base_image_catalog()
        .await
        .inspect_err(|error| {
            tracing::warn!(error=%cluster::describe(error), "base-image catalog unavailable");
        })?;
    Ok(raw.as_deref().map(images::parse).unwrap_or_default())
}

async fn images_page(
    State(app): State<AppState>,
    session: Session,
) -> Result<Html<String>, Failure> {
    let user = auth::user(&session).await?;
    let catalog = base_images(&app).await.map_err(|e| Failure::upstream(&e))?;
    page(&app, &session, &user, ui::Body::Images(catalog)).await
}

fn launch_page(
    app: &AppState,
    values: Vec<(String, String)>,
    errors: Vec<launch::FormError>,
    images: Vec<images::BaseImage>,
) -> ui::Body {
    // user-data is never rendered, not even back to the user who typed it.
    let user_data_cleared = launch::user_data(&values).is_some();
    ui::Body::Launch(ui::LaunchPage {
        values: values
            .into_iter()
            .filter(|(k, _)| k != "csrf" && k != "user_data")
            .collect(),
        user_data_cleared,
        errors,
        namespace: app.namespace.clone(),
        storage_node: app.storage_node.clone(),
        images,
    })
}

async fn launch_form(
    State(app): State<AppState>,
    session: Session,
) -> Result<Html<String>, Failure> {
    let user = auth::user(&session).await?;
    if !access::can_launch(&user.groups, &app.launcher_group) {
        return Err(Failure(
            StatusCode::FORBIDDEN,
            "Launching VMs requires membership of qrab_vm_launchers.".into(),
        ));
    }
    let (catalog, errors) = match base_images(&app).await {
        Ok(catalog) => (catalog, Vec::new()),
        Err(_) => (
            Vec::new(),
            vec!["The base-image catalog is unavailable; only the demo ESP can be chosen.".into()],
        ),
    };
    page(
        &app,
        &session,
        &user,
        launch_page(&app, Vec::new(), errors, catalog),
    )
    .await
}

async fn launch(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(fields): Fields,
) -> Result<Response, Failure> {
    let user = mutation(&app, &session, &headers, &fields).await?;
    if !access::can_launch(&user.groups, &app.launcher_group) {
        return Err(Failure::forbidden());
    }
    let catalog = match base_images(&app).await {
        Ok(catalog) => catalog,
        Err(e) if !matches!(launch::field(&fields, "os"), "" | "none") => {
            return Err(Failure::upstream(&e));
        }
        Err(_) => Vec::new(),
    };
    let request = match launch::parse_form(
        &fields,
        &app.namespace,
        app.storage_node.as_deref(),
        &catalog,
    ) {
        Ok(r) => r,
        Err(errors) => {
            let mut html = page(
                &app,
                &session,
                &user,
                launch_page(&app, fields, errors, catalog),
            )
            .await?
            .into_response();
            *html.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(html);
        }
    };
    let report = launch::Launcher {
        cluster: app.cluster.as_ref(),
        kanidm: &app.kanidm,
        namespace: &app.namespace,
        vm_users_group: &app.vm_users_group,
        storage_node: app.storage_node.as_deref(),
    }
    .run(&request, &user.subject, &user.name)
    .await;
    tracing::info!(subject=%user.subject, vm=%request.name, created=?report.created, failed=?report.failed.as_ref().map(|f| &f.0), "VM launch");
    let path = format!("/vms/{}/{}", app.namespace, request.name);
    if report.failed.is_none() {
        flash(
            &session,
            format!(
                "Launched {} {}. Your sign-in was refreshed so it includes the VM's new admins group (console and power).",
                request.name,
                match &request.os {
                    Some(os) => format!(
                        "from {} ({}): its {} GiB root disk is a copy-on-write clone, provisioned before the first boot",
                        os.image.title,
                        os.image.label(),
                        os.root_gib
                    ),
                    None => "with the demo ESP".into(),
                }
            ),
        )
        .await?;
        // The ID token predates the new admins group, so sign in again through
        // Kanidm's existing session and come back to the VM as its admin.
        return Ok(Redirect::to(&format!("/auth/login?next={path}")).into_response());
    }
    let created_vm = report
        .created
        .iter()
        .any(|c| c.starts_with("VirtualMachine"));
    let mut html = page(
        &app,
        &session,
        &user,
        ui::Body::Launched(ui::Launched {
            vm_path: created_vm.then_some(path),
            report,
        }),
    )
    .await?
    .into_response();
    *html.status_mut() = StatusCode::BAD_GATEWAY;
    Ok(html)
}

async fn logout(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(fields): Fields,
) -> Result<Redirect, Failure> {
    mutation(&app, &session, &headers, &fields).await?;
    session.flush().await.map_err(|_| Failure::internal())?;
    Ok(Redirect::to("/"))
}
