use dioxus::prelude::*;

use bmc::server::vm_actions;
use bmc::types::{BmcConfig, VmPhase};

/// VM configuration panel exposing only the two user-facing knobs:
/// `memory` and `cpus`. All other knobs (arch, machine, SEV-SNP, TPM,
/// KVM, display, disks, secure boot, firmware) are hardcoded in
/// `state::power_on`. Inputs are disabled while the VM is not Off.
#[component]
pub fn ConfigPanel(current_phase: VmPhase) -> Element {
    let mut config = use_signal(|| Option::<BmcConfig>::None);
    let is_off = current_phase == VmPhase::Off;

    // Fetch config on mount.
    use_effect(move || {
        spawn(async move {
            if let Ok(cfg) = vm_actions::get_config().await {
                config.set(Some(cfg));
            }
        });
    });

    let Some(cfg) = config() else {
        return rsx! {
            div { class: "panel config-panel",
                div { class: "panel-header", h2 { "Configuration" } }
                div { class: "status-info", "Loading..." }
            }
        };
    };

    let cpus = cfg.cpus.unwrap_or(1);
    let memory = cfg.memory.clone();

    rsx! {
        div { class: "panel config-panel",
            div { class: "panel-header",
                h2 { "Configuration" }
            }
            div { class: "config-group",
                div { class: "config-field",
                    label { "Memory" }
                    input {
                        r#type: "text",
                        value: "{memory}",
                        disabled: !is_off,
                        placeholder: "1G",
                        onchange: move |evt: Event<FormData>| {
                            let val = evt.value().clone();
                            spawn(async move {
                                if vm_actions::set_memory(val.clone()).await.is_ok() {
                                    if let Some(ref mut cfg) = *config.write() {
                                        cfg.memory = val;
                                    }
                                }
                            });
                        },
                    }
                }
                div { class: "config-field",
                    label { "vCPUs" }
                    input {
                        r#type: "number",
                        value: "{cpus}",
                        min: "1",
                        max: "256",
                        disabled: !is_off,
                        onchange: move |evt: Event<FormData>| {
                            if let Ok(val) = evt.value().parse::<u32>() {
                                spawn(async move {
                                    if vm_actions::set_cpus(val).await.is_ok() {
                                        if let Some(ref mut cfg) = *config.write() {
                                            cfg.cpus = if val <= 1 { None } else { Some(val) };
                                        }
                                    }
                                });
                            }
                        },
                    }
                }
            }
        }
    }
}
