use dioxus::prelude::*;

use bmc::types::{SevStatusResponse, TpmStatusResponse};

/// Determine the SEV status level for CSS badge coloring.
fn sev_badge_class(sev: &Option<SevStatusResponse>) -> &'static str {
    match sev {
        Some(s) if s.enabled => {
            // Check state for measured vs pending
            match s.state.as_deref() {
                Some("RunningMeasured" | "Running") => "badge-green",
                Some("LaunchSecret" | "LaunchUpdate") => "badge-yellow",
                Some("SendUpdate" | "ReceiveUpdate") => "badge-yellow",
                _ => "badge-green", // enabled = at least active
            }
        }
        Some(_) => "badge-red", // present but not enabled = failed
        None => "badge-grey",   // VM off
    }
}

fn sev_status_text(sev: &Option<SevStatusResponse>) -> &'static str {
    match sev {
        Some(s) if s.enabled => "Active",
        Some(_) => "Failed",
        None => "Inactive",
    }
}

/// Full-width security hero panel. SEV-SNP status is the centerpiece.
/// Shows the live state of the running VM only — no host-capability
/// conditionals (the hardcoded BMC profile is SEV-SNP + TPM unconditionally,
/// so capability detection would just be noise).
#[component]
pub fn SecurityHero(
    sev_status: Option<SevStatusResponse>,
    tpm_status: Option<TpmStatusResponse>,
) -> Element {
    let badge_class = sev_badge_class(&sev_status);
    let status_text = sev_status_text(&sev_status);
    rsx! {
        div { class: "panel security-hero",
            div { class: "hero-content",
                // Left: large status indicator
                div { class: "hero-status",
                    div { class: "hero-badge {badge_class}" }
                    div { class: "hero-label",
                        h2 { class: "hero-title", "SEV-SNP" }
                        span { class: "hero-status-text {badge_class}-text", "{status_text}" }
                    }
                }

                // Center: SEV details
                div { class: "hero-details",
                    if let Some(ref sev) = sev_status {
                        if let Some(ref sev_type) = sev.sev_type {
                            div { class: "hero-detail",
                                span { class: "hero-detail-label", "Type" }
                                span { class: "hero-detail-value", "{sev_type}" }
                            }
                        }
                        if let Some(ref state) = sev.state {
                            div { class: "hero-detail",
                                span { class: "hero-detail-label", "State" }
                                span { class: "hero-detail-value mono", "{state}" }
                            }
                        }
                        if let Some(ref api_ver) = sev.api_version {
                            div { class: "hero-detail",
                                span { class: "hero-detail-label", "API" }
                                span { class: "hero-detail-value mono", "v{api_ver}" }
                            }
                        }
                    } else {
                        div { class: "hero-detail",
                            span { class: "hero-detail-label", "Status" }
                            span { class: "hero-detail-value", "VM Off" }
                        }
                    }
                }

                // Right: TPM status compact
                div { class: "hero-tpm",
                    h3 { class: "hero-tpm-title", "TPM" }
                    match tpm_status {
                        Some(ref tpm) => rsx! {
                            div { class: "hero-tpm-status",
                                span { class: if tpm.present { "badge-pill badge-pill-green" } else { "badge-pill badge-pill-grey" },
                                    if tpm.present { "Active" } else { "Inactive" }
                                }
                            }
                            if let Some(ref model) = tpm.model {
                                span { class: "hero-tpm-info mono", "{model}" }
                            }
                        },
                        None => rsx! {
                            div { class: "hero-tpm-status",
                                span { class: "badge-pill badge-pill-grey",
                                    "Inactive"
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}
