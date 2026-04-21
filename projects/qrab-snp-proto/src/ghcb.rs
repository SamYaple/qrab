//! Host-testable bits of the SEV-ES/SNP Guest Hypervisor
//! Communication Block (GHCB) protocol, plus the opcodes the guest
//! uses to issue `SVM_VMGEXIT_GUEST_REQUEST`.
//!
//! The GHCB is a 4 KiB guest-shared page that the SEC / PEI phase
//! of OVMF registers with the hypervisor via the GHCB MSR. The
//! low 0x400 bytes hold a "save area" of per-field QWORD slots;
//! bytes `0x3F0..0x400` are the VALID_BITMAP (128 bits, one per
//! 8-byte save-area slot) that tells the hypervisor which fields
//! the guest populated.
//!
//! For issuing the report request all we touch are:
//!
//!   * `SW_EXIT_CODE`    @ 0x398 = `SVM_VMGEXIT_GUEST_REQUEST`.
//!   * `SW_EXIT_INFO_1`  @ 0x3A0 = GPA of the encrypted request
//!     envelope (4 KiB `SnpGuestMsg`).
//!   * `SW_EXIT_INFO_2`  @ 0x3A8 = GPA of the response buffer
//!     (4 KiB `SnpGuestMsg`). The hypervisor / PSP writes the
//!     result (0 for success, non-zero PSP error) back into this
//!     slot on VMGEXIT return.
//!
//! Each of the three slots above has a corresponding VALID_BITMAP
//! bit that the guest must set before `rep; vmmcall`, and the
//! hypervisor sets the SW_EXIT_INFO_2 bit on return to indicate
//! the slot is populated.
//!
//! The UEFI side (qrab-boot) combines the pure-data helpers here
//! with an MSR read (for the GHCB PA) + inline-asm `rep; vmmcall`
//! to form the full VMGEXIT. Keeping the layout + bitmap logic
//! here means `cargo test -p qrab-snp-proto` can round-trip the
//! save-area fill/parse without SEV-SNP hardware.

/// `SW_EXIT_CODE` value for the SNP guest-request VMGEXIT
/// (AMD APM Vol. 3, §15.35.2).
pub const SVM_VMGEXIT_GUEST_REQUEST: u64 = 0x8000_0011;

/// `SW_EXIT_CODE` value for the *extended* SNP guest-request
/// VMGEXIT (A02): same envelope as the basic variant, but the
/// guest also hands the hypervisor a run of cert-chain data pages
/// via RAX (starting GPA) and RBX (page count). The hypervisor
/// fills those pages with an `snp_cert_table` blob (see the
/// [`cert_table`](crate::cert_table) module) containing the VCEK
/// or VLEK leaf + ASK + ARK DER certs it has cached.
///
/// If the hypervisor has no chain available, or the caller-supplied
/// page count is too small, KVM returns
/// `SNP_GUEST_VMM_ERR(SNP_GUEST_VMM_ERR_INVALID_LEN)` in
/// `SW_EXIT_INFO_2` with the required page count in RBX on exit —
/// the guest is expected to retry with a larger buffer.
pub const SVM_VMGEXIT_EXT_GUEST_REQUEST: u64 = 0x8000_0012;

/// MSR number for the GHCB's physical address on SEV-ES and
/// SEV-SNP (`GHCB_MSR`). Hypervisor registers the PA here during
/// boot; guests read it to locate the shared page.
pub const GHCB_MSR: u32 = 0xC001_0130;

/// Save-area region size (bytes). The GHCB is 4 KiB total; the
/// first 0x400 B is the save area, the rest is reserved and
/// vendor-version fields.
pub const GHCB_SAVE_AREA_SIZE: usize = 0x400;

/// Offset of `SW_EXIT_CODE` within the save area.
///
/// Matches Linux kernel `arch/x86/include/asm/svm.h`
/// `struct vmcb_save_area::sw_exit_code`. The previous value here
/// was 0x398, which is actually `sw_exit_info_1` — the off-by-8
/// silently swapped every field we wrote, causing KVM to see our
/// SVM_VMGEXIT_GUEST_REQUEST exit-code in info_1 and its own
/// `GHCB_ERR_MISSING_INPUT` reply landed where *we* read info_1.
pub const GHCB_SW_EXIT_CODE_OFFSET: usize = 0x390;

