//! A minimal DMTF Redfish service for one qrab VM, reached only through the
//! manager control API. See docs/redfish.md for the resource tree.
pub mod auth;
pub mod config;
mod diagnostics;
mod discovery;
pub mod sessions;
mod subscriptions;
pub mod workload;
pub use subscriptions::{spawn as spawn_subscriptions, SubscriptionStore};
mod boot;
mod firmware;
mod hardware;
pub mod history;
pub use history::HistoryStore;
mod inventory;
mod media;
pub mod prometheus;
pub use prometheus::MetricsExport;
mod secure_boot;
mod serial;
mod tasks;
pub use tasks::TaskStore;
pub use telemetry::{spawn_sampler, TelemetryStore};
mod telemetry;
mod triggers;
pub use triggers::TriggerStore;

use auth::{Authenticator, Principal, Rejection, Role};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Router,
};
use manager::pod::{self, Action, Phase, VmEvent};
use serde_json::{json, Value};
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

/// A resource response attaches its schema link before serialization. This
/// avoids buffering or reparsing response bodies (including streaming bodies).
pub struct Json<T>(pub T);
impl IntoResponse for Json<Value> {
    fn into_response(self) -> Response {
        let schema = self
            .0
            .get("@odata.type")
            .and_then(Value::as_str)
            .and_then(|kind| kind.strip_prefix('#'))
            .and_then(|kind| kind.rsplit_once('.').map(|(namespace, _)| namespace))
            .filter(|namespace| {
                !namespace.starts_with("Qrab")
                    && namespace
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_'))
            })
            .and_then(|namespace| {
                HeaderValue::from_str(&format!(
                    "<https://redfish.dmtf.org/schemas/v1/{namespace}.json>; rel=describedby"
                ))
                .ok()
            });
        let mut response = axum::Json(self.0).into_response();
        if let Some(schema) = schema {
            response.headers_mut().insert(header::LINK, schema);
        }
        response
    }
}

pub const SYSTEM: &str = "/redfish/v1/Systems/1";
pub const MANAGER: &str = "/redfish/v1/Managers/1";
const RESET_TARGET: &str = "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset";
pub const FREEZE_TARGET: &str = "/redfish/v1/Systems/1/Actions/Oem/QrabComputerSystem.Freeze";
pub const THAW_TARGET: &str = "/redfish/v1/Systems/1/Actions/Oem/QrabComputerSystem.Thaw";
pub const UPDATE_TARGET: &str = "/redfish/v1/Systems/1/Actions/Oem/QrabComputerSystem.Update";
const LOG: &str = "/redfish/v1/Managers/1/LogServices/Events";
const REGISTRY: &str = "Base.1.18";
/// Reset types this service performs; see [`reset_action`].
pub const RESET_TYPES: &[&str] = &[
    "On",
    "ForceOn",
    "GracefulShutdown",
    "ForceOff",
    "ForceRestart",
    "PowerCycle",
    "Pause",
    "Resume",
    "Nmi",
    "GracefulRestart",
];
/// DMTF ResetType values that are valid but cannot be performed honestly.
const UNSUPPORTED_RESET_TYPES: &[&str] = &["PushPowerButton", "Suspend", "FullPowerCycle"];

#[derive(Clone)]
pub struct AppState {
    pub auth: Arc<Authenticator>,
    pub control: pod::Backend,
    pub streams: Arc<Semaphore>,
    /// Shared admission held until the response body ends or is dropped.
    pub requests: Arc<Semaphore>,
    pub tasks: TaskStore,
    pub telemetry: TelemetryStore,
    pub triggers: TriggerStore,
    pub subscriptions: SubscriptionStore,
    /// Coalesced Prometheus collection for [`metrics_router`].
    pub exporter: MetricsExport,
    /// Metric history read back from the cluster Prometheus.
    pub history: HistoryStore,
}

/// A Redfish error response with a Base registry message.
#[derive(Debug)]
pub struct Failure {
    status: StatusCode,
    message_id: &'static str,
    message: String,
    resolution: &'static str,
}

impl Failure {
    fn new(
        status: StatusCode,
        message_id: &'static str,
        message: impl Into<String>,
        resolution: &'static str,
    ) -> Self {
        Self {
            status,
            message_id,
            message: message.into(),
            resolution,
        }
    }
    fn unauthenticated() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "NoValidSession",
            "Valid authentication credentials are required.",
            "Present an authorized OIDC bearer token or a supported Redfish credential.",
        )
    }
    fn forbidden() -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "InsufficientPrivilege",
            "The authenticated subject does not have the privilege required for this request.",
            "Ask an administrator for the admin role on this VM.",
        )
    }
    fn not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "ResourceNotFound",
            "The requested resource does not exist.",
            "Use a URI from the service root.",
        )
    }
    fn unavailable(message: &'static str) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "ServiceTemporarilyUnavailable",
            message,
            "Retry later.",
        )
    }
    fn powered_off() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            "OperationNotAllowed",
            "The VM is powered off.",
            "Power the VM on with ComputerSystem.Reset and ResetType On.",
        )
    }
    fn vm(error: &manager::Error, message: &'static str) -> Self {
        match error {
            manager::Error::ShuttingDown => Self::unavailable("The VM is shutting down."),
            manager::Error::PoweredOff => Self::powered_off(),
            _ => Self::unavailable(message),
        }
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let id = format!("{REGISTRY}.{}", self.message_id);
        let body = json!({"error":{
            "code": if self.status.is_server_error() { format!("{REGISTRY}.GeneralError") } else { id.clone() },
            "message": self.message,
            "@Message.ExtendedInfo": [{
                "@odata.type": "#Message.v1_2_0.Message",
                "MessageId": id,
                "Message": self.message,
                "Severity": "Critical",
                "Resolution": self.resolution,
            }],
        }});
        let mut response = (self.status, Json(body)).into_response();
        if self.status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                HeaderValue::from_static("Bearer realm=\"qrab-redfish\""),
            );
        }
        response
    }
}

/// The separate unauthenticated Prometheus listener; see [`prometheus`].
pub fn metrics_router(app: AppState) -> Router {
    prometheus::router(app)
}

