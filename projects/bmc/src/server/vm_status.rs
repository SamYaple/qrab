use dioxus::prelude::*;

use crate::types::{BmcEvent, CpuInfoResponse, KvmInfoResponse, VmStatusResponse};
#[cfg(feature = "server")]
use crate::types::VmPhase;

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
        VmState::Preconfig(pvm) => {
            let status = pvm
                .execute(&qapi::commands::QueryStatus {})
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            Ok(VmStatusResponse {
                phase: VmPhase::Preconfig,
                running: Some(status.running),
                status: Some(format!("{:?}", status.status)),
                qemu_version: None,
            })
        }
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

    let host_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

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
