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
    "GracefulShutdown",
    "ForceRestart",
    "Pause",
    "Resume",
    "Nmi",
    "GracefulRestart",
];
/// DMTF ResetType values that are valid but cannot be performed honestly.
const UNSUPPORTED_RESET_TYPES: &[&str] = &[
    "ForceOff",
    "ForceOn",
    "PushPowerButton",
    "Suspend",
    "FullPowerCycle",
];

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
    fn vm(error: &manager::Error, message: &'static str) -> Self {
        match error {
            manager::Error::ShuttingDown => Self::unavailable("The VM is shutting down."),
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
    let view = app.control.config_view().await.ok()?;
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
    let view = app
        .control
        .config_view()
        .await
        .map_err(|_| Failure::unavailable("The VM configuration is unavailable."))?;
    let status = app.control.status().await.ok();
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
                "ResetType@Redfish.AllowableValues": reset_types(&view),
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
    if let Ok(Ok(hardware)) =
        tokio::time::timeout(Duration::from_millis(200), app.control.inventory()).await
    {
        system["ProcessorSummary"]["Count"] = json!(hardware.processors.len());
        system["ProcessorSummary"]["LogicalProcessorCount"] = json!(hardware
            .processors
            .iter()
            .map(|p| p.total_threads)
            .sum::<u64>());
    }
    if let Some(uuid) = vm_uuid(&app, &who).await {
        system["UUID"] = json!(uuid);
    }
    if app
        .control
        .agent_status()
        .await
        .ok()
        .is_some_and(|status| status.connected)
    {
        if let Ok(snapshot) = app.control.agent_boot().await {
            system["Boot"] = boot::system_boot(&snapshot);
        }
    }
    // The same role-based redaction as the browser BMC.
    let view = match who.role {
        Role::Admin => view,
        Role::Viewer => view.redacted(),
    };
    // Cached connection state from the manager; no guest round trip.
    let agent = app.control.agent_status().await.ok();
    system["Oem"] = json!({"Qrab": {"@odata.type": "#QrabComputerSystem.v1_0_0.QrabComputerSystem", "Configuration": view, "GuestAgent": agent, "Metrics": link(telemetry::METRICS)}});
    Ok(Json(system))
}

/// Map a Redfish ResetType to a manager action. `Ok(None)` is a no-op.
pub fn reset_action(
    reset_type: &str,
    status: Option<&pod::Status>,
) -> Result<Option<Action>, Failure> {
    let power_state = power(status).0;
    match reset_type {
        "GracefulShutdown" => Ok(Some(Action::Powerdown)),
        "ForceRestart" => Ok(Some(Action::Reset)),
        "Nmi" => Ok(Some(Action::Nmi)),
        "GracefulRestart" => Ok(Some(Action::Reboot)),
        "PowerCycle" => Ok(Some(Action::ColdRestart)),
        "Pause" => Ok(Some(Action::Pause)),
        "Resume" => Ok(Some(Action::Resume)),
        "On" if power_state == "Paused" => Ok(Some(Action::Resume)),
        "On" if power_state == "On" => Ok(None),
        "On" => Err(Failure::new(
            StatusCode::CONFLICT,
            "OperationNotAllowed",
            "The VM is not running; a new VM instance is started by the VirtualMachine spec.running field.",
            "Set spec.running on the VirtualMachine resource.",
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
    if reset_type == "PowerCycle" {
        let view = app
            .control
            .config_view()
            .await
            .map_err(|_| Failure::unavailable("Power-cycle capabilities are unavailable."))?;
        if !view.restart_on_exit {
            return Err(Failure::new(
                StatusCode::CONFLICT,
                "OperationNotAllowed",
                "PowerCycle requires an operator-managed restart-on-exit guarantee.",
                "Cold-restart the standalone manager explicitly.",
            ));
        }
    }
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

fn oem(event: &VmEvent) -> Value {
    json!({"Qrab": {
        "@odata.type": "#QrabEvent.v1_0_0.QrabEvent",
        "Sequence": event.seq,
        "Event": event.event,
        "Data": event.data,
        "Truncated": event.truncated,
    }})
}

pub fn log_entry_json(event: &VmEvent) -> Value {
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
        "Oem": oem(event),
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
        page.events.iter().map(log_entry_json).collect(),
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
        .map(|event| Json(log_entry_json(event)))
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

pub fn event_json(event: &VmEvent) -> Value {
    json!({
        "@odata.type": "#Event.v1_10_0.Event",
        "Id": event.seq.to_string(),
        "Name": "qrab VM event",
        "Events": [{
            "EventType": "Other",
            "EventId": event.seq.to_string(),
            "EventTimestamp": event_time(event),
            "Severity": severity(&event.event),
            "Message": event.event,
            "MessageId": "Qrab.1.0.QmpEvent",
            "MessageArgs": [event.event],
            "OriginOfCondition": link(SYSTEM),
            "Oem": oem(event),
        }],
    })
}

/// Live events close on credential expiry or session revocation. Idle streams
/// do not refresh a session's last authenticated request time.
async fn sse(State(app): State<AppState>, headers: HeaderMap) -> Result<Response, Failure> {
    let who = principal(&app, &headers).await?;
    let permit = app
        .streams
        .clone()
        .try_acquire_owned()
        .map_err(|_| Failure::unavailable("Too many event streams are open."))?;
    let resume = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    let cursor = resume.unwrap_or_else(|| app.control.events(None, 1).latest);
    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(who.expires.saturating_sub(auth::now()));
    let state = (
        app,
        cursor,
        std::collections::VecDeque::<VmEvent>::new(),
        permit,
        who,
    );
    let stream = futures::stream::unfold(
        state,
        move |(app, mut cursor, mut queue, permit, who)| async move {
            loop {
                if tokio::time::Instant::now() >= deadline || !app.auth.principal_active(&who) {
                    return None;
                }
                if let Some(event) = queue.pop_front() {
                    let data = serde_json::to_string(&event_json(&event)).unwrap_or_default();
                    let item: Result<Event, Infallible> =
                        Ok(Event::default().id(event.seq.to_string()).data(data));
                    return Some((item, (app, cursor, queue, permit, who)));
                }
                if tokio::time::Instant::now() >= deadline {
                    return None;
                }
                let page = app.control.events(Some(cursor), 64);
                cursor = page.next;
                queue.extend(page.events);
                if queue.is_empty() {
                    tokio::select! {
                        () = app.control.wait_events(cursor) => {}
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

pub(crate) fn reset_types(view: &pod::ConfigView) -> Vec<&'static str> {
    let mut types = RESET_TYPES.to_vec();
    if view.restart_on_exit {
        types.push("PowerCycle");
    }
    types
}
