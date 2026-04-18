//! qrab-boot — Rust UEFI bootloader for SEV-SNP guests.
//!
//! Phase 0: skeleton. Prints a banner to UEFI console + QEMU debugcon and
//! halts the CPU. No networking, no attestation yet. See
//! `reference/qrab-boot-plan.md` in llm-rag for full phase plan.

#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

mod panic;

use core::arch::asm;
use uefi::prelude::*;

/// Banner printed on every boot. Test harness greps for this.
const BANNER: &str = concat!(
    "QRAB BOOT v",
    env!("CARGO_PKG_VERSION"),
    " phase0 (build ",
    env!("CARGO_PKG_NAME"),
    ")",
);

#[entry]
fn efi_main() -> Status {
    // SAFETY: called exactly once at entry, no prior UEFI state touched.
    uefi::helpers::init().expect("uefi::helpers::init failed");

    log::info!("{BANNER}");
    log::info!("QRAB_PHASE0_OK");

    // Intentionally do not return control to firmware. Per plan: fail-closed,
    // hang on any terminal state including "phase not complete".
    halt_forever()
}

/// Disable interrupts and halt the CPU indefinitely.
///
/// Returns `!` masquerading as `Status` — callers rely on the never-return.
fn halt_forever() -> ! {
    // SAFETY: `cli` and `hlt` are safe on x86_64 when the CPU is in the
    // privilege level we run at (ring 0 in a UEFI application context).
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
    }
    loop {
        // SAFETY: `hlt` is a legal halt instruction at our privilege level.
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
