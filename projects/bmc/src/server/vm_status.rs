use dioxus::prelude::*;

use crate::types::{
    BmcCapabilities, BmcEvent, CpuInfoResponse, DashboardState, KvmInfoResponse, VmStatusResponse,
};
#[cfg(feature = "server")]
use crate::types::{SevStatusResponse, TpmStatusResponse, VmPhase};

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
    use crate::state::{bmc, VmState};
    use crate::types::GuestCpu;
    use std::path::PathBuf;

    let state = bmc();

    // -- Phase 1: everything that needs the vm lock. One acquisition. --
    let (status, cpu_info, kvm_info, console_available, serial_log_path, sev_status, tpm_status): (
        VmStatusResponse,
        CpuInfoResponse,
        KvmInfoResponse,
        bool,
        Option<PathBuf>,
        Option<SevStatusResponse>,
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
                    // errors as "not available".
                    let sev_status = vm
                        .execute(&qapi::commands::QuerySev {})
                        .await
                        .ok()
                        .map(|info| SevStatusResponse {
                            enabled: info.enabled,
                            sev_type: info.sev_type.map(|t| format!("{t:?}")),
                            state: Some(format!("{:?}", info.state)),
                            api_version: Some(format!("{}.{}", info.api_major, info.api_minor)),
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
                        tpm_status,
                    };
                    *cache_guard = Some(cached.clone());
                    cached
                };
                drop(cache_guard);

                let console_available = vm.vnc_sock().is_some();
                let serial_log_path = vm.serial_log().cloned();

                (status, cached.cpu_info, cached.kvm_info, console_available, serial_log_path, cached.sev_status, cached.tpm_status)
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
            ),
        }
        // vm_guard drops here.
    };

    // -- Phase 2: slower I/O, vm lock released. --
    // Events are NOT collected here — they flow through the `/api/events`
    // SSE endpoint straight into the EventLog component. The dashboard
    // state only carries slices that are polled.
    let serial_tail = read_serial_tail(serial_log_path.as_deref()).await;

    // Capabilities are host-level — no VM lock needed, cheap syscalls.
    let capabilities = crate::state::query_capabilities();

    Ok(DashboardState {
        status,
        cpu_info,
        kvm_info,
        console_available,
        serial_tail,
        sev_status,
        tpm_status,
        capabilities,
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

/// Query host capabilities (SEV, TPM, OVMF availability).
#[server]
pub async fn get_capabilities() -> Result<BmcCapabilities, ServerFnError> {
    Ok(crate::state::query_capabilities())
}
