use dioxus::prelude::*;

use crate::types::{
    AttestationQrResponse, AttestationReportResponse, AttestationState, BmcEvent, CpuInfoResponse,
    DashboardState, KvmInfoResponse, SevCapabilitiesResponse, VmStatusResponse,
};
#[cfg(feature = "server")]
use crate::types::{AttestationStatus, SevStatusResponse, TpmStatusResponse, VmPhase};

/// Cached host CPU count. `std::thread::available_parallelism()` is a
/// syscall (reads /proc/self/status on Linux) and the value never changes
/// for a process — cache it once.
#[cfg(feature = "server")]
fn host_threads() -> usize {
    use std::sync::OnceLock;
    static CACHED: OnceLock<usize> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    })
}

#[server]
pub async fn get_status() -> Result<VmStatusResponse, ServerFnError> {
    use crate::state::{bmc, VmState};

    let state = bmc();
    let vm_guard = state.vm.lock().await;

    match &*vm_guard {
        VmState::Off => Ok(VmStatusResponse {
            phase: VmPhase::Off,
            running: None,
            status: None,
            qemu_version: None,
        }),
        VmState::Running(vm) => {
            let status = vm
                .execute(&qapi::commands::QueryStatus {})
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            let phase = if status.running {
                VmPhase::Running
            } else {
                VmPhase::Paused
            };
            Ok(VmStatusResponse {
                phase,
                running: Some(status.running),
                status: Some(format!("{:?}", status.status)),
                qemu_version: None,
            })
        }
        VmState::ShuttingDown => Ok(VmStatusResponse {
            phase: VmPhase::ShuttingDown,
            running: None,
            status: None,
            qemu_version: None,
        }),
    }
}

#[server]
pub async fn get_cpu_info() -> Result<CpuInfoResponse, ServerFnError> {
    use crate::state::{bmc, VmState};
    use crate::types::GuestCpu;

    let state = bmc();
    let vm_guard = state.vm.lock().await;

    let guest_cpus = match &*vm_guard {
        VmState::Running(vm) => {
            let cpus: Vec<qapi::CpuInfoFast> = vm
                .execute(&qapi::commands::QueryCpusFast {})
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            cpus.iter()
                .map(|c| GuestCpu {
                    index: c.cpu_index,
                    thread_id: c.thread_id,
                    qom_path: c.qom_path.clone(),
                })
                .collect()
        }
        _ => Vec::new(),
    };

    let host_threads = host_threads();

    Ok(CpuInfoResponse {
        guest_cpus,
        host_threads,
    })
}

#[server]
pub async fn get_kvm_info() -> Result<KvmInfoResponse, ServerFnError> {
    use crate::state::{bmc, VmState};

    let state = bmc();
    let vm_guard = state.vm.lock().await;

    match &*vm_guard {
        VmState::Running(vm) => {
            let kvm: qapi::KvmInfo = vm
                .execute(&qapi::commands::QueryKvm {})
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            Ok(KvmInfoResponse {
                enabled: kvm.enabled,
                present: kvm.present,
            })
        }
        _ => Ok(KvmInfoResponse {
            enabled: false,
            present: false,
        }),
    }
}

#[server]
pub async fn get_serial_output() -> Result<String, ServerFnError> {
    use crate::state::bmc;
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    /// Tail the last N bytes of the log on each poll. Bounds memory for
    /// long-running VMs that produce gigabytes of serial output.
    const TAIL_BYTES: u64 = 64 * 1024;

    let state = bmc();
    let path = match state.serial_log().await {
        Some(p) => p,
        None => return Ok(String::new()),
    };

    let mut file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(_) => return Ok(String::new()),
    };
    let len = match file.metadata().await {
        Ok(m) => m.len(),
        Err(_) => return Ok(String::new()),
    };
    let start = len.saturating_sub(TAIL_BYTES);
    if start > 0 {
        let _ = file.seek(std::io::SeekFrom::Start(start)).await;
    }
    let mut buf = String::new();
    let _ = file.read_to_string(&mut buf).await;

    // If we truncated mid-line, drop the leading partial line so output
    // doesn't start in the middle of a word.
    if start > 0 {
        if let Some(nl) = buf.find('\n') {
            buf = buf[nl + 1..].to_string();
        }
    }
    Ok(buf)
}

