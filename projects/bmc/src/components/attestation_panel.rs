use dioxus::prelude::*;

use bmc::server::vm_status::{
    generate_attestation_qr, request_attestation_report, send_attestation_to_serial,
};
use bmc::types::{AttestationQrResponse, BmcCapabilities, SevStatusResponse};

/// Attestation timeline stages.
const TIMELINE_STAGES: &[&str] = &["Boot", "Measured", "Challenge", "Verified"];

/// Determine how far along the attestation timeline we are based on SEV state.
fn timeline_progress(sev: &Option<SevStatusResponse>) -> usize {
    match sev {
        Some(s) if s.enabled => match s.state.as_deref() {
            Some("LaunchUpdate") => 1,
            Some("LaunchSecret") => 2,
            Some("Running" | "RunningMeasured") => 3,
            _ => 1, // enabled = at least booted+measured
        },
        _ => 0,
    }
}

/// Dedicated attestation panel below the hero. Shows launch measurement,
/// attestation report controls, QR code generation, serial output,
/// SEV capabilities, and status timeline.
#[component]
pub fn AttestationPanel(
    sev_status: Option<SevStatusResponse>,
    capabilities: BmcCapabilities,
) -> Element {
    let progress = timeline_progress(&sev_status);
    let has_sev = sev_status.is_some();

    // Challenge nonce input state
    let mut nonce_input = use_signal(|| String::new());
    // QR response state
    let mut qr_response = use_signal(|| None::<AttestationQrResponse>);
    // Status messages
    let mut report_status = use_signal(|| String::new());
    let mut serial_status = use_signal(|| String::new());
    // Report data display
    let mut report_data = use_signal(|| None::<String>);

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

                // Challenge nonce + attestation report
                div { class: "attestation-section",
                    h3 { "Attestation Challenge" }
                    if has_sev {
                        div { class: "attestation-controls",
                            div { class: "config-field",
                                label { "Challenge Nonce (hex)" }
                                input {
                                    r#type: "text",
                                    class: "input-mono",
                                    placeholder: "64-byte hex nonce for attestation challenge",
                                    disabled: !has_sev,
                                    value: "{nonce_input}",
                                    oninput: move |e| nonce_input.set(e.value()),
                                }
                            }
                            div { class: "attestation-buttons",
                                button {
                                    class: "btn btn-action",
                                    disabled: !has_sev || nonce_input().is_empty(),
                                    onclick: move |_| {
                                        let nonce = nonce_input();
                                        spawn(async move {
                                            report_status.set("Requesting report...".into());
                                            match request_attestation_report(nonce).await {
                                                Ok(report) => {
                                                    report_data.set(Some(report.data.clone()));
                                                    report_status.set(format!(
                                                        "Report received at {}",
                                                        report.timestamp
                                                    ));
                                                    // Auto-generate QR with new report
                                                    match generate_attestation_qr("svg".into()).await {
                                                        Ok(qr) => qr_response.set(Some(qr)),
                                                        Err(e) => {
                                                            report_status.set(format!(
                                                                "Report OK but QR failed: {e}"
                                                            ));
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    report_status.set(format!("Error: {e}"));
                                                }
                                            }
                                        });
                                    },
                                    "Submit Challenge"
                                }
                                button {
                                    class: "btn btn-secondary",
                                    disabled: !has_sev,
                                    onclick: move |_| {
                                        spawn(async move {
                                            match generate_attestation_qr("svg".into()).await {
                                                Ok(qr) => {
                                                    qr_response.set(Some(qr));
                                                    report_status.set("QR generated".into());
                                                }
                                                Err(e) => {
                                                    report_status.set(format!("QR error: {e}"));
                                                }
                                            }
                                        });
                                    },
                                    "Generate QR"
                                }
                                button {
                                    class: "btn btn-secondary",
                                    disabled: !has_sev,
                                    onclick: move |_| {
                                        spawn(async move {
                                            serial_status.set("Sending to serial...".into());
                                            match send_attestation_to_serial().await {
                                                Ok(()) => {
                                                    serial_status
                                                        .set("QR sent to serial console".into());
                                                }
                                                Err(e) => {
                                                    serial_status
                                                        .set(format!("Serial error: {e}"));
                                                }
                                            }
                                        });
                                    },
                                    "Send to Serial"
                                }
                            }
                            p { class: "text-muted text-small",
                                "Type this nonce into VNC console to verify guest-side attestation"
                            }
                        }

                        // Status messages
                        if !report_status().is_empty() {
                            div { class: "attestation-status", "{report_status}" }
                        }
                        if !serial_status().is_empty() {
                            div { class: "attestation-status text-muted", "{serial_status}" }
                        }

                        // Report data display
                        if let Some(data) = report_data() {
                            div { class: "report-display",
                                h4 { "Attestation Report" }
                                pre { class: "mono report-data", "{data}" }
                            }
                        }
                    } else {
                        div { class: "empty-state", "SEV-SNP VM required for attestation" }
                    }
                }

                // QR code display
                if let Some(qr) = qr_response() {
                    div { class: "attestation-section",
                        h3 { "Attestation QR Code" }
                        div { class: "qr-display",
                            div {
                                class: "qr-svg-container",
                                dangerous_inner_html: "{qr.qr_data}",
                            }
                        }
                        div { class: "qr-meta",
                            span { class: "text-muted text-small",
                                "Generated: {qr.timestamp}"
                            }
                        }
                        details { class: "qr-payload-details",
                            summary { "Raw Payload" }
                            pre { class: "mono qr-payload", "{qr.payload}" }
                        }
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
