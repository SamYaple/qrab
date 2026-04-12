use dioxus::prelude::*;

use bmc::server::vm_status::{get_cpu_info, get_kvm_info};
use bmc::types::VmPhase;

use super::util::sleep_ms;

#[component]
pub fn CpuInfo(current_phase: VmPhase, refresh_tick: u64) -> Element {
    let mut poll_tick = use_signal(|| 0u64);

    // Periodic ticker (every 5s). Task is scope-bound and auto-cancels on unmount.
    use_future(move || async move {
        loop {
            sleep_ms(5000).await;
            poll_tick += 1;
        }
    });

    let cpu_resource = use_resource(use_reactive!(|refresh_tick| async move {
        let _ = poll_tick();
        let _ = refresh_tick;
        get_cpu_info().await.ok()
    }));
    let kvm_resource = use_resource(use_reactive!(|refresh_tick| async move {
        let _ = poll_tick();
        let _ = refresh_tick;
        get_kvm_info().await.ok()
    }));

    let cpu_info = cpu_resource.read().clone().flatten();
    let kvm_info = kvm_resource.read().clone().flatten();

    rsx! {
        div { class: "panel cpu-info",
            div { class: "panel-header",
                h2 { "System" }
            }

            div { class: "status-info",
                if let Some(ref info) = cpu_info {
                    div { class: "status-row",
                        span { class: "label", "Host Threads" }
                        span { class: "value", "{info.host_threads}" }
                    }
                }

                if let Some(ref kvm) = kvm_info {
                    div { class: "status-row",
                        span { class: "label", "KVM" }
                        span { class: "value",
                            if kvm.enabled { "Enabled" } else if kvm.present { "Present" } else { "Unavailable" }
                        }
                    }
                }
            }

            if current_phase == VmPhase::Running || current_phase == VmPhase::Paused {
                if let Some(ref info) = cpu_info {
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
