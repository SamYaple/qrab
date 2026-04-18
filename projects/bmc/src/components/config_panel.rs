use dioxus::prelude::*;

use bmc::server::vm_actions;
use bmc::types::{
    BmcConfig, KvmInfoResponse, SevCapabilitiesResponse, SevStatusResponse, TpmStatusResponse,
    VmPhase,
};

use crate::components::store::use_bmc_store;

/// VM configuration panel.
///
/// Two editable knobs — `memory` and `cpus` — gated by `current_phase == Off`.
/// Everything else is fixed by the hardcoded SEV-SNP + TPM + KVM + UEFI
/// x86_64 profile in `state::power_on`, and is shown here read-only so the
/// operator can see what's actually running without digging through logs.
///
/// A live "SEV info" section below mirrors the full `query-sev` +
/// `query-sev-capabilities` QMP replies when the VM is Running.
#[component]
pub fn ConfigPanel(
    current_phase: VmPhase,
    sev_status: Option<SevStatusResponse>,
    sev_capabilities: Option<SevCapabilitiesResponse>,
    tpm_status: Option<TpmStatusResponse>,
    kvm_info: KvmInfoResponse,
) -> Element {
    let is_off = current_phase == VmPhase::Off;
    let is_running = matches!(
        current_phase,
        VmPhase::Running | VmPhase::Paused | VmPhase::ShuttingDown
    );

    // Config is now pushed from the server over SSE. No use_resource,
    // no manual refetch — `set_memory` / `set_cpus` trigger a
    // `BmcPush::Config` frame server-side, and this component re-renders
    // when the store signal updates.
    let store = use_bmc_store();
    let cfg: BmcConfig = store.config.read().clone();

    let cpus = cfg.cpus.unwrap_or(1);
    let memory = cfg.memory.clone();

    rsx! {
        div { class: "panel config-panel",
            div { class: "panel-header", h2 { "Configuration" } }

            // Editable knobs.
            section { class: "config-section",
                h3 { "Resources" }
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
                                    let _ = vm_actions::set_memory(val).await;
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
                                        let _ = vm_actions::set_cpus(val).await;
                                    });
                                }
                            },
                        }
                    }
                }
            }

            // Hardcoded-profile display. Checkboxes are pre-checked and
            // disabled — the panel's job is to make these choices visible,
            // not to let the operator toggle them off.
            section { class: "config-section",
                h3 { "Profile" }
                div { class: "config-group",
                    FixedRow { label: "Architecture", value: "x86_64" }
                    FixedRow { label: "Machine", value: "UEFI (q35)" }
                    FixedRow { label: "Firmware", value: "AmdSev OVMF" }
                    FixedRow { label: "CPU model", value: "EPYC-v4" }
                    LockedToggle {
                        label: "KVM",
                        checked: true,
                        detail: kvm_detail(&kvm_info, is_running),
                    }
                    LockedToggle {
                        label: "SEV-SNP",
                        checked: true,
                        detail: sev_detail(&sev_status, is_running),
                    }
                    LockedToggle {
                        label: "TPM 2.0 (swtpm, CRB)",
                        checked: true,
                        detail: tpm_detail(&tpm_status, is_running),
                    }
                }
            }

            // Live SEV info — everything `query-sev` + `query-sev-capabilities`
            // reports. Shown only while a VM is up; hidden when Off.
            if is_running {
                SevInfoSection { sev_status, sev_capabilities }
            }
        }
    }
}

#[component]
fn FixedRow(label: &'static str, value: &'static str) -> Element {
    rsx! {
        div { class: "config-field",
            label { "{label}" }
            input {
                r#type: "text",
                class: "input-mono",
                value: "{value}",
                disabled: true,
            }
        }
    }
}