#[server]
pub async fn get_console_available() -> Result<bool, ServerFnError> {
    use crate::state::{bmc, VmState};

    let state = bmc();
    let vm_guard = state.vm.lock().await;
    // Single lock acquisition: VNC socket is read from the VM handle while
    // holding vm_guard, so the (running, has_sock) tuple is consistent.
    let available = matches!(&*vm_guard, VmState::Running(vm) if vm.vnc_sock().is_some());
    Ok(available)
}

#[server]
pub async fn get_event_history() -> Result<Vec<BmcEvent>, ServerFnError> {
    use crate::state::bmc;

    let state = bmc();
    let history = state.event_history.lock().await;
    Ok(history
        .iter()
        .map(|e| BmcEvent {
            event: e.event.clone(),
            timestamp_secs: e.timestamp.seconds,
            timestamp_usecs: e.timestamp.microseconds,
            data: e.data.clone(),
        })
        .collect())
}

/// Combined dashboard fetch.
///
/// Acquires `state.vm.lock()` ONCE and extracts every slice of VM state the
/// dashboard needs (status, cpu info, kvm info, vnc availability, serial log
/// path) atomically. The lock is dropped before the (potentially slow) serial
/// log file I/O and the separate `event_history` lock. The result is a
/// self-consistent snapshot of the running VM — there's no observable window
/// where the UI sees "running but no CPUs" or similar races between calls.
///
/// Holds `vm_guard` across multiple sequential QMP awaits. This is acceptable
/// because QMP runs over a local Unix socket (sub-millisecond per command);
/// if QMP ever becomes remote, the awaits should move out of the lock or
/// run concurrently via `tokio::join!`.
///
/// CPU and KVM info are cached in `state.cached_static` for the lifetime of
/// each Running session — the QueryCpusFast / QueryKvm round-trips happen
/// once per power-on, not once per poll.
#[server]
pub async fn get_dashboard_state() -> Result<DashboardState, ServerFnError> {
    let state = crate::state::bmc();
    Ok(build_dashboard(&state).await)
}

