use dioxus::prelude::*;

use bmc::types::{BmcCapabilities, SevStatusResponse, TpmStatusResponse};

#[component]
pub fn SecurityPanel(
    sev_status: Option<SevStatusResponse>,
    tpm_status: Option<TpmStatusResponse>,
    capabilities: BmcCapabilities,
) -> Element {
    rsx! {
        div { class: "panel security-panel",
            div { class: "panel-header",
                h2 { "Security" }
            }

            div { class: "status-info",
                h3 { "SEV-SNP" }
                match sev_status {
                    Some(ref sev) => rsx! {
                        div { class: "status-row",
                            span { class: "label", "Enabled" }
                            span { class: "value",
                                if sev.enabled { "Yes" } else { "No" }
                            }
                        }
                        if let Some(ref sev_type) = sev.sev_type {
                            div { class: "status-row",
                                span { class: "label", "Type" }
                                span { class: "value", "{sev_type}" }
                            }
                        }
                        if let Some(ref state) = sev.state {
                            div { class: "status-row",
                                span { class: "label", "State" }
                                span { class: "value", "{state}" }
                            }
                        }
                        if let Some(ref api_version) = sev.api_version {
                            div { class: "status-row",
                                span { class: "label", "API Version" }
                                span { class: "value", "{api_version}" }
                            }
                        }
                    },
                    None => rsx! {
                        div { class: "status-row",
                            span { class: "label", "Host Support" }
                            span { class: "value",
                                if capabilities.sev_snp_available {
                                    "Available"
                                } else if capabilities.sev_available {
                                    "SEV only (no SNP)"
                                } else {
                                    "Not available"
                                }
                            }
                        }
                        if capabilities.sev_available {
                            div { class: "status-row",
                                span { class: "label", "VM Status" }
                                span { class: "value", "Not enabled" }
                            }
                        }
                    },
                }
            }

            div { class: "status-info",
                h3 { "TPM" }
                match tpm_status {
                    Some(ref tpm) => rsx! {
                        div { class: "status-row",
                            span { class: "label", "Present" }
                            span { class: "value",
                                if tpm.present { "Yes" } else { "No" }
                            }
                        }
                        if let Some(ref model) = tpm.model {
                            div { class: "status-row",
                                span { class: "label", "Model" }
                                span { class: "value", "{model}" }
                            }
                        }
                        if let Some(ref backend_type) = tpm.backend_type {
                            div { class: "status-row",
                                span { class: "label", "Backend" }
                                span { class: "value", "{backend_type}" }
                            }
                        }
                    },
                    None => rsx! {
                        div { class: "status-row",
                            span { class: "label", "Host Support" }
                            span { class: "value",
                                if capabilities.tpm_passthrough_available {
                                    "Passthrough (/dev/tpm0)"
                                } else if capabilities.swtpm_available {
                                    "Emulated (swtpm)"
                                } else {
                                    "Not available"
                                }
                            }
                        }
                        if capabilities.tpm_passthrough_available || capabilities.swtpm_available {
                            div { class: "status-row",
                                span { class: "label", "VM Status" }
                                span { class: "value", "Not enabled" }
                            }
                        }
                    },
                }
            }
        }
    }
}
