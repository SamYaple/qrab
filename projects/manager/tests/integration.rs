//! Low-level manager integration tests.
//!
//! Most of this file was built around `MachineConfig::None` to exercise
//! QEMU / QMP plumbing against a `-machine none` QEMU with no firmware,
//! no accelerator, no guest. That fixture was removed in the
//! scope-tightening round that made qrab VMs CoCo-only
//! (agent/svsm / Phase 3c): there is no "bare QEMU" path on the
//! supported surface anymore. The VM-lifecycle assertions that test
//! required — spawn → preconfig → cont → shutdown, multi-command
//! execution, STOP event propagation — are also exercised end-to-end
//! by `projects/manager/tests/sev_negative.rs` on every SEV-capable
//! host, by the BMC smokes, and by `contrib/matrix-smoke.sh`.
//!
//! This file retains only the assertions that do NOT need a spawned
//! QEMU: version detection, arch availability, and the
//! `PRECONFIG_MIN_VERSION` const sanity check.

use manager::arch::detect_version;
use manager::{Arch, PRECONFIG_MIN_VERSION};

fn require_x86_64() -> bool {
    if !Arch::X86_64.is_available() {
        eprintln!("SKIP: qemu-system-x86_64 not available");
        return false;
    }
    true
}

#[tokio::test]
async fn detect_qemu_version() {
    if !require_x86_64() {
        return;
    }
    let version = detect_version(Arch::X86_64);
    assert!(version.is_some(), "Should detect QEMU version");
    let (major, minor, _patch) = version.unwrap();
    assert!(major >= 2, "Expected QEMU >= 2.0, got {}.{}", major, minor);
}

#[tokio::test]
async fn arch_availability() {
    // X86_64 should be available in our test environment
    assert!(Arch::X86_64.is_available());
    let binary = Arch::X86_64.binary();
    assert!(
        !binary.as_os_str().is_empty(),
        "Binary path should not be empty"
    );
}

#[tokio::test]
async fn preconfig_min_version_constant() {
    // Verify the constant matches expected QEMU 7.2.0 (KVM fd passing + blockdev preconfig)
    assert_eq!(PRECONFIG_MIN_VERSION, (7, 2, 0));
}
