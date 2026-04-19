//! UEFI-side wrapper over the SEV-SNP `SVM_VMGEXIT_GUEST_REQUEST`
//! path.
//!
//! OVMF registers a 4 KiB GHCB page with the hypervisor in SEC
//! phase; its physical address is stored in the `GHCB_MSR`
//! (`0xC001_0130`). This module reads the PA, projects the first
//! 0x400 bytes as the save area, delegates the field / bitmap
//! writes to [`qrab_snp_proto::ghcb::prepare_guest_request`], issues
//! `rep; vmmcall` (the VMGEXIT encoding on SEV-ES / SEV-SNP), and
//! parses the result via
//! [`qrab_snp_proto::ghcb::parse_guest_request_result`].
//!
//! Everything that is easy to get wrong (bitmap bit placement,
//! slot offsets, `SW_EXIT_INFO_2` interpretation) lives in
//! `qrab-snp-proto` with host-side unit tests. The only parts that
//! require SEV-SNP hardware are the `rdmsr` + `rep; vmmcall`
//! instructions.

#![allow(unsafe_code)]

use core::arch::asm;

use qrab_snp_proto::ghcb::{
    GHCB_MSR, GHCB_SAVE_AREA_SIZE, GhcbError, SNP_GUEST_VMM_ERR_INVALID_LEN,
    parse_guest_request_result, prepare_ext_guest_request, prepare_guest_request,
    read_rbx, unpack_ext_error,
};

/// Low 12 bits of `GHCB_MSR` hold a function code used during early
/// boot (MSR protocol). Once OVMF has registered the full GHCB page
/// those bits are 0 and the rest is the GHCB PA. Mask both for
/// safety — reading a non-zero function code here would indicate
/// OVMF never finished the registration.
const GHCB_PA_MASK: u64 = !0xFFF;

/// Read `msr` via `rdmsr`. `msr` goes in `ECX`; the 64-bit result
/// is split across `EDX:EAX`.
///
/// # Safety
///
/// `rdmsr` traps to the hypervisor under SEV-ES (and is intercepted
/// by the microcode on SEV-SNP, delivered as `#VC` / handled by
/// OVMF's `CcExitLib`). Calling it before the guest has registered
/// a GHCB is undefined.  Callers must gate on
/// `ghcb::SevFeatures::sev_es` or stronger; qrab-boot does so
/// transitively by only invoking this fn from the SEV-SNP path.
unsafe fn read_msr(msr: u32) -> u64 {
    let hi: u32;
    let lo: u32;
    // SAFETY: `rdmsr` reads an MSR into `EDX:EAX`; no memory is
    // touched and flags are preserved. The caller upholds the
    // pre-conditions documented above.
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("edx") hi,
            out("eax") lo,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((hi as u64) << 32) | (lo as u64)
}

/// Write `value` to `msr` via `wrmsr` (`msr` in ECX, value in EDX:EAX).
///
/// # Safety
///
/// `wrmsr` is a privileged operation; callers must have already
/// verified SEV / SEV-ES. We only use this to point `GHCB_MSR`
/// back at the GHCB PA before `vmgexit` — intervening boot-service
/// VMGEXITs (IOIO for serial writes, CPUID, etc.) may leave the
/// MSR holding a GHCB-MSR-protocol response value rather than the
/// plain GHCB PA, which causes the hypervisor to misinterpret our
/// GUEST_REQUEST exit.
unsafe fn write_msr(msr: u32, value: u64) {
    let hi = (value >> 32) as u32;
    let lo = value as u32;
    // SAFETY: `wrmsr` writes an MSR from `EDX:EAX`; no memory is
    // touched and flags are preserved.
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("edx") hi,
            in("eax") lo,
            options(nomem, nostack, preserves_flags),
        );
    }
}

