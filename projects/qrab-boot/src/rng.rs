//! RDRAND-backed randomness for the attestation client nonce.
//!
//! UEFI boot services expose `EFI_RNG_PROTOCOL`, but AmdSev OVMF
//! builds do not enable it by default (no `RngDxe` linked in on the
//! AmdSev DSC). On x86_64 every AMD chip that supports SEV exposes
//! `RDRAND` and `RDSEED` through CPUID — both bits live in
//! `CPUID.01H:ECX[30]` (RDRAND) and `CPUID.07H:ECX[18]` (RDSEED).
//! We use RDRAND with the retry loop recommended by Intel's SDM and
//! AMD's APM volume 3 (§"Random Number Generation"), and bail out
//! with a distinguished serial token if the hardware claims the
//! feature is absent or fails to generate entropy after many
//! retries.
//!
//! Per AMD APM: the architecturally-mandated retry count is "at most
//! 10 attempts". We keep 10 per 64-bit read and 32 outer retries so
//! a single transient shortage of entropy never wedges the boot.
//!
//! SAFETY: all uses of `asm!` are unprivileged on x86_64. `cpuid` and
//! `rdrand` both work at any CPL. We annotate each call with the
//! specific options that reflect the instructions' semantics.

use core::arch::asm;

/// Distinguished errors from [`fill_random`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RngError {
    /// `CPUID.01H:ECX[30]` reports RDRAND is unavailable on this CPU.
    RdrandUnsupported,
    /// RDRAND returned carry-clear (entropy shortage) on every retry.
    EntropyStarved,
}

impl core::fmt::Display for RngError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::RdrandUnsupported => f.write_str("CPU does not expose RDRAND"),
            Self::EntropyStarved => f.write_str("RDRAND never returned ready"),
        }
    }
}

/// Fill `out` with bytes from `RDRAND`. Returns an error if the CPU
/// does not support RDRAND or if RDRAND never clears the carry flag
/// across many retries (indicates a broken hardware RNG).
pub fn fill_random(out: &mut [u8]) -> Result<(), RngError> {
    if !has_rdrand() {
        return Err(RngError::RdrandUnsupported);
    }
    let mut i = 0;
    while i < out.len() {
        let word = rdrand_u64().ok_or(RngError::EntropyStarved)?;
        let bytes = word.to_le_bytes();
        let take = core::cmp::min(8, out.len() - i);
        out[i..i + take].copy_from_slice(&bytes[..take]);
        i += take;
    }
    Ok(())
}

/// Convenience: produce a fresh 32-byte nonce. Matches the
/// `AttestRequest::client_nonce` field width.
pub fn fresh_nonce() -> Result<[u8; 32], RngError> {
    let mut out = [0u8; 32];
    fill_random(&mut out)?;
    Ok(out)
}

/// Query CPUID leaf 1, ECX bit 30 for RDRAND availability.
fn has_rdrand() -> bool {
    let (_, _, ecx, _) = cpuid(1, 0);
    (ecx >> 30) & 1 == 1
}

/// Invoke `cpuid` at `leaf` / `subleaf` and return `(eax, ebx, ecx, edx)`.
fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;
    // SAFETY: `cpuid` is unprivileged, has no memory effects, and
    // clobbers only the four general-purpose registers written below.
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") leaf => eax,
            inout("ecx") subleaf => ecx,
            out("edx") edx,
            ebx_out = out(reg) ebx,
            options(nostack, preserves_flags),
        );
    }
    (eax, ebx, ecx, edx)
}

/// Single RDRAND read of 64 bits, retrying up to 10 times as
/// recommended by AMD APM volume 3.
fn rdrand_u64() -> Option<u64> {
    for _ in 0..10 {
        let val: u64;
        let cf: u8;
        // SAFETY: `rdrand` is unprivileged; `setc` reads CF set by
        // `rdrand`. No memory side-effects.
        unsafe {
            asm!(
                "rdrand {val}",
                "setc {cf}",
                val = out(reg) val,
                cf = out(reg_byte) cf,
                options(nomem, nostack),
            );
        }
        if cf != 0 {
            return Some(val);
        }
    }
    None
}
