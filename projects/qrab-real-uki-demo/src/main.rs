//! Minimal UEFI PE used as a real UKI fixture for `matrix-smoke.sh`
//! scenario 1r (follow-up **F10**).
//!
//! qrab-boot's happy-path terminus is `BootServices::load_image`
//! followed by `start_image` — see `projects/qrab-boot/src/uki.rs`.
//! The matrix harness needs proof that this chain really hands
//! control to a fresh PE (as opposed to failing `LoadImage` on the
//! built-in stub that scenario 1 ships). This binary is that proof:
//! a legal UEFI image that, on entry, writes a distinctive marker
//! token to the QEMU debugcon port (0x402) and halts.
//!
//! ## Why no UEFI helpers / logger
//!
//! `qrab-boot` has already called [`uefi::helpers::init`] in its own
//! image context. The library stores the image handle and system
//! table in crate-local statics; a second init inside a nested
//! `start_image`'d child would either be a no-op, panic, or install
//! a logger that writes through `BootServices` that the parent owns.
//! Any of those outcomes risks obscuring the thing scenario 1r is
//! verifying. Bypassing the helpers and writing directly to debugcon
//! keeps the test deterministic and the binary tiny — both of which
//! matter because the whole PE has to traverse the vsock wire inside
//! one `qrab_boot_proto::MAX_FRAMED_PAYLOAD` (128 KiB) frame.
//!
//! Port `0x402` is the QEMU `debugcon` convention and is the same
//! sink `qrab-boot`'s `uefi::helpers::init` routes `log::info!`
//! output to via the `log-debugcon` feature. The marker therefore
//! lands on the same `target/qrab-boot-vsock-smoke/serial.log`
//! file the smoke `grep`s.

#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

/// Marker the smoke harness greps for on the guest serial log. The
/// trailing newline keeps it on its own line in `serial.log`.
const MARKER: &[u8] = concat!(
    "QRAB_REAL_UKI_BOOT_OK version=",
    env!("CARGO_PKG_VERSION"),
    "\n",
)
.as_bytes();

/// QEMU debugcon I/O port. See `docs/qemu/system/invocation.rst`:
/// `-debugcon` defaults to 0xe9 on PC, but qrab-boot binds
/// `isa-debugcon,iobase=0x402,chardev=...` (the SeaBIOS /
/// tianocore-standard `OUT_DEBUG_PORT` constant) — so we write to
/// 0x402 to match.
const DEBUGCON_PORT: u16 = 0x402;

#[uefi::entry]
fn efi_main() -> uefi::Status {
    for &byte in MARKER {
        // SAFETY: `out dx, al` on a UEFI x86_64 image is permitted
        // in firmware context — the CPU runs at ring 0 and QEMU's
        // debugcon is an unrestricted I/O port. No side effect
        // other than the debugcon write.
        unsafe {
            asm!(
                "out dx, al",
                in("dx") DEBUGCON_PORT,
                in("al") byte,
                options(nomem, nostack, preserves_flags),
            );
        }
    }
    halt_forever();
}

/// Halt forever with interrupts disabled. Never returns.
fn halt_forever() -> ! {
    // SAFETY: `cli` + `hlt` are legal at CPL 0; we are about to
    // loop indefinitely and never service interrupts again.
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
    }
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

/// Fail-closed panic handler. No logging (the parent's logger is
/// gone by the time we're running). Just halt.
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    halt_forever();
}
