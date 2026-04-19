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

/// MSR number for the GHCB's physical address on SEV-ES and
/// SEV-SNP (`GHCB_MSR`). Hypervisor registers the PA here during
/// boot; guests read it to locate the shared page.
pub const GHCB_MSR: u32 = 0xC001_0130;

/// Save-area region size (bytes). The GHCB is 4 KiB total; the
/// first 0x400 B is the save area, the rest is reserved and
/// vendor-version fields.
pub const GHCB_SAVE_AREA_SIZE: usize = 0x400;

/// Offset of `SW_EXIT_CODE` within the save area.
pub const GHCB_SW_EXIT_CODE_OFFSET: usize = 0x398;

/// Offset of `SW_EXIT_INFO_1` within the save area.
pub const GHCB_SW_EXIT_INFO_1_OFFSET: usize = 0x3A0;

/// Offset of `SW_EXIT_INFO_2` within the save area.
pub const GHCB_SW_EXIT_INFO_2_OFFSET: usize = 0x3A8;

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

/// Distinguished errors from [`prepare_guest_request`] /
/// [`parse_guest_request_result`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhcbError {
    /// Hypervisor returned a non-zero `SW_EXIT_INFO_2`. The embedded
    /// `u64` is the PSP-defined status code (0x16 =
    /// INVALID_PARAMETER, 0x27 = INVALID_KEY, etc.).
    PspError(u64),
    /// Hypervisor did not set the `SW_EXIT_INFO_2` VALID_BITMAP bit
    /// on return. Either the exit was rejected before reaching the
    /// PSP or the VMM is misbehaving; caller should treat the
    /// response envelope as invalid.
    ResultBitNotSet,
}

/// Set bit `bit` in `bitmap` (little-endian within each byte).
#[inline]
pub fn set_valid_bit(bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN], bit: usize) {
    bitmap[bit / 8] |= 1 << (bit % 8);
}

/// Clear bit `bit` in `bitmap`.
#[inline]
pub fn clear_valid_bit(bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN], bit: usize) {
    bitmap[bit / 8] &= !(1 << (bit % 8));
}

/// Return `true` if bit `bit` is set in `bitmap`.
#[inline]
pub fn is_valid_bit_set(bitmap: &[u8; GHCB_VALID_BITMAP_LEN], bit: usize) -> bool {
    (bitmap[bit / 8] >> (bit % 8)) & 1 == 1
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
    write_u64_at(save_area, GHCB_SW_EXIT_CODE_OFFSET, SVM_VMGEXIT_GUEST_REQUEST);
    write_u64_at(save_area, GHCB_SW_EXIT_INFO_1_OFFSET, req_pa);
    write_u64_at(save_area, GHCB_SW_EXIT_INFO_2_OFFSET, resp_pa);

    // VALID_BITMAP slice is the last 16 B of the save area.
    let bitmap_slice = &mut save_area[GHCB_VALID_BITMAP_OFFSET..];
    let bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN] =
        (&mut bitmap_slice[..GHCB_VALID_BITMAP_LEN]).try_into().unwrap();
    set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_CODE);
    set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_INFO_1);
    set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_INFO_2);
}