pub fn router(app: AppState) -> Router {
    Router::new()
        .merge(sessions::routes())
        .merge(discovery::routes())
        .merge(hardware::routes())
        .merge(boot::routes())
        .merge(serial::routes())
        .route(
            triggers::COLLECTION,
            get(triggers::collection_handler).post(triggers::create),
        )
        .route(
            "/redfish/v1/TelemetryService/Triggers/Members",
            post(triggers::create),
        )
        .route(
            "/redfish/v1/TelemetryService/Triggers/{id}",
            get(triggers::get)
                .patch(triggers::patch)
                .delete(triggers::delete),
        )
        .route(triggers::LOG_SERVICE, get(triggers::log_service))
        .route(triggers::ENTRIES, get(triggers::entries))
        .route(
            "/redfish/v1/Managers/1/LogServices/Telemetry/Entries/{id}",
            get(triggers::entry),
        )
        .merge(media::routes())
        .merge(firmware::routes())
        .merge(secure_boot::routes())
        .route(tasks::SERVICE, get(tasks::service))
        .route(tasks::TASKS, get(tasks::collection_handler))
        .route("/redfish/v1/TaskService/Tasks/{id}", get(tasks::task))
        .route(
            "/redfish/v1/TaskService/TaskMonitors/{id}",
            get(tasks::monitor),
        )
        .route(tasks::START_UPDATE, post(tasks::start_update))
        .route(diagnostics::SERVICE, get(diagnostics::service))
        .route(diagnostics::ENTRIES, get(diagnostics::entries))
        .route(
            "/redfish/v1/Managers/1/LogServices/Diagnostics/Entries/{id}",
            get(diagnostics::entry),
        )
        .route(
            "/redfish/v1/Managers/1/LogServices/Diagnostics/Entries/{id}/attachment",
            get(diagnostics::attachment),
        )
        .route(diagnostics::COLLECT, post(diagnostics::collect))
        .route("/redfish/v1/UpdateService", get(update_service))
        .route("/redfish", get(versions))
        .route("/redfish/v1", get(service_root))
        .route("/redfish/v1/", get(service_root))
        .route("/redfish/v1/Systems", get(systems))
        .route(SYSTEM, get(system).patch(boot::patch))
        .route(
            "/redfish/v1/Systems/1/{kind}",
            get(inventory::inventory_collection),
        )
        .route(
            "/redfish/v1/Systems/1/{kind}/{id}",
            get(inventory::inventory_item),
        )
        .route("/redfish/v1/Chassis", get(inventory::chassis_collection))
        .route(inventory::CHASSIS, get(inventory::chassis))
        .route(inventory::RESET_INFO, get(inventory::reset_info))
        .route(RESET_TARGET, post(reset))
        .route(FREEZE_TARGET, post(agent_freeze))
        .route(THAW_TARGET, post(agent_thaw))
        .route(UPDATE_TARGET, post(agent_update))
        .route("/redfish/v1/Managers", get(managers))
        .route(MANAGER, get(manager_resource))
        .route("/redfish/v1/Managers/1/LogServices", get(log_services))
        .route(LOG, get(log_service))
        .route(
            "/redfish/v1/Managers/1/LogServices/Events/Entries",
            get(log_entries),
        )
        .route(
            "/redfish/v1/Managers/1/LogServices/Events/Entries/{id}",
            get(log_entry),
        )
        .route(telemetry::DEFINITIONS, get(telemetry::definitions))
        .route(
            "/redfish/v1/TelemetryService/MetricDefinitions/{id}",
            get(telemetry::definition),
        )
        .route(
            telemetry::REPORT_DEFINITIONS,
            get(telemetry::report_definitions),
        )
        .route(
            "/redfish/v1/TelemetryService/MetricReportDefinitions/VM",
            get(telemetry::report_definition),
        )
        .route(
            "/redfish/v1/TelemetryService/MetricReports/{id}",
            get(telemetry::historical_report),
        )
        .route(
            "/redfish/v1/Managers/1/Oem/Qrab/Resources",
            get(telemetry::resources),
        )
        .route(telemetry::SERVICE, get(telemetry::service))
        .route(history::PATH, get(history::history))
        .route(
            "/redfish/v1/TelemetryService/MetricReports",
            get(telemetry::reports),
        )
        .route(telemetry::REPORT, get(telemetry::vm_report))
        .route(telemetry::METRICS, get(telemetry::metrics))
        .route("/redfish/v1/EventService", get(event_service))
        .route(
            "/redfish/v1/EventService/Subscriptions",
            get(subscriptions::collection_handler).post(subscriptions::create),
        )
        .route(
            "/redfish/v1/EventService/Subscriptions/Members",
            post(subscriptions::create),
        )
        .route(
            "/redfish/v1/EventService/Subscriptions/{id}",
            get(subscriptions::get).delete(subscriptions::delete),
        )
        .route("/redfish/v1/EventService/SSE", get(sse))
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(ready))
        .fallback(|| async { Failure::not_found() })
        .method_not_allowed_fallback(|| async {
            Failure::new(
                StatusCode::METHOD_NOT_ALLOWED,
                "OperationNotAllowed",
                "The HTTP method is not allowed on this resource.",
                "Use a method the resource supports.",
            )
        })
        .layer(DefaultBodyLimit::max(4096))
        .layer(axum::middleware::from_fn_with_state(app.clone(), admission))
        .layer(axum::middleware::from_fn_with_state(app.clone(), headers))
        .with_state(app)
}

/// Reject overload instead of retaining unbounded request/response state.
async fn admission(
    State(app): State<AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if matches!(request.uri().path(), "/healthz" | "/readyz") {
        return next.run(request).await;
    }
    let Ok(permit) = app.requests.clone().try_acquire_owned() else {
        return Failure::unavailable("The Redfish request limit has been reached.").into_response();
    };
    let response = next.run(request).await;
    response.map(|body| {
        axum::body::Body::from_stream(futures::stream::unfold(
            (body.into_data_stream(), permit),
            |(mut stream, permit)| async move {
                use futures::StreamExt;
                stream.next().await.map(|chunk| (chunk, (stream, permit)))
            },
        ))
    })
}

async fn headers(
    State(app): State<AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let read = matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    );
    let matched = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .cloned();
    let version_ok = request.headers().get_all("odata-version").iter().count() <= 1
        && request
            .headers()
            .get("odata-version")
            .is_none_or(|value| value == "4.0");
    let mut response = if version_ok {
        next.run(request).await
    } else {
        Failure::new(
            StatusCode::PRECONDITION_FAILED,
            "HeaderInvalid",
            "Only OData-Version 4.0 is supported.",
            "Use OData-Version 4.0.",
        )
        .into_response()
    };
    if read && response.status().is_success() {
        if let Some(matched) = matched {
            let allow = match matched.as_str() {
                SYSTEM | firmware::SECURE_BOOT | "/redfish/v1/Systems/1/SecureBoot/Settings" => {
                    "GET, HEAD, PATCH"
                }
                triggers::COLLECTION
                | sessions::COLLECTION
                | "/redfish/v1/EventService/Subscriptions" => "GET, HEAD, POST",
                "/redfish/v1/TelemetryService/Triggers/{id}" => "GET, HEAD, PATCH, DELETE",
                "/redfish/v1/EventService/Subscriptions/{id}"
                | "/redfish/v1/SessionService/Sessions/{id}" => "GET, HEAD, DELETE",
                _ => "GET, HEAD",
            };
            response
                .headers_mut()
                .insert(header::ALLOW, HeaderValue::from_static(allow));
        }
    }
    if response.status() == StatusCode::UNAUTHORIZED && app.auth.basic_enabled() {
        response.headers_mut().append(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"qrab-redfish\", charset=\"UTF-8\""),
        );
    }
    for (key, value) in [
        ("odata-version", "4.0"),
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
    ] {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static(key),
            HeaderValue::from_static(value),
        );
    }
    response
}

async fn principal(app: &AppState, headers: &HeaderMap) -> Result<Principal, Failure> {
    app.auth
        .authenticate_headers(headers)
        .await
        .map_err(|rejection| match rejection {
            Rejection::Unauthenticated => Failure::unauthenticated(),
            Rejection::Forbidden => Failure::forbidden(),
        })
}

fn has_credentials(headers: &HeaderMap) -> bool {
    headers.contains_key(header::AUTHORIZATION) || headers.contains_key("x-auth-token")
}