/// Offset of `SW_EXIT_INFO_1` within the save area.
pub const GHCB_SW_EXIT_INFO_1_OFFSET: usize = 0x398;

/// Offset of `SW_EXIT_INFO_2` within the save area.
pub const GHCB_SW_EXIT_INFO_2_OFFSET: usize = 0x3A0;

/// Offset of `RAX` within the save area. Per the GHCB save-area
/// layout (see `MdePkg/Include/Register/Amd/Ghcb.h` `GHCB_SAVE_AREA`
/// — matches Linux's `struct vmcb_save_area`): 203 bytes reserved +
/// 1 byte cpl + 300 bytes reserved = 0x1F8.
pub const GHCB_RAX_OFFSET: usize = 0x1F8;

/// Offset of `RBX` within the save area. Follows `RAX` + 264 B
/// reserved + `RCX` + `RDX` = 0x1F8 + 8 + 264 + 8 + 8 = 0x318.
pub const GHCB_RBX_OFFSET: usize = 0x318;

/// Offset of the VALID_BITMAP inside the save area (last 16 B).
pub const GHCB_VALID_BITMAP_OFFSET: usize = 0x3F0;

/// VALID_BITMAP length in bytes (128 bits).
pub const GHCB_VALID_BITMAP_LEN: usize = 0x10;

/// Each save-area slot is 8 bytes. Bit `offset / 8` of the
/// VALID_BITMAP corresponds to the slot at that offset.
pub const GHCB_SLOT_STRIDE: usize = 8;

/// Bit index within the VALID_BITMAP for `SW_EXIT_CODE`.
pub const GHCB_BIT_SW_EXIT_CODE: usize =
    GHCB_SW_EXIT_CODE_OFFSET / GHCB_SLOT_STRIDE;

/// Bit index within the VALID_BITMAP for `SW_EXIT_INFO_1`.
pub const GHCB_BIT_SW_EXIT_INFO_1: usize =
    GHCB_SW_EXIT_INFO_1_OFFSET / GHCB_SLOT_STRIDE;

/// Bit index within the VALID_BITMAP for `SW_EXIT_INFO_2`.
pub const GHCB_BIT_SW_EXIT_INFO_2: usize =
    GHCB_SW_EXIT_INFO_2_OFFSET / GHCB_SLOT_STRIDE;

/// Bit index within the VALID_BITMAP for `RAX`. Required by the
/// extended guest-request VMGEXIT so the hypervisor knows the
/// starting GPA of the cert-chain data pages.
pub const GHCB_BIT_RAX: usize = GHCB_RAX_OFFSET / GHCB_SLOT_STRIDE;

/// Bit index within the VALID_BITMAP for `RBX`. Required by the
/// extended guest-request VMGEXIT so the hypervisor knows the
/// page count.
pub const GHCB_BIT_RBX: usize = GHCB_RBX_OFFSET / GHCB_SLOT_STRIDE;

/// Shift the hypervisor uses to pack its own error code into the
/// high 32 bits of `SW_EXIT_INFO_2` (AMD GHCB spec §4.1.8, Linux
/// `arch/x86/include/asm/sev-common.h` `SNP_GUEST_VMM_ERR_SHIFT`).
/// The low 32 bits carry the PSP firmware error.
pub const SNP_GUEST_VMM_ERR_SHIFT: u64 = 32;

/// `SNP_GUEST_VMM_ERR_INVALID_LEN = 1`. The only VMM-layer error
/// the extended guest-request path can recover from: the cert-chain
/// data buffer is too small; retry with the page count RBX now
/// carries on return.
pub const SNP_GUEST_VMM_ERR_INVALID_LEN: u64 = 1;

/// `SNP_GUEST_VMM_ERR_BUSY = 2`. Transient; retry the full request.
pub const SNP_GUEST_VMM_ERR_BUSY: u64 = 2;

