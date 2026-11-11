//! Wire types shared by the agent and host-side clients.
//!
//! Standard commands use the generated `qapi::qga` types. The namespaced
//! `qrab-*` commands are defined here so a host can call them through the
//! ordinary typed `qapi_protocol::qga::Qga::execute`.
use serde::{Deserialize, Serialize};

/// QGA schema level this agent implements. Clients serialize requests using
/// the version reported by guest-info, so it names the vendored QEMU schema.
pub const SCHEMA_VERSION: (u32, u32, u32) = (11, 1, 3);
/// Reported by guest-info. Exactly three dot-separated components keep it
/// parseable by QGA clients; the qrab build version is in `UpdateStatus`.
pub const AGENT_VERSION: &str = "11.1.3-qrab";
/// The only update program the agent will ever run.
pub const UPDATE_HOOK: &str = "/etc/qrab/update-hook";
pub const MAX_UPDATE_ARGS: usize = 16;
pub const MAX_UPDATE_ARG_BYTES: usize = 256;
/// Bytes of combined hook stdout/stderr retained in `UpdateStatus::output`.
pub const MAX_UPDATE_OUTPUT_BYTES: usize = 8192;

/// Start the fixed update hook asynchronously. Fails while a run is active,
/// while filesystems are frozen, or when the hook is missing or unsafe.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QrabUpdateTrigger {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

/// Report the most recent update hook run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QrabUpdateStatus {}

macro_rules! qrab_command {
    ($type:ty, $name:literal, $returns:ty) => {
        impl qapi::QapiCommand for $type {
            type Returns = $returns;
            const NAME: &'static str = $name;
            const ALLOW_OOB: bool = false;
            const ALLOW_PRECONFIG: bool = false;
            const SUCCESS_RESPONSE: bool = true;
            const UNSTABLE: bool = false;
            const AVAILABLE_SINCE: (u32, u32, u32) = SCHEMA_VERSION;
            const REMOVED_IN: Option<(u32, u32, u32)> = None;
            const PRECONFIG_SINCE: Option<(u32, u32, u32)> = None;
        }
    };
}
qrab_command!(QrabUpdateTrigger, "qrab-update-trigger", UpdateStatus);
qrab_command!(QrabUpdateStatus, "qrab-update-status", UpdateStatus);

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateState {
    /// No hook has run since the agent started.
    #[default]
    Idle,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct UpdateStatus {
    pub state: UpdateState,
    /// Increments for each started run; 0 means none since agent start.
    pub run_id: u64,
    pub args: Vec<String>,
    pub started_unix_ms: Option<u64>,
    pub finished_unix_ms: Option<u64>,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub error: Option<String>,
    /// Tail of the hook's combined stdout and stderr, lossily decoded.
    pub output: String,
    pub output_truncated: bool,
    pub hook_installed: bool,
    pub agent_version: String,
}

/// Arguments are passed to the hook verbatim, never through a shell. They are
/// still restricted to a conservative token alphabet and may not look like
/// options, so a caller cannot change how the hook parses its own flags.
pub fn validate_update_args(args: &[String]) -> Result<(), String> {
    if args.len() > MAX_UPDATE_ARGS {
        return Err(format!("at most {MAX_UPDATE_ARGS} update arguments"));
    }
    for arg in args {
        if arg.is_empty()
            || arg.len() > MAX_UPDATE_ARG_BYTES
            || arg.starts_with('-')
            || !arg
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._:/=+@,-".contains(&b))
        {
            return Err(format!(
                "update arguments must be 1..={MAX_UPDATE_ARG_BYTES} bytes of [A-Za-z0-9._:/=+@,-] \
                 and must not start with '-'"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_arguments_are_bounded_tokens() {
        let ok =
            |a: &[&str]| validate_update_args(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(ok(&[]).is_ok());
        assert!(ok(&["channel=stable", "v1.2.3", "registry.local/qrab@sha256:ab"]).is_ok());
        for bad in ["", "-x", "--force", "a b", "a;b", "$(id)", "a\nb", "ü"] {
            assert!(ok(&[bad]).is_err(), "{bad:?}");
        }
        assert!(ok(&[&"a".repeat(MAX_UPDATE_ARG_BYTES + 1)]).is_err());
        assert!(ok(&["a"; MAX_UPDATE_ARGS + 1]).is_err());
    }
}
