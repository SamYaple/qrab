//! KVM_CAP_READONLY_MEM per-VM-type probe.
//!
//! Pins the claim in `llm-rag/issues/sev-snp-pflash.md` that `KVM_CAP_READONLY_MEM`
//! returns `0` for SEV-SNP VMs (`KVM_CREATE_VM(type=3)`) even though the kernel
//! advertises it globally. Previously only a one-shot Python snippet documented
//! this.
//!
//! Skips on hosts without `/dev/kvm` or without SEV-SNP (type 3 unsupported).
//!
//! Reference: Linux KVM ioctl numbers in `include/uapi/linux/kvm.h`.
//! `KVMIO = 0xae`, `KVM_CREATE_VM = _IO(KVMIO, 0x01) = 0xae01`,
//! `KVM_CHECK_EXTENSION = _IO(KVMIO, 0x03) = 0xae03`,
//! `KVM_CAP_READONLY_MEM = 81`.

use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

use nix::ioctl_write_int_bad;

ioctl_write_int_bad!(kvm_create_vm, 0xae01);
ioctl_write_int_bad!(kvm_check_extension, 0xae03);

const KVM_CAP_READONLY_MEM: i32 = 81;

const TYPE_NORMAL: i32 = 0;
const TYPE_SEV: i32 = 1;
const TYPE_SEV_ES: i32 = 2;
const TYPE_SEV_SNP: i32 = 3;

fn open_kvm() -> Option<OwnedFd> {
    let fd = match std::fs::OpenOptions::new().read(true).write(true).open("/dev/kvm") {
        Ok(f) => f,
        Err(e) => {
            eprintln!("SKIP: cannot open /dev/kvm: {e}");
            return None;
        }
    };
    Some(OwnedFd::from(fd))
}

/// Helper: create a VM of the given type, check the capability, close the VM.
///
/// Returns `None` when `KVM_CREATE_VM` rejects the type (e.g. type 3 on a
/// non-SNP host).
fn check_cap(kvm: &OwnedFd, vm_type: i32, cap: i32) -> Option<i32> {
    let vm_fd = unsafe { kvm_create_vm(kvm.as_raw_fd(), vm_type) };
    let vm_fd = match vm_fd {
        Ok(fd) => fd,
        Err(e) => {
            eprintln!("KVM_CREATE_VM(type={vm_type}) → {e} (skipping this type)");
            return None;
        }
    };
    // Wrap so it's dropped/closed on scope exit.
    let vm = unsafe { OwnedFd::from_raw_fd(vm_fd) };
    let r = unsafe { kvm_check_extension(vm.as_raw_fd(), cap) }.ok()?;
    Some(r)
}

/// Non-SNP types (0, 1, 2) must report `KVM_CAP_READONLY_MEM = 1`.
///
/// Passes on any host with `/dev/kvm`. Tests that are unsupported on the host
/// (e.g. type 1 on Intel) are skipped gracefully.
#[test]
fn kvm_cap_readonly_mem_non_snp_types_support_it() {
    let kvm = match open_kvm() { Some(f) => f, None => return };

    // Type 0 (normal) must always support readonly slots on any modern kernel.
    match check_cap(&kvm, TYPE_NORMAL, KVM_CAP_READONLY_MEM) {
        Some(r) => assert_eq!(
            r, 1,
            "KVM_CAP_READONLY_MEM for normal VM (type 0) should be 1, got {r}"
        ),
        None => panic!("KVM_CREATE_VM(type=0) must succeed on any host with /dev/kvm"),
    }

    // SEV (type 1) and SEV-ES (type 2): only check if the host supports them.
    for t in [TYPE_SEV, TYPE_SEV_ES] {
        if let Some(r) = check_cap(&kvm, t, KVM_CAP_READONLY_MEM) {
            assert_eq!(
                r, 1,
                "KVM_CAP_READONLY_MEM for VM type {t} should be 1, got {r}"
            );
        } else {
            eprintln!("SKIP: VM type {t} unsupported on this host");
        }
    }
}

/// SEV-SNP (type 3) must report `KVM_CAP_READONLY_MEM = 0`.
///
/// This is the claim from `issues/sev-snp-pflash.md` that used to live only in
/// a Python snippet. Fails loudly if the invariant ever flips — at that point
/// the pflash ban for SNP no longer applies and the doc should be revisited.
#[test]
fn kvm_cap_readonly_mem_snp_type_returns_zero() {
    let kvm = match open_kvm() { Some(f) => f, None => return };

    match check_cap(&kvm, TYPE_SEV_SNP, KVM_CAP_READONLY_MEM) {
        Some(r) => {
            assert_eq!(
                r, 0,
                "KVM_CAP_READONLY_MEM for SEV-SNP (type 3) expected 0, got {r}. \
                 If this flipped to 1, the pflash-on-SNP restriction documented \
                 in llm-rag/issues/sev-snp-pflash.md may no longer hold — re-audit."
            );
            eprintln!("VERIFIED: KVM_CAP_READONLY_MEM = 0 for SNP VM (type 3)");
        }
        None => {
            eprintln!("SKIP: KVM_CREATE_VM(type=3) failed — SEV-SNP not available on this host");
        }
    }
}