/// Distinguished errors from [`prepare_guest_request`] /
/// [`parse_guest_request_result`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhcbError {
    /// Hypervisor returned a non-zero `SW_EXIT_INFO_1`. The VMM
    /// rejected the exit before it reached the PSP (e.g. invalid
    /// request GPA, request/response page not in RMP-Shared state,
    /// GHCB version mismatch). Embedded `u64` is the VMM-defined
    /// code — `0x4` on KVM 6.18 means the pages weren't shared.
    VmmError(u64),
    /// Hypervisor returned a non-zero `SW_EXIT_INFO_2`. The embedded
    /// `u64` is the PSP-defined status code (0x16 =
    /// INVALID_PARAMETER, 0x27 = INVALID_KEY, etc.).
    PspError(u64),
    /// Extended guest-request only: VMM packed
    /// `SNP_GUEST_VMM_ERR_INVALID_LEN = 1` into the high 32 bits
    /// of `SW_EXIT_INFO_2`, meaning the cert-chain data pages the
    /// caller passed were too small. The embedded `u64` is the
    /// required page count the hypervisor wrote back into `RBX`
    /// — caller should re-allocate that many pages and retry the
    /// exit. Not produced by the basic [`SVM_VMGEXIT_GUEST_REQUEST`]
    /// path.
    InvalidLen { required_pages: u64 },
}

/// Set bit `bit` in `bitmap` (little-endian within each byte).
///
/// The GHCB is a hypervisor-shared page, so the read-modify-write
/// goes through `AtomicU8::fetch_or` with `Release` ordering — a
/// future AP-concurrency refactor or hypervisor-side reader cannot
/// observe a torn byte, and prior guest writes to save-area slots
/// happen-before the hypervisor observes the valid bit.
#[inline]
pub fn set_valid_bit(bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN], bit: usize) {
    use core::sync::atomic::{AtomicU8, Ordering};
    // SAFETY: `AtomicU8` and `u8` share layout + alignment
    // (guaranteed by `core::sync::atomic`). Callers are the sole
    // guest-side holder of the `&mut [u8; _]` before VMGEXIT, so
    // the only concurrent observer is the hypervisor reading via
    // its own mapping of the shared page — aliasing by a second
    // Rust reference is ruled out by the `&mut` we hold. We cast
    // through a raw pointer rather than `AtomicU8::from_mut` only
    // because the latter is still unstable on this toolchain.
    let byte: &AtomicU8 =
        unsafe { &*(&mut bitmap[bit / 8] as *mut u8 as *const AtomicU8) };
    byte.fetch_or(1 << (bit % 8), Ordering::Release);
}

/// Clear bit `bit` in `bitmap`. Atomic `fetch_and` with `Release`
/// ordering — see [`set_valid_bit`] for why.
#[inline]
pub fn clear_valid_bit(bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN], bit: usize) {
    use core::sync::atomic::{AtomicU8, Ordering};
    // SAFETY: see `set_valid_bit` — same aliasing + layout
    // invariants apply to the clear path.
    let byte: &AtomicU8 =
        unsafe { &*(&mut bitmap[bit / 8] as *mut u8 as *const AtomicU8) };
    byte.fetch_and(!(1 << (bit % 8)), Ordering::Release);
}

/// Return `true` if bit `bit` is set in `bitmap`. Atomic `load`
/// with `Acquire` ordering pairs with [`set_valid_bit`]'s `Release`
/// so a concurrent reader (hypervisor or future AP) sees the guest
/// save-area writes that preceded the bit flip.
#[inline]
pub fn is_valid_bit_set(bitmap: &[u8; GHCB_VALID_BITMAP_LEN], bit: usize) -> bool {
    use core::sync::atomic::{AtomicU8, Ordering};
    // SAFETY: `AtomicU8` and `u8` share layout + alignment. The
    // reference lives only for the `load` below; callers pass a
    // `&[u8; _]` so no `&mut u8` aliases this byte on the Rust
    // side. Going through an atomic load gives the hypervisor-
    // concurrent reader the Acquire fence pairing even when this
    // side holds only a shared reference.
    let byte: &AtomicU8 =
        unsafe { &*(&bitmap[bit / 8] as *const u8 as *const AtomicU8) };
    (byte.load(Ordering::Acquire) >> (bit % 8)) & 1 == 1
}

