use dioxus::prelude::*;

use bmc::types::{CpuInfoResponse, KvmInfoResponse, VmPhase};

/// System panel: host threads, KVM status, and guest vCPUs when running.
///
/// Pure props-driven — the parent Dashboard polls `get_dashboard_state()`
/// and slices off these fields, so there's no independent polling here.
#[component]
pub fn CpuInfo(
    current_phase: VmPhase,
    cpu_info: CpuInfoResponse,
    kvm_info: KvmInfoResponse,
) -> Element {
    rsx! {
        div { class: "panel cpu-info",
            div { class: "panel-header",
                h2 { "System" }
            }

            div { class: "status-info",
                div { class: "status-row",
                    span { class: "label", "Host Threads" }
                    span { class: "value", "{cpu_info.host_threads}" }
                }
                div { class: "status-row",
                    span { class: "label", "KVM" }
                    span { class: "value",
                        if kvm_info.enabled { "Enabled" } else if kvm_info.present { "Present" } else { "Unavailable" }
                    }
                }
            }

            if current_phase == VmPhase::Running || current_phase == VmPhase::Paused {
                if !cpu_info.guest_cpus.is_empty() {
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
                            for cpu in cpu_info.guest_cpus.iter() {
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