/// Emit the `rep; vmmcall` sequence. Under SEV-ES / SEV-SNP
/// microcode reinterprets this as the VMGEXIT instruction.
///
/// # Safety
///
/// Trap-based entry to the hypervisor; must be called only after
/// the caller has populated the GHCB save area with a valid
/// exit code + VALID_BITMAP. Called exclusively from
/// [`guest_request`] after `prepare_guest_request` succeeds.
unsafe fn vmgexit() {
    // SAFETY: `rep; vmmcall` is the defined VMGEXIT encoding on
    // SEV-ES/SEV-SNP. The hypervisor *reads* the GHCB save area we
    // just populated via `prepare_guest_request` and *writes* the
    // response (SW_EXIT_INFO_2, VALID_BITMAP bits) back into the
    // same page, so this asm block *must not* be `nomem` — with
    // `nomem` LLVM may treat the pre-vmgexit save-area writes as
    // dead and elide them. Observed symptom on EPYC 9354 + SNP +
    // Linux 6.18: host dmesg prints
    //   kvm_amd: vcpu0, guest rIP: 0x0 vmgexit: exit code 0x7b
    //   input is not valid
    // where 0x7b is `SVM_EXIT_EFER_WRITE_TRAP` (stale data from
    // OVMF's last VMGEXIT) rather than the
    // `SVM_VMGEXIT_GUEST_REQUEST = 0x8000_0011` we *thought* we
    // wrote. Dropping `nomem` forces LLVM to flush the save-area
    // stores before the `rep; vmmcall` executes. `nostack` stays
    // because we truly don't touch the stack; flags are not
    // preserved across VMGEXIT so `preserves_flags` is dropped.
    unsafe {
        asm!(
            "rep; vmmcall",
            options(nostack),
        );
    }
}

