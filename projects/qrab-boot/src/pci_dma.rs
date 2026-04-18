//! Safe-ish wrappers around `uefi-raw`'s
//! [`PciRootBridgeIoProtocol`] DMA entry points.
//!
//! `uefi` 0.36's high-level wrapper only exposes the PCI
//! configuration-space half of the protocol (segment_nr / pci /
//! flush); `Map`, `Unmap`, `AllocateBuffer`, `FreeBuffer` are
//! explicitly marked `TODO` in
//! `uefi-0.36.1/src/proto/pci/root_bridge.rs`. qrab-boot's
//! virtio-vsock HAL needs those four entry points to DMA-map guest
//! buffers correctly under SEV-SNP (where OVMF's `IoMmuDxe` allocates
//! bounce buffers / common buffers in shared memory and clears the
//! C-bit on the guest PTEs); falling back to the raw FFI in
//! `uefi-raw` is the smallest change that unblocks the HAL.
//!
//! ## Identity-map fallback on non-confidential guests
//!
//! On plain TCG / non-SEV boots, OVMF does not install
//! `gEdkiiIoMmuProtocol`, so `PciHostBridgeDxe`'s `RootBridgeIoMap`
//! short-circuits to "DeviceAddress := HostAddress" with no copy (see
//! `MdeModulePkg/Bus/Pci/PciHostBridgeDxe/PciRootBridgeIo.c`, ~l.1360).
//! The same wrapper calls therefore work on both TCG and SNP — we do
//! not need a runtime SEV branch inside this module.
//!
//! ## Lifetime
//!
//! [`init`] stashes a raw pointer to an opened protocol. The caller
//! (`vsock_driver::init`) keeps a `ManuallyDrop<ScopedProtocol<…>>`
//! alive for the remainder of boot; qrab-boot never calls
//! `ExitBootServices`, so the pointer is valid until `hlt`.
//!
//! ## Thread safety
//!
//! UEFI boot services run single-threaded. No interrupts or
//! multi-processor code paths enter qrab-boot after `uefi::helpers::
//! init`. A plain [`Cell`] holding the raw pointer is enough; the
//! [`BridgeSlot`] wrapper provides the `Sync` impl the `static` slot
//! needs.
//!
//! ## Logging tokens
//!
//! Every failing FFI call emits a distinctive `QRAB_DMA_*` token so
//! the smoke harness can grep for the exact cause. Success of [`init`]
//! is announced once as `QRAB_DMA_INIT_OK`.
//!
//! [`PciRootBridgeIoProtocol`]:
//!     uefi_raw::protocol::pci::root_bridge::PciRootBridgeIoProtocol

// Every fn lives without a caller in this commit; phase-follow-up
// commits wire the module into `vsock_driver::init` and
// `vsock_hal`. Rather than sprinkle `#[allow(dead_code)]` on each
// symbol only to peel them off in the next commit, allow dead code
// for the whole file.
#![allow(dead_code)]

use core::cell::Cell;
use core::ffi::c_void;
use core::ptr;

use uefi::Status;
use uefi_raw::PhysicalAddress;
use uefi_raw::protocol::pci::root_bridge::{
    PciRootBridgeIoProtocol, PciRootBridgeIoProtocolOperation,
};
use uefi_raw::table::boot::{AllocateType, MemoryType};

/// Wrapper for the `*mut PciRootBridgeIoProtocol` `static`. The raw
/// pointer is not `Sync` by itself; we assert single-threaded use and
/// impl `Sync` on the newtype.
struct BridgeSlot(Cell<*mut PciRootBridgeIoProtocol>);

// SAFETY: UEFI boot services are single-threaded during DXE
// execution. qrab-boot never enables multiprocessing or IRQs that
// would re-enter this module, and every access goes through the
// non-aliasing wrappers below.
unsafe impl Sync for BridgeSlot {}

static BRIDGE: BridgeSlot = BridgeSlot(Cell::new(ptr::null_mut()));

