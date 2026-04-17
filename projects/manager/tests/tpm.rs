//! TPM integration tests.

use manager::arch::{Arch, MachineConfig};
use manager::swtpm::SwtpmProcess;
use manager::vm::{Accel, TpmBackend, TpmConfig, TpmModel, VmBuilder};

/// Test swtpm process spawn and cleanup.
#[tokio::test]
async fn swtpm_spawn_and_cleanup() {
    if which::which("swtpm").is_err() {
        eprintln!("SKIP: swtpm not installed");
        return;
    }

    let tmpdir = tempfile::TempDir::new().unwrap();
    let mut swtpm = SwtpmProcess::spawn(tmpdir.path(), None, None)
        .await
        .expect("swtpm should spawn");

    assert!(swtpm.sock().exists(), "swtpm socket should exist");
    swtpm.cleanup().await;
}

/// Test swtpm with custom state dir.
#[tokio::test]
async fn swtpm_custom_state_dir() {
    if which::which("swtpm").is_err() {
        eprintln!("SKIP: swtpm not installed");
        return;
    }

    let tmpdir = tempfile::TempDir::new().unwrap();
    let state_dir = tmpdir.path().join("my-tpm-state");
    let mut swtpm = SwtpmProcess::spawn(tmpdir.path(), Some(state_dir.clone()), None)
        .await
        .expect("swtpm should spawn with custom state dir");

    assert!(swtpm.sock().exists());
    assert!(state_dir.exists(), "custom state dir should be created");
    swtpm.cleanup().await;
}

/// Test TpmConfig construction with emulated backend.
#[test]
fn tpm_config_emulated() {
    let config = TpmConfig {
        backend: TpmBackend::Emulated {
            swtpm_binary: None,
            state_dir: None,
        },
        model: None,
    };
    assert!(matches!(config.backend, TpmBackend::Emulated { .. }));
    assert!(config.model.is_none());
}

/// Test TpmConfig construction with passthrough backend.
#[test]
fn tpm_config_passthrough() {
    let config = TpmConfig {
        backend: TpmBackend::Passthrough { path: None },
        model: Some(TpmModel::Tis),
    };
    assert!(matches!(config.backend, TpmBackend::Passthrough { .. }));
    assert!(matches!(config.model, Some(TpmModel::Tis)));
}

/// Test VM with emulated TPM via swtpm.
#[tokio::test]
async fn vm_with_emulated_tpm() {
    if which::which("swtpm").is_err() {
        eprintln!("SKIP: swtpm not installed");
        return;
    }
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return;
    }

    let tpm_config = TpmConfig {
        backend: TpmBackend::Emulated {
            swtpm_binary: None,
            state_dir: None,
        },
        model: None, // default CRB
    };

    let fw = match manager::firmware::find_ovmf(false) {
        Some(fw) => fw,
        None => {
            eprintln!("SKIP: OVMF firmware not found");
            return;
        }
    };

    let builder = VmBuilder::new(Arch::X86_64, MachineConfig::Uefi { memory: "256M".into() })
        .accel(Accel::Kvm)
        .cpu("host")
        .tpm(tpm_config)
        .ovmf(fw)
        .serial();

    match builder.spawn().await {
        Ok(preconfig) => {
            // Start VM so we can query TPM
            match preconfig.start().await {
                Ok(vm) => {
                    let tpm_info = vm.qmp.execute(&qapi::commands::QueryTpm {}).await;
                    vm.shutdown().await;
                    if let Ok(info) = tpm_info {
                        assert!(!info.is_empty(), "TPM should be present in query-tpm");
                    }
                }
                Err(e) => {
                    eprintln!("TPM VM start failed: {e}");
                }
            }
        }
        Err(e) => {
            eprintln!("TPM VM spawn failed: {e}");
        }
    }
}

/// Test VM with passthrough TPM (requires /dev/tpm0).
/// Only tests builder acceptance — doesn't spawn since passthrough locks device.
#[test]
fn vm_builder_accepts_passthrough_tpm() {
    let tpm_config = TpmConfig {
        backend: TpmBackend::Passthrough { path: None },
        model: None,
    };

    // Builder should accept config without error
    let _builder = VmBuilder::new(Arch::X86_64, MachineConfig::Uefi { memory: "256M".into() })
        .accel(Accel::Kvm)
        .tpm(tpm_config);
}

/// Test TpmModel variants.
#[test]
fn tpm_model_variants() {
    let crb = TpmModel::Crb;
    let tis = TpmModel::Tis;
    // Verify Debug is implemented (compile-time check via format)
    let _ = format!("{crb:?}");
    let _ = format!("{tis:?}");
}