fn link(path: &str) -> Value {
    json!({ "@odata.id": path })
}

fn collection(path: &str, kind: &str, name: &str, members: Vec<Value>) -> Value {
    json!({
        "@odata.id": path,
        "@odata.type": format!("#{kind}.{kind}"),
        "Name": name,
        "Members@odata.count": members.len(),
        "Members": members,
    })
}

async fn versions(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    if !app.auth.public_discovery() || has_credentials(&headers) {
        principal(&app, &headers).await?;
    }
    Ok(Json(json!({"v1": "/redfish/v1/"})))
}

async fn service_root(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let who = if app.auth.public_discovery() && !has_credentials(&headers) {
        None
    } else {
        Some(principal(&app, &headers).await?)
    };
    let mut root = json!({
        "@odata.id": "/redfish/v1/",
        "@odata.type": "#ServiceRoot.v1_17_0.ServiceRoot",
        "Id": "RootService",
        "Name": "qrab Redfish Service",
        "RedfishVersion": "1.20.0",
        "Vendor": "qrab",
        "Product": "qrab VM",
        "Systems": link("/redfish/v1/Systems"),
        "Managers": link("/redfish/v1/Managers"),
        "Chassis": link("/redfish/v1/Chassis"),
        "EventService": link("/redfish/v1/EventService"),
        "TelemetryService": link(telemetry::SERVICE),
        "Tasks": link(tasks::SERVICE),
        "UpdateService": link("/redfish/v1/UpdateService"),
        "SessionService": link("/redfish/v1/SessionService"),
        "AccountService": link("/redfish/v1/AccountService"),
        "Links": {"Sessions":link("/redfish/v1/SessionService/Sessions")},
    });
    if let Some(who) = who {
        if let Some(uuid) = vm_uuid(&app, &who).await {
            root["UUID"] = json!(uuid);
        }
    }
    Ok(Json(root))
}

/// The VM's immutable identity (the CR UID) is admin-only, as in the BMC.
async fn vm_uuid(app: &AppState, who: &Principal) -> Option<String> {
    if who.role != Role::Admin {
        return None;
    }
    admin_uuid(&app.control.config_view().await.ok()?, who)
}

/// [`vm_uuid`] from an already-read configuration view.
fn admin_uuid(view: &pod::ConfigView, who: &Principal) -> Option<String> {
    if who.role != Role::Admin {
        return None;
    }
    uuid::Uuid::parse_str(view.state_id.as_deref()?)
        .ok()
        .map(|id| id.to_string())
}

async fn systems(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        "/redfish/v1/Systems",
        "ComputerSystemCollection",
        "Computer System Collection",
        vec![link(SYSTEM)],
    )))
}

/// Redfish power and status for a manager status record.
pub fn power(status: Option<&pod::Status>) -> (Value, &'static str, &'static str) {
    let Some(status) = status else {
        return (Value::Null, "UnavailableOffline", "Warning");
    };
    match status.phase {
        Phase::Starting => (json!("PoweringOn"), "Starting", "OK"),
        Phase::Stopping => (json!("PoweringOff"), "Disabled", "OK"),
        // The manager stays up to power the VM on again.
        Phase::Off => (json!("Off"), "StandbyOffline", "OK"),
        Phase::Exited => (json!("Off"), "Disabled", "OK"),
        Phase::Failed => (json!("Off"), "Disabled", "Critical"),
        Phase::Running => match status.run_state.as_deref() {
            Some("running") | None => (json!("On"), "Enabled", "OK"),
            Some("guest-panicked" | "internal-error" | "io-error") => {
                (json!("On"), "Enabled", "Critical")
            }
            Some(_) => (json!("Paused"), "Quiesced", "OK"),
        },
    }
}

async fn system(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    let who = principal(&app, &headers).await?;
    // Every manager and guest read is bounded and run concurrently, so a
    // stalled manager or guest agent costs at most one bound per phase.
    let (view, status, hardware, agent) = tokio::join!(
        bounded(MANAGER_READ_TIMEOUT, app.control.config_view()),
        bounded(MANAGER_READ_TIMEOUT, app.control.status()),
        bounded(INVENTORY_READ_TIMEOUT, app.control.inventory()),
        // Cached connection state from the manager; no guest round trip.
        bounded(MANAGER_READ_TIMEOUT, app.control.agent_status()),
    );
    let view = view.ok_or_else(|| Failure::unavailable("The VM configuration is unavailable."))?;
    let (power_state, state, health) = power(status.as_ref());
    let status_object = json!({"State": state, "Health": health});
    let (cpus, memory_mib) = inventory::size(&view);
    let mut system = json!({
        "@odata.id": SYSTEM,
        "@odata.type": "#ComputerSystem.v1_22_0.ComputerSystem",
        "Id": "1",
        "Name": view.name,
        "SystemType": "Virtual",
        "Manufacturer": "QEMU",
        "Model": "q35",
        "PowerState": power_state,
        "Status": status_object,
        "ProcessorSummary": {"LogicalProcessorCount": cpus, "Status": status_object},
        "MemorySummary": {"TotalSystemMemoryGiB": memory_mib as f64 / 1024.0, "Status": status_object},
        "Boot":{"BootOptions":link(boot::OPTIONS)},
        "Storage": link(hardware::STORAGE),
        "VirtualMedia": link(media::COLLECTION),
        "SecureBoot": link(firmware::SECURE_BOOT),
        "Processors": link(&format!("{SYSTEM}/Processors")),
        "Memory": link(&format!("{SYSTEM}/Memory")),
        "EthernetInterfaces": link(&format!("{SYSTEM}/EthernetInterfaces")),
        "SimpleStorage": link(&format!("{SYSTEM}/SimpleStorage")),
        "Actions": {
            "#ComputerSystem.Reset": {
                "target": RESET_TARGET,
                "@Redfish.ActionInfo": inventory::RESET_INFO,
                "ResetType@Redfish.AllowableValues": RESET_TYPES,
            },
            // Guest-agent operations; admin only, like Reset.
            "Oem": {
                "#QrabComputerSystem.Freeze": {"target": FREEZE_TARGET},
                "#QrabComputerSystem.Thaw": {"target": THAW_TARGET},
                "#QrabComputerSystem.Update": {"target": UPDATE_TARGET},
            },
        },
        "Links": {"ManagedBy": [link(MANAGER)], "Chassis": [link(inventory::CHASSIS)]},
    });
    if let Some(hardware) = hardware {
        system["ProcessorSummary"]["Count"] = json!(hardware.processors.len());
        system["ProcessorSummary"]["LogicalProcessorCount"] = json!(hardware
            .processors
            .iter()
            .map(|p| p.total_threads)
            .sum::<u64>());
    }
    if let Some(uuid) = admin_uuid(&view, &who) {
        system["UUID"] = json!(uuid);
    }
    let connected = agent.as_ref().is_some_and(|status| status.connected);
    let (boot, agent) = tokio::join!(
        async {
            if connected {
                bounded(AGENT_READ_TIMEOUT, app.control.agent_boot()).await
            } else {
                None
            }
        },
        async {
            match agent {
                Some(status) => guest_agent(&app, status, who.role).await,
                None => Value::Null,
            }
        },
    );
    if let Some(snapshot) = boot {
        system["Boot"] = boot::system_boot(&snapshot);
    }
    // The same role-based redaction as the browser BMC.
    let view = match who.role {
        Role::Admin => view,
        Role::Viewer => view.redacted(),
    };
    system["Oem"] = json!({"Qrab": {"@odata.type": "#QrabComputerSystem.v1_0_0.QrabComputerSystem", "Configuration": view, "GuestAgent": agent, "Metrics": link(telemetry::METRICS)}});
    Ok(Json(system))
}

