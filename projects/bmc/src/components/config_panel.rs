use dioxus::prelude::*;

use bmc::server::vm_actions;
use bmc::types::{BmcCapabilities, BmcConfig, VmPhase};

/// Collapsible section wrapper.
#[component]
fn ConfigSection(
    title: &'static str,
    #[props(default = false)] initially_collapsed: bool,
    children: Element,
) -> Element {
    let mut collapsed = use_signal(move || initially_collapsed);

    rsx! {
        div { class: "config-section",
            div {
                class: "config-section-header",
                onclick: move |_| collapsed.set(!collapsed()),
                h3 { "{title}" }
                span { class: if collapsed() { "collapse-icon collapsed" } else { "collapse-icon" },
                    "\u{25B6}" // right-pointing triangle
                }
            }
            if !collapsed() {
                div { class: "config-section-body",
                    {children}
                }
            }
        }
    }
}

/// VM configuration panel with full SEV-SNP config editing.
/// All settings are only editable when VM is off.
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

    // SEV-SNP sub-fields
    let sev_policy = cfg.sev_snp.as_ref()
        .and_then(|s| s.policy)
        .map(|p| format!("0x{p:x}"))
        .unwrap_or_default();
    let sev_reduced_phys = cfg.sev_snp.as_ref()
        .map(|s| s.reduced_phys_bits)
        .unwrap_or(1);
    let sev_id_block = cfg.sev_snp.as_ref()
        .and_then(|s| s.id_block.clone())
        .unwrap_or_default();
    let sev_host_data = cfg.sev_snp.as_ref()
        .and_then(|s| s.host_data.clone())
        .unwrap_or_default();

    rsx! {
        div { class: "panel config-panel",
            div { class: "panel-header",
                h2 { "Configuration" }
            }

            // Security section
            ConfigSection { title: "Security",
                div { class: "config-group",
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

                    // SEV-SNP detail fields (visible when enabled)
                    if sev_enabled {
                        div { class: "config-sub-fields",
                            div { class: "config-field",
                                label { "Policy (hex)" }
                                input {
                                    r#type: "text",
                                    class: "input-mono",
                                    value: "{sev_policy}",
                                    disabled: !is_off,
                                    placeholder: "0x30000",
                                }
                            }
                            div { class: "config-field",
                                label { "Reduced Phys Bits" }
                                input {
                                    r#type: "number",
                                    value: "{sev_reduced_phys}",
                                    min: "1",
                                    max: "6",
                                    disabled: !is_off,
                                }
                            }
                            div { class: "config-field config-field-wide",
                                label { "ID Block (base64)" }
                                textarea {
                                    class: "input-mono",
                                    value: "{sev_id_block}",
                                    disabled: !is_off,
                                    rows: "3",
                                    placeholder: "Base64-encoded ID block for attestation",
                                }
                            }
                            div { class: "config-field config-field-wide",
                                label { "Host Data (hex, read-only)" }
                                input {
                                    r#type: "text",
                                    class: "input-mono",
                                    value: "{sev_host_data}",
                                    disabled: true,
                                    placeholder: "Set by host firmware",
                                }
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
            }

            // Hardware section
            ConfigSection { title: "Hardware",
                div { class: "config-group",
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
}