/// Memory type requested of [`allocate_buffer`]. `LOADER_DATA` keeps
/// firmware bookkeeping consistent with the rest of qrab-boot's own
/// allocations (`UefiHal::dma_alloc` previously used `LOADER_DATA`
/// via `boot::allocate_pages`).
const DMA_MEM_TYPE: MemoryType = MemoryType::LOADER_DATA;

/// Record the `PciRootBridgeIoProtocol` pointer for subsequent DMA
/// calls. Must be called exactly once, before any other fn in this
/// module; subsequent invocations overwrite the slot (this is
/// deliberate so tests can swap the pointer, but in production there
/// is never a reason to re-init).
pub fn init(ptr: *mut PciRootBridgeIoProtocol) {
    debug_assert!(!ptr.is_null(), "pci_dma::init called with null");
    BRIDGE.0.set(ptr);
    log::info!("QRAB_DMA_INIT_OK bridge={ptr:p}");
}

/// `true` iff [`init`] has been called with a non-null pointer.
#[allow(dead_code)]
pub fn is_initialised() -> bool {
    !BRIDGE.0.get().is_null()
}

/// Internal accessor returning `None` when [`init`] has not been
/// called. Every public entry point that fails with `NOT_READY`
/// checks this first so the log carries a distinctive token.
fn bridge() -> Option<*mut PciRootBridgeIoProtocol> {
    let p = BRIDGE.0.get();
    if p.is_null() { None } else { Some(p) }
}

/// Wrapper for `EFI_PCI_ROOT_BRIDGE_IO_PROTOCOL.AllocateBuffer`. The
/// returned host virtual pointer is page-aligned and covers `pages *
/// 4096` bytes of shared (under SNP) / plain (under TCG) memory.
///
/// On SNP, OVMF's `IoMmuAllocateBuffer` clears the C-bit on the pages
/// it hands back, so the guest and device observe the same physical
/// layout. The caller still has to `map(..CommonBuffer..)` the
/// allocation to obtain the device-side address.
pub fn allocate_buffer(pages: usize) -> Result<*mut u8, Status> {
    let Some(bridge) = bridge() else {
        log::warn!("QRAB_DMA_NOT_READY op=allocate_buffer pages={pages}");
        return Err(Status::NOT_READY);
    };
    let mut host: *const c_void = ptr::null();
    // SAFETY: `bridge` came from `init`, which asserted non-null;
    // boot services are live for the duration of qrab-boot; the
    // function pointer has the UEFI-spec'd signature.
    let status = unsafe {
        ((*bridge).allocate_buffer)(
            bridge,
            AllocateType::ANY_PAGES,
            DMA_MEM_TYPE,
            pages,
            &mut host,
            0, // attributes: no special cacheability bits requested.
        )
    };
    if status != Status::SUCCESS {
        log::warn!(
            "QRAB_DMA_ALLOC_ERR status={:#x} pages={pages}",
            status.0,
        );
        return Err(status);
    }
    Ok(host as *mut u8)
}

/// Wrapper for `EFI_PCI_ROOT_BRIDGE_IO_PROTOCOL.FreeBuffer`. Releases
/// `pages` pages starting at `host`, which must have been returned by
/// a matching [`allocate_buffer`] call on the same bridge.
pub fn free_buffer(host: *mut u8, pages: usize) -> Result<(), Status> {
    let Some(bridge) = bridge() else {
        log::warn!("QRAB_DMA_NOT_READY op=free_buffer pages={pages}");
        return Err(Status::NOT_READY);
    };
    // SAFETY: `host` must be the matching allocate_buffer return
    // value (caller's responsibility); the rest matches
    // `allocate_buffer`'s safety.
    let status = unsafe {
        ((*bridge).free_buffer)(bridge, pages, host as *const c_void)
    };
    if status != Status::SUCCESS {
        log::warn!(
            "QRAB_DMA_FREE_ERR status={:#x} pages={pages}",
            status.0,
        );
        return Err(status);
    }
    Ok(())
}