/// Bound on each manager-local read behind a Systems/1 read (configuration,
/// status and cached agent state), so a wedged manager cannot stall it.
const MANAGER_READ_TIMEOUT: Duration = Duration::from_secs(1);

/// Bound on the live QMP hardware inventory behind a Systems/1 read.
const INVENTORY_READ_TIMEOUT: Duration = Duration::from_millis(200);

/// Bound on each guest round trip behind a Systems/1 read, so a busy or
/// freezing agent cannot stall it.
const AGENT_READ_TIMEOUT: Duration = Duration::from_secs(1);

/// `read`'s value, or `None` when it fails or outlasts `limit`.
async fn bounded<T, E>(
    limit: Duration,
    read: impl std::future::Future<Output = Result<T, E>>,
) -> Option<T> {
    tokio::time::timeout(limit, read)
        .await
        .ok()
        .and_then(Result::ok)
}

/// `Oem.Qrab.GuestAgent`: the cached connection state plus, while the agent
/// is connected and answers in time, the filesystem freeze state and the last
/// update-hook run. Both are `null` otherwise.
async fn guest_agent(app: &AppState, status: pod::AgentStatus, role: Role) -> Value {
    let (freeze, update) = if status.connected {
        tokio::join!(
            bounded(AGENT_READ_TIMEOUT, app.control.agent_freeze_status()),
            bounded(AGENT_READ_TIMEOUT, app.control.agent_update_status()),
        )
    } else {
        (None, None)
    };
    let mut agent = serde_json::to_value(status).unwrap_or_else(|_| json!({}));
    agent["FreezeState"] = json!(freeze.map(|state| match state {
        pod::FreezeState::Frozen => "Frozen",
        pod::FreezeState::Thawed => "Thawed",
    }));
    agent["LastUpdate"] = update.map_or(Value::Null, |update| last_update(&update, role));
    agent
}

/// The last update-hook run. Hook arguments, output and errors can carry
/// deployment detail, so viewers see only the outcome.
fn last_update(update: &pod::UpdateStatus, role: Role) -> Value {
    let state = match update.state {
        pod::UpdateState::Idle => "Idle",
        pod::UpdateState::Running => "Running",
        pod::UpdateState::Succeeded => "Succeeded",
        pod::UpdateState::Failed => "Failed",
    };
    let mut run = json!({
        "State": state,
        "RunId": update.run_id,
        "HookInstalled": update.hook_installed,
        "ExitCode": update.exit_code,
        "Signal": update.signal,
        "StartTime": update.started_unix_ms.map(rfc3339),
        "EndTime": update.finished_unix_ms.map(rfc3339),
    });
    if role == Role::Admin {
        run["Args"] = json!(update.args);
        run["Output"] = json!(update.output);
        run["OutputTruncated"] = json!(update.output_truncated);
        run["Error"] = json!(update.error);
    }
    run
}

/// Map a Redfish ResetType to a manager action. `Ok(None)` is a no-op.
///
/// A powered-off VM's manager keeps running, so On starts QEMU in the same
/// pod; an instance that has ended (or not started) is the operator's to
/// start through `spec.running`.
///
/// ```
/// use manager::pod::{Action, Phase, Status};
/// let mut status: Status = serde_json::from_value(serde_json::json!({
///     "phase": "off", "ready": false, "manager_pid": 1, "manager_start_ticks": 1,
///     "qemu_pid": null, "qemu_start_ticks": null, "qemu_version": null,
///     "run_state": null, "observed_unix_ms": 0, "exit_code": 0, "detail": null,
/// })).unwrap();
/// let action = |reset| qrab_redfish::reset_action(reset, Some(&status)).ok().flatten();
/// assert_eq!(action("On"), Some(Action::On));
/// assert_eq!(action("ForceOn"), Some(Action::On));
/// assert_eq!(action("ForceOff"), Some(Action::ForceOff));
/// assert_eq!(action("PowerCycle"), Some(Action::ColdRestart));
/// status.phase = Phase::Running;
/// status.run_state = Some("running".into());
/// assert_eq!(qrab_redfish::reset_action("On", Some(&status)).unwrap(), None);
/// status.phase = Phase::Exited;
/// assert!(qrab_redfish::reset_action("On", Some(&status)).is_err());
/// ```
pub fn reset_action(
    reset_type: &str,
    status: Option<&pod::Status>,
) -> Result<Option<Action>, Failure> {
    let power_state = power(status).0;
    let phase = status.map(|status| status.phase);
    match reset_type {
        "GracefulShutdown" => Ok(Some(Action::Powerdown)),
        "ForceOff" => Ok(Some(Action::ForceOff)),
        "ForceRestart" => Ok(Some(Action::Reset)),
        "Nmi" => Ok(Some(Action::Nmi)),
        "GracefulRestart" => Ok(Some(Action::Reboot)),
        "PowerCycle" => Ok(Some(Action::ColdRestart)),
        "Pause" => Ok(Some(Action::Pause)),
        "Resume" => Ok(Some(Action::Resume)),
        "On" | "ForceOn" if phase == Some(Phase::Off) => Ok(Some(Action::On)),
        "On" | "ForceOn" if power_state == "Paused" => Ok(Some(Action::Resume)),
        "On" | "ForceOn" if matches!(phase, Some(Phase::Running | Phase::Starting)) => Ok(None),
        "On" | "ForceOn" => Err(Failure::new(
            StatusCode::CONFLICT,
            "OperationNotAllowed",
            "The VM instance is not running; a new instance is started by the VirtualMachine spec.running field.",
            "Set spec.running on the VirtualMachine resource, or retry once the VM has powered off.",
        )),
        other if UNSUPPORTED_RESET_TYPES.contains(&other) => Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterNotSupported",
            format!("ResetType {other} is not supported by this VM."),
            "Use one of the ResetType@Redfish.AllowableValues.",
        )),
        other => Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterValueNotInList",
            format!("The value {other:?} for ResetType is not in the list of acceptable values."),
            "Use one of the ResetType@Redfish.AllowableValues.",
        )),
    }
}

async fn admin(app: &AppState, headers: &HeaderMap) -> Result<Principal, Failure> {
    let who = principal(app, headers).await?;
    if who.role != Role::Admin {
        return Err(Failure::forbidden());
    }
    Ok(who)
}

fn agent_failure(error: manager::Error) -> Failure {
    match error {
        manager::Error::Config(message) => Failure::new(
            StatusCode::BAD_REQUEST,
            "ActionParameterValueFormatError",
            message,
            "Use at most 16 tokens of [A-Za-z0-9._:/=+@,-] that do not start with '-'.",
        ),
        error => Failure::vm(&error, "The guest agent did not complete the request."),
    }
}

