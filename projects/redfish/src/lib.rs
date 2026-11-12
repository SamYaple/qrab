//! A minimal DMTF Redfish service for one qrab VM, reached only through the
//! manager control API. See docs/redfish.md for the resource tree.
pub mod auth;
pub mod config;

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
    Json, Router,
};
use manager::pod::{self, Action, Phase, VmEvent};
use serde_json::{json, Value};
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::Semaphore;

pub const SYSTEM: &str = "/redfish/v1/Systems/1";
pub const MANAGER: &str = "/redfish/v1/Managers/1";
const RESET_TARGET: &str = "/redfish/v1/Systems/1/Actions/ComputerSystem.Reset";
const LOG: &str = "/redfish/v1/Managers/1/LogServices/Events";
const REGISTRY: &str = "Base.1.18";
/// Reset types this service performs; see [`reset_action`].
pub const RESET_TYPES: &[&str] = &["On", "GracefulShutdown", "ForceRestart", "Pause", "Resume"];
/// DMTF ResetType values that are valid but cannot be performed honestly.
const UNSUPPORTED_RESET_TYPES: &[&str] = &[
    "ForceOff",
    "GracefulRestart",
    "ForceOn",
    "Nmi",
    "PushPowerButton",
    "PowerCycle",
    "Suspend",
    "FullPowerCycle",
];

#[derive(Clone)]
pub struct AppState {
    pub auth: Arc<Authenticator>,
    pub control: pod::Backend,
    pub streams: Arc<Semaphore>,
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
            "A valid OIDC bearer token is required.",
            "Present an unexpired ID token for this service's audience.",
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

pub fn router(app: AppState) -> Router {
    Router::new()
        .route("/redfish", get(versions))
        .route("/redfish/v1", get(service_root))
        .route("/redfish/v1/", get(service_root))
        .route("/redfish/v1/Systems", get(systems))
        .route(SYSTEM, get(system))
        .route(RESET_TARGET, post(reset))
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
        .route("/redfish/v1/EventService", get(event_service))
        .route("/redfish/v1/EventService/Subscriptions", get(subscriptions))
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
        .layer(axum::middleware::from_fn(headers))
        .with_state(app)
}

async fn headers(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let mut response = next.run(request).await;
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
    let header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    app.auth
        .authenticate(header)
        .await
        .map_err(|rejection| match rejection {
            Rejection::Unauthenticated => Failure::unauthenticated(),
            Rejection::Forbidden => Failure::forbidden(),
        })
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
    principal(&app, &headers).await?;
    Ok(Json(json!({"v1": "/redfish/v1/"})))
}

async fn service_root(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let who = principal(&app, &headers).await?;
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
        "EventService": link("/redfish/v1/EventService"),
        "Links": {},
    });
    if let Some(uuid) = vm_uuid(&app, &who).await {
        root["UUID"] = json!(uuid);
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
        "ProcessorSummary": {"Count": view.cpus, "LogicalProcessorCount": view.cpus, "Status": status_object},
        "MemorySummary": {"TotalSystemMemoryGiB": view.memory_mib as f64 / 1024.0, "Status": status_object},
        "Actions": {"#ComputerSystem.Reset": {
            "target": RESET_TARGET,
            "ResetType@Redfish.AllowableValues": RESET_TYPES,
        }},
        "Links": {"ManagedBy": [link(MANAGER)]},
    });
    if let Some(uuid) = vm_uuid(&app, &who).await {
        system["UUID"] = json!(uuid);
    }
    // The same role-based redaction as the browser BMC.
    let view = match who.role {
        Role::Admin => view,
        Role::Viewer => view.redacted(),
    };
    system["Oem"] = json!({"Qrab": {"@odata.type": "#QrabComputerSystem.v1_0_0.QrabComputerSystem", "Configuration": view}});
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
        "Links": {"ManagerForServers": [link(SYSTEM)]},
    })))
}

async fn log_services(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        "/redfish/v1/Managers/1/LogServices",
        "LogServiceCollection",
        "Log Service Collection",
        vec![link(LOG)],
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
    Ok(Json(json!({
        "@odata.id": "/redfish/v1/EventService",
        "@odata.type": "#EventService.v1_10_0.EventService",
        "Id": "EventService",
        "Name": "Event Service",
        "ServiceEnabled": true,
        "ServerSentEventUri": "/redfish/v1/EventService/SSE",
        "Subscriptions": link("/redfish/v1/EventService/Subscriptions"),
        "Status": {"State": "Enabled", "Health": "OK"},
    })))
}

async fn subscriptions(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    // Push delivery is not offered; use ServerSentEventUri.
    Ok(Json(collection(
        "/redfish/v1/EventService/Subscriptions",
        "EventDestinationCollection",
        "Event Subscriptions",
        Vec::new(),
    )))
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

/// Live Redfish events. The bearer token is checked at admission; the
/// stream ends when the token expires, since bearer tokens cannot be revoked.
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
    );
    let stream = futures::stream::unfold(
        state,
        move |(app, mut cursor, mut queue, permit)| async move {
            loop {
                if let Some(event) = queue.pop_front() {
                    let data = serde_json::to_string(&event_json(&event)).unwrap_or_default();
                    let item: Result<Event, Infallible> =
                        Ok(Event::default().id(event.seq.to_string()).data(data));
                    return Some((item, (app, cursor, queue, permit)));
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
