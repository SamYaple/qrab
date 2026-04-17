//! Host capabilities detection test.
//! Tests the query_capabilities() function.

#![cfg(feature = "server")]

#[test]
fn query_capabilities_detects_host() {
    let caps = bmc::state::query_capabilities();

    // On this host, all should be available
    assert!(caps.kvm_available, "KVM should be available");
    assert!(caps.sev_available, "/dev/sev should exist");
    assert!(caps.sev_snp_available, "SEV-SNP should be enabled");
    assert!(caps.tpm_passthrough_available, "/dev/tpm0 should exist");
    assert!(caps.swtpm_available, "swtpm should be installed");
    assert!(caps.ovmf_found, "OVMF should be found");
    assert!(caps.ovmf_secure_found, "OVMF secure should be found");
}

#[test]
fn query_capabilities_fields_are_bools() {
    let caps = bmc::state::query_capabilities();
    // Verify serialization round-trip works (BmcCapabilities derives Serialize/Deserialize)
    let json = serde_json::to_string(&caps).expect("should serialize");
    let deser: bmc::types::BmcCapabilities =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(caps, deser);
}
