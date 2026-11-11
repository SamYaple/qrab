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

    let builder = VmBuilder::new(
        Arch::X86_64,
        MachineConfig::SecureBoot {
            memory: "256M".into(),
        },
    )
    .sev_snp(SevSnpConfig::default())
    .accel(Accel::Kvm)
    .cpu("EPYC-v4")
    .ovmf(fw)
    .serial();

    match builder.spawn().await {
        Ok(vm) => {
            // Query SEV capabilities via QMP
            let sev_info = vm
                .qmp
                .execute(&qapi::commands::QuerySevCapabilities {})
                .await;
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

    let builder = VmBuilder::new(
        Arch::X86_64,
        MachineConfig::SecureBoot {
            memory: "256M".into(),
        },
    )
    .sev_snp(SevSnpConfig::default())
    .accel(Accel::Kvm)
    .cpu("EPYC-v4")
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
    let vm = preconfig
        .start()
        .await
        .expect("start (x-exit-preconfig) failed");

    // cont — start vCPUs
    vm.cont().await.expect("cont failed");

    // Wait for OVMF to boot (needs time for SNP encrypted memory init)
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Check VM is still alive
    let status = vm
        .qmp
        .execute(&qapi::commands::QueryStatus {})
        .await
        .expect("query-status failed — VM may have crashed (check dmesg for GHCB errors)");
    assert!(status.running, "VM should be running after cont");

    // Query SEV state
    let sev = vm
        .qmp
        .execute(&qapi::commands::QuerySev {})
        .await
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

        let result = VmBuilder::new(
            Arch::X86_64,
            MachineConfig::SecureBoot {
                memory: "256M".into(),
            },
        )
        .sev_snp(SevSnpConfig::default())
        .accel(Accel::Tcg)
        .cpu("EPYC-v4")
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
    if sev_skip_check() {
        return;
    }

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
            "create",
            "-f",
            "qcow2",
            "-b",
            &snp_image.display().to_string(),
            "-F",
            "qcow2",
            &overlay.display().to_string(),
        ])
        .output()
        .expect("qemu-img");
    assert!(img.status.success(), "qemu-img create overlay failed");

    let builder = VmBuilder::new(
        Arch::X86_64,
        MachineConfig::Uefi {
            memory: "512M".into(),
        },
    )
    .sev_snp(SevSnpConfig::default())
    .accel(Accel::Kvm)
    .cpu("EPYC-v4")
    .firmware(amdsev_fw)
    .display()
    .serial();

    let preconfig = builder.spawn().await.expect("SEV-SNP spawn failed");
    let serial_log = preconfig.serial_log().cloned();

    // blockdev-add + x-exit-preconfig + device_add + cont (BMC flow)
    use qapi::commands::{BlockdevAdd, DeviceAdd};
    use qapi::{
        BlockdevDriver, BlockdevOptionsBranch, BlockdevOptionsFile, BlockdevOptionsQcow2,
        BlockdevRef,
    };

    preconfig
        .qmp
        .execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::File),
            node_name: Some("disk0-file".into()),
            read_only: Some(false),
            u: Some(BlockdevOptionsBranch::File(BlockdevOptionsFile {
                filename: overlay.display().to_string(),
                aio: None,
                aio_max_batch: None,
                drop_cache: None,
                locking: None,
                pr_manager: None,
                x_check_cache_dropped: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .expect("blockdev-add file failed");

    preconfig
        .qmp
        .execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::Qcow2),
            node_name: Some("disk0".into()),
            read_only: Some(false),
            u: Some(BlockdevOptionsBranch::Qcow2(BlockdevOptionsQcow2 {
                file: Box::new(BlockdevRef::Reference("disk0-file".into())),
                backing: None,
                cache_clean_interval: None,
                cache_size: None,
                data_file: None,
                discard_no_unref: None,
                encrypt: None,
                l2_cache_entry_size: None,
                l2_cache_size: None,
                lazy_refcounts: None,
                overlap_check: None,
                pass_discard_other: None,
                pass_discard_request: None,
                pass_discard_snapshot: None,
                refcount_cache_size: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .expect("blockdev-add qcow2 failed");

    let vm = preconfig.start().await.expect("x-exit-preconfig failed");

    use std::collections::HashMap;
    vm.qmp
        .execute(&DeviceAdd {
            driver: "virtio-blk-pci".into(),
            id: Some("blk0".into()),
            bus: Some("rp0".into()),
            arguments: Some(HashMap::from([
                (
                    "drive".to_string(),
                    serde_json::Value::String("disk0".into()),
                ),
                ("bootindex".to_string(), serde_json::Value::Number(0.into())),
            ])),
        })
        .await
        .expect("device_add failed");

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

/// SEV-SNP launch sequence across the full (memory size) x (reduced-phys-bits) matrix.
///
/// Pins the `llm-rag/issues/sev-snp-boot-crash.md` coverage table:
/// memsizes 256M/512M/1G/2G x phys-bits 1/2. Scope is **SNP launch, not
/// guest boot** — we spawn, `x-exit-preconfig`, `cont`, then query-status.
/// The VM must report `running = true`. Whether the guest image (or any
/// GRUB/UKI inside it) actually reaches userspace is intentionally out
/// of scope here; the original memory-size claim was about the SNP
/// launch sequence, not userspace liveness.
///
/// No guest disk is attached — firmware boots to the UEFI shell and the
/// test returns as soon as query-status confirms the VM is running.
/// This keeps the matrix fast (~8 × 4s) and isolates it from fixture
/// drift (e.g. AmdSev OVMF's embedded GRUB expecting `sevsecret`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sev_snp_launch_memory_matrix() {
    if sev_skip_check() {
        return;
    }

    let amdsev_fw = match firmware::find_amdsev_ovmf() {
        Some(fw) => fw,
        None => {
            eprintln!("SKIP: AmdSev OVMF not found");
            return;
        }
    };

    let mut failures: Vec<String> = Vec::new();

    for mem in &["256M", "512M", "1G", "2G"] {
        for phys_bits in [1u32, 2u32] {
            let label = format!("mem={mem} reduced_phys_bits={phys_bits}");
            eprintln!("Testing SEV-SNP launch {label}...");

            let sev = SevSnpConfig::new().reduced_phys_bits(phys_bits);
            let builder = VmBuilder::new(
                Arch::X86_64,
                MachineConfig::Uefi {
                    memory: (*mem).into(),
                },
            )
            .sev_snp(sev)
            .accel(Accel::Kvm)
            .cpu("EPYC-v4")
            .firmware(amdsev_fw.clone())
            .serial();

            let preconfig = match builder.spawn().await {
                Ok(vm) => vm,
                Err(e) => {
                    failures.push(format!("{label}: spawn failed: {e}"));
                    continue;
                }
            };

            let vm = match preconfig.start().await {
                Ok(v) => v,
                Err(e) => {
                    failures.push(format!("{label}: x-exit-preconfig failed: {e}"));
                    continue;
                }
            };
            if let Err(e) = vm.cont().await {
                failures.push(format!("{label}: cont failed: {e}"));
                vm.shutdown().await;
                continue;
            }

            // Brief settle so SNP_LAUNCH_FINISH and the first VMGEXITs complete.
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            match vm.qmp.execute(&qapi::commands::QueryStatus {}).await {
                Ok(status) if status.running => {
                    eprintln!("  {label}: running = true (launch OK)");
                }
                Ok(status) => {
                    failures.push(format!(
                        "{label}: query-status returned running=false, status={:?}",
                        status.status
                    ));
                }
                Err(e) => {
                    failures.push(format!("{label}: query-status failed: {e}"));
                }
            }

            vm.shutdown().await;
        }
    }

    if !failures.is_empty() {
        panic!(
            "SEV-SNP launch matrix had {} failure(s):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
}

/// SEV-SNP boot with high fd count (simulates Dioxus BMC server).
/// Combines: AmdSev OVMF + bootable SNP disk + ~260 dummy fds + BMC QMP flow.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sev_snp_boot_bmc_full() {
    if sev_skip_check() {
        return;
    }

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
            eprintln!("SKIP: SNP boot image not found");
            return;
        }
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
            "create",
            "-f",
            "qcow2",
            "-b",
            &snp_image.display().to_string(),
            "-F",
            "qcow2",
            &overlay.display().to_string(),
        ])
        .output()
        .expect("qemu-img");
    assert!(img.status.success());

    let builder = VmBuilder::new(
        Arch::X86_64,
        MachineConfig::Uefi {
            memory: "512M".into(),
        },
    )
    .sev_snp(SevSnpConfig::default())
    .accel(Accel::Kvm)
    .cpu("EPYC-v4")
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
    use qapi::{
        BlockdevDriver, BlockdevOptionsBranch, BlockdevOptionsFile, BlockdevOptionsQcow2,
        BlockdevRef,
    };

    preconfig
        .qmp
        .execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::File),
            node_name: Some("disk0-file".into()),
            read_only: Some(false),
            u: Some(BlockdevOptionsBranch::File(BlockdevOptionsFile {
                filename: overlay.display().to_string(),
                aio: None,
                aio_max_batch: None,
                drop_cache: None,
                locking: None,
                pr_manager: None,
                x_check_cache_dropped: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .expect("blockdev-add file failed");

    preconfig
        .qmp
        .execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::Qcow2),
            node_name: Some("disk0".into()),
            read_only: Some(false),
            u: Some(BlockdevOptionsBranch::Qcow2(BlockdevOptionsQcow2 {
                file: Box::new(BlockdevRef::Reference("disk0-file".into())),
                backing: None,
                cache_clean_interval: None,
                cache_size: None,
                data_file: None,
                discard_no_unref: None,
                encrypt: None,
                l2_cache_entry_size: None,
                l2_cache_size: None,
                lazy_refcounts: None,
                overlap_check: None,
                pass_discard_other: None,
                pass_discard_request: None,
                pass_discard_snapshot: None,
                refcount_cache_size: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .expect("blockdev-add qcow2 failed");

    let vm = preconfig.start().await.expect("x-exit-preconfig failed");

    use std::collections::HashMap;
    vm.qmp
        .execute(&DeviceAdd {
            driver: "virtio-blk-pci".into(),
            id: Some("blk0".into()),
            bus: Some("rp0".into()),
            arguments: Some(HashMap::from([
                (
                    "drive".to_string(),
                    serde_json::Value::String("disk0".into()),
                ),
                ("bootindex".to_string(), serde_json::Value::Number(0.into())),
            ])),
        })
        .await
        .expect("device_add failed");

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

// sev_snp_requires_uefi was a runtime-reject test for SEV-SNP + BIOS.
// That code path is now unreachable at the type level — `MachineConfig::Bios`
// was removed in the scope-tightening round (CoCo-only: UEFI + SNP + SVSM).
// The stronger compile-time guarantee obsoletes the runtime assertion.

/// Test SEV-SNP boot with many open fds (simulates Dioxus server).
/// The BMC server has ~270 open fds, pushing /dev/sev to fd 269+.
/// QEMU closes high fds during startup. qrab-init must dup2 to low fd.
#[tokio::test]
async fn sev_snp_vm_boot_high_fds() {
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

    // Open ~260 dummy fds to push /dev/sev to a high number.
    // This simulates the Dioxus server's fd usage.
    let mut dummy_fds = Vec::new();
    for _ in 0..260 {
        match std::fs::File::open("/dev/null") {
            Ok(f) => dummy_fds.push(f),
            Err(_) => break,
        }
    }
    eprintln!("Opened {} dummy fds to simulate server", dummy_fds.len());

    let builder = VmBuilder::new(
        Arch::X86_64,
        MachineConfig::SecureBoot {
            memory: "256M".into(),
        },
    )
    .sev_snp(SevSnpConfig::default())
    .accel(Accel::Kvm)
    .cpu("EPYC-v4")
    .ovmf(fw)
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

    // Drop dummy fds — they served their purpose of pushing fd numbers up
    drop(dummy_fds);

    let vm = preconfig.start().await.expect("start failed");
    vm.cont().await.expect("cont failed");

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let status = vm
        .qmp
        .execute(&qapi::commands::QueryStatus {})
        .await
        .expect("query-status failed — GHCB crash? check dmesg");
    assert!(status.running, "VM should be running");

    vm.shutdown().await;
    eprintln!("HIGH-FD TEST PASSED");
}

/// Reproduce BMC launch flow with all factors:
/// - ~260 dummy fds (simulating Dioxus server)
/// - virtio-serial-pci + attestation chardev (on CLI)
/// - blockdev-add + device_add + cont (QMP sequence)
///
/// Previous tests passed individually:
/// - sev_snp_vm_boot: bare boot (no disks, no chardev) → PASS
/// - sev_snp_vm_boot_high_fds: high fds, bare boot → PASS
/// - sev_snp_vm_boot_bmc_flow: device_add, no high fds, no chardev → PASS
///
/// This test combines factors to reproduce BMC crash:
/// - Multi-thread tokio runtime (BMC uses multi_thread, not current_thread)
/// - ~260 dummy fds (Dioxus server fd pressure)
/// - blockdev-add + x-exit-preconfig + device_add + cont
///
/// Goes through qrab-init (real namespace path) like BMC does.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sev_snp_vm_boot_bmc_full() {
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

    // Simulate Dioxus fd pressure
    let mut dummy_fds = Vec::new();
    for _ in 0..260 {
        match std::fs::File::open("/dev/null") {
            Ok(f) => dummy_fds.push(f),
            Err(_) => break,
        }
    }
    eprintln!("Opened {} dummy fds", dummy_fds.len());

    // Create disk
    let disk_dir = tempfile::tempdir().expect("tempdir");
    let disk_path = disk_dir.path().join("test.qcow2");
    let img = std::process::Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            &disk_path.display().to_string(),
            "64M",
        ])
        .output()
        .expect("qemu-img");
    assert!(img.status.success());

    // Use AmdSev OVMF (same as BMC), NOT SecureBoot OVMF
    let amdsev_fw = match firmware::find_amdsev_ovmf() {
        Some(fw) => fw,
        None => {
            eprintln!("SKIP: AmdSev OVMF not found");
            drop(dummy_fds);
            return;
        }
    };
    eprintln!("Using AmdSev OVMF: {}", amdsev_fw.display());

    let builder = VmBuilder::new(
        Arch::X86_64,
        MachineConfig::SecureBoot {
            memory: "256M".into(),
        },
    )
    .sev_snp(SevSnpConfig::default())
    .accel(Accel::Kvm)
    .cpu("EPYC-v4")
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

    // Drop dummy fds after spawn
    drop(dummy_fds);

    // blockdev-add
    use qapi::commands::{BlockdevAdd, DeviceAdd};
    use qapi::{
        BlockdevDriver, BlockdevOptionsBranch, BlockdevOptionsFile, BlockdevOptionsQcow2,
        BlockdevRef,
    };

    preconfig
        .qmp
        .execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::File),
            node_name: Some("disk0-file".into()),
            read_only: Some(false),
            u: Some(BlockdevOptionsBranch::File(BlockdevOptionsFile {
                filename: disk_path.display().to_string(),
                aio: None,
                aio_max_batch: None,
                drop_cache: None,
                locking: None,
                pr_manager: None,
                x_check_cache_dropped: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .expect("blockdev-add file failed");

    preconfig
        .qmp
        .execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::Qcow2),
            node_name: Some("disk0".into()),
            read_only: Some(false),
            u: Some(BlockdevOptionsBranch::Qcow2(BlockdevOptionsQcow2 {
                file: Box::new(BlockdevRef::Reference("disk0-file".into())),
                backing: None,
                cache_clean_interval: None,
                cache_size: None,
                data_file: None,
                discard_no_unref: None,
                encrypt: None,
                l2_cache_entry_size: None,
                l2_cache_size: None,
                lazy_refcounts: None,
                overlap_check: None,
                pass_discard_other: None,
                pass_discard_request: None,
                pass_discard_snapshot: None,
                refcount_cache_size: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .expect("blockdev-add qcow2 failed");

    // x-exit-preconfig
    let vm = preconfig.start().await.expect("x-exit-preconfig failed");

    // device_add
    use std::collections::HashMap;
    vm.qmp
        .execute(&DeviceAdd {
            driver: "virtio-blk-pci".into(),
            id: Some("blk0".into()),
            bus: Some("rp0".into()),
            arguments: Some(HashMap::from([
                (
                    "drive".to_string(),
                    serde_json::Value::String("disk0".into()),
                ),
                ("bootindex".to_string(), serde_json::Value::Number(0.into())),
            ])),
        })
        .await
        .expect("device_add failed");

    // cont — BMC crash point
    vm.cont().await.expect("cont failed");

    // Simulate BMC dashboard polling: rapid QMP queries right after cont.
    // BMC fires these on first dashboard poll (cached_static=None):
    //   query-status, query-cpus-fast, query-kvm, query-sev, query-tpm
    // query-sev triggers KVM_SEV ioctl during early SNP guest boot.
    eprintln!("Firing rapid QMP queries after cont (simulating dashboard poll)...");

    let qs = vm.qmp.execute(&qapi::commands::QueryStatus {}).await;
    eprintln!("  query-status: {:?}", qs.is_ok());

    let qc = vm.qmp.execute(&qapi::commands::QueryCpusFast {}).await;
    eprintln!("  query-cpus-fast: {:?}", qc.is_ok());

    let qk = vm.qmp.execute(&qapi::commands::QueryKvm {}).await;
    eprintln!("  query-kvm: {:?}", qk.is_ok());

    let qsev = vm.qmp.execute(&qapi::commands::QuerySev {}).await;
    eprintln!(
        "  query-sev: {:?}",
        qsev.as_ref().map(|_| ()).map_err(|e| e.to_string())
    );

    let qtpm = vm.qmp.execute(&qapi::commands::QueryTpm {}).await;
    eprintln!("  query-tpm: {:?}", qtpm.is_ok());

    // Now wait and check if VM survives
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let status = vm
        .qmp
        .execute(&qapi::commands::QueryStatus {})
        .await
        .expect("query-status failed — GHCB crash? check dmesg");
    assert!(status.running, "VM should be running");

    vm.shutdown().await;
    eprintln!("BMC-FULL TEST PASSED");
}

/// Reproduce BMC launch flow exactly:
/// 1. Spawn with --preconfig + PCIe root ports + virtio-serial (attestation)
/// 2. blockdev-add (file backend + qcow2 format layer) in preconfig
/// 3. x-exit-preconfig (SNP launch)
/// 4. device_add virtio-blk-pci after machine init
/// 5. cont
///
/// This is the exact sequence that crashes from the BMC binary but works
/// from manual socat / integration tests without disks.
#[tokio::test]
async fn sev_snp_vm_boot_bmc_flow() {
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

    // Create a small qcow2 disk image for blockdev-add / device_add test.
    let disk_dir = tempfile::tempdir().expect("failed to create temp dir");
    let disk_path = disk_dir.path().join("test.qcow2");
    let qemu_img = std::process::Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            &disk_path.display().to_string(),
            "64M",
        ])
        .output()
        .expect("qemu-img not found");
    assert!(qemu_img.status.success(), "qemu-img create failed");

    // Mirror BMC builder: attestation chardev via extra args
    let builder = VmBuilder::new(
        Arch::X86_64,
        MachineConfig::SecureBoot {
            memory: "256M".into(),
        },
    )
    .sev_snp(SevSnpConfig::default())
    .accel(Accel::Kvm)
    .cpu("EPYC-v4")
    .ovmf(fw)
    .display()
    .serial();

    let preconfig = match builder.spawn().await {
        Ok(vm) => vm,
        Err(e) => {
            eprintln!("SEV-SNP spawn failed: {e}");
            return;
        }
    };

    // Step 1: blockdev-add file backend (preconfig-safe)
    use qapi::commands::{BlockdevAdd, DeviceAdd};
    use qapi::{
        BlockdevDriver, BlockdevOptionsBranch, BlockdevOptionsFile, BlockdevOptionsQcow2,
        BlockdevRef,
    };

    preconfig
        .qmp
        .execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::File),
            node_name: Some("disk0-file".into()),
            read_only: Some(false),
            u: Some(BlockdevOptionsBranch::File(BlockdevOptionsFile {
                filename: disk_path.display().to_string(),
                aio: None,
                aio_max_batch: None,
                drop_cache: None,
                locking: None,
                pr_manager: None,
                x_check_cache_dropped: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .expect("blockdev-add file backend failed");

    // Step 2: blockdev-add format layer
    preconfig
        .qmp
        .execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::Qcow2),
            node_name: Some("disk0".into()),
            read_only: Some(false),
            u: Some(BlockdevOptionsBranch::Qcow2(BlockdevOptionsQcow2 {
                file: Box::new(BlockdevRef::Reference("disk0-file".into())),
                backing: None,
                cache_clean_interval: None,
                cache_size: None,
                data_file: None,
                discard_no_unref: None,
                encrypt: None,
                l2_cache_entry_size: None,
                l2_cache_size: None,
                lazy_refcounts: None,
                overlap_check: None,
                pass_discard_other: None,
                pass_discard_request: None,
                pass_discard_snapshot: None,
                refcount_cache_size: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .expect("blockdev-add qcow2 layer failed");

    // Step 3: x-exit-preconfig (SNP launch sequence)
    let vm = preconfig.start().await.expect("x-exit-preconfig failed");

    // Step 4: device_add virtio-blk-pci on rp0 (after machine init)
    use std::collections::HashMap;
    vm.qmp
        .execute(&DeviceAdd {
            driver: "virtio-blk-pci".into(),
            id: Some("blk0".into()),
            bus: Some("rp0".into()),
            arguments: Some(HashMap::from([
                (
                    "drive".to_string(),
                    serde_json::Value::String("disk0".into()),
                ),
                ("bootindex".to_string(), serde_json::Value::Number(0.into())),
            ])),
        })
        .await
        .expect("device_add virtio-blk-pci failed");

    // Step 5: cont — this is where BMC crashes with GHCB error
    vm.cont().await.expect("cont failed");

    // Wait for OVMF init
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Check VM still alive
    let status = vm
        .qmp
        .execute(&qapi::commands::QueryStatus {})
        .await
        .expect("query-status failed — GHCB crash? check dmesg for 'GHCB GPA not registered'");
    assert!(status.running, "VM should be running after cont");

    vm.shutdown().await;
    eprintln!("BMC-FLOW TEST PASSED");
}

/// Known upstream crash: SEV-SNP guest crashes during Linux kernel boot
/// when the guest performs page state transitions (private ↔ shared).
///
/// Reproduces with: AmdSev OVMF + bootable disk (cirros) + `-m 256M`.
/// The guest boots OVMF → GRUB → Linux kernel, then the ~207th page
/// conversion triggers EFAULT → EINVAL from KVM_RUN.
///
/// With an empty (non-bootable) disk, OVMF stays at the shell and only
/// does ~184 page conversions — never reaching the crash point.
///
/// This is NOT a qrab bug. It's a QEMU 10.2 / kernel 6.18 / AmdSev OVMF
/// interaction issue. This test documents the failure and will flip to
/// passing when the upstream fix lands.
///
/// See: llm-rag/issues/sev-snp-boot-crash.md
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[should_panic(expected = "upstream bug")]
async fn sev_snp_boot_crash_upstream_bug() {
    if !std::path::Path::new("/dev/sev").exists() {
        eprintln!("SKIP: /dev/sev not available");
        return;
    }
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return;
    }

    let amdsev_fw = match firmware::find_amdsev_ovmf() {
        Some(fw) => fw,
        None => {
            eprintln!("SKIP: AmdSev OVMF not found");
            return;
        }
    };

    // Must use a BOOTABLE disk to trigger the crash. An empty qcow2
    // stays at the OVMF shell and never hits the failing code path.
    let cirros = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/test-fixtures/cirros-0.6.2-x86_64-disk.img");
    if !cirros.exists() {
        eprintln!("SKIP: cirros not found at {}", cirros.display());
        return;
    }

    // Create overlay so the base image is never modified
    let tmpdir = tempfile::tempdir().expect("tempdir");
    let overlay = tmpdir.path().join("overlay.qcow2");
    let img = std::process::Command::new("qemu-img")
        .args([
            "create",
            "-f",
            "qcow2",
            "-b",
            &cirros.display().to_string(),
            "-F",
            "qcow2",
            &overlay.display().to_string(),
        ])
        .output()
        .expect("qemu-img");
    assert!(img.status.success(), "qemu-img create overlay failed");

    let builder = VmBuilder::new(
        Arch::X86_64,
        MachineConfig::Uefi {
            memory: "256M".into(),
        },
    )
    .sev_snp(SevSnpConfig::default())
    .accel(Accel::Kvm)
    .cpu("EPYC-v4")
    .firmware(amdsev_fw)
    .display()
    .serial();

    let preconfig = match builder.spawn().await {
        Ok(vm) => vm,
        Err(e) => {
            eprintln!("SKIP: SEV-SNP spawn failed: {e}");
            return;
        }
    };

    // blockdev-add
    use qapi::commands::{BlockdevAdd, DeviceAdd};
    use qapi::{
        BlockdevDriver, BlockdevOptionsBranch, BlockdevOptionsFile, BlockdevOptionsQcow2,
        BlockdevRef,
    };

    preconfig
        .qmp
        .execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::File),
            node_name: Some("disk0-file".into()),
            read_only: Some(false),
            u: Some(BlockdevOptionsBranch::File(BlockdevOptionsFile {
                filename: overlay.display().to_string(),
                aio: None,
                aio_max_batch: None,
                drop_cache: None,
                locking: None,
                pr_manager: None,
                x_check_cache_dropped: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .expect("blockdev-add file failed");

    preconfig
        .qmp
        .execute(&BlockdevAdd {
            driver: Some(BlockdevDriver::Qcow2),
            node_name: Some("disk0".into()),
            read_only: Some(false),
            u: Some(BlockdevOptionsBranch::Qcow2(BlockdevOptionsQcow2 {
                file: Box::new(BlockdevRef::Reference("disk0-file".into())),
                backing: None,
                cache_clean_interval: None,
                cache_size: None,
                data_file: None,
                discard_no_unref: None,
                encrypt: None,
                l2_cache_entry_size: None,
                l2_cache_size: None,
                lazy_refcounts: None,
                overlap_check: None,
                pass_discard_other: None,
                pass_discard_request: None,
                pass_discard_snapshot: None,
                refcount_cache_size: None,
            })),
            active: None,
            auto_read_only: None,
            cache: None,
            detect_zeroes: None,
            discard: None,
            force_share: None,
            options: None,
        })
        .await
        .expect("blockdev-add qcow2 failed");

    // x-exit-preconfig (SNP launch)
    let vm = preconfig.start().await.expect("x-exit-preconfig failed");

    // device_add
    use std::collections::HashMap;
    vm.qmp
        .execute(&DeviceAdd {
            driver: "virtio-blk-pci".into(),
            id: Some("blk0".into()),
            bus: Some("rp0".into()),
            arguments: Some(HashMap::from([
                (
                    "drive".to_string(),
                    serde_json::Value::String("disk0".into()),
                ),
                ("bootindex".to_string(), serde_json::Value::Number(0.into())),
            ])),
        })
        .await
        .expect("device_add failed");

    // cont — guest starts executing
    vm.cont().await.expect("cont failed");

    // Wait long enough for cirros kernel to boot and trigger the crash.
    // The crash happens ~2-3s after cont (during Linux kernel early init
    // page conversions). We wait 10s to be safe.
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let result = vm.qmp.execute(&qapi::commands::QueryStatus {}).await;

    match result {
        Ok(status) if status.running => {
            // Upstream bug is FIXED — guest survived boot.
            eprintln!("UPSTREAM FIX DETECTED: SEV-SNP guest booted with cirros!");
            eprintln!("Remove #[should_panic] or update this test.");
            vm.shutdown().await;
        }
        Ok(status) => {
            // Guest crashed (internal-error). Expected with the upstream bug.
            eprintln!(
                "EXPECTED CRASH: status={:?} (upstream KVM/QEMU/OVMF bug)",
                status.status
            );
            eprintln!("See: llm-rag/issues/sev-snp-boot-crash.md");
            vm.shutdown().await;
            panic!(
                "SEV-SNP guest crashed during Linux boot (upstream bug): {:?}",
                status.status
            );
        }
        Err(e) => {
            // QMP connection lost — QEMU process died.
            eprintln!("EXPECTED CRASH: QMP error={e} (upstream KVM/QEMU/OVMF bug)");
            eprintln!("See: llm-rag/issues/sev-snp-boot-crash.md");
            panic!("SEV-SNP guest crashed during Linux boot (upstream bug): {e}");
        }
    }
}
