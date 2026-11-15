//! The guest power state of a Ready VM Pod, read from its Redfish sidecar's
//! unauthenticated Prometheus export (`qrab_vm_power_state`, port 9181).
//!
//! The read is bounded and best effort: a failure reports the state as
//! unknown and never fails, replaces or restarts the Pod.
use crate::{
    api::{VirtualMachine, VirtualMachineCondition, VirtualMachineStatus},
    resources::REDFISH_METRICS_PORT,
};
use k8s_openapi::api::core::v1::Pod;
use std::{
    fmt,
    net::{IpAddr, SocketAddr},
    sync::{Arc, OnceLock},
    time::Duration,
};

/// The condition type that is True while the VM is powered off.
pub const POWERED_OFF: &str = "PoweredOff";
/// The Redfish exporter's power family: 1 for the current state.
pub const FAMILY: &str = "qrab_vm_power_state";
/// Above the exporter's own two-second deadline per manager call.
pub const READ_TIMEOUT: Duration = Duration::from_secs(3);
/// The export is a few kilobytes; anything far larger is not ours.
const BODY_LIMIT: usize = 256 * 1024;

/// A Redfish `ComputerSystem.PowerState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    On,
    Off,
    PoweringOn,
    PoweringOff,
    Paused,
}

impl PowerState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::On => "On",
            Self::Off => "Off",
            Self::PoweringOn => "PoweringOn",
            Self::PoweringOff => "PoweringOff",
            Self::Paused => "Paused",
        }
    }

    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "On" => Self::On,
            "Off" => Self::Off,
            "PoweringOn" => Self::PoweringOn,
            "PoweringOff" => Self::PoweringOff,
            "Paused" => Self::Paused,
            _ => return None,
        })
    }
}

impl fmt::Display for PowerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The current state in a Prometheus text exposition: the one known
/// `qrab_vm_power_state{state=...}` sample at 1. None when all are 0 (Redfish
/// could not reach the manager), when several are 1, or when it is missing.
///
/// ```
/// use qrab_operator::power::{parse, PowerState};
/// let text = "# TYPE qrab_vm_power_state gauge\n\
///     qrab_vm_power_state{state=\"On\"} 0\n\
///     qrab_vm_power_state{state=\"Off\"} 1\n";
/// assert_eq!(parse(text), Some(PowerState::Off));
/// assert_eq!(parse("qrab_vm_power_state{state=\"On\"} 0\n"), None);
/// ```
pub fn parse(exposition: &str) -> Option<PowerState> {
    let mut current = None;
    for line in exposition.lines().map(str::trim) {
        let Some(rest) = line
            .strip_prefix(FAMILY)
            .and_then(|rest| rest.strip_prefix('{'))
        else {
            continue;
        };
        let Some((labels, value)) = rest.split_once('}') else {
            continue;
        };
        let state = labels.split(',').find_map(|label| {
            let (name, value) = label.trim().split_once('=')?;
            (name.trim() == "state").then(|| value.trim().trim_matches('"'))
        });
        let one = value
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            == Some(1.0);
        if !one {
            continue;
        }
        // A state this operator does not know is as good as no reading.
        let state = PowerState::named(state?)?;
        if current.replace(state).is_some() {
            return None;
        }
    }
    current
}

/// What the operator knows about a Ready Pod's guest power.
pub type Reading = std::result::Result<PowerState, String>;

fn client() -> std::result::Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<std::result::Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            // Plain HTTP only; the TLS backend is configured so building the
            // client never depends on a process-wide crypto provider.
            let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
            reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .http1_only()
                .pool_max_idle_per_host(0)
                .timeout(READ_TIMEOUT)
                .tls_backend_preconfigured(tls)
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// The metrics URL of a Pod, from its IP.
pub fn metrics_url(pod: &Pod) -> std::result::Result<String, String> {
    let ip: IpAddr = pod
        .status
        .as_ref()
        .and_then(|s| s.pod_ip.as_deref())
        .ok_or("the VM Pod has no IP")?
        .parse()
        .map_err(|_| "the VM Pod IP is malformed")?;
    Ok(format!(
        "http://{}/metrics",
        SocketAddr::new(ip, REDFISH_METRICS_PORT)
    ))
}

/// Read the guest power state from the Pod's metrics export, within
/// [`READ_TIMEOUT`] and [`BODY_LIMIT`].
pub async fn read(pod: &Pod) -> Reading {
    fetch(&metrics_url(pod)?).await
}