/// After the VMGEXIT returns, read `SW_EXIT_INFO_2` + its
/// VALID_BITMAP bit to decide whether the PSP successfully filled
/// the response page.
pub fn parse_guest_request_result(
    save_area: &[u8; GHCB_SAVE_AREA_SIZE],
) -> Result<(), GhcbError> {
    let bitmap_slice = &save_area[GHCB_VALID_BITMAP_OFFSET..];
    let bitmap: &[u8; GHCB_VALID_BITMAP_LEN] =
        (&bitmap_slice[..GHCB_VALID_BITMAP_LEN]).try_into().unwrap();
    if !is_valid_bit_set(bitmap, GHCB_BIT_SW_EXIT_INFO_2) {
        return Err(GhcbError::ResultBitNotSet);
    }
    let info2 = read_u64_at(save_area, GHCB_SW_EXIT_INFO_2_OFFSET);
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
        assert_eq!(GHCB_SW_EXIT_CODE_OFFSET, 0x398);
        assert_eq!(GHCB_SW_EXIT_INFO_1_OFFSET, 0x3A0);
        assert_eq!(GHCB_SW_EXIT_INFO_2_OFFSET, 0x3A8);
        assert_eq!(GHCB_VALID_BITMAP_OFFSET, 0x3F0);
        assert_eq!(GHCB_VALID_BITMAP_LEN, 0x10);

        // Bit indices derived from offsets.
        assert_eq!(GHCB_BIT_SW_EXIT_CODE, 0x73);
        assert_eq!(GHCB_BIT_SW_EXIT_INFO_1, 0x74);
        assert_eq!(GHCB_BIT_SW_EXIT_INFO_2, 0x75);
    }

    #[test]
    fn bitmap_set_clear_query() {
        let mut bm = [0u8; GHCB_VALID_BITMAP_LEN];
        set_valid_bit(&mut bm, 0x73);
        assert!(is_valid_bit_set(&bm, 0x73));
        assert!(!is_valid_bit_set(&bm, 0x72));
        assert!(!is_valid_bit_set(&bm, 0x74));
        clear_valid_bit(&mut bm, 0x73);
        assert!(!is_valid_bit_set(&bm, 0x73));
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
        let mut save_area = [0u8; GHCB_SAVE_AREA_SIZE];
        // Simulate hypervisor fill: SW_EXIT_INFO_2 = 0 (success),
        // SW_EXIT_INFO_2 bit set.
        let bm_start = GHCB_VALID_BITMAP_OFFSET;
        let bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN] =
            (&mut save_area[bm_start..bm_start + GHCB_VALID_BITMAP_LEN])
                .try_into()
                .unwrap();
        set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_INFO_2);

        assert_eq!(parse_guest_request_result(&save_area), Ok(()));
    }

    #[test]
    fn parse_psp_error_reported() {
        let mut save_area = [0u8; GHCB_SAVE_AREA_SIZE];
        write_u64_at(&mut save_area, GHCB_SW_EXIT_INFO_2_OFFSET, 0x16); // INVALID_PARAMETER
        let bm_start = GHCB_VALID_BITMAP_OFFSET;
        let bitmap: &mut [u8; GHCB_VALID_BITMAP_LEN] =
            (&mut save_area[bm_start..bm_start + GHCB_VALID_BITMAP_LEN])
                .try_into()
                .unwrap();
        set_valid_bit(bitmap, GHCB_BIT_SW_EXIT_INFO_2);

        assert_eq!(
            parse_guest_request_result(&save_area),
            Err(GhcbError::PspError(0x16))
        );
    }

    #[test]
    fn parse_result_bit_not_set() {
        let save_area = [0u8; GHCB_SAVE_AREA_SIZE];
        assert_eq!(
            parse_guest_request_result(&save_area),
            Err(GhcbError::ResultBitNotSet)
        );
    }

    #[test]
    fn prepare_does_not_touch_unrelated_slots() {
        let mut save_area = [0xAAu8; GHCB_SAVE_AREA_SIZE];
        prepare_guest_request(&mut save_area, 0xC0FFEE, 0xBABE);

        // Bytes outside our four touched regions should still be 0xAA.
        // Touched: 0x398..0x3A0, 0x3A0..0x3A8, 0x3A8..0x3B0,
        //          and the three VALID_BITMAP bytes covering bits 0x73,0x74,0x75
        //          (byte 0x0E of bitmap = save_area[0x3FE]).
        assert_eq!(save_area[0x0000], 0xAA);
        assert_eq!(save_area[0x0390], 0xAA);
        // byte before SW_EXIT_CODE is untouched:
        assert_eq!(save_area[GHCB_SW_EXIT_CODE_OFFSET - 1], 0xAA);
        // gap between SW_EXIT_INFO_2 and VALID_BITMAP:
        assert_eq!(save_area[GHCB_SW_EXIT_INFO_2_OFFSET + 8], 0xAA);
        assert_eq!(save_area[GHCB_VALID_BITMAP_OFFSET - 1], 0xAA);
    }
}