/// Core dashboard builder used by both `get_dashboard_state` (on-demand
/// client fetch) and `BmcState::push_dashboard` (server-initiated push).
///
/// Errors from individual QMP probes collapse to `Default::default()`
/// fields — the push path must not fail silently, and the client already
/// treats missing slices as "not available".
#[cfg(feature = "server")]
pub async fn build_dashboard(state: &std::sync::Arc<crate::state::BmcState>) -> DashboardState {
    use crate::state::VmState;
    use crate::types::GuestCpu;
    use std::path::PathBuf;

    // Inner fallible closure keeps the `?` style. Any QMP probe failure
    // collapses the whole snapshot to the `Off` default — the UI already
    // renders that case sensibly while the client resyncs.
    let inner: Result<DashboardState, ServerFnError> = async {
    // -- Phase 1: everything that needs the vm lock. One acquisition. --
    let (
        status,
        cpu_info,
        kvm_info,
        console_available,
        serial_log_path,
        sev_status,
        sev_capabilities,
        tpm_status,
    ): (
        VmStatusResponse,
        CpuInfoResponse,
        KvmInfoResponse,
        bool,
        Option<PathBuf>,
        Option<SevStatusResponse>,
        Option<SevCapabilitiesResponse>,
        Option<TpmStatusResponse>,
    ) = {
        let vm_guard = state.vm.lock().await;

        // host_threads doesn't depend on VM state but is part of CpuInfoResponse.
        let host_threads = host_threads();

        match &*vm_guard {
            VmState::Off => (
                VmStatusResponse {
                    phase: VmPhase::Off,
                    running: None,
                    status: None,
                    qemu_version: None,
                },
                CpuInfoResponse {
                    guest_cpus: Vec::new(),
                    host_threads,
                },
                KvmInfoResponse {
                    enabled: false,
                    present: false,
                },
                false,
                None,
                None,
                None,
                None,
            ),
            VmState::Running(vm) => {
                // QueryStatus is the only QMP command we issue every poll;
                // it's the cheapest and tells us whether the guest is paused
                // or running.
                let status_reply = vm
                    .execute(&qapi::commands::QueryStatus {})
                    .await
                    .map_err(|e| ServerFnError::new(e.to_string()))?;
                let phase = if status_reply.running {
                    VmPhase::Running
                } else {
                    VmPhase::Paused
                };
                let status = VmStatusResponse {
                    phase,
                    running: Some(status_reply.running),
                    status: Some(format!("{:?}", status_reply.status)),
                    qemu_version: None,
                };

                // CPU/KVM/security info are static for the lifetime of the
                // QEMU process, so query them once per Running session and
                // reuse the cached value. cached_static is cleared on every
                // transition out of Running.
                let mut cache_guard = state.cached_static.lock().await;
                let cached = if let Some(cached) = cache_guard.as_ref() {
                    cached.clone()
                } else {
                    let cpus: Vec<qapi::CpuInfoFast> = vm
                        .execute(&qapi::commands::QueryCpusFast {})
                        .await
                        .map_err(|e| ServerFnError::new(e.to_string()))?;
                    let cpu_info = CpuInfoResponse {
                        guest_cpus: cpus
                            .iter()
                            .map(|c| GuestCpu {
                                index: c.cpu_index,
                                thread_id: c.thread_id,
                                qom_path: c.qom_path.clone(),
                            })
                            .collect(),
                        host_threads,
                    };
                    let kvm: qapi::KvmInfo = vm
                        .execute(&qapi::commands::QueryKvm {})
                        .await
                        .map_err(|e| ServerFnError::new(e.to_string()))?;
                    let kvm_info = KvmInfoResponse {
                        enabled: kvm.enabled,
                        present: kvm.present,
                    };

                    // SEV: query-sev fails on VMs without SEV — treat
                    // errors as "not available". Pull every branch-specific
                    // field we can surface (policy / handle for legacy SEV,
                    // snp_policy for SEV-SNP) so the config panel has the
                    // full picture in one place.
                    let sev_result = vm.execute(&qapi::commands::QuerySev {}).await;
                    if let Err(ref e) = sev_result {
                        tracing::debug!("query-sev failed: {e}");
                    }
                    let sev_status = sev_result.ok().map(|info| {
                        let (legacy_policy, snp_policy) = match info.u {
                            Some(qapi::SevInfoBranch::Sev(g)) => (
                                Some(format!("0x{:x}", g.policy)),
                                None,
                            ),
                            Some(qapi::SevInfoBranch::SevSnp(g)) => (
                                None,
                                Some(format!("0x{:x}", g.snp_policy)),
                            ),
                            None => (
                                info.policy.map(|p| format!("0x{p:x}")),
                                None,
                            ),
                        };
                        SevStatusResponse {
                            enabled: info.enabled,
                            sev_type: info.sev_type.map(|t| format!("{t:?}")),
                            state: Some(format!("{:?}", info.state)),
                            api_version: Some(format!("{}.{}", info.api_major, info.api_minor)),
                            api_major: Some(info.api_major),
                            api_minor: Some(info.api_minor),
                            build_id: Some(info.build_id),
                            handle: info.handle,
                            policy: legacy_policy,
                            snp_policy,
                        }
                    });

                    // query-sev-capabilities: static per host/platform.
                    // Failures mean either no SEV on the host or the QEMU
                    // target doesn't support it — treat as absent.
                    let sev_cap_result =
                        vm.execute(&qapi::commands::QuerySevCapabilities {}).await;
                    if let Err(ref e) = sev_cap_result {
                        tracing::debug!("query-sev-capabilities failed: {e}");
                    }
                    let sev_capabilities = sev_cap_result.ok().map(|c| SevCapabilitiesResponse {
                        pdh: c.pdh,
                        cert_chain: c.cert_chain,
                        cbitpos: c.cbitpos,
                        reduced_phys_bits: c.reduced_phys_bits,
                    });

                    // TPM: query-tpm returns empty vec when no TPM
                    // configured. Errors mean TPM support not compiled in.
                    let tpm_status = vm
                        .execute(&qapi::commands::QueryTpm {})
                        .await
                        .ok()
                        .and_then(|tpms| {
                            tpms.into_iter().next().map(|tpm| TpmStatusResponse {
                                present: true,
                                model: Some(format!("{:?}", tpm.model)),
                                backend_type: Some(format!("{:?}", tpm.options)),
                            })
                        });

                    let cached = crate::state::CachedStaticInfo {
                        cpu_info,
                        kvm_info,
                        sev_status,
                        sev_capabilities,
                        tpm_status,
                    };
                    *cache_guard = Some(cached.clone());
                    cached
                };
                drop(cache_guard);

                let console_available = vm.vnc_sock().is_some();
                let serial_log_path = vm.serial_log().cloned();

                (
                    status,
                    cached.cpu_info,
                    cached.kvm_info,
                    console_available,
                    serial_log_path,
                    cached.sev_status,
                    cached.sev_capabilities,
                    cached.tpm_status,
                )
            }
            VmState::ShuttingDown => (
                VmStatusResponse {
                    phase: VmPhase::ShuttingDown,
                    running: None,
                    status: None,
                    qemu_version: None,
                },
                CpuInfoResponse {
                    guest_cpus: Vec::new(),
                    host_threads,
                },
                KvmInfoResponse {
                    enabled: false,
                    present: false,
                },
                false,
                None,
                None,
                None,
                None,
            ),
        }
        // vm_guard drops here.
    };

    // -- Phase 2: slower I/O, vm lock released. --
    // Events are NOT collected here — they flow through the `/api/events`
    // SSE endpoint straight into the EventLog component. The dashboard
    // state only carries slices that are polled.
    let serial_tail = read_serial_tail(serial_log_path.as_deref()).await;

    // Attestation state — separate lock, not under vm_guard.
    let attestation = {
        let att = state.attestation.lock().await;
        if att.status != AttestationStatus::NotAvailable {
            Some(att.clone())
        } else {
            None
        }
    };

    Ok(DashboardState {
        status,
        cpu_info,
        kvm_info,
        console_available,
        serial_tail,
        sev_status,
        sev_capabilities,
        tpm_status,
        attestation,
    })
    }
    .await;
    inner.unwrap_or_else(|e| {
        tracing::warn!("build_dashboard failed: {e}");
        DashboardState::default()
    })
}

