use super::Config;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Starting,
    Running,
    Stopping,
    Exited,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub phase: Phase,
    pub ready: bool,
    pub manager_pid: u32,
    pub manager_start_ticks: u64,
    pub qemu_pid: Option<u32>,
    pub qemu_start_ticks: Option<u64>,
    pub qemu_version: Option<(u32, u32, u32)>,
    pub run_state: Option<String>,
    pub observed_unix_ms: u64,
    pub exit_code: Option<i32>,
    pub detail: Option<String>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

/// A zombie still has a /proc record and start time, but cannot be healthy.
/// The final ')' belongs to comm, which may itself contain spaces/parentheses.
fn live_start_ticks(stat: &str) -> Result<u64> {
    let (_, tail) = stat
        .rsplit_once(") ")
        .ok_or_else(|| Error::Spawn("invalid process identity".into()))?;
    let mut fields = tail.split_whitespace();
    let state = fields.next().unwrap_or("");
    if state.len() != 1
        || !state.as_bytes()[0].is_ascii_alphabetic()
        || matches!(state, "Z" | "X" | "x")
    {
        return Err(Error::Spawn(
            "process is dead or its identity is invalid".into(),
        ));
    }
    // Field 3 (state) was consumed; starttime is field 22.
    fields
        .nth(18)
        .and_then(|ticks| ticks.parse().ok())
        .ok_or_else(|| Error::Spawn("invalid process start time".into()))
}

/// PID reuse must not make an old readiness record look current.
pub(super) fn start_ticks(pid: u32) -> Result<u64> {
    live_start_ticks(&std::fs::read_to_string(format!("/proc/{pid}/stat"))?)
}

pub(super) fn publish(config: &Config, status: &mut Status) -> Result<()> {
    status.observed_unix_ms = now_ms();
    let mut file = tempfile::NamedTempFile::new_in(&config.runtime_dir)?;
    serde_json::to_writer(&mut file, status)?;
    file.write_all(b"\n")?;
    file.as_file().sync_data()?;
    file.persist(config.runtime_dir.join("status.json"))
        .map_err(|e| Error::Io(e.error))?;
    Ok(())
}

pub fn status(config: &Config) -> Result<Status> {
    Ok(serde_json::from_slice(&std::fs::read(
        config.runtime_dir.join("status.json"),
    )?)?)
}

/// Readiness means QMP reports running, not that guest applications are ready.
/// Liveness permits paused VMs, but both probes require a fresh QMP heartbeat
/// and exact manager/child process identities. No second QMP client is opened.
pub fn probe(config: &Config, readiness: bool) -> Result<()> {
    let status = status(config)?;
    let age = now_ms()
        .checked_sub(status.observed_unix_ms)
        .ok_or_else(|| Error::Spawn("status timestamp is in the future".into()))?;
    let child_alive = status
        .qemu_pid
        .zip(status.qemu_start_ticks)
        .is_some_and(|(pid, ticks)| start_ticks(pid).ok() == Some(ticks));
    if age > 5_000
        || start_ticks(status.manager_pid).ok() != Some(status.manager_start_ticks)
        || !child_alive
        || status.phase != Phase::Running
        || (readiness && !status.ready)
    {
        return Err(Error::Spawn(
            "VM is not healthy or its status is stale".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(state: &str) -> String {
        let mut fields = vec!["0"; 20];
        fields[0] = state;
        fields[19] = "12345";
        format!("42 (name with ) parentheses) {}", fields.join(" "))
    }

    #[test]
    fn process_identity_handles_comm_and_rejects_dead_states() {
        for state in ["R", "S", "T", "I"] {
            assert_eq!(live_start_ticks(&record(state)).unwrap(), 12345);
        }
        for state in ["Z", "X", "x", "", "invalid"] {
            assert!(
                live_start_ticks(&record(state)).is_err(),
                "accepted state {state:?}"
            );
        }
        assert!(live_start_ticks("truncated").is_err());
        assert!(live_start_ticks("42 (process) R 0").is_err());
    }

    fn fixture() -> (tempfile::TempDir, Config, Status) {
        let dir = tempfile::tempdir().unwrap();
        let config = serde_json::from_value(serde_json::json!({
            "kernel": "/unused", "runtime_dir": dir.path()
        }))
        .unwrap();
        let pid = std::process::id();
        let ticks = start_ticks(pid).unwrap();
        let status = Status {
            phase: Phase::Running,
            ready: true,
            manager_pid: pid,
            manager_start_ticks: ticks,
            qemu_pid: Some(pid),
            qemu_start_ticks: Some(ticks),
            qemu_version: Some((11, 1, 3)),
            run_state: Some("running".into()),
            observed_unix_ms: now_ms(),
            exit_code: None,
            detail: None,
        };
        (dir, config, status)
    }

    fn write(config: &Config, status: &Status) {
        std::fs::write(
            config.runtime_dir.join("status.json"),
            serde_json::to_vec(status).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn readiness_requires_running_while_liveness_allows_a_paused_guest() {
        let (_dir, config, mut status) = fixture();
        write(&config, &status);
        probe(&config, true).unwrap();
        status.ready = false;
        status.run_state = Some("paused".into());
        write(&config, &status);
        probe(&config, false).unwrap();
        assert!(probe(&config, true).is_err());
    }

    #[test]
    fn stale_or_future_heartbeats_cannot_pass_probes() {
        let (_dir, config, mut status) = fixture();
        status.observed_unix_ms = now_ms().saturating_sub(60_000);
        write(&config, &status);
        assert!(probe(&config, false).is_err());
        status.observed_unix_ms = now_ms() + 60_000;
        write(&config, &status);
        assert!(probe(&config, false).is_err());
    }

    #[test]
    fn pid_reuse_and_terminal_phases_cannot_look_ready() {
        let (_dir, config, mut status) = fixture();
        status.qemu_start_ticks = status.qemu_start_ticks.map(|ticks| ticks + 1);
        write(&config, &status);
        assert!(probe(&config, true).is_err());
        status.qemu_start_ticks = Some(status.manager_start_ticks);
        status.phase = Phase::Exited;
        write(&config, &status);
        assert!(probe(&config, false).is_err());
    }
}
