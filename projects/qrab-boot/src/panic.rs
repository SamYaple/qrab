//! Fail-closed panic handler.
//!
//! On any panic: emit a single `QRAB_PANIC` line via the `log` facade (routed
//! through uefi's logger to the UEFI console and the QEMU debugcon port when
//! present), then disable interrupts and halt forever. No reset, no shell
//! drop-back, no unwind.

use core::arch::asm;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    // Best-effort log. If logging has not been initialised yet this is a
    // no-op; we still halt below.
    log::error!("QRAB_PANIC: {info}");

    // SAFETY: disabling interrupts is always safe in a UEFI application
    // context; we are about to halt forever anyway.
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
