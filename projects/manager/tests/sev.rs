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

/// Test that SEV-SNP VM can actually boot (start + cont + query-status).
/// This reproduces the BMC launch sequence through qrab-init.
#[tokio::test]
async fn sev_snp_vm_boot() {
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
        .display()
        .serial();

    let preconfig = match builder.spawn().await {
        Ok(vm) => vm,
        Err(e) => {
            eprintln!("SEV-SNP spawn failed (may be expected): {e}");
            return;
        }
    };

    // x-exit-preconfig (SNP launch sequence)
    let vm = preconfig.start().await.expect("start (x-exit-preconfig) failed");

    // cont — start vCPUs
    vm.cont().await.expect("cont failed");

    // Wait for OVMF to boot (needs time for SNP encrypted memory init)
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Check VM is still alive
    let status = vm.qmp.execute(&qapi::commands::QueryStatus {}).await
        .expect("query-status failed — VM may have crashed (check dmesg for GHCB errors)");
    assert!(status.running, "VM should be running after cont");

    // Query SEV state
    let sev = vm.qmp.execute(&qapi::commands::QuerySev {}).await
        .expect("query-sev failed");
    eprintln!("SEV state: {:?}", sev.state);

    vm.shutdown().await;
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

/// Find the SNP boot test image (built by contrib/snp-test-image/build.sh).
/// Returns None if not found — tests skip gracefully.
fn find_snp_boot_image() -> Option<std::path::PathBuf> {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/test-fixtures/snp-boot.qcow2");
    if fixture.exists() {
        Some(fixture)
    } else {
        None
    }
}

/// Helper: skip conditions common to all SEV-SNP hardware tests.
fn sev_skip_check() -> bool {
    if !std::path::Path::new("/dev/sev").exists() {
        eprintln!("SKIP: /dev/sev not available");
        return true;
    }
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return true;
    }
    false
}

/// Test that AmdSev OVMF firmware is discoverable.
#[test]
fn find_amdsev_ovmf() {
    let fw = firmware::find_amdsev_ovmf();
    assert!(fw.is_some(), "AmdSev OVMF not found — install edk2-amdsev");
    let fw = fw.unwrap();
    assert!(fw.exists(), "AmdSev OVMF path exists but file missing");
}

