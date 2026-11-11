//! KVM_CAP_READONLY_MEM per-VM-type probe.
//!
//! Historical context: `llm-rag/issues/sev-snp-pflash.md` used to claim
//! that `KVM_CAP_READONLY_MEM` returns `0` when the VM is created as
//! SEV-SNP, and the supporting Python snippet passed `KVM_CREATE_VM(3)`.
//! On kernel 6.18 the actual `KVM_X86_*` enum (from
//! `arch/x86/include/uapi/asm/kvm.h`) is:
//!
//! ```text
//! KVM_X86_DEFAULT_VM       = 0
//! KVM_X86_SW_PROTECTED_VM  = 1
//! KVM_X86_SEV_VM           = 2
//! KVM_X86_SEV_ES_VM        = 3
//! KVM_X86_SNP_VM           = 4
//! ```
//!
//! So the old snippet's "type 3" was actually **SEV-ES**, not SEV-SNP.
//! This module uses the correct ordinals.
//!
//! `KVMIO = 0xae`, `KVM_CREATE_VM = _IO(KVMIO, 0x01) = 0xae01`,
//! `KVM_CHECK_EXTENSION = _IO(KVMIO, 0x03) = 0xae03`,
//! `KVM_CAP_READONLY_MEM = 81`, `KVM_CAP_VM_TYPES = 235`.
//!
//! Skips on hosts without `/dev/kvm`.

use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;

use nix::ioctl_write_int_bad;

ioctl_write_int_bad!(kvm_create_vm, 0xae01);
ioctl_write_int_bad!(kvm_check_extension, 0xae03);

const KVM_CAP_READONLY_MEM: i32 = 81;
const KVM_CAP_VM_TYPES: i32 = 235;

const KVM_X86_DEFAULT_VM: i32 = 0;
const KVM_X86_SW_PROTECTED_VM: i32 = 1;
const KVM_X86_SEV_VM: i32 = 2;
const KVM_X86_SEV_ES_VM: i32 = 3;
const KVM_X86_SNP_VM: i32 = 4;

fn open_kvm() -> Option<OwnedFd> {
    let fd = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/kvm")
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("SKIP: cannot open /dev/kvm: {e}");
            return None;
        }
    };
    Some(OwnedFd::from(fd))
}

/// Query which VM types the host KVM advertises as creatable, as a bitmask.
/// `KVM_CHECK_EXTENSION(KVM_CAP_VM_TYPES)` returns a `u64` bitmask where
/// `(1 << n)` is set for every supported `KVM_X86_*_VM` ordinal.
fn supported_vm_type_mask(kvm: &OwnedFd) -> u64 {
    // KVM_CAP_VM_TYPES is queried at the system level (not per-VM), so we
    // call KVM_CHECK_EXTENSION directly on the /dev/kvm fd.
    let r = unsafe { kvm_check_extension(kvm.as_raw_fd(), KVM_CAP_VM_TYPES) };
    match r {
        Ok(mask) => mask as u64,
        Err(_) => {
            // Older kernels don't export this cap; fall back to "default VM
            // is always supported".
            1u64 << KVM_X86_DEFAULT_VM
        }
    }
}

/// Try to create a VM of the given type and read the per-VM capability.
/// Returns `None` when `KVM_CREATE_VM` rejects the type (e.g. host without
/// SNP support).
fn check_cap(kvm: &OwnedFd, vm_type: i32, cap: i32) -> Option<i32> {
    let vm_fd = unsafe { kvm_create_vm(kvm.as_raw_fd(), vm_type) };
    let vm_fd = match vm_fd {
        Ok(fd) => fd,
        Err(e) => {
            eprintln!("KVM_CREATE_VM(type={vm_type}) → {e} (skipping this type)");
            return None;
        }
    };
    let vm = unsafe { OwnedFd::from_raw_fd(vm_fd) };
    let r = unsafe { kvm_check_extension(vm.as_raw_fd(), cap) }.ok()?;
    Some(r)
}

/// Supported VM types must include default. On an SEV-SNP-capable host the
/// mask should also include SEV, SEV-ES and SNP.
#[test]
fn kvm_advertises_vm_types_bitmask() {
    let kvm = match open_kvm() {
        Some(f) => f,
        None => return,
    };
    let mask = supported_vm_type_mask(&kvm);
    eprintln!("KVM_CAP_VM_TYPES mask = 0x{mask:x}");

    assert!(
        mask & (1u64 << KVM_X86_DEFAULT_VM) != 0,
        "default VM type (0) must always be supported; mask=0x{mask:x}"
    );

    // On the dev host the expected full mask is 0x1f (bits 0..4). Log the
    // individual bits so regressions are readable.
    for (name, ord) in [
        ("DEFAULT_VM", KVM_X86_DEFAULT_VM),
        ("SW_PROTECTED_VM", KVM_X86_SW_PROTECTED_VM),
        ("SEV_VM", KVM_X86_SEV_VM),
        ("SEV_ES_VM", KVM_X86_SEV_ES_VM),
        ("SNP_VM", KVM_X86_SNP_VM),
    ] {
        let on = mask & (1u64 << ord) != 0;
        eprintln!("  {name} ({ord}) = {}", if on { "supported" } else { "—" });
    }
}

