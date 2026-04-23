//! SEV status MSR (AMD64 APM Vol. 2 §15, `MSR_AMD64_SEV = 0xC0010131`).
//!
//! Per `level-5-init.md` step 2, qrab-guest-init requires bits 0 (SEV),
//! 1 (SEV-ES), and 2 (SNP) to all be set. MSR is read via
//! `/dev/cpu/0/msr` — the kernel's `msr` driver exposes a seekable
//! character device where a read at offset `MSR_NUM` returns the 8-byte
//! little-endian MSR value.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::tokens::Token;

/// Address of the SEV status MSR.
pub const MSR_AMD64_SEV: u64 = 0xC001_0131;

/// Bit mask for (SEV | SEV-ES | SNP) — the L5 requirement.
pub const REQUIRED_BITS: u64 = 0b0000_0111;

/// Decode 8 bytes of little-endian MSR contents into a bitfield.
///
/// Kept `pub` and file-bytes-only so unit tests can exercise the bit
/// logic without touching `/dev/cpu/0/msr`.
pub fn decode_msr_bytes(raw: [u8; 8]) -> u64 {
    u64::from_le_bytes(raw)
}

/// Apply the L5 policy to a decoded MSR value.
///
/// Returns `Ok(())` iff all of SEV, SEV-ES, SNP bits are set. Any
/// missing bit collapses to `Token::FailNoSnp` — the single "not a
/// confidential guest" indicator. Finer-grained tokens aren't worth
/// it: a VM with SEV but not SNP is as failed-closed as one with no
/// SEV at all, from qrab-guest-init's perspective.
pub fn check_sev_status(value: u64) -> Result<(), Token> {
    if value & REQUIRED_BITS == REQUIRED_BITS {
        Ok(())
    } else {
        Err(Token::FailNoSnp)
    }
}

/// Read and decode `/dev/cpu/0/msr` at `MSR_AMD64_SEV`. Separated from
/// `check_sev_status` so the IO layer can be swapped for a byte
/// reader in tests.
pub fn read_msr_file(path: &Path, address: u64) -> std::io::Result<u64> {
    let mut f = std::fs::File::open(path)?;
    f.seek(SeekFrom::Start(address))?;
    let mut buf = [0u8; 8];
    f.read_exact(&mut buf)?;
    Ok(decode_msr_bytes(buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_little_endian() {
        // bit 2 set (SNP) encodes as 0x04 in the LSB.
        assert_eq!(decode_msr_bytes([0x04, 0, 0, 0, 0, 0, 0, 0]), 0b100);
        // Three low bits all set → 0x07.
        assert_eq!(decode_msr_bytes([0x07, 0, 0, 0, 0, 0, 0, 0]), 0b111);
        // High bits still decode sensibly (bit 63 = 1 << 63).
        assert_eq!(
            decode_msr_bytes([0, 0, 0, 0, 0, 0, 0, 0x80]),
            1u64 << 63,
        );
    }

    #[test]
    fn all_required_bits_set_passes() {
        assert!(check_sev_status(0b0000_0111).is_ok());
        // Extra high bits (vTOM, DebugSwap, etc.) don't gate us.
        assert!(check_sev_status(0b0111_1111).is_ok());
    }

    #[test]
    fn missing_snp_bit_fails() {
        // SEV + SEV-ES but no SNP.
        assert_eq!(check_sev_status(0b0000_0011), Err(Token::FailNoSnp));
    }

    #[test]
    fn missing_sev_es_bit_fails() {
        // SEV + SNP but no SEV-ES (unreachable in practice but tests
        // the exact-mask intent).
        assert_eq!(check_sev_status(0b0000_0101), Err(Token::FailNoSnp));
    }

    #[test]
    fn missing_sev_bit_fails() {
        // SEV-ES + SNP but no base SEV.
        assert_eq!(check_sev_status(0b0000_0110), Err(Token::FailNoSnp));
    }

    #[test]
    fn all_zero_fails() {
        assert_eq!(check_sev_status(0), Err(Token::FailNoSnp));
    }
}