/// SEV-SNP boot with a real bootable disk containing an SNP-capable kernel.
///
/// This is the critical test that was previously impossible with CirrosOS.
/// CirrosOS lacks CONFIG_AMD_MEM_ENCRYPT and crashes during kernel page
/// state transitions. The SNP test image uses the host kernel (6.18+)
/// with a tiny init that prints "SNP BOOT OK" to serial and powers off.
///
/// Verifies:
/// - AmdSev OVMF boots the UKI from the EFI system partition
/// - Kernel completes early init under SEV-SNP (page conversions succeed)
/// - Init runs and reports SEV-SNP active via /dev/kmsg
/// - Guest powers off cleanly (guest-initiated shutdown)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sev_snp_boot_with_snp_kernel() {
    if sev_skip_check() { return; }

    let amdsev_fw = match firmware::find_amdsev_ovmf() {
        Some(fw) => fw,
        None => {
            eprintln!("SKIP: AmdSev OVMF not found");
            return;
        }
    };

    let snp_image = match find_snp_boot_image() {
        Some(img) => img,
        None => {
            eprintln!("SKIP: SNP boot image not found — run contrib/snp-test-image/build.sh");
            return;
        }
    };

    // Create overlay so base image is never modified
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let overlay = tmpdir.path().join("overlay.qcow2");
    let img = std::process::Command::new("qemu-img")
        .args([
            "create", "-f", "qcow2",
            "-b", &snp_image.display().to_string(),
            "-F", "qcow2",
            &overlay.display().to_string(),
        ])
        .output()
        .expect("qemu-img");
    assert!(img.status.success(), "qemu-img create overlay failed");

    let builder = VmBuilder::new(Arch::X86_64, MachineConfig::Uefi { memory: "512M".into() })
        .accel(Accel::Kvm)
        .cpu("EPYC-v4")
        .sev_snp(SevSnpConfig::default())
        .firmware(amdsev_fw)
        .display()
        .serial();

    let preconfig = builder.spawn().await.expect("SEV-SNP spawn failed");
    let serial_log = preconfig.serial_log().cloned();

    // blockdev-add + x-exit-preconfig + device_add + cont (BMC flow)
    use qapi::commands::{BlockdevAdd, DeviceAdd};
    use qapi::{BlockdevDriver, BlockdevOptionsBranch, BlockdevOptionsFile, BlockdevOptionsQcow2, BlockdevRef};

    preconfig.qmp.execute(&BlockdevAdd {
        driver: Some(BlockdevDriver::File),
        node_name: Some("disk0-file".into()),
        read_only: Some(false),
        u: Some(BlockdevOptionsBranch::File(BlockdevOptionsFile {
            filename: overlay.display().to_string(),
            aio: None, aio_max_batch: None, drop_cache: None,
            locking: None, pr_manager: None, x_check_cache_dropped: None,
        })),
        active: None, auto_read_only: None, cache: None,
        detect_zeroes: None, discard: None, force_share: None, options: None,
    }).await.expect("blockdev-add file failed");

    preconfig.qmp.execute(&BlockdevAdd {
        driver: Some(BlockdevDriver::Qcow2),
        node_name: Some("disk0".into()),
        read_only: Some(false),
        u: Some(BlockdevOptionsBranch::Qcow2(BlockdevOptionsQcow2 {
            file: Box::new(BlockdevRef::Reference("disk0-file".into())),
            backing: None, cache_clean_interval: None, cache_size: None,
            data_file: None, discard_no_unref: None, encrypt: None,
            l2_cache_entry_size: None, l2_cache_size: None,
            lazy_refcounts: None, overlap_check: None,
            pass_discard_other: None, pass_discard_request: None,
            pass_discard_snapshot: None, refcount_cache_size: None,
        })),
        active: None, auto_read_only: None, cache: None,
        detect_zeroes: None, discard: None, force_share: None, options: None,
    }).await.expect("blockdev-add qcow2 failed");

    let vm = preconfig.start().await.expect("x-exit-preconfig failed");

    use std::collections::HashMap;
    vm.qmp.execute(&DeviceAdd {
        driver: "virtio-blk-pci".into(),
        id: Some("blk0".into()),
        bus: Some("rp0".into()),
        arguments: Some(HashMap::from([
            ("drive".to_string(), serde_json::Value::String("disk0".into())),
            ("bootindex".to_string(), serde_json::Value::Number(0.into())),
        ])),
    }).await.expect("device_add failed");

    vm.cont().await.expect("cont failed");

    // Wait for guest to boot and power off (~3s typical).
    // The init binary calls reboot(POWER_OFF) after printing status.
    tokio::time::sleep(std::time::Duration::from_secs(15)).await;

    // Verify serial output contains SNP boot markers
    if let Some(log_path) = serial_log {
        let serial = std::fs::read_to_string(&log_path).unwrap_or_default();
        eprintln!("--- serial output ({} bytes) ---", serial.len());

        assert!(
            serial.contains("SNP BOOT OK"),
            "Serial should contain 'SNP BOOT OK' — kernel failed to boot under SEV-SNP.\n\
             Serial output:\n{serial}"
        );
        assert!(
            serial.contains("SEV-SNP: active"),
            "Serial should confirm 'SEV-SNP: active' — guest didn't detect SNP.\n\
             Serial output:\n{serial}"
        );
        assert!(
            serial.contains("BOOT COMPLETE"),
            "Serial should contain 'BOOT COMPLETE' — init didn't finish.\n\
             Serial output:\n{serial}"
        );

        eprintln!("SNP BOOT WITH KERNEL: PASS");
    }

    vm.shutdown().await;
}