/// Issue one `SVM_VMGEXIT_GUEST_REQUEST` to the hypervisor with
/// `req_pa` as the request-envelope GPA and `resp_pa` as the
/// response-envelope GPA. Both must be:
///
///   * 4 KiB-aligned,
///   * allocated from the PSP-shared pool (on SEV-SNP that means
///     via `pci_dma::allocate_buffer`, which routes through
///     `IoMmuAllocateBuffer` and clears the C-bit),
///   * live for the duration of the call (the hypervisor reads
///     `req_pa` and writes into `resp_pa` during the VMGEXIT).
///
/// Returns:
///
///   * `Ok(())` if the GHCB's `SW_EXIT_INFO_2` is `0` on return
///     and the VALID_BITMAP bit for the slot is set.
///   * `Err(GhcbError::PspError(status))` if `SW_EXIT_INFO_2` is
///     non-zero (PSP-defined status code — 0x16 = INVALID_PARAMETER,
///     0x27 = INVALID_KEY, etc).
///   * `Err(GhcbError::ResultBitNotSet)` if the VALID_BITMAP bit
///     was never set — the hypervisor rejected the exit before the
///     PSP could process it.
pub fn guest_request(req_pa: u64, resp_pa: u64) -> Result<(), GhcbError> {
    // SAFETY: `GHCB_MSR` is unprivileged-safe to read on AMD CPUs
    // that support SEV; callers have already verified
    // `SevFeatures::sev_snp`.
    let ghcb_msr_raw = unsafe { read_msr(GHCB_MSR) };
    let ghcb_pa = ghcb_msr_raw & GHCB_PA_MASK;
    // NOTE: we intentionally do *not* log until after vmgexit has
    // finished — the OVMF serial console itself issues an IOIO
    // VMGEXIT through the same GHCB, which would overwrite the
    // SW_EXIT_CODE / SW_EXIT_INFO_{1,2} slots we are about to
    // populate.

    // SAFETY: `ghcb_pa` points at the 4 KiB GHCB page OVMF
    // registered during SEC; the first 0x400 bytes are the save
    // area. UEFI identity-maps this PA, and no other code in
    // qrab-boot touches the save area between the writes we emit
    // here and the `rep; vmmcall` that consumes them.
    let save_area = unsafe {
        &mut *(ghcb_pa as *mut [u8; GHCB_SAVE_AREA_SIZE])
    };
    prepare_guest_request(save_area, req_pa, resp_pa);

    // GHCB footer: protocol_version (u16 @ 0xFFA) and ghcb_usage
    // (u32 @ 0xFFC) must be set on every VMGEXIT. KVM's
    // `sev_es_validate_vmgexit` checks `protocol_version` against
    // the range it negotiated with the guest via the GHCB MSR
    // info-request protocol; OVMF rewrites these every VMGEXIT via
    // `CcExitLib`. We re-stamp so our exit is not at OVMF's mercy.
    // `protocol_version = 2` is the SEV-SNP mandatory minimum
    // (AMD GHCB spec v2); `ghcb_usage = 0` = `GHCB_DEFAULT_USAGE`
    // ("use the save area, not a usage-specific protocol").
    //
    // SAFETY: raw writes via ghcb_pa to avoid the borrow-checker
    // conflict with `save_area` (same underlying page, different
    // non-overlapping byte ranges).
    unsafe {
        (ghcb_pa as *mut u16).byte_add(0xFFA).write_volatile(2);
        (ghcb_pa as *mut u32).byte_add(0xFFC).write_volatile(0);
    }

    // Re-point GHCB_MSR at our ghcb_pa immediately before vmgexit.
    // Intervening UEFI boot-service activity (serial writes, CPUID,
    // etc.) uses the same GHCB via OVMF's CcExitLib, which leaves
    // the MSR holding a GHCB-MSR-protocol response code rather than
    // the plain PA. KVM's SNP_GUEST_REQUEST handler reads the MSR
    // to locate the GHCB; a stale response value there makes it
    // misinterpret our exit (observed as info2 = ghcb_pa + 0x800,
    // which is the "Shared Buffer" field in GHCB v2 that KVM was
    // expecting to parse instead of our SW_EXIT_INFO_2). This
    // mirrors the Linux kernel's per-VMGEXIT `wrmsr(GHCB_MSR,
    // ghcb_pa)` in arch/x86/coco/sev/core.c.
    //
    // SAFETY: `ghcb_pa` is the PA we read from the same MSR a few
    // lines earlier, so writing it back is a no-op in the "no
    // intervening use" case and a correction in the "intervening
    // use" case.
    // Snapshot SW_EXIT_CODE + info1 + info2 BEFORE vmgexit so we
    // can spot LLVM elision, MSR-rewrite ordering issues, or any
    // OVMF VMGEXIT that slipped in and clobbered the page.
    let pre_code = {
        let mut b = [0u8; 8];
        b.copy_from_slice(&save_area[0x390..0x398]);
        u64::from_le_bytes(b)
    };
    let pre_info1 = {
        let mut b = [0u8; 8];
        b.copy_from_slice(&save_area[0x398..0x3A0]);
        u64::from_le_bytes(b)
    };
    let pre_info2 = {
        let mut b = [0u8; 8];
        b.copy_from_slice(&save_area[0x3A0..0x3A8]);
        u64::from_le_bytes(b)
    };

    unsafe { write_msr(GHCB_MSR, ghcb_pa); }

    // SAFETY: we just populated SW_EXIT_CODE + the two info slots
    // plus their VALID_BITMAP bits; the hypervisor now has enough
    // information to execute the guest-request exit.
    unsafe { vmgexit(); }

    // Snapshot the hypervisor-written fields *before* emitting any
    // log calls. Each `log::info!` call ends up issuing an IOIO
    // VMGEXIT for the UART write, and that VMGEXIT reuses the same
    // GHCB save area — so any log call between `rep; vmmcall` and
    // `parse_guest_request_result` would overwrite the guest-request
    // result and leave us parsing zeros (observed as a false-success
    // followed by `Envelope::HeaderMismatch` downstream).
    let info1_after = {
        let mut b = [0u8; 8];
        b.copy_from_slice(&save_area[0x398..0x3A0]);
        u64::from_le_bytes(b)
    };
    let info2_after = {
        let mut b = [0u8; 8];
        b.copy_from_slice(&save_area[0x3A0..0x3A8]);
        u64::from_le_bytes(b)
    };
    let result = parse_guest_request_result(save_area);

    // Now that `result` carries the hypervisor's answer, it is safe
    // to log — subsequent VMGEXITs may clobber the save area.
    log::info!(
        "QRAB_SNP_GHCB ghcb_msr_raw={:#x} ghcb_pa={:#x} req_pa={:#x} resp_pa={:#x}",
        ghcb_msr_raw, ghcb_pa, req_pa, resp_pa,
    );
    log::info!(
        "QRAB_SNP_GHCB_POST pre_code={:#x} pre_info1={:#x} pre_info2={:#x} info1={:#x} info2={:#x}",
        pre_code, pre_info1, pre_info2, info1_after, info2_after,
    );

    result
}

