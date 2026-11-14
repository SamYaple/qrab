//! The console page's power controls, as JSON for its script. In-pod actions
//! are Redfish `ComputerSystem.Reset` requests to the VM's own Pod with the
//! user's ID token; power on and off set the VM's `spec.running`, since an off
//! VM has no Pod or Redfish service to ask.
use crate::{
    AppState, Failure,
    access::Role,
    auth, pod_ip,
    redfish::{self, RedfishError, ResetType},
    visible_vm,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use tower_sessions::Session;

/// What the console page can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    /// Sent to the running VM's Redfish service.
    Reset(ResetType),
    /// `spec.running: true`: the operator starts a Pod.
    On,
    /// `spec.running: false`: the operator stops the VM and keeps it off.
    Off,
}

impl Power {
    /// ```
    /// use qrab_dashboard::{power::Power, redfish::ResetType};
    /// assert_eq!(Power::parse("on"), Some(Power::On));
    /// assert_eq!(Power::parse("ForceRestart"), Some(Power::Reset(ResetType::ForceRestart)));
    /// assert_eq!(Power::parse("system_reset"), None);
    /// assert!(Power::Off.needs_confirmation() && !Power::On.needs_confirmation());
    /// ```
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "on" => Some(Self::On),
            "off" => Some(Self::Off),
            other => ResetType::parse(other).map(Self::Reset),
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Reset(reset) => reset.name(),
            Self::On => "on",
            Self::Off => "off",
        }
    }
    pub fn needs_confirmation(self) -> bool {
        match self {
            Self::Reset(reset) => reset.needs_confirmation(),
            Self::On => false,
            Self::Off => true,
        }
    }
}

#[derive(Deserialize)]
pub struct PowerRequest {
    action: String,
    #[serde(default)]
    confirm: bool,
}

/// What the page needs to enable its buttons.
#[derive(Debug, Serialize)]
pub struct PowerState {
    /// The VM's desired power (`spec.running`).
    pub running: bool,
    /// The operator's phase for the VM, empty before its first status.
    pub phase: String,
    /// Whether the VM's own Pod is up with an address.
    pub pod: bool,
    /// Redfish `Systems/1` `PowerState` (`On`, `Paused`, `PoweringOn`, …).
    pub power_state: Option<String>,
    /// Redfish `Systems/1` `Status.Health`.
    pub health: Option<String>,
    /// The in-pod ResetTypes Redfish allows (`ResetActionInfo`), in button
    /// order; `None` when Redfish could not be asked.
    pub reset_types: Option<Vec<ResetType>>,
    /// Why Redfish could not be asked, when it could not.
    pub redfish_error: Option<String>,
}

/// Errors as plain text: the page shows them in its alert line.
fn plain(failure: Failure) -> Response {
    (failure.0, failure.1).into_response()
}

pub async fn status(
    State(app): State<AppState>,
    session: Session,
    Path((namespace, name)): Path<(String, String)>,
) -> Response {
    state(&app, &session, &namespace, &name)
        .await
        .map_or_else(plain, |state| Json(state).into_response())
}

async fn state(
    app: &AppState,
    session: &Session,
    namespace: &str,
    name: &str,
) -> Result<PowerState, Failure> {
    let user = auth::user(session).await?;
    let (vm, role) = visible_vm(app, &user, namespace, name).await?;
    if role != Role::Admin {
        return Err(Failure::forbidden());
    }
    let ip = pod_ip(app, &vm).await?;
    let mut state = PowerState {
        running: vm.spec.running,
        phase: vm
            .status
            .as_ref()
            .map(|s| s.phase.clone())
            .unwrap_or_default(),
        pod: ip.is_some(),
        power_state: None,
        health: None,
        reset_types: None,
        redfish_error: None,
    };
    let Some(ip) = ip.filter(|_| vm.spec.components.bmc.enabled) else {
        return Ok(state);
    };
    let (system, reset_types) = tokio::join!(
        app.redfish
            .get::<redfish::System>(ip, redfish::SYSTEM, &user.id_token, redfish::MAX_BODY),
        app.redfish.reset_types(ip, &user.id_token),
    );
    match (system, reset_types) {
        (Ok(system), Ok(reset_types)) => {
            state.power_state = system.power_state;
            state.health = system.status.health;
            state.reset_types = Some(reset_types);
        }
        (Err(error), _) | (_, Err(error)) => state.redfish_error = Some(error.to_string()),
    }
    Ok(state)
}

pub async fn act(
    State(app): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Path((namespace, name)): Path<(String, String)>,
    Json(request): Json<PowerRequest>,
) -> Response {
    match power(&app, &session, &headers, &namespace, &name, request).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(failure) => plain(failure),
    }
}

/// Sends one reset to the VM's own Pod. A Redfish refusal (403) stays a
/// refusal; other failures are the upstream's.
pub(crate) async fn reset(
    app: &AppState,
    ip: IpAddr,
    token: &str,
    reset: ResetType,
) -> Result<(), Failure> {
    app.redfish.reset(ip, token, reset).await.map_err(|error| {
        let code = match error {
            RedfishError::Status(403) => StatusCode::FORBIDDEN,
            RedfishError::Status(409) => StatusCode::CONFLICT,
            _ => StatusCode::BAD_GATEWAY,
        };
        let verb = if code == StatusCode::FORBIDDEN {
            "was refused"
        } else {
            "failed"
        };
        Failure(code, format!("{} {verb}: {error}.", reset.label()))
    })
}

async fn power(
    app: &AppState,
    session: &Session,
    headers: &HeaderMap,
    namespace: &str,
    name: &str,
    request: PowerRequest,
) -> Result<(), Failure> {
    let user = auth::user(session).await?;
    let csrf = headers
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !auth::mutation_allowed(&user, csrf, headers, &app.origin) {
        return Err(Failure::forbidden());
    }
    let power =
        Power::parse(&request.action).ok_or_else(|| Failure::invalid("Unknown power action."))?;
    if power.needs_confirmation() && !request.confirm {
        return Err(Failure::invalid("Confirm the power action."));
    }
    let (vm, role) = visible_vm(app, &user, namespace, name).await?;
    if role != Role::Admin {
        return Err(Failure::forbidden());
    }
    let result = match power {
        Power::On | Power::Off => {
            let uid = vm.metadata.uid.as_deref().ok_or_else(Failure::internal)?;
            app.cluster
                .set_running(name, uid, power == Power::On)
                .await
                .map_err(|e| Failure::upstream(&e))
        }
        Power::Reset(kind) => {
            if !vm.spec.components.bmc.enabled {
                return Err(Failure::forbidden());
            }
            let ip = pod_ip(app, &vm)
                .await?
                .ok_or_else(|| Failure::unavailable("The VM's Pod is not running."))?;
            reset(app, ip, &user.id_token, kind).await
        }
    };
    tracing::info!(subject=%user.subject, vm=%name, action=power.key(), success=result.is_ok(), "power action");
    result
}
