use dioxus::prelude::*;

use bmc::server::vm_status::{get_cpu_info, get_kvm_info};
use bmc::types::VmPhase;

#[component]
pub fn CpuInfo(current_phase: VmPhase) -> Element {
    let cpu_info = use_resource(get_cpu_info);
    let kvm_info = use_resource(get_kvm_info);

    rsx! {
        div { class: "cpu-info",
            h2 { "CPU Information" }

            // Host info
            match cpu_info.read().as_ref() {
                Some(Ok(info)) => rsx! {
                    div { class: "status-row",
                        span { class: "label", "Host Threads:" }
                        span { class: "value", "{info.host_threads}" }
                    }
                },
                _ => rsx! {},
            }

            // KVM info
            match kvm_info.read().as_ref() {
                Some(Ok(kvm)) => rsx! {
                    div { class: "status-row",
                        span { class: "label", "KVM:" }
                        span { class: "value",
                            if kvm.enabled { "Enabled" } else if kvm.present { "Present (disabled)" } else { "Not available" }
                        }
                    }
                },
                _ => rsx! {},
            }

            // Guest vCPUs table
            if current_phase == VmPhase::Running || current_phase == VmPhase::Paused {
                match cpu_info.read().as_ref() {
                    Some(Ok(info)) if !info.guest_cpus.is_empty() => rsx! {
                        h3 { "Guest vCPUs" }
                        table { class: "cpu-table",
                            thead {
                                tr {
                                    th { "Index" }
                                    th { "Thread ID" }
                                    th { "QOM Path" }
                                }
                            }
                            tbody {
                                for cpu in info.guest_cpus.iter() {
                                    tr {
                                        td { "{cpu.index}" }
                                        td { "{cpu.thread_id}" }
                                        td { "{cpu.qom_path}" }
                                    }
                                }
                            }
                        }
                    },
                    Some(Ok(_)) => rsx! {
                        p { class: "muted", "No vCPUs reported." }
                    },
                    Some(Err(e)) => rsx! {
                        p { class: "error", "Error: {e}" }
                    },
                    None => rsx! {
                        p { "Loading..." }
                    },
                }
            }
        }
    }
}