/// Clear all relevant save-area slots and write the three fields
/// needed for `SVM_VMGEXIT_GUEST_REQUEST`: `SW_EXIT_CODE`,
/// `SW_EXIT_INFO_1` (request GPA), `SW_EXIT_INFO_2` (response GPA).
/// Sets the matching VALID_BITMAP bits. The caller is responsible
/// for zero-filling or otherwise cleaning the save area before
/// calling if other fields could hold stale data; this routine
/// only touches the four offsets it owns.
pub fn prepare_guest_request(
    save_area: &mut [u8; GHCB_SAVE_AREA_SIZE],
    req_pa: u64,
    resp_pa: u64,
) {
    // Zero the VALID_BITMAP entirely *before* setting our three
    // bits — OVMF's `CcExitLib` reuses the same GHCB for every
    // VMGEXIT it issues (IOIO, CPUID, etc.) and leaves those
    // valid bits set in the bitmap. If we only flip our bits on
    // top, the hypervisor sees a save area that claims to carry
    // valid data in fields we never populated, and rejects the
    // exit as malformed (VMM error / garbage return). This
    // mirrors Linux 6.18's `vc_ghcb_invalidate` called from
    // `snp_issue_guest_request` before `sev_es_ghcb_hv_call`.
    let bitmap_slice = &mut save_area[GHCB_VALID_BITMAP_OFFSET..];
    let bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN] =
        (&mut bitmap_slice[..GHCB_VALID_BITMAP_LEN]).try_into().unwrap();
    *bitmap = [0u8; GHCB_VALID_BITMAP_LEN];

    write_u64_at(save_area, GHCB_SW_EXIT_CODE_OFFSET, SVM_VMGEXIT_GUEST_REQUEST);
    write_u64_at(save_area, GHCB_SW_EXIT_INFO_1_OFFSET, req_pa);
    write_u64_at(save_area, GHCB_SW_EXIT_INFO_2_OFFSET, resp_pa);

    // Re-borrow the bitmap for the per-bit sets (the bitmap
    // slice borrow above ended with the assignment).
    let bitmap_slice = &mut save_area[GHCB_VALID_BITMAP_OFFSET..];
    let bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN] =
        (&mut bitmap_slice[..GHCB_VALID_BITMAP_LEN]).try_into().unwrap();
    set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_CODE);
    set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_INFO_1);
    set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_INFO_2);
}

/// Extended-variant of [`prepare_guest_request`]. Same three save-
/// area slots (SW_EXIT_CODE, SW_EXIT_INFO_1, SW_EXIT_INFO_2) plus
/// RAX = starting GPA of cert-chain data pages and RBX = page count.
/// Exit code is flipped to `SVM_VMGEXIT_EXT_GUEST_REQUEST`.
pub fn prepare_ext_guest_request(
    save_area: &mut [u8; GHCB_SAVE_AREA_SIZE],
    req_pa: u64,
    resp_pa: u64,
    data_pa: u64,
    data_pages: u64,
) {
    // Mirror the scrub-then-set discipline used by
    // `prepare_guest_request`: zero the whole VALID_BITMAP first
    // so left-over bits from any prior OVMF CcExitLib VMGEXIT
    // don't confuse KVM's `sev_es_validate_vmgexit`.
    let bitmap_slice = &mut save_area[GHCB_VALID_BITMAP_OFFSET..];
    let bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN] =
        (&mut bitmap_slice[..GHCB_VALID_BITMAP_LEN]).try_into().unwrap();
    *bitmap = [0u8; GHCB_VALID_BITMAP_LEN];

    write_u64_at(save_area, GHCB_SW_EXIT_CODE_OFFSET, SVM_VMGEXIT_EXT_GUEST_REQUEST);
    write_u64_at(save_area, GHCB_SW_EXIT_INFO_1_OFFSET, req_pa);
    write_u64_at(save_area, GHCB_SW_EXIT_INFO_2_OFFSET, resp_pa);
    write_u64_at(save_area, GHCB_RAX_OFFSET, data_pa);
    write_u64_at(save_area, GHCB_RBX_OFFSET, data_pages);

    let bitmap_slice = &mut save_area[GHCB_VALID_BITMAP_OFFSET..];
    let bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN] =
        (&mut bitmap_slice[..GHCB_VALID_BITMAP_LEN]).try_into().unwrap();
    set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_CODE);
    set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_INFO_1);
    set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_INFO_2);
    set_valid_bit(bitmap, GHCB_BIT_RAX);
    set_valid_bit(bitmap, GHCB_BIT_RBX);
}

/// Read RBX out of the save area. Used by the extended guest-request
/// retry path: on `SNP_GUEST_VMM_ERR_INVALID_LEN` the hypervisor
/// writes the required page count back into RBX.
pub fn read_rbx(save_area: &[u8; GHCB_SAVE_AREA_SIZE]) -> u64 {
    read_u64_at(save_area, GHCB_RBX_OFFSET)
}

