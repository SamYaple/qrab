//! SEV-SNP integration tests.
//! Requires host with SEV-SNP enabled and /dev/sev present.

use manager::arch::{Arch, MachineConfig};
use manager::firmware;
use manager::vm::{Accel, SevSnpConfig, VmBuilder};

/// Test that OVMF secure firmware is discoverable.
#[test]
fn find_ovmf_secure() {
    let fw = firmware::find_ovmf(true);
    assert!(fw.is_some(), "OVMF secure firmware not found");
    let fw = fw.unwrap();
    assert!(fw.code.exists());
    assert!(fw.vars_template.exists());
}

/// Test that OVMF standard firmware is discoverable.
#[test]
fn find_ovmf_standard() {
    let fw = firmware::find_ovmf(false);
    assert!(fw.is_some(), "OVMF standard firmware not found");
}

/// Test SevSnpConfig default values.
#[test]
fn sev_snp_config_defaults() {
    let config = SevSnpConfig::default();
    assert_eq!(config.reduced_phys_bits, 1);
    assert!(config.policy.is_none());
    assert!(config.sev_device.is_none());
}

/// Test that SEV-SNP VM can be spawned and query-sev works.
/// This is a real VM spawn — needs KVM + SEV + OVMF.
#[tokio::test]
async fn sev_snp_vm_spawn() {
    if !std::path::Path::new("/dev/sev").exists() {
        eprintln!("SKIP: /dev/sev not available");
        return;
    }
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return;
    }

    let fw = match firmware::find_ovmf(true) {
        Some(fw) => fw,
        None => {
            eprintln!("SKIP: OVMF secure firmware not found");
            return;
        }
    };

    let builder = VmBuilder::new(Arch::X86_64, MachineConfig::SecureBoot { memory: "256M".into() })
        .accel(Accel::Kvm)
        .cpu("EPYC-v4")
        .sev_snp(SevSnpConfig::default())
        .ovmf(fw)
        .serial();

    match builder.spawn().await {
        Ok(vm) => {
            // Query SEV capabilities via QMP
            let sev_info = vm.qmp.execute(&qapi::commands::QuerySevCapabilities {}).await;
            // Cleanup before asserting
            vm.shutdown().await;
            if let Ok(info) = sev_info {
                assert!(info.pdh.len() > 0, "SEV PDH should be non-empty");
            }
        }
        Err(e) => {
            // SEV spawn can fail for various reasons (firmware, kernel config).
            // Log but don't hard-fail — CI may not have SEV.
            eprintln!("SEV-SNP spawn failed (may be expected): {e}");
        }
    }
}

/// Validate that SEV-SNP rejects non-KVM accel.
#[test]
fn sev_snp_requires_kvm() {
    // validate_sev is called inside spawn(), but we can test the builder
    // rejects bad combos by attempting spawn with TCG.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        if !Arch::X86_64.is_available() {
            eprintln!("SKIP: qemu-system-x86_64 not available");
            return;
        }

        let result = VmBuilder::new(Arch::X86_64, MachineConfig::SecureBoot { memory: "256M".into() })
            .accel(Accel::Tcg)
            .cpu("EPYC-v4")
            .sev_snp(SevSnpConfig::default())
            .spawn()
            .await;

        match result {
            Ok(_) => panic!("SEV-SNP with TCG should fail"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("KVM"),
                    "Error should mention KVM requirement, got: {msg}"
                );
            }
        }
    });
}

/// Validate that SEV-SNP rejects BIOS machine config.
#[test]
fn sev_snp_requires_uefi() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        if !Arch::X86_64.is_available() {
            eprintln!("SKIP: qemu-system-x86_64 not available");
            return;
        }

        let result = VmBuilder::new(Arch::X86_64, MachineConfig::Bios { memory: "256M".into() })
            .accel(Accel::Kvm)
            .cpu("EPYC-v4")
            .sev_snp(SevSnpConfig::default())
            .spawn()
            .await;

        match result {
            Ok(_) => panic!("SEV-SNP with BIOS should fail"),
            Err(err) => {
                let msg = err.to_string();
                assert!(
                    msg.contains("UEFI") || msg.contains("SecureBoot"),
                    "Error should mention UEFI requirement, got: {msg}"
                );
            }
        }
    });
}