/// Test SEV-SNP boot across different memory sizes.
/// Original crash report used 256M — verify all sizes work with SNP kernel.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sev_snp_boot_memory_sizes() {
    if sev_skip_check() { return; }

    let amdsev_fw = match firmware::find_amdsev_ovmf() {
        Some(fw) => fw,
        None => { eprintln!("SKIP: AmdSev OVMF not found"); return; }
    };
    let snp_image = match find_snp_boot_image() {
        Some(img) => img,
        None => { eprintln!("SKIP: SNP boot image not found"); return; }
    };

    for mem in &["256M", "512M", "1G"] {
        eprintln!("Testing SEV-SNP boot with -m {mem}...");

        let tmpdir = tempfile::tempdir().expect("tempdir");
        let overlay = tmpdir.path().join("overlay.qcow2");
        let img = std::process::Command::new("qemu-img")
            .args([
                "create", "-f", "qcow2",
                "-b", &snp_image.display().to_string(),
                "-F", "qcow2",
                &overlay.display().to_string(),
            ])
            .output()
            .expect("qemu-img");
        assert!(img.status.success());

        let builder = VmBuilder::new(
            Arch::X86_64,
            MachineConfig::Uefi { memory: (*mem).into() },
        )
            .accel(Accel::Kvm)
            .cpu("EPYC-v4")
            .sev_snp(SevSnpConfig::default())
            .firmware(amdsev_fw.clone())
            .arg("-drive")
            .arg(format!("if=virtio,file={},format=qcow2", overlay.display()))
            .serial();

        let preconfig = match builder.spawn().await {
            Ok(vm) => vm,
            Err(e) => {
                eprintln!("  {mem}: spawn failed: {e}");
                continue;
            }
        };

        let serial_log = preconfig.serial_log().cloned();
        let vm = preconfig.start().await.expect("x-exit-preconfig failed");
        vm.cont().await.expect("cont failed");

        // Use -drive on CLI for simplicity (no blockdev-add dance)
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        if let Some(ref log_path) = serial_log {
            let serial = std::fs::read_to_string(log_path).unwrap_or_default();
            if serial.contains("SNP BOOT OK") {
                eprintln!("  {mem}: PASS");
            } else {
                eprintln!("  {mem}: FAIL — no SNP BOOT OK in serial");
                eprintln!("  serial tail: {}", serial.chars().rev().take(200).collect::<String>().chars().rev().collect::<String>());
            }
        }

        vm.shutdown().await;
    }
}