/// Successful [`map`] result. Carries the device-side physical
/// address the DMA engine should use plus the opaque `mapping`
/// cookie `Unmap` requires to release the mapping.
pub struct Mapping {
    /// Device-visible physical address. Under SNP this is the shared
    /// bounce-buffer PA; on TCG it equals the host pointer.
    pub device_addr: u64,
    /// Opaque cookie allocated by the IOMMU, fed back through
    /// [`unmap`] to release the mapping.
    pub cookie: *mut c_void,
}

/// Wrapper for `EFI_PCI_ROOT_BRIDGE_IO_PROTOCOL.Map`. On SNP the
/// `IoMmuMap` implementation copies the plaintext into a shared
/// bounce buffer for `BusMasterRead*` operations, and allocates an
/// empty shared buffer for `BusMasterWrite*`. The returned
/// `device_addr` is the bounce buffer's PA.
///
/// `num_bytes` is updated in place by the firmware to the number of
/// bytes actually mapped; a short mapping is logged as
/// `QRAB_DMA_MAP_SHORT` but still returned to the caller so they can
/// decide how to handle it (the virtio-drivers HAL treats a short
/// map as a fatal DMA error).
pub fn map(
    op: PciRootBridgeIoProtocolOperation,
    host: *const u8,
    bytes: usize,
) -> Result<Mapping, Status> {
    let Some(bridge) = bridge() else {
        log::warn!("QRAB_DMA_NOT_READY op=map bytes={bytes}");
        return Err(Status::NOT_READY);
    };
    let mut num_bytes: usize = bytes;
    let mut device_addr: PhysicalAddress = 0;
    let mut cookie: *mut c_void = ptr::null_mut();
    // SAFETY: `bridge` non-null per `init`; `num_bytes`, `device_addr`,
    // `cookie` are `&mut`s to local stack slots; function pointer has
    // the UEFI-spec'd signature.
    let status = unsafe {
        ((*bridge).map)(
            bridge,
            op,
            host as *const c_void,
            &mut num_bytes,
            &mut device_addr,
            &mut cookie,
        )
    };
    if status != Status::SUCCESS {
        log::warn!(
            "QRAB_DMA_MAP_ERR status={:#x} op={} bytes={bytes}",
            status.0,
            op.0,
        );
        return Err(status);
    }
    if num_bytes < bytes {
        log::warn!(
            "QRAB_DMA_MAP_SHORT op={} got={num_bytes} want={bytes}",
            op.0,
        );
    }
    Ok(Mapping {
        device_addr,
        cookie,
    })
}

/// Wrapper for `EFI_PCI_ROOT_BRIDGE_IO_PROTOCOL.Unmap`. Releases the
/// mapping identified by `cookie`; for a `BusMasterWrite*` mapping
/// the firmware copies the device-written bytes back into the
/// caller's host buffer as part of `Unmap`.
pub fn unmap(cookie: *mut c_void) -> Result<(), Status> {
    let Some(bridge) = bridge() else {
        log::warn!("QRAB_DMA_NOT_READY op=unmap");
        return Err(Status::NOT_READY);
    };
    // SAFETY: `cookie` must be the value previously returned by a
    // matching `map` (caller's responsibility); bridge is non-null.
    let status = unsafe { ((*bridge).unmap)(bridge, cookie) };
    if status != Status::SUCCESS {
        log::warn!("QRAB_DMA_UNMAP_ERR status={:#x}", status.0);
        return Err(status);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // The entire module is UEFI-runtime-only: every call touches the
    // real firmware protocol. Host-side unit tests would require a
    // mock `PciRootBridgeIoProtocol` vtable, which is deliberately
    // out of scope — the integration coverage is the TCG + SNP smoke
    // pair (`contrib/smoke-vsock-qemu.sh` +
    // `contrib/smoke-snp-attest.sh`).
    //
    // This stub exists only to pin the module's public surface at
    // `cargo check` time.
    #[allow(dead_code)]
    fn _surface_check() {
        let _ = super::is_initialised();
    }
}