/// Interpret the packed `SW_EXIT_INFO_2` value the hypervisor writes
/// on `SVM_VMGEXIT_EXT_GUEST_REQUEST` return. High 32 bits carry the
/// VMM-layer error, low 32 bits carry the PSP firmware error.
#[inline]
pub fn unpack_ext_error(info2: u64) -> (u32, u32) {
    ((info2 >> SNP_GUEST_VMM_ERR_SHIFT) as u32, info2 as u32)
}

/// After the VMGEXIT returns, read `SW_EXIT_INFO_1` + `SW_EXIT_INFO_2`
/// to decide whether the PSP successfully filled the response page.
///
/// We do *not* gate on the VALID_BITMAP bits because KVM's SEV-SNP
/// VMGEXIT handler clears the bitmap on return rather than echoing
/// it, and the Linux sev-guest driver also skips the bitmap check —
/// a strict reading of AMD's GHCB spec says the hypervisor should
/// set the bits for fields it wrote, but in practice KVM only does
/// so for fields whose value is non-zero. Trusting the bitmap here
/// causes false `ResultBitNotSet` errors on every successful
/// `SNP_GUEST_REQUEST` (observed on EPYC 9354 + Linux 6.18).
///
/// The VMM-layer error (info1) takes precedence over the PSP error
/// (info2) because a non-zero info1 means the VMM rejected the
/// exit before it ever reached the PSP — reporting the PSP error
/// in that case would be misleading.
pub fn parse_guest_request_result(
    save_area: &[u8; GHCB_SAVE_AREA_SIZE],
) -> Result<(), GhcbError> {
    let info1 = read_u64_at(save_area, GHCB_SW_EXIT_INFO_1_OFFSET);
    let info2 = read_u64_at(save_area, GHCB_SW_EXIT_INFO_2_OFFSET);
    if info1 != 0 {
        return Err(GhcbError::VmmError(info1));
    }
    if info2 != 0 {
        return Err(GhcbError::PspError(info2));
    }
    Ok(())
}