/// An optional JSON object body; empty bodies are allowed for parameterless actions.
fn action_body(body: &Bytes) -> Result<Value, Failure> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(json!({}));
    }
    serde_json::from_slice::<Value>(body)
        .ok()
        .filter(Value::is_object)
        .ok_or_else(|| {
            Failure::new(
                StatusCode::BAD_REQUEST,
                "MalformedJSON",
                "The request body is not a valid JSON object.",
                "Send a JSON object.",
            )
        })
}

fn string_list(body: &Value, name: &'static str) -> Result<Option<Vec<String>>, Failure> {
    match body.get(name) {
        None => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|_| {
                Failure::new(
                    StatusCode::BAD_REQUEST,
                    "ActionParameterValueTypeError",
                    format!("{name} must be an array of strings."),
                    "Send an array of strings.",
                )
            }),
    }
}

async fn agent_freeze(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Failure> {
    let who = admin(&app, &headers).await?;
    let mountpoints = string_list(&action_body(&body)?, "Mountpoints")?;
    let result = app.control.agent_freeze(mountpoints).await;
    tracing::info!(subject=%who.subject, success=result.is_ok(), "Redfish guest freeze requested");
    Ok(Json(
        json!({"FrozenFilesystems": result.map_err(agent_failure)?}),
    ))
}

async fn agent_thaw(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Failure> {
    let who = admin(&app, &headers).await?;
    action_body(&body)?;
    let result = app.control.agent_thaw().await;
    tracing::info!(subject=%who.subject, success=result.is_ok(), "Redfish guest thaw requested");
    Ok(Json(
        json!({"ThawedFilesystems": result.map_err(agent_failure)?}),
    ))
}

async fn agent_update(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Failure> {
    let who = admin(&app, &headers).await?;
    let args = string_list(&action_body(&body)?, "Args")?.unwrap_or_default();
    let result = app.control.agent_update_trigger(args).await;
    tracing::info!(subject=%who.subject, success=result.is_ok(), "Redfish guest update requested");
    let status = result.map_err(agent_failure)?;
    Ok(Json(json!({"Update": status})))
}

async fn reset(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, Failure> {
    let who = principal(&app, &headers).await?;
    if who.role != Role::Admin {
        return Err(Failure::forbidden());
    }
    let body: Value = serde_json::from_slice(&body).map_err(|_| {
        Failure::new(
            StatusCode::BAD_REQUEST,
            "MalformedJSON",
            "The request body is not valid JSON.",
            "Send a JSON object with a ResetType property.",
        )
    })?;
    let reset_type = body
        .get("ResetType")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            Failure::new(
                StatusCode::BAD_REQUEST,
                "ActionParameterMissing",
                "The action ComputerSystem.Reset requires the parameter ResetType.",
                "Supply ResetType.",
            )
        })?;
    let status = app.control.status().await.ok();
    let action = reset_action(reset_type, status.as_ref())?;
    let result = match action {
        Some(action) => app.control.action(action).await,
        None => Ok(()),
    };
    tracing::info!(subject=%who.subject, reset_type, ?action, success=result.is_ok(), "Redfish reset requested");
    result
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(|error| Failure::vm(&error, "The VM did not complete the reset."))
}

async fn managers(State(app): State<AppState>, headers: HeaderMap) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        "/redfish/v1/Managers",
        "ManagerCollection",
        "Manager Collection",
        vec![link(MANAGER)],
    )))
}

async fn manager_resource(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let health = if app.control.probe(false).await.is_ok() {
        "OK"
    } else {
        "Warning"
    };
    Ok(Json(json!({
        "@odata.id": MANAGER,
        "@odata.type": "#Manager.v1_19_0.Manager",
        "Id": "1",
        "Name": "qrab VM manager",
        "ManagerType": "BMC",
        "FirmwareVersion": env!("CARGO_PKG_VERSION"),
        "Status": {"State": "Enabled", "Health": health},
        "LogServices": link("/redfish/v1/Managers/1/LogServices"),
        "Oem":{"Qrab":{"@odata.type":"#QrabManager.v1_0_0.QrabManager","Resources":link("/redfish/v1/Managers/1/Oem/Qrab/Resources")}},
        "Links": {"ManagerForServers": [link(SYSTEM)], "ManagerForChassis": [link(inventory::CHASSIS)]},
    })))
}

async fn log_services(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let who = principal(&app, &headers).await?;
    Ok(Json(collection(
        "/redfish/v1/Managers/1/LogServices",
        "LogServiceCollection",
        "Log Service Collection",
        {
            let mut services = vec![
                link(LOG),
                link(diagnostics::SERVICE),
                link(triggers::LOG_SERVICE),
            ];
            if who.role == Role::Admin {
                services.push(link(serial::SERVICE));
            }
            services
        },
    )))
}

async fn log_service(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let dropped = app.control.events(None, 1).qmp_dropped;
    Ok(Json(json!({
        "@odata.id": LOG,
        "@odata.type": "#LogService.v1_7_0.LogService",
        "Id": "Events",
        "Name": "QMP event log",
        "Description": "Bounded history of QEMU QMP events kept by the VM manager",
        "ServiceEnabled": true,
        "MaxNumberOfRecords": pod::EVENT_CAPACITY,
        "OverWritePolicy": "WrapsWhenFull",
        "LogEntryType": "OEM",
        "Entries": link(&format!("{LOG}/Entries")),
        // Events QEMU emitted that the manager's QMP client had to discard.
        "Status": {"State": "Enabled", "Health": if dropped > 0 { "Warning" } else { "OK" }},
        "Oem": {"Qrab": {"@odata.type": "#QrabLogService.v1_0_0.QrabLogService", "QmpDropped": dropped}},
    })))
}

/// QMP events that indicate a fault rather than an operator action.
fn severity(event: &str) -> &'static str {
    match event {
        "BLOCK_IO_ERROR" | "GUEST_PANICKED" | "BLOCK_IMAGE_CORRUPTED" | "MEMORY_FAILURE" => {
            "Critical"
        }
        "GUEST_CRASHLOADED" | "WATCHDOG" | "BLOCK_JOB_ERROR" => "Warning",
        _ => "OK",
    }
}

/// RFC 3339 UTC time for Unix milliseconds.
pub fn rfc3339(unix_ms: u64) -> String {
    let secs = unix_ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant), valid for the proleptic Gregorian calendar.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        unix_ms % 1000
    )
}

fn event_time(event: &VmEvent) -> String {
    let millis = event
        .qemu_time
        .and_then(|t| {
            u64::try_from(t.seconds)
                .ok()?
                .checked_mul(1000)?
                .checked_add(u64::try_from(t.microseconds).ok()? / 1000)
        })
        .unwrap_or(event.received_unix_ms);
    rfc3339(millis)
}

/// Whether a QMP record carries the raw QMP `Data`. Authenticated readers of
/// the event log and the SSE stream get it; push destinations never do.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Payload {
    Included,
    Withheld,
}

fn oem(event: &VmEvent, epoch: Option<&str>, payload: Payload) -> Value {
    let data = match payload {
        Payload::Included => event.data.clone(),
        Payload::Withheld => Value::Null,
    };
    json!({"Qrab": {
        "@odata.type": "#QrabEvent.v1_0_0.QrabEvent",
        "Sequence": event.seq,
        "Epoch": epoch,
        "Event": event.event,
        "Data": data,
        "Truncated": event.truncated,
    }})
}

