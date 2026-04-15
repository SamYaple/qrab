//! Namespace isolation verification tests.
//!
//! These tests verify that QEMU processes spawned by `VmBuilder` are
//! actually running inside Linux namespaces (user, PID, net, UTS, mount).
//!
//! Requires: `cargo build -p qrab-init` before running (binary resolved
//! via sibling-of-exe).

use manager::{Arch, MachineConfig, VmBuilder};
use std::fs;

fn require_x86_64() -> bool {
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return false;
    }
    true
}

/// Find the qrab-init child process of the current test process.
fn find_qrab_init_pid() -> Option<u32> {
    let our_pid = std::process::id();
    // Scan /proc for qrab-init processes. Due to tokio runtime threads,
    // the direct parent might be a thread, so search recursively.
    let entries = fs::read_dir("/proc").ok()?;
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let pid: u32 = match name.to_str().and_then(|s| s.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        let status = match fs::read_to_string(format!("/proc/{pid}/status")) {
            Ok(s) => s,
            Err(_) => continue,
        };
        // Check if this is qrab-init
        if !status.contains("Name:\tqrab-init") {
            continue;
        }
        // Walk parent chain to see if it leads back to us
        let mut check_pid = pid;
        for _ in 0..10 {
            let st = match fs::read_to_string(format!("/proc/{check_pid}/status")) {
                Ok(s) => s,
                Err(_) => break,
            };
            let ppid_line = match st.lines().find(|l| l.starts_with("PPid:")) {
                Some(l) => l.to_string(),
                None => break,
            };
            let ppid: u32 = match ppid_line.split_whitespace().nth(1).and_then(|s| s.parse().ok()) {
                Some(p) => p,
                None => break,
            };
            if ppid == our_pid {
                return Some(pid);
            }
            if ppid <= 1 {
                break;
            }
            check_pid = ppid;
        }
    }
    None
}

/// Read a namespace inode for a given PID.
fn read_ns_inode(pid: u32, ns: &str) -> Option<u64> {
    let link = fs::read_link(format!("/proc/{pid}/ns/{ns}")).ok()?;
    let s = link.to_str()?;
    let start = s.find('[')?;
    let end = s.find(']')?;
    s[start + 1..end].parse().ok()
}

/// Read the Uid line from /proc/{pid}/status → returns (real, effective, saved, fs).
fn read_uid_status(pid: u32) -> Option<(u32, u32, u32, u32)> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let uid_line = status.lines().find(|l| l.starts_with("Uid:"))?;
    let parts: Vec<u32> = uid_line
        .split_whitespace()
        .skip(1)
        .filter_map(|s| s.parse().ok())
        .collect();
    if parts.len() >= 4 {
        Some((parts[0], parts[1], parts[2], parts[3]))
    } else {
        None
    }
}

#[tokio::test]
async fn user_namespace_active() {
    if !require_x86_64() {
        return;
    }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap()
        .start()
        .await
        .unwrap();

    let our_pid = std::process::id();
    let child_pid = find_qrab_init_pid().expect("qrab-init process not found");

    let our_userns = read_ns_inode(our_pid, "user").unwrap();
    let child_userns = read_ns_inode(child_pid, "user").unwrap();
    assert_ne!(
        our_userns, child_userns,
        "qrab-init should be in a different user namespace"
    );

    vm.shutdown().await;
}

#[tokio::test]
async fn pid_namespace_active() {
    if !require_x86_64() {
        return;
    }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap()
        .start()
        .await
        .unwrap();

    let our_pid = std::process::id();
    let child_pid = find_qrab_init_pid().expect("qrab-init process not found");

    // qrab-init itself stays in parent PID ns (unshare(CLONE_NEWPID) only
    // affects children). Check pid_for_children to verify the PID ns for
    // QEMU (qrab-init's child) differs from ours.
    let our_pidns = read_ns_inode(our_pid, "pid_for_children")
        .or_else(|| read_ns_inode(our_pid, "pid"))
        .unwrap();
    let child_pidns = read_ns_inode(child_pid, "pid_for_children")
        .or_else(|| read_ns_inode(child_pid, "pid"))
        .unwrap();
    assert_ne!(
        our_pidns, child_pidns,
        "qrab-init's children should be in a different PID namespace"
    );

    vm.shutdown().await;
}