#[inline]
fn write_u64_at(buf: &mut [u8; GHCB_SAVE_AREA_SIZE], offset: usize, value: u64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[inline]
fn read_u64_at(buf: &[u8; GHCB_SAVE_AREA_SIZE], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_constants() {
        assert_eq!(SVM_VMGEXIT_GUEST_REQUEST, 0x8000_0011);
        assert_eq!(GHCB_MSR, 0xC001_0130);
        assert_eq!(GHCB_SAVE_AREA_SIZE, 0x400);
        assert_eq!(GHCB_SW_EXIT_CODE_OFFSET, 0x390);
        assert_eq!(GHCB_SW_EXIT_INFO_1_OFFSET, 0x398);
        assert_eq!(GHCB_SW_EXIT_INFO_2_OFFSET, 0x3A0);
        assert_eq!(GHCB_VALID_BITMAP_OFFSET, 0x3F0);
        assert_eq!(GHCB_VALID_BITMAP_LEN, 0x10);

        // Bit indices derived from offsets.
        assert_eq!(GHCB_BIT_SW_EXIT_CODE, 0x72);
        assert_eq!(GHCB_BIT_SW_EXIT_INFO_1, 0x73);
        assert_eq!(GHCB_BIT_SW_EXIT_INFO_2, 0x74);
    }

    #[test]
    fn bitmap_set_clear_query() {
        let mut bm = [0u8; GHCB_VALID_BITMAP_LEN];
        set_valid_bit(&mut bm, 0x72);
        assert!(is_valid_bit_set(&bm, 0x72));
        assert!(!is_valid_bit_set(&bm, 0x71));
        assert!(!is_valid_bit_set(&bm, 0x73));
        clear_valid_bit(&mut bm, 0x72);
        assert!(!is_valid_bit_set(&bm, 0x72));
    }

    #[test]
    fn bitmap_boundaries() {
        let mut bm = [0u8; GHCB_VALID_BITMAP_LEN];
        // 0x00 → first bit of first byte
        set_valid_bit(&mut bm, 0);
        // 0x7F → last bit (128-bit bitmap)
        set_valid_bit(&mut bm, 0x7F);
        assert_eq!(bm[0], 0x01);
        assert_eq!(bm[0x0F], 0x80);
        assert!(is_valid_bit_set(&bm, 0));
        assert!(is_valid_bit_set(&bm, 0x7F));
    }

    #[test]
    fn bitmap_set_preserves_neighbours_in_same_byte() {
        // Atomic `fetch_or` must act as a read-modify-write, not a
        // store: setting one bit in a byte that already has other
        // bits set must leave the pre-existing bits untouched. A
        // plain `|=` happens to do this too, but wire the assertion
        // in explicitly so a future refactor to e.g. `store` is
        // caught immediately.
        let mut bm = [0u8; GHCB_VALID_BITMAP_LEN];
        bm[0] = 0b1010_0101;
        set_valid_bit(&mut bm, 1); // bit 1 within byte 0
        assert_eq!(bm[0], 0b1010_0111);
        clear_valid_bit(&mut bm, 0); // bit 0 within byte 0
        assert_eq!(bm[0], 0b1010_0110);
    }

    #[test]
    fn prepare_writes_fields_and_bits() {
        let mut save_area = [0u8; GHCB_SAVE_AREA_SIZE];
        prepare_guest_request(&mut save_area, 0x1111_2222_3333_4444, 0x5555_6666_7777_8888);

        assert_eq!(
            read_u64_at(&save_area, GHCB_SW_EXIT_CODE_OFFSET),
            SVM_VMGEXIT_GUEST_REQUEST
        );
        assert_eq!(
            read_u64_at(&save_area, GHCB_SW_EXIT_INFO_1_OFFSET),
            0x1111_2222_3333_4444
        );
        assert_eq!(
            read_u64_at(&save_area, GHCB_SW_EXIT_INFO_2_OFFSET),
            0x5555_6666_7777_8888
        );

        let bm: &[u8; GHCB_VALID_BITMAP_LEN] = (&save_area
            [GHCB_VALID_BITMAP_OFFSET..GHCB_VALID_BITMAP_OFFSET + GHCB_VALID_BITMAP_LEN])
            .try_into()
            .unwrap();
        assert!(is_valid_bit_set(bm, GHCB_BIT_SW_EXIT_CODE));
        assert!(is_valid_bit_set(bm, GHCB_BIT_SW_EXIT_INFO_1));
        assert!(is_valid_bit_set(bm, GHCB_BIT_SW_EXIT_INFO_2));
    }

    #[test]
    fn parse_success_returns_ok() {
        // Both info fields zero = success. VALID_BITMAP is
        // intentionally left untouched because the real hypervisor
        // clears it on return and we no longer check it.
        let save_area = [0u8; GHCB_SAVE_AREA_SIZE];
        assert_eq!(parse_guest_request_result(&save_area), Ok(()));
    }

    #[test]
    fn parse_psp_error_reported() {
        let mut save_area = [0u8; GHCB_SAVE_AREA_SIZE];
        write_u64_at(&mut save_area, GHCB_SW_EXIT_INFO_2_OFFSET, 0x16); // INVALID_PARAMETER
        assert_eq!(
            parse_guest_request_result(&save_area),
            Err(GhcbError::PspError(0x16))
        );
    }

    #[test]
    fn parse_vmm_error_reported() {
        // info1 = 0x4 = KVM's "pages not shared" rejection.
        let mut save_area = [0u8; GHCB_SAVE_AREA_SIZE];
        write_u64_at(&mut save_area, GHCB_SW_EXIT_INFO_1_OFFSET, 0x4);
        assert_eq!(
            parse_guest_request_result(&save_area),
            Err(GhcbError::VmmError(0x4))
        );
    }

    #[test]
    fn parse_vmm_error_takes_priority_over_psp_error() {
        // Both fields non-zero — the VMM-layer error wins because
        // the exit never reached the PSP, so the info2 value is
        // likely stale.
        let mut save_area = [0u8; GHCB_SAVE_AREA_SIZE];
        write_u64_at(&mut save_area, GHCB_SW_EXIT_INFO_1_OFFSET, 0x4);
        write_u64_at(&mut save_area, GHCB_SW_EXIT_INFO_2_OFFSET, 0x16);
        assert_eq!(
            parse_guest_request_result(&save_area),
            Err(GhcbError::VmmError(0x4))
        );
    }

    #[test]
    fn ext_spec_constants() {
        assert_eq!(SVM_VMGEXIT_EXT_GUEST_REQUEST, 0x8000_0012);
        assert_eq!(GHCB_RAX_OFFSET, 0x1F8);
        assert_eq!(GHCB_RBX_OFFSET, 0x318);
        assert_eq!(GHCB_BIT_RAX, 0x3F);
        assert_eq!(GHCB_BIT_RBX, 0x63);
        assert_eq!(SNP_GUEST_VMM_ERR_SHIFT, 32);
        assert_eq!(SNP_GUEST_VMM_ERR_INVALID_LEN, 1);
    }

    #[test]
    fn prepare_ext_writes_rax_rbx_bits() {
        let mut save_area = [0u8; GHCB_SAVE_AREA_SIZE];
        prepare_ext_guest_request(
            &mut save_area,
            0x1111_2222_3333_4444,
            0x5555_6666_7777_8888,
            0xDEAD_BEEF_CAFE_0000,
            4,
        );

        assert_eq!(
            read_u64_at(&save_area, GHCB_SW_EXIT_CODE_OFFSET),
            SVM_VMGEXIT_EXT_GUEST_REQUEST
        );
        assert_eq!(read_u64_at(&save_area, GHCB_RAX_OFFSET), 0xDEAD_BEEF_CAFE_0000);
        assert_eq!(read_u64_at(&save_area, GHCB_RBX_OFFSET), 4);
        assert_eq!(read_rbx(&save_area), 4);

        let bm: &[u8; GHCB_VALID_BITMAP_LEN] = (&save_area
            [GHCB_VALID_BITMAP_OFFSET..GHCB_VALID_BITMAP_OFFSET + GHCB_VALID_BITMAP_LEN])
            .try_into()
            .unwrap();
        assert!(is_valid_bit_set(bm, GHCB_BIT_RAX));
        assert!(is_valid_bit_set(bm, GHCB_BIT_RBX));
        assert!(is_valid_bit_set(bm, GHCB_BIT_SW_EXIT_CODE));
        assert!(is_valid_bit_set(bm, GHCB_BIT_SW_EXIT_INFO_1));
        assert!(is_valid_bit_set(bm, GHCB_BIT_SW_EXIT_INFO_2));
    }

    #[test]
    fn unpack_ext_error_splits_high_low() {
        // vmm_err=1 (INVALID_LEN), fw_err=0
        let packed = SNP_GUEST_VMM_ERR_INVALID_LEN << SNP_GUEST_VMM_ERR_SHIFT;
        assert_eq!(unpack_ext_error(packed), (1, 0));
        // vmm_err=2 (BUSY), fw_err=0x16 (PSP INVALID_PARAMETER)
        let packed = (SNP_GUEST_VMM_ERR_BUSY << SNP_GUEST_VMM_ERR_SHIFT) | 0x16;
        assert_eq!(unpack_ext_error(packed), (2, 0x16));
    }

    #[test]
    fn prepare_does_not_touch_unrelated_slots() {
        let mut save_area = [0xAAu8; GHCB_SAVE_AREA_SIZE];
        prepare_guest_request(&mut save_area, 0xC0FFEE, 0xBABE);

        // Bytes outside our touched regions should still be 0xAA.
        // Touched:
        //   * 0x390..0x398  (SW_EXIT_CODE)
        //   * 0x398..0x3A0  (SW_EXIT_INFO_1)
        //   * 0x3A0..0x3A8  (SW_EXIT_INFO_2)
        //   * 0x3F0..0x400  (VALID_BITMAP — zeroed wholesale by
        //     the `vc_ghcb_invalidate` equivalent in
        //     `prepare_guest_request`, then the three relevant
        //     bits re-set)
        assert_eq!(save_area[0x0000], 0xAA);
        // byte before SW_EXIT_CODE is untouched:
        assert_eq!(save_area[GHCB_SW_EXIT_CODE_OFFSET - 1], 0xAA);
        // gap between SW_EXIT_INFO_2 and VALID_BITMAP
        // (SwScratch, reserved, XCR0) is untouched:
        assert_eq!(save_area[GHCB_SW_EXIT_INFO_2_OFFSET + 8], 0xAA);
        assert_eq!(save_area[GHCB_VALID_BITMAP_OFFSET - 1], 0xAA);
    }
}
