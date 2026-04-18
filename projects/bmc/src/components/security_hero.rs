use dioxus::prelude::*;

use bmc::types::{BmcCapabilities, SevStatusResponse, TpmStatusResponse};

/// Determine the SEV status level for CSS badge coloring.
fn sev_badge_class(sev: &Option<SevStatusResponse>, caps: &BmcCapabilities) -> &'static str {
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
        None => {
            if caps.sev_snp_available {
                "badge-grey" // available but VM off
            } else {
                "badge-grey"
            }
        }
    }
}

fn sev_status_text(sev: &Option<SevStatusResponse>, caps: &BmcCapabilities) -> &'static str {
    match sev {
        Some(s) if s.enabled => "Active",
        Some(_) => "Failed",
        None if caps.sev_snp_available => "Available",
        None if caps.sev_available => "SEV Only",
        None => "Unavailable",
    }
}

/// Full-width security hero panel. SEV-SNP status is the centerpiece.
#[component]
pub fn SecurityHero(
    sev_status: Option<SevStatusResponse>,
    tpm_status: Option<TpmStatusResponse>,
    capabilities: BmcCapabilities,
) -> Element {
    let badge_class = sev_badge_class(&sev_status, &capabilities);
    let status_text = sev_status_text(&sev_status, &capabilities);
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
                        // Host capability summary when VM off
                        div { class: "hero-detail",
                            span { class: "hero-detail-label", "Host SEV" }
                            span { class: "hero-detail-value",
                                if capabilities.sev_snp_available {
                                    "SNP Available"
                                } else if capabilities.sev_available {
                                    "SEV Only"
                                } else {
                                    "Not Available"
                                }
                            }
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
                                span { class: if capabilities.swtpm_available { "badge-pill badge-pill-grey" } else { "badge-pill badge-pill-red" },
                                    if capabilities.swtpm_available {
                                        "Available"
                                    } else {
                                        "Unavailable"
                                    }
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}
