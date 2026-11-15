//! The console page's power controls, as JSON for its script, and the power
//! actions the detail page's form shares with them. In-pod actions are Redfish
//! `ComputerSystem.Reset` requests to the VM's own Pod with the user's ID
//! token, for any VM with the Redfish component (which the dashboard calls
//! directly, so the BMC is not needed). A powered-off VM keeps its Pod, so
//! power on is Redfish `On` while that Pod exists and `spec.running`
//! otherwise; power off sets `spec.running: false`, which ends the Pod and
//! keeps the VM off. A VM without Redfish gets only the `spec.running` path.
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
use qrab_operator::api::VirtualMachine;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use tower_sessions::Session;

/// What the console page can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    /// Sent to the running VM's Redfish service.
    Reset(ResetType),
    /// Redfish `On` to the VM's Pod, or `spec.running: true` when there is
    /// none; see [`PowerOn::route`].
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
    pub fn label(self) -> &'static str {
        match self {
            Self::Reset(reset) => reset.label(),
            Self::On => "Power on",
            Self::Off => "Power off",
        }
    }
}

/// Where Power on goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerOn {
    /// The VM's Pod is up: Redfish `On` starts its QEMU in place (a no-op
    /// while on; a paused VM resumes).
    Redfish(IpAddr),
    /// No Pod, or the VM is set to stop: `spec.running: true`.
    Spec,
}

impl PowerOn {
    /// ```
    /// use qrab_dashboard::power::PowerOn;
    /// let pod = Some("10.0.0.7".parse().unwrap());
    /// assert_eq!(PowerOn::route(true, true, pod), PowerOn::Redfish(pod.unwrap()));
    /// // No Pod yet, a VM set to stop (its Pod is ending) or no Redfish.
    /// assert_eq!(PowerOn::route(true, true, None), PowerOn::Spec);
    /// assert_eq!(PowerOn::route(false, true, pod), PowerOn::Spec);
    /// assert_eq!(PowerOn::route(true, false, pod), PowerOn::Spec);
    /// ```
    pub fn route(running: bool, redfish: bool, pod: Option<IpAddr>) -> Self {
        match pod {
            Some(ip) if running && redfish => Self::Redfish(ip),
            _ => Self::Spec,
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
    let Some(ip) = ip.filter(|_| vm.spec.components.redfish_enabled()) else {
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
    let result = apply(app, &user, &vm, power).await;
    tracing::info!(subject=%user.subject, vm=%name, action=power.key(), success=result.is_ok(), "power action");
    result
}

/// The refusal for an in-pod action on a VM without the Redfish component.
fn no_redfish(reset: ResetType) -> Failure {
    Failure(
        StatusCode::CONFLICT,
        format!(
            "{} needs the VM's Redfish service, and components.redfish is not enabled for this VM. Power on and Power off still work.",
            reset.label()
        ),
    )
}

/// Carries out one power action on a VM the caller has already checked the
/// user is an admin of.
pub(crate) async fn apply(
    app: &AppState,
    user: &auth::User,
    vm: &VirtualMachine,
    power: Power,
) -> Result<(), Failure> {
    let redfish = vm.spec.components.redfish_enabled();
    let on = match power {
        Power::On if vm.spec.running && redfish => {
            PowerOn::route(true, redfish, pod_ip(app, vm).await?)
        }
        _ => PowerOn::Spec,
    };
    match (power, on) {
        (Power::On, PowerOn::Redfish(ip)) => reset(app, ip, &user.id_token, ResetType::On).await,
        (Power::On | Power::Off, _) => {
            let name = vm.metadata.name.as_deref().ok_or_else(Failure::internal)?;
            let uid = vm.metadata.uid.as_deref().ok_or_else(Failure::internal)?;
            app.cluster
                .set_running(name, uid, power == Power::On)
                .await
                .map_err(|e| Failure::upstream(&e))
        }
        (Power::Reset(kind), _) => {
            if !redfish {
                return Err(no_redfish(kind));
            }
            let ip = pod_ip(app, vm)
                .await?
                .ok_or_else(|| Failure::unavailable("The VM's Pod is not running."))?;
            reset(app, ip, &user.id_token, kind).await
        }
    }
}