/// Non-CoCo VM types must report `KVM_CAP_READONLY_MEM = 1` on any modern
/// kernel. Types the host doesn't support are skipped gracefully.
#[test]
fn kvm_cap_readonly_mem_non_coco_types_support_it() {
    let kvm = match open_kvm() {
        Some(f) => f,
        None => return,
    };

    // Default VM is non-negotiable — if this fails the kernel is broken.
    match check_cap(&kvm, KVM_X86_DEFAULT_VM, KVM_CAP_READONLY_MEM) {
        Some(r) => assert_eq!(
            r, 1,
            "KVM_CAP_READONLY_MEM for KVM_X86_DEFAULT_VM (0) should be 1, got {r}"
        ),
        None => panic!("KVM_CREATE_VM(type=0) must succeed on any host with /dev/kvm"),
    }

    // SW-protected VMs opt into the same memslot model as default VMs,
    // so readonly memory should still be supported when the type exists.
    if let Some(r) = check_cap(&kvm, KVM_X86_SW_PROTECTED_VM, KVM_CAP_READONLY_MEM) {
        assert_eq!(
            r, 1,
            "KVM_CAP_READONLY_MEM for KVM_X86_SW_PROTECTED_VM (1) should be 1, got {r}"
        );
    } else {
        eprintln!("SKIP: KVM_X86_SW_PROTECTED_VM not supported on this host");
    }
}

/// SEV (type 2) must still report `KVM_CAP_READONLY_MEM = 1` — SEV does not
/// use the Reverse Map Table, so readonly memslots are still valid.
#[test]
fn kvm_cap_readonly_mem_sev_type_returns_one() {
    let kvm = match open_kvm() {
        Some(f) => f,
        None => return,
    };

    match check_cap(&kvm, KVM_X86_SEV_VM, KVM_CAP_READONLY_MEM) {
        Some(r) => {
            assert_eq!(
                r, 1,
                "KVM_CAP_READONLY_MEM for KVM_X86_SEV_VM (2) expected 1, got {r}. \
                 If this flipped, the pflash-on-SEV claim in sev-snp-pflash.md \
                 may need revisiting."
            );
            eprintln!("VERIFIED: KVM_CAP_READONLY_MEM = 1 for SEV VM (type 2)");
        }
        None => {
            eprintln!("SKIP: KVM_CREATE_VM(type=2 SEV) failed — SEV not available on this host");
        }
    }
}

/// SEV-ES (type 3) behaviour. sev-snp-pflash.md's original Python snippet
/// called this "SEV-SNP" and reported cap=0. Record the real value so the
/// doc can be corrected against the test.
#[test]
fn kvm_cap_readonly_mem_sev_es_type_recorded() {
    let kvm = match open_kvm() {
        Some(f) => f,
        None => return,
    };

    match check_cap(&kvm, KVM_X86_SEV_ES_VM, KVM_CAP_READONLY_MEM) {
        Some(r) => {
            eprintln!(
                "OBSERVED: KVM_CAP_READONLY_MEM = {r} for KVM_X86_SEV_ES_VM (3) \
                 on this host. sev-snp-pflash.md Python snippet mis-labelled \
                 this type as SEV-SNP."
            );
            // No hard assertion on the value yet — this test exists to
            // capture whichever behaviour the current kernel has. If the
            // value changes, CI logs preserve history.
            assert!(
                r == 0 || r == 1,
                "KVM_CAP_READONLY_MEM returned unexpected value {r} (not 0 or 1) \
                 for KVM_X86_SEV_ES_VM"
            );
        }
        None => {
            eprintln!("SKIP: KVM_CREATE_VM(type=3 SEV-ES) failed");
        }
    }
}

/// The real SEV-SNP VM type is 4, not 3. Records the observed
/// `KVM_CAP_READONLY_MEM` so the doc can cite a real test run.
#[test]
fn kvm_cap_readonly_mem_snp_type_recorded() {
    let kvm = match open_kvm() {
        Some(f) => f,
        None => return,
    };

    let mask = supported_vm_type_mask(&kvm);
    if mask & (1u64 << KVM_X86_SNP_VM) == 0 {
        eprintln!(
            "SKIP: KVM_X86_SNP_VM (4) not in KVM_CAP_VM_TYPES mask (0x{mask:x}) — \
             host lacks SEV-SNP support"
        );
        return;
    }

    match check_cap(&kvm, KVM_X86_SNP_VM, KVM_CAP_READONLY_MEM) {
        Some(r) => {
            eprintln!(
                "OBSERVED: KVM_CAP_READONLY_MEM = {r} for KVM_X86_SNP_VM (4) \
                 on this host."
            );
            assert!(
                r == 0 || r == 1,
                "KVM_CAP_READONLY_MEM returned unexpected value {r} (not 0 or 1) \
                 for KVM_X86_SNP_VM"
            );
            if r == 0 {
                eprintln!(
                    "  pflash-on-SNP claim in sev-snp-pflash.md holds: SNP VMs \
                     cannot use readonly memslots."
                );
            } else {
                eprintln!(
                    "  Unexpected: SNP now reports readonly-mem = 1. pflash \
                     rejection may be driven by a different constraint — \
                     re-audit the sev-snp-pflash.md explanation."
                );
            }
        }
        None => {
            eprintln!(
                "SKIP: KVM_CREATE_VM(type=4 SNP) rejected — SNP may need \
                 extra host config (e.g. kvm_amd.sev_snp=1 and firmware)"
            );
        }
    }
}