/// Read the tail of a serial log file. Mirrors `get_serial_output` but takes
/// an explicit path so the caller can capture it under the vm lock and then
/// do the (potentially slow) I/O without holding it.
#[cfg(feature = "server")]
async fn read_serial_tail(path: Option<&std::path::Path>) -> String {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    /// Tail the last N bytes of the log on each poll. Bounds memory for
    /// long-running VMs that produce gigabytes of serial output.
    const TAIL_BYTES: u64 = 64 * 1024;

    let Some(path) = path else {
        return String::new();
    };

    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let len = match file.metadata().await {
        Ok(m) => m.len(),
        Err(_) => return String::new(),
    };
    let start = len.saturating_sub(TAIL_BYTES);
    if start > 0 {
        let _ = file.seek(std::io::SeekFrom::Start(start)).await;
    }
    let mut buf = String::new();
    let _ = file.read_to_string(&mut buf).await;

    // If we truncated mid-line, drop the leading partial line so output
    // doesn't start in the middle of a word.
    if start > 0 {
        if let Some(nl) = buf.find('\n') {
            buf = buf[nl + 1..].to_string();
        }
    }
    buf
}

/// Get current attestation state.
#[server]
pub async fn get_attestation_state() -> Result<AttestationState, ServerFnError> {
    use crate::state::bmc;

    let state = bmc();
    let att = state.attestation.lock().await;
    Ok(att.clone())
}

/// Request a fresh attestation report with the given nonce.
///
/// Calls `query_sev_attestation_report(mnonce)` on the running VM, stores
/// the result in `BmcState.attestation`, and returns it.
#[server]
pub async fn request_attestation_report(
    mnonce: String,
) -> Result<AttestationReportResponse, ServerFnError> {
    use crate::state::{bmc, VmState};

    let state = bmc();
    let vm_guard = state.vm.lock().await;

    match &*vm_guard {
        VmState::Running(vm) => {
            let report = vm
                .query_sev_attestation_report(mnonce.clone())
                .await
                .map_err(|e| ServerFnError::new(format!("attestation report failed: {e}")))?;

            let timestamp = chrono_now_iso8601();
            let response = AttestationReportResponse {
                data: report.data,
                mnonce,
                timestamp,
            };

            // Store in attestation state
            drop(vm_guard);
            let mut att = state.attestation.lock().await;
            att.last_attestation_report = Some(response.clone());
            att.status = AttestationStatus::Verified;

            Ok(response)
        }
        _ => Err(ServerFnError::new("VM is not running")),
    }
}

