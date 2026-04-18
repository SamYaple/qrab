use dioxus::prelude::*;

use bmc::types::{BmcCapabilities, SevStatusResponse};

/// Attestation timeline stages.
const TIMELINE_STAGES: &[&str] = &["Boot", "Measured", "Challenge", "Verified"];

/// Determine how far along the attestation timeline we are based on SEV state.
fn timeline_progress(sev: &Option<SevStatusResponse>) -> usize {
    match sev {
        Some(s) if s.enabled => {
            match s.state.as_deref() {
                Some("LaunchUpdate") => 1,
                Some("LaunchSecret") => 2,
                Some("Running" | "RunningMeasured") => 3,
                _ => 1, // enabled = at least booted+measured
            }
        }
        _ => 0,
    }
}

/// Dedicated attestation panel below the hero. Shows launch measurement,
/// attestation report controls, SEV capabilities, and status timeline.
#[component]
pub fn AttestationPanel(
    sev_status: Option<SevStatusResponse>,
    capabilities: BmcCapabilities,
) -> Element {
    let progress = timeline_progress(&sev_status);
    let has_sev = sev_status.is_some();

    rsx! {
        div { class: "panel attestation-panel",
            div { class: "panel-header",
                h2 { "Attestation" }
            }

            // Status timeline
            div { class: "attestation-timeline",
                for (i, stage) in TIMELINE_STAGES.iter().enumerate() {
                    div {
                        class: if i < progress { "timeline-step completed" }
                               else if i == progress { "timeline-step active" }
                               else { "timeline-step" },
                        div { class: "timeline-dot" }
                        span { class: "timeline-label", "{stage}" }
                        if i < TIMELINE_STAGES.len() - 1 {
                            div { class: "timeline-connector" }
                        }
                    }
                }
            }

            div { class: "attestation-sections",
                // Launch measurement
                div { class: "attestation-section",
                    h3 { "Launch Measurement" }
                    if has_sev {
                        div { class: "measurement-display",
                            code { class: "measurement-hash mono",
                                "Measurement available after guest launch"
                            }
                        }
                    } else {
                        div { class: "empty-state", "Start a SEV-SNP VM to see launch measurement" }
                    }
                }

                // Attestation report
                div { class: "attestation-section",
                    h3 { "Attestation Report" }
                    if has_sev {
                        div { class: "attestation-controls",
                            div { class: "config-field",
                                label { "Nonce (hex)" }
                                input {
                                    r#type: "text",
                                    class: "input-mono",
                                    placeholder: "64-byte hex nonce for attestation challenge",
                                    disabled: !has_sev,
                                }
                            }
                            button {
                                class: "btn btn-action",
                                disabled: !has_sev,
                                "Request Report"
                            }
                        }
                        div { class: "report-display",
                            span { class: "text-muted", "No report requested yet" }
                        }
                    } else {
                        div { class: "empty-state", "SEV-SNP VM required for attestation" }
                    }
                }

                // Host capabilities
                div { class: "attestation-section",
                    h3 { "Host Capabilities" }
                    div { class: "capabilities-grid",
                        div { class: "cap-item",
                            span { class: "cap-label", "SEV" }
                            span { class: if capabilities.sev_available { "cap-value cap-yes" } else { "cap-value cap-no" },
                                if capabilities.sev_available { "Yes" } else { "No" }
                            }
                        }
                        div { class: "cap-item",
                            span { class: "cap-label", "SEV-SNP" }
                            span { class: if capabilities.sev_snp_available { "cap-value cap-yes" } else { "cap-value cap-no" },
                                if capabilities.sev_snp_available { "Yes" } else { "No" }
                            }
                        }
                        div { class: "cap-item",
                            span { class: "cap-label", "KVM" }
                            span { class: if capabilities.kvm_available { "cap-value cap-yes" } else { "cap-value cap-no" },
                                if capabilities.kvm_available { "Yes" } else { "No" }
                            }
                        }
                        div { class: "cap-item",
                            span { class: "cap-label", "OVMF" }
                            span { class: if capabilities.ovmf_found { "cap-value cap-yes" } else { "cap-value cap-no" },
                                if capabilities.ovmf_found { "Yes" } else { "No" }
                            }
                        }
                        div { class: "cap-item",
                            span { class: "cap-label", "Secure Boot FW" }
                            span { class: if capabilities.ovmf_secure_found { "cap-value cap-yes" } else { "cap-value cap-no" },
                                if capabilities.ovmf_secure_found { "Yes" } else { "No" }
                            }
                        }
                        div { class: "cap-item",
                            span { class: "cap-label", "swtpm" }
                            span { class: if capabilities.swtpm_available { "cap-value cap-yes" } else { "cap-value cap-no" },
                                if capabilities.swtpm_available { "Yes" } else { "No" }
                            }
                        }
                    }
                }
            }
        }
    }
}
