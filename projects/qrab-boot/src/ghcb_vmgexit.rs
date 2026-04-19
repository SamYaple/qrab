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
    GHCB_MSR, GHCB_SAVE_AREA_SIZE, GhcbError, parse_guest_request_result,
    prepare_guest_request,
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
    // SEV-ES/SEV-SNP. No memory effect observable to Rust through
    // this instruction; the GHCB mutations happen via the shared
    // page that `prepare_guest_request` already wrote through a
    // `&mut [u8]` that the caller guarantees is still live.
    unsafe {
        asm!(
            "rep; vmmcall",
            options(nomem, nostack, preserves_flags),
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
    let ghcb_pa = unsafe { read_msr(GHCB_MSR) } & GHCB_PA_MASK;

    // SAFETY: `ghcb_pa` points at the 4 KiB GHCB page OVMF
    // registered during SEC; the first 0x400 bytes are the save
    // area.  UEFI identity-maps this PA, and no other code in
    // qrab-boot touches the save area between the writes we emit
    // here and the `rep; vmmcall` that consumes them.
    let save_area = unsafe {
        &mut *(ghcb_pa as *mut [u8; GHCB_SAVE_AREA_SIZE])
    };
    prepare_guest_request(save_area, req_pa, resp_pa);

    // SAFETY: we just populated SW_EXIT_CODE + the two info slots
    // plus their VALID_BITMAP bits; the hypervisor now has enough
    // information to execute the guest-request exit.
    unsafe { vmgexit(); }

    parse_guest_request_result(save_area)
}
