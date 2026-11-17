//! Explicit real-QEMU guest-agent test. It needs a current QEMU with
//! virtio-serial and a direct-boot demo guest whose initramfs includes
//! qrab-guest-agent, the example update hook and the opt-in freeze fixture
//! mount (see docs/guest-agent.md). The host also needs mke2fs:
//!
//! QRAB_TEST_QEMU=... QRAB_TEST_FIRMWARE_DIR=... QRAB_TEST_KERNEL=... QRAB_TEST_INITRD=... \
//!   cargo test -p manager --test pod_guest_agent -- --ignored --nocapture
use manager::pod::{self, Config, FreezeState, UpdateState};
use std::{path::PathBuf, time::Duration};

fn path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("{name} is required")))
}

#[tokio::test]
#[ignore = "requires QEMU and a guest image containing qrab-guest-agent"]
async fn manager_controls_the_real_guest_agent() {
    let dir = tempfile::tempdir().unwrap();
    // Format only this test's private regular file, before QEMU opens it.
    // The fixture guest mounts it only for the exact opt-in kernel argument,
    // UUID, label, type and capacity. Ordinary guest disks stay untouched.
    const UUID: &str = "9f908867-3cac-45c8-8586-38166fb10f00";
    let disk = dir.path().join("scratch.raw");
    std::fs::File::create(&disk)
        .unwrap()
        .set_len(64 << 20)
        .unwrap();
    let formatted = std::process::Command::new("mke2fs")
        .args([
            "-q",
            "-F",
            "-t",
            "ext4",
            "-m",
            "0",
            "-U",
            UUID,
            "-L",
            "QRAB-FREEZE",
            "-E",
            "lazy_itable_init=0,lazy_journal_init=0",
        ])
        .arg(&disk)
        .status()
        .expect("mke2fs is required to prepare the disposable freeze filesystem");
    assert!(
        formatted.success(),
        "could not format the private scratch file"
    );
    let mut config: Config = serde_json::from_value(serde_json::json!({
        "name": "agent-test",
        "qemu_binary": path("QRAB_TEST_QEMU"),
        "kernel": path("QRAB_TEST_KERNEL"),
        "initrd": path("QRAB_TEST_INITRD"),
        "kernel_args": format!("console=ttyS0 rdinit=/init panic=0 quiet qrab.freeze_fixture={UUID}"),
        "memory_mib": 512,
        "runtime_dir": dir.path().join("run"),
        "startup_timeout_secs": 30,
        "shutdown_grace_secs": 0,
        "disk": {"path": disk},
    }))
    .unwrap();
    config.firmware_dir = std::env::var_os("QRAB_TEST_FIRMWARE_DIR").map(PathBuf::from);
    let (control, receiver) = pod::control_channel();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let supervisor = tokio::spawn(pod::run_controlled(
        config,
        async {
            let _ = stopped.await;
        },
        receiver,
    ));
    let connected = tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            if let Ok(status) = control.agent_status().await {
                if status.connected {
                    return status;
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .expect("guest agent never connected");
    eprintln!("agent: {connected:?}");
    control.agent_ping().await.unwrap();

    let frozen = control
        .agent_freeze(Some(vec!["/run/qrab-freeze-fixture".into()]))
        .await
        .unwrap();
    eprintln!("froze {frozen} filesystem(s)");
    assert_eq!(frozen, 1, "the selected scratch filesystem was not frozen");
    assert_eq!(
        control.agent_freeze_status().await.unwrap(),
        FreezeState::Frozen
    );
    assert!(control.agent_update_trigger(vec![]).await.is_err());
    assert_eq!(control.agent_thaw().await.unwrap(), frozen);
    assert_eq!(
        control.agent_freeze_status().await.unwrap(),
        FreezeState::Thawed
    );

    let started = control
        .agent_update_trigger(vec!["channel=stable".into()])
        .await
        .unwrap();
    assert_eq!(started.state, UpdateState::Running);
    let finished = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let status = control.agent_update_status().await.unwrap();
            if status.state != UpdateState::Running {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap();
    eprintln!("update: {finished:?}");
    assert_eq!(finished.state, UpdateState::Succeeded);
    assert!(finished.output.contains("QRAB_UPDATE_HOOK_OK"));

    let _ = stop.send(());
    supervisor.await.unwrap().unwrap();
}