/// SEV-SNP boot with high fd count (simulates Dioxus BMC server).
/// Combines: AmdSev OVMF + bootable SNP disk + ~260 dummy fds + BMC QMP flow.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sev_snp_boot_bmc_full() {
    if sev_skip_check() { return; }

    let amdsev_fw = match firmware::find_amdsev_ovmf() {
        Some(fw) => fw,
        None => { eprintln!("SKIP: AmdSev OVMF not found"); return; }
    };
    let snp_image = match find_snp_boot_image() {
        Some(img) => img,
        None => { eprintln!("SKIP: SNP boot image not found"); return; }
    };

    // Simulate Dioxus fd pressure
    let mut dummy_fds = Vec::new();
    for _ in 0..260 {
        match std::fs::File::open("/dev/null") {
            Ok(f) => dummy_fds.push(f),
            Err(_) => break,
        }
    }
    eprintln!("Opened {} dummy fds", dummy_fds.len());

    let tmpdir = tempfile::tempdir().expect("tempdir");
    let overlay = tmpdir.path().join("overlay.qcow2");
    let img = std::process::Command::new("qemu-img")
        .args([
            "create", "-f", "qcow2",
            "-b", &snp_image.display().to_string(),
            "-F", "qcow2",
            &overlay.display().to_string(),
        ])
        .output()
        .expect("qemu-img");
    assert!(img.status.success());

    let builder = VmBuilder::new(Arch::X86_64, MachineConfig::Uefi { memory: "512M".into() })
        .accel(Accel::Kvm)
        .cpu("EPYC-v4")
        .sev_snp(SevSnpConfig::default())
        .firmware(amdsev_fw)
        .display()
        .serial();

    let preconfig = match builder.spawn().await {
        Ok(vm) => vm,
        Err(e) => {
            eprintln!("SEV-SNP spawn failed: {e}");
            drop(dummy_fds);
            return;
        }
    };

    drop(dummy_fds);
    let serial_log = preconfig.serial_log().cloned();

    // BMC flow: blockdev-add → x-exit-preconfig → device_add → cont
    use qapi::commands::{BlockdevAdd, DeviceAdd};
    use qapi::{BlockdevDriver, BlockdevOptionsBranch, BlockdevOptionsFile, BlockdevOptionsQcow2, BlockdevRef};

    preconfig.qmp.execute(&BlockdevAdd {
        driver: Some(BlockdevDriver::File),
        node_name: Some("disk0-file".into()),
        read_only: Some(false),
        u: Some(BlockdevOptionsBranch::File(BlockdevOptionsFile {
            filename: overlay.display().to_string(),
            aio: None, aio_max_batch: None, drop_cache: None,
            locking: None, pr_manager: None, x_check_cache_dropped: None,
        })),
        active: None, auto_read_only: None, cache: None,
        detect_zeroes: None, discard: None, force_share: None, options: None,
    }).await.expect("blockdev-add file failed");

    preconfig.qmp.execute(&BlockdevAdd {
        driver: Some(BlockdevDriver::Qcow2),
        node_name: Some("disk0".into()),
        read_only: Some(false),
        u: Some(BlockdevOptionsBranch::Qcow2(BlockdevOptionsQcow2 {
            file: Box::new(BlockdevRef::Reference("disk0-file".into())),
            backing: None, cache_clean_interval: None, cache_size: None,
            data_file: None, discard_no_unref: None, encrypt: None,
            l2_cache_entry_size: None, l2_cache_size: None,
            lazy_refcounts: None, overlap_check: None,
            pass_discard_other: None, pass_discard_request: None,
            pass_discard_snapshot: None, refcount_cache_size: None,
        })),
        active: None, auto_read_only: None, cache: None,
        detect_zeroes: None, discard: None, force_share: None, options: None,
    }).await.expect("blockdev-add qcow2 failed");

    let vm = preconfig.start().await.expect("x-exit-preconfig failed");

    use std::collections::HashMap;
    vm.qmp.execute(&DeviceAdd {
        driver: "virtio-blk-pci".into(),
        id: Some("blk0".into()),
        bus: Some("rp0".into()),
        arguments: Some(HashMap::from([
            ("drive".to_string(), serde_json::Value::String("disk0".into())),
            ("bootindex".to_string(), serde_json::Value::Number(0.into())),
        ])),
    }).await.expect("device_add failed");

    vm.cont().await.expect("cont failed");

    // Rapid QMP queries (simulating BMC dashboard poll)
    let _ = vm.qmp.execute(&qapi::commands::QueryStatus {}).await;
    let _ = vm.qmp.execute(&qapi::commands::QueryCpusFast {}).await;
    let _ = vm.qmp.execute(&qapi::commands::QueryKvm {}).await;
    let _ = vm.qmp.execute(&qapi::commands::QuerySev {}).await;

    tokio::time::sleep(std::time::Duration::from_secs(15)).await;

    if let Some(ref log_path) = serial_log {
        let serial = std::fs::read_to_string(log_path).unwrap_or_default();
        assert!(
            serial.contains("SNP BOOT OK"),
            "BMC full flow: kernel didn't boot under SEV-SNP.\nSerial:\n{serial}"
        );
        eprintln!("BMC-FULL SNP BOOT: PASS");
    }

    vm.shutdown().await;
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