/// One `LogEntry`; `epoch` is the page's [`pod::EventPage::epoch`].
pub fn log_entry_json(event: &VmEvent, epoch: Option<&str>) -> Value {
    json!({
        "@odata.id": format!("{LOG}/Entries/{}", event.seq),
        "@odata.type": "#LogEntry.v1_17_0.LogEntry",
        "Id": event.seq.to_string(),
        "Name": "QMP event",
        "EntryType": "Oem",
        "OemRecordFormat": "QAPI",
        "Created": event_time(event),
        "Severity": severity(&event.event),
        "Message": event.event,
        "Oem": oem(event, epoch, Payload::Included),
    })
}

async fn log_entries(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let page = app.control.events(None, pod::EVENT_PAGE_LIMIT);
    Ok(Json(collection(
        &format!("{LOG}/Entries"),
        "LogEntryCollection",
        "QMP event log entries",
        page.events
            .iter()
            .map(|event| log_entry_json(event, page.epoch.as_deref()))
            .collect(),
    )))
}

async fn log_entry(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let seq: u64 = id.parse().map_err(|_| Failure::not_found())?;
    let page = app.control.events(Some(seq.saturating_sub(1)), 1);
    page.events
        .first()
        .filter(|event| event.seq == seq)
        .map(|event| Json(log_entry_json(event, page.epoch.as_deref())))
        .ok_or_else(Failure::not_found)
}

async fn event_service(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    let mut value = json!({
        "@odata.id": "/redfish/v1/EventService",
        "@odata.type": "#EventService.v1_10_0.EventService",
        "Id": "EventService",
        "Name": "Event Service",
        "ServiceEnabled": true,
        "ServerSentEventUri": "/redfish/v1/EventService/SSE",
        "DeliveryRetryAttempts":subscriptions::DELIVERY_RETRY_ATTEMPTS,
        "DeliveryRetryIntervalSeconds":subscriptions::DELIVERY_RETRY_INTERVAL,
        "EventFormatTypes":["Event"],"RegistryPrefixes":["Qrab"],
        "ResourceTypes":["ComputerSystem"],"Severities":["OK","Warning","Critical"],
        "Oem":{"Qrab":{"@odata.type":"#QrabEventService.v1_0_0.QrabEventService",
            "PushDeliveryEnabled":app.subscriptions.enabled(),"PersistentSubscriptions":app.subscriptions.persistent(),
            "MaxSubscriptions":32,"MaxDeliveryConcurrency":2}},
        "Status": {"State": "Enabled", "Health": "OK"},
    });
    if app.subscriptions.enabled() {
        value["Subscriptions"] = link(subscriptions::COLLECTION);
    }
    Ok(Json(value))
}

/// An `Event` envelope around one `EventRecord`.
fn event_envelope(id: String, name: &str, record: Value) -> Value {
    json!({
        "@odata.type": "#Event.v1_10_0.Event",
        "Id": id,
        "Name": name,
        "Events": [record],
    })
}

/// The one `EventRecord` for a QMP event, shared by the SSE stream and push
/// delivery so both carry the same fields, including the required `MemberId`.
fn qmp_event_record(event: &VmEvent, epoch: Option<&str>, payload: Payload) -> Value {
    json!({
        "MemberId": "0",
        "EventType": "Other",
        "EventId": EventCursor::new(epoch, event.seq).to_string(),
        "EventTimestamp": event_time(event),
        "Severity": severity(&event.event),
        "Message": event.event,
        "MessageId": "Qrab.1.0.QmpEvent",
        "MessageArgs": [event.event],
        "OriginOfCondition": link(SYSTEM),
        "Oem": oem(event, epoch, payload),
    })
}

fn qmp_event(event: &VmEvent, epoch: Option<&str>, payload: Payload) -> Value {
    event_envelope(
        EventCursor::new(epoch, event.seq).to_string(),
        "qrab VM event",
        qmp_event_record(event, epoch, payload),
    )
}

/// The SSE `Event` for a QMP event of numbering `epoch` (the page's
/// [`pod::EventPage::epoch`]). Its `Id` and `EventId` are the event's
/// [`EventCursor`], the same value as its SSE `id`. Push delivery sends the
/// same record with `Oem.Qrab.Data` withheld.
///
/// ```
/// # let event = manager::pod::VmEvent { seq: 4, received_unix_ms: 0, qemu_time: None,
/// #     event: "STOP".into(), data: serde_json::json!({}), truncated: false };
/// let value = qrab_redfish::event_json(&event, Some("0123456789abcdef"));
/// let record = &value["Events"][0];
/// assert_eq!(record["MemberId"], "0");
/// assert_eq!(record["Oem"]["Qrab"]["Sequence"], 4);
/// assert_eq!(record["Oem"]["Qrab"]["Epoch"], "0123456789abcdef");
/// assert_eq!(record["EventId"], "0123456789abcdef.4");
/// assert_eq!(value["Id"], "0123456789abcdef.4");
/// // A manager that predates epochs: the plain sequence.
/// assert_eq!(qrab_redfish::event_json(&event, None)["Id"], "4");
/// ```
pub fn event_json(event: &VmEvent, epoch: Option<&str>) -> Value {
    qmp_event(event, epoch, Payload::Included)
}

/// A position in the event stream: the last delivered sequence of one manager
/// numbering. Its text form is the SSE `id`, the `Last-Event-ID` and `after=`
/// value and the `Event` `Id`: `<epoch>.<sequence>`, or a plain `<sequence>`
/// when the epoch is unknown (older clients, Redfish services and managers).
/// An epoch is 1 to 64 ASCII letters, digits or `-`; the sequence is decimal.
///
/// ```
/// use qrab_redfish::EventCursor;
/// let cursor: EventCursor = "0123456789abcdef.42".parse().unwrap();
/// assert_eq!((cursor.epoch.as_deref(), cursor.seq), (Some("0123456789abcdef"), 42));
/// assert_eq!(cursor.to_string(), "0123456789abcdef.42");
/// let plain: EventCursor = "42".parse().unwrap();
/// assert_eq!((plain.epoch.as_deref(), plain.seq), (None, 42));
/// assert_eq!(plain.to_string(), "42");
/// for bad in ["", "-1", "+1", ".4", "abc.", "abc.-1", "a b.4", "abc.4.5", "x"] {
///     assert!(bad.parse::<EventCursor>().is_err(), "{bad}");
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventCursor {
    pub epoch: Option<String>,
    pub seq: u64,
}

impl EventCursor {
    pub fn new(epoch: Option<&str>, seq: u64) -> Self {
        Self {
            epoch: epoch.map(str::to_owned),
            seq,
        }
    }

    /// The newest position of `page`'s numbering.
    pub fn latest(page: &pod::EventPage) -> Self {
        Self::new(page.epoch.as_deref(), page.latest)
    }

    pub(crate) fn valid_epoch(epoch: &str) -> bool {
        (1..=64).contains(&epoch.len())
            && epoch
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }
}

impl std::fmt::Display for EventCursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.epoch {
            Some(epoch) => write!(f, "{epoch}.{}", self.seq),
            None => write!(f, "{}", self.seq),
        }
    }
}