async fn fetch(url: &str) -> Reading {
    let client = client()?;
    let fetch = async {
        let mut response = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("metrics read failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("metrics read returned {}", response.status()));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("metrics read failed: {e}"))?
        {
            if chunk.len() > BODY_LIMIT.saturating_sub(body.len()) {
                return Err("the metrics export exceeds its bound".into());
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    };
    let body = tokio::time::timeout(READ_TIMEOUT, fetch)
        .await
        .map_err(|_| "metrics read timed out".to_owned())??;
    parse(&String::from_utf8_lossy(&body)).ok_or_else(|| {
        format!("Redfish reports no current {FAMILY}; the manager may be unreachable")
    })
}

/// The VM phase, readiness and reason of a Ready Pod given its reading: an
/// off or paused guest is not Running. Unknown keeps the Pod-derived answer.
fn ready_phase(reading: Option<&Reading>) -> (&'static str, bool, &'static str) {
    match reading {
        Some(Ok(PowerState::Off)) => ("PoweredOff", false, "GuestPoweredOff"),
        Some(Ok(PowerState::Paused)) => ("Paused", false, "GuestPaused"),
        _ => ("Running", true, "VMRunning"),
    }
}

fn ready_message(vm: &VirtualMachine, reading: Option<&Reading>) -> &'static str {
    let no_nics = vm.spec.nics.is_empty();
    match reading {
        Some(Ok(PowerState::Off)) => {
            "The guest is powered off inside its Pod, which stays up so Redfish can power it on; spec.running=false removes the Pod"
        }
        Some(Ok(PowerState::Paused)) => "The guest is paused; Redfish Resume continues it",
        Some(Ok(_)) if no_nics => {
            "The VM manager, the enabled BMC/Redfish components and the guest are up; guest networking is disabled"
        }
        Some(Ok(_)) => {
            "The VM manager, the enabled BMC/Redfish components, the guest and the NIC Pods are up"
        }
        _ if no_nics => {
            "The VM manager and the enabled BMC/Redfish components are live; the guest's power state is unknown (see the PoweredOff condition); guest networking is disabled"
        }
        _ => {
            "The VM manager, the enabled BMC/Redfish components and the NIC Pods are live; the guest's power state is unknown (see the PoweredOff condition)"
        }
    }
}

/// The status of a VM whose Pod is converged: Pending until the Pod is
/// Ready, then Running, PoweredOff or Paused by the guest power `reading`
/// (`None` without Redfish), with `powerState` and the `PoweredOff`
/// condition.
pub fn pod_status(
    vm: &VirtualMachine,
    pod: &Pod,
    ready: bool,
    reading: Option<&Reading>,
    now: &str,
) -> VirtualMachineStatus {
    if !ready {
        return crate::controller::state(
            vm,
            Some(pod),
            "Pending",
            false,
            "PodNotReady",
            "Waiting for the VM Pod to become ready",
        );
    }
    let (phase, ready, reason) = ready_phase(reading);
    let mut status = crate::controller::state(
        vm,
        Some(pod),
        phase,
        ready,
        reason,
        ready_message(vm, reading),
    );
    observe(&mut status, vm, reading, now);
    status
}

/// Set `powerState` and the `PoweredOff` condition of a Ready Pod's status.
/// `None` means the VM has no Redfish to read.
fn observe(
    status: &mut VirtualMachineStatus,
    vm: &VirtualMachine,
    reading: Option<&Reading>,
    now: &str,
) {
    status.power_state = reading
        .and_then(|r| r.as_ref().ok())
        .map(|s| s.as_str().to_owned());
    let (state, reason, message) = match reading {
        None => (
            "Unknown",
            "PowerStateUnavailable".to_owned(),
            "Redfish is disabled, so the operator cannot read the guest's power state".to_owned(),
        ),
        Some(Err(error)) => ("Unknown", "PowerStateUnknown".to_owned(), error.clone()),
        Some(Ok(PowerState::Off)) => (
            "True",
            "GuestPoweredOff".to_owned(),
            "The guest is powered off inside its Pod; Redfish On starts it again".to_owned(),
        ),
        Some(Ok(state)) => (
            "False",
            format!("PowerState{state}"),
            format!("Redfish PowerState is {state}"),
        ),
    };
    set_condition(status, vm, state, &reason, &message, now);
}

/// Record the VM as off without a Pod (`spec.running=false`).
pub fn stopped(status: &mut VirtualMachineStatus, vm: &VirtualMachine, now: &str) {
    status.power_state = None;
    set_condition(
        status,
        vm,
        "True",
        "PowerOff",
        "spec.running is false; the VM has no Pod",
        now,
    );
}

/// Replace the `PoweredOff` condition, keeping its transition time while its
/// status is unchanged in the VM's current status.
fn set_condition(
    status: &mut VirtualMachineStatus,
    vm: &VirtualMachine,
    state: &str,
    reason: &str,
    message: &str,
    now: &str,
) {
    let last_transition_time = vm
        .status
        .iter()
        .flat_map(|s| &s.conditions)
        .find(|c| c.type_ == POWERED_OFF && c.status == state)
        .map_or_else(|| now.to_owned(), |c| c.last_transition_time.clone());
    status.conditions.retain(|c| c.type_ != POWERED_OFF);
    status.conditions.push(VirtualMachineCondition {
        type_: POWERED_OFF.into(),
        status: state.into(),
        reason: reason.into(),
        message: message.into(),
        last_transition_time,
        observed_generation: vm.metadata.generation.unwrap_or_default(),
    });
}

/// The current time as Kubernetes serializes it.
pub fn now() -> String {
    k8s_openapi::jiff::Timestamp::now()
        .strftime("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vm(conditions: serde_json::Value) -> VirtualMachine {
        serde_json::from_value(json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
            "metadata":{"name":"test-vm","namespace":"qrab","uid":"vm-uid","generation":4},
            "spec":{"stateVolumeClaim":"state"},
            "status":{"observedGeneration":4,"phase":"Running","ready":true,"reason":"","message":"",
                "conditions":conditions}}))
        .unwrap()
    }

    const EXPORT: &str = "# HELP qrab_redfish_build_info Redfish sidecar build; always 1.\n\
        # TYPE qrab_redfish_build_info gauge\n\
        qrab_redfish_build_info{version=\"0.1.0\"} 1\n\
        # HELP qrab_vm_power_state Redfish PowerState of the VM: 1 for the current state; all 0 when unknown.\n\
        # TYPE qrab_vm_power_state gauge\n\
        qrab_vm_power_state{state=\"On\"} 0\n\
        qrab_vm_power_state{state=\"Off\"} 1\n\
        qrab_vm_power_state{state=\"PoweringOn\"} 0\n\
        qrab_vm_power_state{state=\"PoweringOff\"} 0\n\
        qrab_vm_power_state{state=\"Paused\"} 0\n\
        # TYPE qrab_vm_health gauge\n\
        qrab_vm_health 0\n";

    #[test]
    fn parses_the_one_current_state_of_the_redfish_export() {
        assert_eq!(parse(EXPORT), Some(PowerState::Off));
        let on = EXPORT
            .replace("{state=\"On\"} 0", "{state=\"On\"} 1")
            .replace("{state=\"Off\"} 1", "{state=\"Off\"} 0");
        assert_eq!(parse(&on), Some(PowerState::On));
        // Timestamps and float spellings are accepted.
        assert_eq!(
            parse("qrab_vm_power_state{state=\"Paused\"} 1.0 1700000000000\n"),
            Some(PowerState::Paused)
        );
        for state in ["On", "Off", "PoweringOn", "PoweringOff", "Paused"] {
            let line = format!("qrab_vm_power_state{{state=\"{state}\"}} 1");
            assert_eq!(parse(&line).map(PowerState::as_str), Some(state));
        }
    }

    #[test]
    fn unknown_ambiguous_or_missing_states_are_no_reading() {
        // All 0: Redfish could not reach the manager.
        assert_eq!(
            parse(&EXPORT.replace("{state=\"Off\"} 1", "{state=\"Off\"} 0")),
            None
        );
        assert_eq!(
            parse(&EXPORT.replace("{state=\"On\"} 0", "{state=\"On\"} 1")),
            None
        );
        assert_eq!(parse("qrab_vm_power_state{state=\"Exploded\"} 1\n"), None);
        assert_eq!(parse("qrab_vm_health 0\n"), None);
        assert_eq!(parse(""), None);
        // A longer family name sharing the prefix is not the power family.
        assert_eq!(parse("qrab_vm_power_state_total{state=\"On\"} 1\n"), None);
        assert_eq!(
            parse("qrab_vm_power_state{state=\"On\"} 1\nqrab_vm_power_state{state=\"On\"} 1\n"),
            None
        );
    }

    #[test]
    fn off_and_paused_guests_are_not_running_and_unknown_keeps_the_pod_answer() {
        assert_eq!(ready_phase(None), ("Running", true, "VMRunning"));
        for state in [
            PowerState::On,
            PowerState::PoweringOn,
            PowerState::PoweringOff,
        ] {
            assert_eq!(
                ready_phase(Some(&Ok(state))),
                ("Running", true, "VMRunning")
            );
        }
        assert_eq!(
            ready_phase(Some(&Ok(PowerState::Off))),
            ("PoweredOff", false, "GuestPoweredOff")
        );
        assert_eq!(
            ready_phase(Some(&Ok(PowerState::Paused))),
            ("Paused", false, "GuestPaused")
        );
    }

    #[test]
    fn the_powered_off_condition_follows_the_reading_and_keeps_its_transition_time() {
        let off = json!([{"type":"PoweredOff","status":"True","reason":"GuestPoweredOff","message":"",
            "lastTransitionTime":"2026-11-15T10:00:00Z","observedGeneration":3}]);
        let machine = vm(off);
        let mut status = VirtualMachineStatus::default();
        observe(
            &mut status,
            &machine,
            Some(&Ok(PowerState::Off)),
            "2026-11-15T11:00:00Z",
        );
        assert_eq!(status.power_state.as_deref(), Some("Off"));
        assert_eq!(status.conditions.len(), 1);
        let condition = &status.conditions[0];
        assert_eq!(
            (condition.type_.as_str(), condition.status.as_str()),
            ("PoweredOff", "True")
        );
        assert_eq!(condition.reason, "GuestPoweredOff");
        assert_eq!(condition.last_transition_time, "2026-11-15T10:00:00Z");
        assert_eq!(condition.observed_generation, 4);

        // Powered on: False with a fresh transition time.
        let mut status = VirtualMachineStatus::default();
        observe(
            &mut status,
            &machine,
            Some(&Ok(PowerState::On)),
            "2026-11-15T11:00:00Z",
        );
        assert_eq!(status.power_state.as_deref(), Some("On"));
        let condition = &status.conditions[0];
        assert_eq!(
            (condition.status.as_str(), condition.reason.as_str()),
            ("False", "PowerStateOn")
        );
        assert_eq!(condition.last_transition_time, "2026-11-15T11:00:00Z");

        // A failed read is Unknown and drops the stale powerState.
        let mut status = VirtualMachineStatus {
            power_state: Some("Off".into()),
            ..Default::default()
        };
        observe(
            &mut status,
            &machine,
            Some(&Err("metrics read timed out".into())),
            "2026-11-15T11:00:00Z",
        );
        assert_eq!(status.power_state, None);
        assert_eq!(status.conditions[0].status, "Unknown");
        assert_eq!(status.conditions[0].reason, "PowerStateUnknown");
        assert_eq!(status.conditions[0].message, "metrics read timed out");

        // No Redfish: no read path.
        let mut status = VirtualMachineStatus::default();
        observe(&mut status, &machine, None, "2026-11-15T11:00:00Z");
        assert_eq!(status.conditions[0].reason, "PowerStateUnavailable");

        // Stopped by spec keeps the True transition time it already had.
        let mut status = VirtualMachineStatus::default();
        stopped(&mut status, &machine, "2026-11-15T11:00:00Z");
        assert_eq!(status.conditions[0].reason, "PowerOff");
        assert_eq!(
            status.conditions[0].last_transition_time,
            "2026-11-15T10:00:00Z"
        );
    }

    #[test]
    fn the_condition_is_stable_across_reconciles_so_status_is_not_rewritten() {
        let mut first = VirtualMachineStatus::default();
        observe(
            &mut first,
            &vm(json!([])),
            Some(&Ok(PowerState::Off)),
            "2026-11-15T11:00:00Z",
        );
        let machine: VirtualMachine = {
            let mut machine = vm(json!([]));
            machine.status = Some(first.clone());
            machine
        };
        let mut second = VirtualMachineStatus::default();
        observe(
            &mut second,
            &machine,
            Some(&Ok(PowerState::Off)),
            "2026-11-15T11:00:15Z",
        );
        assert_eq!(first, second);
        // The condition round-trips through the API's JSON shape.
        let value = serde_json::to_value(&second).unwrap();
        assert_eq!(value["powerState"], "Off");
        assert_eq!(value["conditions"][0]["type"], "PoweredOff");
        assert_eq!(
            value["conditions"][0]["lastTransitionTime"],
            "2026-11-15T11:00:00Z"
        );
        // Merge patches clear the fields rather than leaving them stale.
        let cleared = serde_json::to_value(VirtualMachineStatus::default()).unwrap();
        assert!(cleared["powerState"].is_null() && cleared.get("powerState").is_some());
        assert_eq!(cleared["conditions"], json!([]));
    }

    #[test]
    fn metrics_url_uses_the_pod_ip_and_metrics_port() {
        let pod = |ip: serde_json::Value| -> Pod {
            serde_json::from_value(json!({"metadata":{"name":"test-vm"},"status":{"podIP":ip}}))
                .unwrap()
        };
        assert_eq!(
            metrics_url(&pod(json!("10.244.1.5"))).unwrap(),
            "http://10.244.1.5:9181/metrics"
        );
        assert_eq!(
            metrics_url(&pod(json!("fd00::5"))).unwrap(),
            "http://[fd00::5]:9181/metrics"
        );
        assert!(metrics_url(&pod(json!("not-an-ip"))).is_err());
        assert!(metrics_url(&pod(json!(null))).is_err());
        assert!(now().ends_with('Z') && now().len() == "2026-11-15T11:00:00Z".len());
    }

    #[test]
    fn a_ready_pod_reports_running_powered_off_or_paused_by_its_guest() {
        let machine = vm(json!([]));
        let pod: Pod =
            serde_json::from_value(json!({"metadata":{"name":"test-vm","uid":"pod-uid"},
            "spec":{"nodeName":"node-a","containers":[]},"status":{"podIP":"10.244.1.5"}}))
            .unwrap();
        let now = "2026-11-15T11:00:00Z";
        let status = pod_status(&machine, &pod, false, None, now);
        assert_eq!(
            (status.phase.as_str(), status.ready, status.reason.as_str()),
            ("Pending", false, "PodNotReady")
        );
        assert!(status.conditions.is_empty() && status.power_state.is_none());

        let off = pod_status(&machine, &pod, true, Some(&Ok(PowerState::Off)), now);
        assert_eq!(
            (off.phase.as_str(), off.ready, off.reason.as_str()),
            ("PoweredOff", false, "GuestPoweredOff")
        );
        assert_eq!(off.power_state.as_deref(), Some("Off"));
        assert_eq!(off.conditions[0].status, "True");
        assert_eq!(off.pod_name.as_deref(), Some("test-vm"));
        assert_eq!(off.node_name.as_deref(), Some("node-a"));

        let on = pod_status(&machine, &pod, true, Some(&Ok(PowerState::On)), now);
        assert_eq!(
            (on.phase.as_str(), on.ready, on.reason.as_str()),
            ("Running", true, "VMRunning")
        );
        assert_eq!(on.conditions[0].status, "False");
        assert!(on.message.contains("guest are up"));

        // An unknown reading never makes a Ready Pod look failed.
        let unknown = pod_status(
            &machine,
            &pod,
            true,
            Some(&Err("metrics read timed out".into())),
            now,
        );
        assert_eq!((unknown.phase.as_str(), unknown.ready), ("Running", true));
        assert_eq!(unknown.conditions[0].status, "Unknown");
        assert!(unknown.message.contains("unknown"));
        let without = pod_status(&machine, &pod, true, None, now);
        assert_eq!((without.phase.as_str(), without.ready), ("Running", true));
        assert_eq!(without.conditions[0].reason, "PowerStateUnavailable");

        let paused = pod_status(&machine, &pod, true, Some(&Ok(PowerState::Paused)), now);
        assert_eq!((paused.phase.as_str(), paused.ready), ("Paused", false));
        assert_eq!(paused.conditions[0].status, "False");
    }

    #[tokio::test]
    async fn reads_the_state_over_http_and_reports_failures_as_errors() {
        use axum::{http::StatusCode, routing::get, Router};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/metrics", get(|| async { EXPORT }))
            .route("/empty", get(|| async { "qrab_vm_health 0\n" }))
            .route(
                "/busy",
                get(|| async { (StatusCode::SERVICE_UNAVAILABLE, "busy") }),
            )
            .route("/huge", get(|| async { "#".repeat(BODY_LIMIT + 1) }))
            .route(
                "/stalled",
                get(|| async {
                    tokio::time::sleep(READ_TIMEOUT * 2).await;
                    EXPORT
                }),
            );
        tokio::spawn(async move { axum::serve(listener, app).await });
        assert_eq!(
            fetch(&format!("{origin}/metrics")).await,
            Ok(PowerState::Off)
        );
        assert!(fetch(&format!("{origin}/empty"))
            .await
            .unwrap_err()
            .contains("no current"));
        assert!(fetch(&format!("{origin}/busy"))
            .await
            .unwrap_err()
            .contains("503"));
        assert!(fetch(&format!("{origin}/huge"))
            .await
            .unwrap_err()
            .contains("bound"));
        let started = std::time::Instant::now();
        assert!(fetch(&format!("{origin}/stalled")).await.is_err());
        assert!(started.elapsed() < READ_TIMEOUT * 2);
        let pod: Pod = serde_json::from_value(json!({"metadata":{"name":"test-vm"}})).unwrap();
        assert_eq!(read(&pod).await, Err("the VM Pod has no IP".into()));
    }
}