#[component]
fn LockedToggle(label: &'static str, checked: bool, detail: String) -> Element {
    rsx! {
        div { class: "config-toggle",
            label {
                input {
                    r#type: "checkbox",
                    checked,
                    disabled: true,
                }
                " {label}"
                if !detail.is_empty() {
                    span { class: "config-hint", " \u{2014} {detail}" }
                }
            }
        }
    }
}

fn kvm_detail(info: &KvmInfoResponse, is_running: bool) -> String {
    if !is_running {
        return String::new();
    }
    if info.enabled {
        "active".into()
    } else if info.present {
        "present, not enabled".into()
    } else {
        "not reported".into()
    }
}

fn sev_detail(status: &Option<SevStatusResponse>, is_running: bool) -> String {
    if !is_running {
        return String::new();
    }
    match status {
        Some(s) if s.enabled => match s.sev_type.as_deref() {
            Some(t) => format!("active ({t})"),
            None => "active".into(),
        },
        Some(_) => "inactive".into(),
        None => "query-sev unavailable".into(),
    }
}

fn tpm_detail(status: &Option<TpmStatusResponse>, is_running: bool) -> String {
    if !is_running {
        return String::new();
    }
    match status {
        Some(t) if t.present => match (t.model.as_deref(), t.backend_type.as_deref()) {
            (Some(m), Some(b)) => format!("{m} / {b}"),
            (Some(m), None) => m.to_string(),
            _ => "present".into(),
        },
        _ => "not reported".into(),
    }
}

/// Live SEV info section — read-only dump of every field `query-sev` and
/// `query-sev-capabilities` return. When the VM is freshly Running and the
/// cached_static snapshot hasn't refreshed yet these may be `None`, in which
/// case we show a placeholder instead of pretending values are "0".
#[component]
fn SevInfoSection(
    sev_status: Option<SevStatusResponse>,
    sev_capabilities: Option<SevCapabilitiesResponse>,
) -> Element {
    rsx! {
        section { class: "config-section",
            h3 { "SEV Info" }
            if sev_status.is_none() && sev_capabilities.is_none() {
                div { class: "status-info", "Querying SEV..." }
            }
            if let Some(s) = sev_status {
                div { class: "config-group",
                    InfoRow { label: "Enabled", value: s.enabled.to_string() }
                    InfoRow {
                        label: "Guest type",
                        value: s.sev_type.unwrap_or_else(|| "\u{2014}".into()),
                    }
                    InfoRow {
                        label: "State",
                        value: s.state.unwrap_or_else(|| "\u{2014}".into()),
                    }
                    InfoRow {
                        label: "API version",
                        value: match (s.api_major, s.api_minor, s.build_id) {
                            (Some(maj), Some(min), Some(build)) => format!("{maj}.{min} build {build}"),
                            _ => s.api_version.unwrap_or_else(|| "\u{2014}".into()),
                        },
                    }
                    if let Some(p) = s.snp_policy {
                        InfoRow { label: "SNP policy", value: p }
                    }
                    if let Some(p) = s.policy {
                        InfoRow { label: "SEV policy", value: p }
                    }
                    if let Some(h) = s.handle {
                        InfoRow { label: "Handle", value: h.to_string() }
                    }
                }
            }
            if let Some(c) = sev_capabilities {
                div { class: "config-group",
                    InfoRow { label: "C-bit position", value: c.cbitpos.to_string() }
                    InfoRow {
                        label: "Reduced phys bits",
                        value: c.reduced_phys_bits.to_string(),
                    }
                    InfoRow {
                        label: "PDH (bytes)",
                        value: format!("{} base64 chars", c.pdh.len()),
                    }
                    InfoRow {
                        label: "Cert chain (bytes)",
                        value: format!("{} base64 chars", c.cert_chain.len()),
                    }
                }
            }
        }
    }
}

#[component]
fn InfoRow(label: String, value: String) -> Element {
    rsx! {
        div { class: "config-field",
            label { "{label}" }
            input {
                r#type: "text",
                class: "input-mono",
                value: "{value}",
                disabled: true,
            }
        }
    }
}
