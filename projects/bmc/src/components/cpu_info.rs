use dioxus::prelude::*;

use bmc::server::vm_status::{get_cpu_info, get_kvm_info};
use bmc::types::{CpuInfoResponse, KvmInfoResponse, VmPhase};

use super::util::sleep_ms;

#[component]
pub fn CpuInfo(current_phase: VmPhase, refresh_tick: u64) -> Element {
    let mut cpu_info = use_signal(|| None::<CpuInfoResponse>);
    let mut kvm_info = use_signal(|| None::<KvmInfoResponse>);

    // Poll every 5s
    use_future(move || async move {
        loop {
            if let Ok(info) = get_cpu_info().await {
                cpu_info.set(Some(info));
            }
            if let Ok(info) = get_kvm_info().await {
                kvm_info.set(Some(info));
            }
            sleep_ms(5000).await;
        }
    });

    // Refresh on tick
    use_effect(move || {
        let _tick = refresh_tick;
        spawn(async move {
            if let Ok(info) = get_cpu_info().await {
                cpu_info.set(Some(info));
            }
            if let Ok(info) = get_kvm_info().await {
                kvm_info.set(Some(info));
            }
        });
    });

    rsx! {
        div { class: "panel cpu-info",
            div { class: "panel-header",
                h2 { "System" }
            }

            div { class: "status-info",
                if let Some(ref info) = *cpu_info.read() {
                    div { class: "status-row",
                        span { class: "label", "Host Threads" }
                        span { class: "value", "{info.host_threads}" }
                    }
                }

                if let Some(ref kvm) = *kvm_info.read() {
                    div { class: "status-row",
                        span { class: "label", "KVM" }
                        span { class: "value",
                            if kvm.enabled { "Enabled" } else if kvm.present { "Present" } else { "Unavailable" }
                        }
                    }
                }
            }

            if current_phase == VmPhase::Running || current_phase == VmPhase::Paused {
                if let Some(ref info) = *cpu_info.read() {
                    if !info.guest_cpus.is_empty() {
                        h3 { "Guest vCPUs" }
                        table { class: "cpu-table",
                            thead {
                                tr {
                                    th { "#" }
                                    th { "TID" }
                                    th { "QOM Path" }
                                }
                            }
                            tbody {
                                for cpu in info.guest_cpus.iter() {
                                    tr {
                                        td { "{cpu.index}" }
                                        td { class: "mono", "{cpu.thread_id}" }
                                        td { class: "mono", "{cpu.qom_path}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
