//! Guest-agent status for the dashboard. Freeze, thaw and the update trigger
//! are Redfish's `Oem/QrabComputerSystem` actions.
use crate::{auth, AppState, Failure};
use axum::{extract::State, Json};
use manager::pod::{FreezeState, UpdateStatus};
use serde::Serialize;
use std::time::Duration;

/// Bound on the guest round trips behind a status read.
const READ_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Serialize)]
pub struct AgentView {
    status: manager::pod::AgentStatus,
    /// `None` while the agent is disconnected or did not answer in time.
    freeze: Option<FreezeState>,
    update: Option<UpdateStatus>,
}

/// Hook arguments, output and errors can carry deployment detail.
fn redacted(mut update: UpdateStatus) -> UpdateStatus {
    update.args.clear();
    update.output.clear();
    update.output_truncated = false;
    update.error = None;
    update
}

pub async fn view(
    State(app): State<AppState>,
    principal: auth::Principal,
) -> Result<Json<AgentView>, Failure> {
    let status = app
        .control
        .agent_status()
        .await
        .map_err(|error| Failure::vm(&error, "The guest agent status is unavailable."))?;
    let (freeze, update) = if status.connected {
        let freeze = tokio::time::timeout(READ_TIMEOUT, app.control.agent_freeze_status());
        let update = tokio::time::timeout(READ_TIMEOUT, app.control.agent_update_status());
        let (freeze, update) = tokio::join!(freeze, update);
        (
            freeze.ok().and_then(Result::ok),
            update.ok().and_then(Result::ok),
        )
    } else {
        (None, None)
    };
    let update = match principal.role {
        auth::Role::Admin => update,
        auth::Role::Viewer => update.map(redacted),
    };
    Ok(Json(AgentView {
        status,
        freeze,
        update,
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn viewers_never_see_update_arguments_output_or_errors() {
        let update = manager::pod::UpdateStatus {
            run_id: 3,
            args: vec!["release=secret".into()],
            output: "token=abc".into(),
            output_truncated: true,
            error: Some("hook failed at /srv/private".into()),
            hook_installed: true,
            ..Default::default()
        };
        let shown = super::redacted(update);
        let json = serde_json::to_string(&shown).unwrap();
        for hidden in ["release=secret", "token=abc", "/srv/private"] {
            assert!(!json.contains(hidden), "{json}");
        }
        assert_eq!((shown.run_id, shown.hook_installed), (3, true));
    }
}