/// Extended variant of [`guest_request`]: issues
/// `SVM_VMGEXIT_EXT_GUEST_REQUEST` (`0x8000_0012`) so the hypervisor
/// also fills a caller-provided run of cert-chain data pages. On
/// `SNP_GUEST_VMM_ERR_INVALID_LEN` the hypervisor writes the
/// required page count back into RBX — we decode that and surface
/// it as [`GhcbError::InvalidLen { required_pages }`]; the caller
/// is expected to realloc and retry.
pub fn guest_request_extended(
    req_pa: u64,
    resp_pa: u64,
    data_pa: u64,
    data_pages: u64,
) -> Result<(), GhcbError> {
    // SAFETY: same preconditions as `guest_request` — caller has
    // gated on `SevFeatures::sev_snp`, so `rdmsr(GHCB_MSR)` is
    // legal and the returned PA is a mapped 4 KiB page.
    let ghcb_msr_raw = unsafe { read_msr(GHCB_MSR) };
    let ghcb_pa = ghcb_msr_raw & GHCB_PA_MASK;

    // SAFETY: `ghcb_pa` is the GHCB registered by OVMF at SEC; the
    // first 0x400 B are the save area. See `guest_request` for the
    // equivalent SAFETY argument. Re-borrow pattern mirrors the
    // basic-variant fn.
    let save_area = unsafe { &mut *(ghcb_pa as *mut [u8; GHCB_SAVE_AREA_SIZE]) };
    prepare_ext_guest_request(save_area, req_pa, resp_pa, data_pa, data_pages);

    // Footer / MSR restamp — identical rationale to `guest_request`.
    unsafe {
        (ghcb_pa as *mut u16).byte_add(0xFFA).write_volatile(2);
        (ghcb_pa as *mut u32).byte_add(0xFFC).write_volatile(0);
    }

    let pre_code = {
        let mut b = [0u8; 8];
        b.copy_from_slice(&save_area[0x390..0x398]);
        u64::from_le_bytes(b)
    };

    unsafe { write_msr(GHCB_MSR, ghcb_pa); }
    unsafe { vmgexit(); }

    // Snapshot the four interesting slots before any logging clobbers
    // them via a piggybacking IOIO VMGEXIT.
    let info1 = {
        let mut b = [0u8; 8];
        b.copy_from_slice(&save_area[0x398..0x3A0]);
        u64::from_le_bytes(b)
    };
    let info2 = {
        let mut b = [0u8; 8];
        b.copy_from_slice(&save_area[0x3A0..0x3A8]);
        u64::from_le_bytes(b)
    };
    let rbx_out = read_rbx(save_area);

    let result = if info1 != 0 {
        Err(GhcbError::VmmError(info1))
    } else if info2 != 0 {
        // Extended path packs `(vmm_err, fw_err)` into info2. A
        // bare PSP error has vmm_err=0; an `INVALID_LEN` has
        // vmm_err=1 and the required page count in RBX.
        let (vmm_err, fw_err) = unpack_ext_error(info2);
        if vmm_err as u64 == SNP_GUEST_VMM_ERR_INVALID_LEN {
            Err(GhcbError::InvalidLen { required_pages: rbx_out })
        } else if vmm_err != 0 {
            // Surface other VMM errors (e.g. BUSY=2) as VmmError
            // so the caller can distinguish; the 32-bit value is
            // widened back for symmetry with the basic path.
            Err(GhcbError::VmmError(vmm_err as u64))
        } else {
            Err(GhcbError::PspError(fw_err as u64))
        }
    } else {
        // On success the hypervisor writes the actual bytes-filled
        // count into RBX (in bytes, not pages). We do not surface
        // that here because the caller already framed the data
        // buffer in complete pages; the `cert_table` parser ignores
        // trailing zeros past the terminator entry.
        Ok(())
    };

    log::info!(
        "QRAB_SNP_GHCB_EXT ghcb_pa={:#x} req_pa={:#x} resp_pa={:#x} data_pa={:#x} data_pages={}",
        ghcb_pa, req_pa, resp_pa, data_pa, data_pages,
    );
    log::info!(
        "QRAB_SNP_GHCB_EXT_POST pre_code={:#x} info1={:#x} info2={:#x} rbx_out={}",
        pre_code, info1, info2, rbx_out,
    );

    result
}
