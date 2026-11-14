//! The console page's power controls, as JSON for its script. In-pod actions
//! go to the VM's BMC with the user's ID token; power on and off set the
//! VM's `spec.running`, since an off VM has no Pod or BMC to ask.
use crate::{AppState, Failure, access::Role, auth, bmc, pod_ip, visible_vm};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

/// What the console page can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    /// Sent to the running VM's BMC.
    Bmc(bmc::Action),
    /// `spec.running: true`: the operator starts a Pod.
    On,
    /// `spec.running: false`: the operator stops the VM and keeps it off.
    Off,
}

impl Power {
    /// ```
    /// use qrab_dashboard::{bmc::Action, power::Power};
    /// assert_eq!(Power::parse("on"), Some(Power::On));
    /// assert_eq!(Power::parse("reset"), Some(Power::Bmc(Action::Reset)));
    /// assert_eq!(Power::parse("system_reset"), None);
    /// assert!(Power::Off.needs_confirmation() && !Power::On.needs_confirmation());
    /// ```
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "on" => Some(Self::On),
            "off" => Some(Self::Off),
            other => bmc::Action::parse(other).map(Self::Bmc),
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Bmc(action) => action.key(),
            Self::On => "on",
            Self::Off => "off",
        }
    }
    pub fn needs_confirmation(self) -> bool {
        match self {
            Self::Bmc(action) => action.needs_confirmation(),
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
    /// The manager's QEMU run state through the BMC (`running`, `paused`, …).
    pub run_state: Option<String>,
    /// Why the BMC could not be asked, when it could not.
    pub bmc_error: Option<String>,
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
    let (run_state, bmc_error) = match ip {
        Some(ip) if vm.spec.components.bmc.enabled => {
            match app
                .bmc
                .get::<bmc::Status>(ip, bmc::STATUS, &user.id_token)
                .await
            {
                Ok(status) => (status.run_state, None),
                Err(error) => (None, Some(error.to_string())),
            }
        }
        _ => (None, None),
    };
    Ok(PowerState {
        running: vm.spec.running,
        phase: vm
            .status
            .as_ref()
            .map(|s| s.phase.clone())
            .unwrap_or_default(),
        pod: ip.is_some(),
        run_state,
        bmc_error,
    })
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
        Power::Bmc(action) => {
            if !vm.spec.components.bmc.enabled {
                return Err(Failure::forbidden());
            }
            let ip = pod_ip(app, &vm)
                .await?
                .ok_or_else(|| Failure::unavailable("The VM's Pod is not running."))?;
            app.bmc
                .action(ip, &user.id_token, action)
                .await
                .map_err(|e| {
                    Failure(
                        StatusCode::BAD_GATEWAY,
                        format!("{} failed: {e}.", action.label()),
                    )
                })
        }
    };
    tracing::info!(subject=%user.subject, vm=%name, action=power.key(), success=result.is_ok(), "power action");
    result
}