#[tokio::test]
async fn net_namespace_active() {
    if !require_x86_64() {
        return;
    }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap()
        .start()
        .await
        .unwrap();

    let our_pid = std::process::id();
    let child_pid = find_qrab_init_pid().expect("qrab-init process not found");

    let our_netns = read_ns_inode(our_pid, "net").unwrap();
    let child_netns = read_ns_inode(child_pid, "net").unwrap();
    assert_ne!(
        our_netns, child_netns,
        "qrab-init should be in a different network namespace"
    );

    vm.shutdown().await;
}

#[tokio::test]
async fn uts_namespace_active() {
    if !require_x86_64() {
        return;
    }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap()
        .start()
        .await
        .unwrap();

    let our_pid = std::process::id();
    let child_pid = find_qrab_init_pid().expect("qrab-init process not found");

    let our_utsns = read_ns_inode(our_pid, "uts").unwrap();
    let child_utsns = read_ns_inode(child_pid, "uts").unwrap();
    assert_ne!(
        our_utsns, child_utsns,
        "qrab-init should be in a different UTS namespace"
    );

    vm.shutdown().await;
}

#[tokio::test]
async fn mount_namespace_active() {
    if !require_x86_64() {
        return;
    }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap()
        .start()
        .await
        .unwrap();

    let our_pid = std::process::id();
    let child_pid = find_qrab_init_pid().expect("qrab-init process not found");

    let our_mntns = read_ns_inode(our_pid, "mnt").unwrap();
    let child_mntns = read_ns_inode(child_pid, "mnt").unwrap();
    assert_ne!(
        our_mntns, child_mntns,
        "qrab-init should be in a different mount namespace"
    );

    vm.shutdown().await;
}

#[tokio::test]
async fn qrab_init_runs_as_uid_zero() {
    if !require_x86_64() {
        return;
    }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap()
        .start()
        .await
        .unwrap();

    let child_pid = find_qrab_init_pid().expect("qrab-init process not found");

    // From host perspective, /proc shows the REAL uid (not namespace uid).
    // Verify uid_map confirms the namespace mapping: "0 {real_uid} 1"
    // uid_map format is space-padded columns: "         0       1002          1"
    // Normalize whitespace for comparison.
    let uid_map = fs::read_to_string(format!("/proc/{child_pid}/uid_map"))
        .expect("failed to read uid_map");
    let uid_fields: Vec<&str> = uid_map.split_whitespace().collect();
    let real_uid = nix::unistd::Uid::current().as_raw();
    assert_eq!(
        uid_fields,
        vec!["0", &real_uid.to_string(), "1"],
        "uid_map should map ns uid 0 → real uid {real_uid}"
    );

    // Also verify gid_map
    let gid_map = fs::read_to_string(format!("/proc/{child_pid}/gid_map"))
        .expect("failed to read gid_map");
    let gid_fields: Vec<&str> = gid_map.split_whitespace().collect();
    let real_gid = nix::unistd::Gid::current().as_raw();
    assert_eq!(
        gid_fields,
        vec!["0", &real_gid.to_string(), "1"],
        "gid_map should map ns gid 0 → real gid {real_gid}"
    );

    vm.shutdown().await;
}

#[tokio::test]
async fn signal_forwarding_terminates_vm() {
    if !require_x86_64() {
        return;
    }
    let vm = VmBuilder::new(Arch::X86_64, MachineConfig::None)
        .spawn()
        .await
        .unwrap()
        .start()
        .await
        .unwrap();

    // Verify running.
    let status: qapi::StatusInfo = vm.execute(&qapi::commands::QueryStatus {}).await.unwrap();
    assert!(status.running);

    let child_pid = find_qrab_init_pid().expect("qrab-init process not found");

    // Send SIGTERM to qrab-init — it should forward to QEMU.
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(child_pid as i32),
        nix::sys::signal::Signal::SIGTERM,
    )
    .unwrap();

    // Wait for exit via exit_watch.
    let mut watch = vm.exit_watch();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match tokio::time::timeout_at(deadline, watch.changed()).await {
            Ok(Ok(())) => {
                if watch.borrow().is_some() {
                    break; // Process exited.
                }
            }
            _ => panic!("qrab-init did not exit within 5s after SIGTERM"),
        }
    }
}