/// Get SEV capabilities from cached attestation state.
#[server]
pub async fn get_sev_capabilities() -> Result<Option<SevCapabilitiesResponse>, ServerFnError> {
    use crate::state::bmc;

    let state = bmc();
    let att = state.attestation.lock().await;
    Ok(att.sev_capabilities.clone())
}

/// ISO 8601 timestamp without pulling in chrono crate.
#[cfg(feature = "server")]
fn chrono_now_iso8601() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    // Simple UTC timestamp: seconds since epoch as string.
    // Full ISO 8601 would need chrono — keep deps minimal.
    format!("{}Z", now.as_secs())
}

/// Build attestation JSON payload from current state.
#[cfg(feature = "server")]
fn build_attestation_payload(att: &AttestationState) -> String {
    serde_json::json!({
        "launch_measure": att.launch_measure,
        "status": format!("{:?}", att.status),
        "report": att.last_attestation_report.as_ref().map(|r| {
            serde_json::json!({
                "data": r.data,
                "mnonce": r.mnonce,
                "timestamp": r.timestamp,
            })
        }),
        "capabilities": att.sev_capabilities.as_ref().map(|c| {
            serde_json::json!({
                "pdh": c.pdh,
                "cbitpos": c.cbitpos,
                "reduced_phys_bits": c.reduced_phys_bits,
            })
        }),
    })
    .to_string()
}

/// Generate QR code from current attestation data.
///
/// `format` must be `"ascii"` or `"svg"`.
#[server]
pub async fn generate_attestation_qr(
    format: String,
) -> Result<AttestationQrResponse, ServerFnError> {
    use crate::state::bmc;

    let state = bmc();
    let att = state.attestation.lock().await;

    if att.launch_measure.is_none() && att.last_attestation_report.is_none() {
        return Err(ServerFnError::new(
            "No attestation data available — request a report first",
        ));
    }

    let payload = build_attestation_payload(&att);
    drop(att);

    let qr_data = match format.as_str() {
        "ascii" => crate::server::qrcode::generate_qr_ascii(&payload)
            .map_err(|e| ServerFnError::new(e))?,
        "svg" => crate::server::qrcode::generate_qr_svg(&payload)
            .map_err(|e| ServerFnError::new(e))?,
        _ => return Err(ServerFnError::new("format must be 'ascii' or 'svg'")),
    };

    Ok(AttestationQrResponse {
        qr_data,
        format,
        payload,
        timestamp: chrono_now_iso8601(),
    })
}

/// Append attestation QR code (ASCII) to serial log file.
///
/// Serial is configured as `file:` chardev so we append directly to the
/// log file. The BMC serial output panel reads the same file — the QR
/// will appear in the serial console view on next poll.
#[server]
pub async fn send_attestation_to_serial() -> Result<(), ServerFnError> {
    use crate::state::{bmc, VmState};
    use tokio::io::AsyncWriteExt;

    let state = bmc();

    // Get serial log path under vm lock
    let serial_path = {
        let vm_guard = state.vm.lock().await;
        match &*vm_guard {
            VmState::Running(vm) => vm.serial_log().cloned(),
            _ => None,
        }
    };

    let serial_path = serial_path
        .ok_or_else(|| ServerFnError::new("No serial log available — VM not running or serial not enabled"))?;

    // Build attestation payload + ASCII QR
    let att = state.attestation.lock().await;
    if att.launch_measure.is_none() && att.last_attestation_report.is_none() {
        return Err(ServerFnError::new("No attestation data available"));
    }
    let payload = build_attestation_payload(&att);
    let launch_measure = att.launch_measure.clone().unwrap_or_default();
    let mnonce = att
        .last_attestation_report
        .as_ref()
        .map(|r| r.mnonce.clone())
        .unwrap_or_default();
    drop(att);

    let qr_ascii = crate::server::qrcode::generate_qr_ascii(&payload)
        .map_err(|e| ServerFnError::new(e))?;

    // Build serial output block
    let output = format!(
        "\n\
         === SEV-SNP ATTESTATION ===\n\
         Launch Measure: {launch_measure}\n\
         Nonce: {mnonce}\n\
         \n\
         {qr_ascii}\
         ===========================\n"
    );

    // Append to serial log file
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(&serial_path)
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to open serial log: {e}")))?;

    file.write_all(output.as_bytes())
        .await
        .map_err(|e| ServerFnError::new(format!("Failed to write to serial log: {e}")))?;

    Ok(())
}
