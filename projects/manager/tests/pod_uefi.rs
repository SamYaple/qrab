use manager::pod::{self, Config, Phase};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, BufReader};

fn path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required")))
}

async fn boot_and_stop(config: &Config, path: &Path) -> String {
    std::fs::write(path, serde_json::to_vec(config).unwrap()).unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_qrab-manager"))
        .args(["run", "--config"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let fingerprint = tokio::time::timeout(Duration::from_secs(90), async {
        let mut seen = std::collections::HashSet::new();
        let mut fingerprint = None;
        loop {
            let mut line = Vec::new();
            if output.read_until(b'\n', &mut line).await.unwrap() == 0 {
                panic!("guest exited before UEFI/TPM evidence");
            }
            let line = String::from_utf8_lossy(&line);
            for marker in [
                "QRAB_UEFI_BOOTED",
                "QRAB_TPM2_READY",
                "QRAB_TPM_EVENTLOG_PRESENT",
                "QRAB_GUEST_NO_NIC_OK",
            ] {
                if line.contains(marker) {
                    seen.insert(marker);
                }
            }
            if let Some(index) = line.find("QRAB_TPM_EK_SHA256=") {
                let hash: String = line[index + "QRAB_TPM_EK_SHA256=".len()..]
                    .chars()
                    .take_while(char::is_ascii_hexdigit)
                    .collect();
                assert_eq!(hash.len(), 64);
                fingerprint = Some(hash);
            }
            if seen.len() == 4 {
                if let Some(hash) = fingerprint.take() {
                    break hash;
                }
            }
        }
    })
    .await
    .expect("UEFI/TPM guest did not become ready");
    pod::probe(config, true).unwrap();
    let before = pod::status(config).unwrap();
    assert!(before.swtpm_pid.is_some());
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child.id().unwrap() as i32),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(15), child.wait())
        .await
        .unwrap()
        .unwrap()
        .success());
    assert_eq!(pod::status(config).unwrap().phase, Phase::Exited);
    for pid in [before.qemu_pid, before.swtpm_pid].into_iter().flatten() {
        assert!(
            !PathBuf::from(format!("/proc/{pid}")).exists(),
            "child {pid} was not reaped"
        );
    }
    fingerprint
}

#[tokio::test]
#[ignore = "requires current QEMU with TPM, SWTPM, OVMF and demo UEFI artifacts; set QRAB_TEST_QEMU, QRAB_TEST_QEMU_DATA, QRAB_TEST_GUEST, QRAB_TEST_FIRMWARE"]
async fn uefi_tpm_identity_survives_replacement_and_corruption_fails_closed() {
    let directory = tempfile::tempdir().unwrap();
    let firmware = path("QRAB_TEST_FIRMWARE");
    let config:Config=serde_json::from_value(serde_json::json!({
        "name":"uefi-test", "state_id":"immutable-cr-uid", "qemu_binary":path("QRAB_TEST_QEMU"), "firmware_dir":path("QRAB_TEST_QEMU_DATA"),
        "memory_mib":512,"console":true,"uefi":{"code":firmware.join("OVMF_CODE_4M.fd"),"vars_template":firmware.join("OVMF_VARS_4M.fd"),"boot_disk":path("QRAB_TEST_GUEST").join("boot.img")},
        "tpm":{"swtpm_binary":std::env::var("QRAB_TEST_SWTPM").unwrap_or_else(|_|"/usr/bin/swtpm".into())},
        "runtime_dir":directory.path().join("runtime"),"state_dir":directory.path().join("state"),"startup_timeout_secs":5,"shutdown_grace_secs":0
    })).unwrap();
    let file = directory.path().join("vm.json");
    let first = boot_and_stop(&config, &file).await;
    let mut replacement = config.clone();
    replacement.runtime_dir = directory.path().join("replacement");
    let second = boot_and_stop(&replacement, &file).await;
    assert_eq!(
        first, second,
        "pod replacement generated a new TPM endorsement identity"
    );
    // A TPM backend dying must fail the VM and reap QEMU, rather than leave
    // an apparently healthy guest running without its identity device.
    let mut fault = config.clone();
    fault.runtime_dir = directory.path().join("tpm-failure");
    let running = fault.clone();
    let task = tokio::spawn(async move { pod::run(running, std::future::pending()).await });
    tokio::time::timeout(Duration::from_secs(10), async {
        while pod::probe(&fault, true).is_err() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let state = pod::status(&fault).unwrap();
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(state.swtpm_pid.unwrap() as i32),
        nix::sys::signal::Signal::SIGKILL,
    )
    .unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    assert_eq!(pod::status(&fault).unwrap().phase, Phase::Failed);
    for pid in [state.qemu_pid, state.swtpm_pid].into_iter().flatten() {
        assert!(
            !PathBuf::from(format!("/proc/{pid}")).exists(),
            "child survived TPM failure"
        );
    }
    let permanent = config.state_dir.join("identity/tpm/tpm2-00.permall");
    std::fs::write(&permanent, b"broken retained TPM state").unwrap();
    assert!(tokio::time::timeout(
        Duration::from_secs(10),
        pod::run(config, std::future::pending())
    )
    .await
    .unwrap()
    .is_err());
    assert_eq!(
        std::fs::read(permanent).unwrap(),
        b"broken retained TPM state",
        "corrupt TPM state was silently replaced"
    );
}