impl std::str::FromStr for EventCursor {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, ()> {
        let (epoch, seq) = match value.split_once('.') {
            Some((epoch, seq)) if Self::valid_epoch(epoch) => (Some(epoch), seq),
            Some(_) => return Err(()),
            None => (None, value),
        };
        // `u64::from_str` accepts a leading `+`; a cursor is digits only.
        if seq.is_empty() || !seq.bytes().all(|b| b.is_ascii_digit()) {
            return Err(());
        }
        Ok(Self::new(epoch, seq.parse().map_err(|_| ())?))
    }
}

/// A stream-only notice: these describe the stream, not a QMP event, so they
/// have no sequence number and are never written to the event log.
fn notice(
    id: String,
    severity: &str,
    message: String,
    message_id: &str,
    args: Vec<String>,
    oem: Value,
) -> Value {
    event_envelope(
        id.clone(),
        "qrab event stream notice",
        json!({
            "MemberId": "0",
            "EventType": "Other",
            "EventId": id,
            "EventTimestamp": rfc3339(now_ms()),
            "Severity": severity,
            "Message": message,
            "MessageId": message_id,
            "MessageArgs": args,
            "OriginOfCondition": link(LOG),
            "Oem": {"Qrab": oem},
        }),
    )
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Why a cursor read cannot continue gaplessly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GapCause {
    /// Events after the cursor were evicted from the bounded history.
    Evicted,
    /// The cursor names an event of an earlier numbering: the manager
    /// restarted and numbers from 1 again (see [`read_after`]).
    SequenceRestarted,
}

/// One gap a reader must be told about before the events that follow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Gap {
    pub cause: GapCause,
    /// The reader's cursor (its last delivered sequence).
    pub after: u64,
    /// Evicted events of the current numbering. The earlier numbering's events
    /// a restarted reader never got are unknown, so its notice reports `null`.
    pub evicted: u64,
    /// The first retained sequence delivered next.
    pub resumes_at: Option<u64>,
}

impl Gap {
    pub fn json(&self) -> Value {
        let missed = match self.cause {
            GapCause::Evicted => Some(self.evicted),
            GapCause::SequenceRestarted => None,
        };
        gap_json(self.cause, self.after, missed, self.resumes_at)
    }
}

/// Read the retained events after `cursor` with `read` (which takes a
/// sequence), the one gap to report before them, if any, and the cursor to
/// continue from.
///
/// The cursor names an earlier manager instance's numbering when its epoch
/// differs from the page's, or, when either epoch is unknown, when its
/// sequence is above the newest one. Every event of the current numbering is
/// then newer than anything that reader received, so the read restarts at 0:
/// nothing retained is skipped and nothing it already saw is replayed. A page
/// without an epoch and without events (a sidecar mirror that has not yet
/// synchronized) gives nothing to compare or deliver, so the cursor is kept.
/// A plain cursor not above the newest sequence is taken as current and
/// continues under the page's epoch.
pub(crate) fn read_after(
    cursor: &EventCursor,
    read: impl Fn(u64) -> pod::EventPage,
) -> (pod::EventPage, Option<Gap>, EventCursor) {
    let page = read(cursor.seq);
    let restarted = match (&cursor.epoch, &page.epoch) {
        (Some(ours), Some(theirs)) => ours != theirs,
        (_, None) if page.latest == 0 => false,
        _ => cursor.seq > page.latest,
    };
    if restarted {
        let page = read(0);
        let gap = Gap {
            cause: GapCause::SequenceRestarted,
            after: cursor.seq,
            evicted: page.missed,
            resumes_at: page.events.first().map(|event| event.seq),
        };
        let next = EventCursor::new(page.epoch.as_deref(), page.next);
        return (page, Some(gap), next);
    }
    let gap = (page.missed > 0).then(|| Gap {
        cause: GapCause::Evicted,
        after: cursor.seq,
        evicted: page.missed,
        resumes_at: page.events.first().map(|event| event.seq),
    });
    let next = EventCursor {
        epoch: page.epoch.clone().or_else(|| cursor.epoch.clone()),
        // A cursor of this numbering above a lagging mirror's newest
        // sequence waits there instead of replaying what it already has.
        seq: cursor.seq.max(page.next),
    };
    (page, gap, next)
}

/// The SSE gap record: `missed` QMP events after sequence `after` were evicted
/// from the bounded history before this stream read them; `resumes_at` is the
/// first retained sequence delivered next.
///
/// ```
/// let gap = qrab_redfish::missed_event_json(3, 7, Some(11));
/// let record = &gap["Events"][0];
/// assert_eq!(record["MessageId"], "Qrab.1.0.EventsMissed");
/// assert_eq!(record["MessageArgs"], serde_json::json!(["7", "3"]));
/// assert_eq!(record["Oem"]["Qrab"]["MissedEvents"], 7);
/// assert_eq!(record["Oem"]["Qrab"]["Reason"], "Evicted");
/// assert_eq!(record["Oem"]["Qrab"]["OldestAvailableSequence"], 11);
/// ```
pub fn missed_event_json(after: u64, missed: u64, resumes_at: Option<u64>) -> Value {
    gap_json(GapCause::Evicted, after, Some(missed), resumes_at)
}

/// The gap record for a cursor from an earlier manager numbering: how many
/// events after `after` the reader missed is unknown (`MissedEvents: null`);
/// delivery resumes at the oldest retained event of the current numbering.
///
/// ```
/// let gap = qrab_redfish::sequence_restarted_event_json(500, Some(1));
/// let record = &gap["Events"][0];
/// assert_eq!(record["MessageId"], "Qrab.1.0.EventsMissed");
/// assert_eq!(record["MessageArgs"], serde_json::json!(["unknown", "500"]));
/// assert!(record["Oem"]["Qrab"]["MissedEvents"].is_null());
/// assert_eq!(record["Oem"]["Qrab"]["Reason"], "SequenceRestarted");
/// ```
pub fn sequence_restarted_event_json(after: u64, resumes_at: Option<u64>) -> Value {
    gap_json(GapCause::SequenceRestarted, after, None, resumes_at)
}

fn gap_json(cause: GapCause, after: u64, missed: Option<u64>, resumes_at: Option<u64>) -> Value {
    let (id, reason, message) = match (cause, missed) {
        (GapCause::Evicted, Some(missed)) => (
            format!("missed-{after}"),
            "Evicted",
            format!("{missed} QMP event(s) after sequence {after} were evicted from the event history before they were delivered."),
        ),
        _ => (
            format!("restarted-{after}"),
            "SequenceRestarted",
            format!("The event sequence restarted after sequence {after} because the VM manager restarted; an unknown number of QMP events may have been missed."),
        ),
    };
    notice(
        id,
        "Warning",
        message,
        MISSED_MESSAGE,
        vec![
            missed.map_or_else(|| "unknown".into(), |missed| missed.to_string()),
            after.to_string(),
        ],
        json!({
            "@odata.type": "#QrabEventGap.v1_0_0.QrabEventGap",
            "Reason": reason,
            "MissedEvents": missed,
            "After": after,
            "OldestAvailableSequence": resumes_at,
        }),
    )
}

