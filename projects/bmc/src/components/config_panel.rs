use dioxus::prelude::*;

use bmc::server::vm_actions;
use bmc::types::{BmcCapabilities, BmcConfig, VmPhase};

/// VM configuration panel. All settings are only editable when VM is off.
#[component]
pub fn ConfigPanel(
    current_phase: VmPhase,
    capabilities: BmcCapabilities,
) -> Element {
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

    let kvm_enabled = cfg.kvm;
    let sev_enabled = cfg.sev_snp.is_some();
    let tpm_enabled = cfg.tpm.is_some();
    let cpus = cfg.cpus.unwrap_or(1);
    let memory = cfg.memory.clone();

    rsx! {
        div { class: "panel config-panel",
            div { class: "panel-header",
                h2 { "Configuration" }
            }

            div { class: "status-info",
                h3 { "Acceleration" }

                div { class: "config-toggle",
                    label {
                        input {
                            r#type: "checkbox",
                            checked: kvm_enabled,
                            disabled: !is_off,
                            onchange: move |evt: Event<FormData>| {
                                let checked = evt.checked();
                                spawn(async move {
                                    if let Ok(val) = vm_actions::set_kvm(checked).await {
                                        if let Some(ref mut cfg) = *config.write() {
                                            cfg.kvm = val;
                                        }
                                    }
                                });
                            },
                        }
                        " KVM"
                        if !capabilities.kvm_available {
                            span { class: "config-hint", " (not available)" }
                        }
                    }
                }
            }

            div { class: "status-info",
                h3 { "Security" }

                div { class: "config-toggle",
                    label {
                        input {
                            r#type: "checkbox",
                            checked: sev_enabled,
                            disabled: !is_off || !capabilities.sev_snp_available,
                            onchange: move |evt: Event<FormData>| {
                                let checked = evt.checked();
                                spawn(async move {
                                    if vm_actions::set_sev_snp(checked).await.is_ok() {
                                        if let Ok(cfg) = vm_actions::get_config().await {
                                            config.set(Some(cfg));
                                        }
                                    }
                                });
                            },
                        }
                        " SEV-SNP"
                        if !capabilities.sev_snp_available {
                            span { class: "config-hint", " (not available)" }
                        }
                    }
                }

                div { class: "config-toggle",
                    label {
                        input {
                            r#type: "checkbox",
                            checked: tpm_enabled,
                            disabled: !is_off || !capabilities.swtpm_available,
                            onchange: move |evt: Event<FormData>| {
                                let checked = evt.checked();
                                spawn(async move {
                                    if vm_actions::set_tpm(checked).await.is_ok() {
                                        if let Ok(cfg) = vm_actions::get_config().await {
                                            config.set(Some(cfg));
                                        }
                                    }
                                });
                            },
                        }
                        " TPM (swtpm)"
                        if !capabilities.swtpm_available {
                            span { class: "config-hint", " (not available)" }
                        }
                    }
                }
            }

            div { class: "status-info",
                h3 { "Resources" }

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

                div { class: "config-field",
                    label { "Memory" }
                    input {
                        r#type: "text",
                        value: "{memory}",
                        disabled: !is_off,
                        placeholder: "e.g. 256M, 1G, 4096M",
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
            }
        }
    }
}