/// The SSE notice for a new `LogServices/Events` `Oem.Qrab.QmpDropped` total.
///
/// ```
/// let notice = qrab_redfish::qmp_dropped_event_json(2);
/// assert_eq!(notice["Events"][0]["MessageId"], "Qrab.1.0.QmpEventsDropped");
/// assert_eq!(notice["Events"][0]["Oem"]["Qrab"]["QmpDropped"], 2);
/// assert_eq!(qrab_redfish::qmp_dropped_event_json(0)["Events"][0]["Severity"], "OK");
/// ```
pub fn qmp_dropped_event_json(total: u64) -> Value {
    notice(
        format!("qmp-dropped-{total}"),
        if total > 0 { "Warning" } else { "OK" },
        format!("The QMP client has discarded {total} QEMU event(s) during the current QEMU connection."),
        DROPPED_MESSAGE,
        vec![total.to_string()],
        json!({
            "@odata.type": "#QrabQmpDropped.v1_0_0.QrabQmpDropped",
            "QmpDropped": total,
        }),
    )
}

pub const MISSED_MESSAGE: &str = "Qrab.1.0.EventsMissed";
pub const DROPPED_MESSAGE: &str = "Qrab.1.0.QmpEventsDropped";

fn bad_cursor(message: String) -> Failure {
    Failure::new(
        StatusCode::BAD_REQUEST,
        "QueryParameterValueFormatError",
        message,
        "Pass the last received SSE id (<epoch>.<sequence>, or a decimal QMP sequence), once.",
    )
}

/// The stream's resume cursor: the `Last-Event-ID` header, else the `after`
/// query parameter, else none (start at the newest event). Both take an
/// [`EventCursor`]. The header wins because a browser's automatic reconnect
/// repeats the original URL but sends the newest id it received; a malformed
/// header is ignored. Other query parameters are ignored.
///
/// ```
/// use axum::http::HeaderMap;
/// use qrab_redfish::{resume_cursor, EventCursor};
/// let none = HeaderMap::new();
/// assert_eq!(resume_cursor(&none, Some("after=7")).unwrap(), Some(EventCursor::new(None, 7)));
/// let mut header = HeaderMap::new();
/// header.insert("last-event-id", "9".parse().unwrap());
/// assert_eq!(resume_cursor(&header, Some("after=7")).unwrap(), Some(EventCursor::new(None, 9)));
/// header.insert("last-event-id", "0123456789abcdef.9".parse().unwrap());
/// assert_eq!(
///     resume_cursor(&header, Some("after=7")).unwrap(),
///     Some(EventCursor::new(Some("0123456789abcdef"), 9))
/// );
/// assert_eq!(
///     resume_cursor(&none, Some("after=0123456789abcdef.7")).unwrap(),
///     Some(EventCursor::new(Some("0123456789abcdef"), 7))
/// );
/// assert_eq!(resume_cursor(&none, None).unwrap(), None);
/// assert!(resume_cursor(&none, Some("after=-1")).is_err());
/// assert!(resume_cursor(&none, Some("after=1&after=2")).is_err());
/// assert!(resume_cursor(&none, Some("after=.7")).is_err());
/// ```
pub fn resume_cursor(
    headers: &HeaderMap,
    query: Option<&str>,
) -> Result<Option<EventCursor>, Failure> {
    let mut after = None;
    for pair in query.unwrap_or("").split('&') {
        let Some(value) = pair.strip_prefix("after=") else {
            continue;
        };
        if after.is_some() {
            return Err(bad_cursor("The query parameter after is repeated.".into()));
        }
        let shown: String = value.chars().take(32).collect();
        after = Some(value.parse::<EventCursor>().map_err(|()| {
            bad_cursor(format!(
                "The value {shown:?} for the query parameter after is not an event stream cursor."
            ))
        })?);
    }
    let header = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    Ok(header.or(after))
}

/// Live events close on credential expiry or session revocation. Idle streams
/// do not refresh a session's last authenticated request time.
///
/// Only QMP events carry an SSE `id` (their [`EventCursor`]). Gap and
/// QMP-dropped notices omit it, so the client's Last-Event-ID stays the last
/// QMP event it received: a resume neither skips events nor depends on notice
/// ordering.
/// A gap is reported whenever the cursor falls behind the retained history
/// (an evicted resume cursor or a reader slower than the history) or names an
/// earlier manager numbering (see [`read_after`]); the dropped total is sent
/// at open when nonzero and again on every change.
async fn sse(
    State(app): State<AppState>,
    headers: HeaderMap,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> Result<Response, Failure> {
    let who = principal(&app, &headers).await?;
    let resume = resume_cursor(&headers, query.as_deref())?;
    let permit = app
        .streams
        .clone()
        .try_acquire_owned()
        .map_err(|_| Failure::unavailable("Too many event streams are open."))?;
    let cursor = resume.unwrap_or_else(|| EventCursor::latest(&app.control.events(None, 1)));
    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(who.expires.saturating_sub(auth::now()));
    let state = (
        app,
        cursor,
        0u64,
        std::collections::VecDeque::<(Option<String>, Value)>::new(),
        permit,
        who,
    );
    let stream = futures::stream::unfold(
        state,
        move |(app, mut cursor, mut dropped, mut queue, permit, who)| async move {
            loop {
                if tokio::time::Instant::now() >= deadline || !app.auth.principal_active(&who) {
                    return None;
                }
                if let Some((id, value)) = queue.pop_front() {
                    let mut event =
                        Event::default().data(serde_json::to_string(&value).unwrap_or_default());
                    if let Some(id) = id {
                        event = event.id(id);
                    }
                    let item: Result<Event, Infallible> = Ok(event);
                    return Some((item, (app, cursor, dropped, queue, permit, who)));
                }
                if tokio::time::Instant::now() >= deadline {
                    return None;
                }
                let (page, gap, next) =
                    read_after(&cursor, |after| app.control.events(Some(after), 64));
                if let Some(gap) = gap {
                    queue.push_back((None, gap.json()));
                }
                if page.qmp_dropped != dropped {
                    dropped = page.qmp_dropped;
                    queue.push_back((None, qmp_dropped_event_json(dropped)));
                }
                cursor = next;
                let epoch = page.epoch.as_deref();
                queue.extend(page.events.iter().map(|event| {
                    (
                        Some(EventCursor::new(epoch, event.seq).to_string()),
                        event_json(event, epoch),
                    )
                }));
                if queue.is_empty() {
                    tokio::select! {
                        () = app.control.wait_events(cursor.seq) => {}
                        () = app.control.wait_qmp_dropped(dropped) => {}
                        () = tokio::time::sleep_until(deadline) => return None,
                        () = tokio::time::sleep(Duration::from_secs(1)) => {},
                    }
                }
            }
        },
    );
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response())
}

async fn ready(State(app): State<AppState>) -> StatusCode {
    if app.control.probe(false).await.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn update_service(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(
        json!({"@odata.id":"/redfish/v1/UpdateService","@odata.type":"#UpdateService.v1_16_0.UpdateService",
        "Id":"UpdateService","Name":"Firmware inventory","ServiceEnabled":true,"FirmwareInventory":link(hardware::FIRMWARE),
        "Actions":{"Oem":{"#QrabUpdateService.StageFirmware":{"target":firmware::STAGE}}},
        "Oem":{"Qrab":{"@odata.type":"#QrabUpdateService.v1_0_0.QrabUpdateService","Artifacts":link(firmware::ARTIFACTS),"StagedFirmware":link(firmware::STAGED)}}}),
    ))
}
