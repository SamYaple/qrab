//! `virtio_drivers::Hal` implementation backed by
//! `PciRootBridgeIoProtocol`'s Map / Unmap / AllocateBuffer /
//! FreeBuffer entry points (see [`crate::pci_dma`]).
//!
//! ## Why not identity-map?
//!
//! Every allocation from UEFI `BootServices::allocate_pages` is
//! identity-mapped and page-aligned, which made the identity-map
//! HAL a one-liner — but that memory is **encrypted private** under
//! SEV-SNP. Handing the device an encrypted PA means virtio DMA
//! reads back ciphertext and the first feature-negotiation / virtqueue
//! operation either hangs or fails. `issues/qrab-boot-snp-dma.md`
//! tracked this gap; this commit closes it.
//!
//! ## Single code path for TCG + SNP
//!
//! Under AmdSev OVMF the `IoMmuDxe` driver installs
//! `gEdkiiIoMmuProtocol`; `PciHostBridgeDxe::RootBridgeIoMap` then
//! routes every `Map` / `AllocateBuffer` call through
//! `IoMmuMap` / `IoMmuAllocateBuffer`, which clear the C-bit on the
//! allocated pages (`MemEncryptSevClearPageEncMask`) or bounce
//! plaintext through shared memory. Under plain TCG OVMF the same
//! protocol is not installed, so `RootBridgeIoMap` short-circuits to
//! identity-mapping (`DeviceAddress := HostAddress`) with no copy.
//! The HAL therefore does not need a runtime SEV branch — the same
//! calls DTRT on both.
//!
//! ## State
//!
//! The virtio-drivers `Hal` trait uses static methods (no `self`),
//! so the HAL keeps two heap-allocated tables in statics:
//!
//!   * `ALLOC_MAP`: `host_addr → map_cookie` — so `dma_dealloc` can
//!     `unmap` the common-buffer mapping it made during `dma_alloc`.
//!   * `SHARE_MAP`: `(ptr, len) → map_cookie` — so `unshare` can
//!     close the transient bus-master mapping from `share`.
//!
//! Both are `RefCell<BTreeMap<…>>` guarded by `unsafe impl Sync`:
//! UEFI boot services run single-threaded, and qrab-boot never
//! re-enters the HAL from an interrupt. A missing `SHARE_MAP` entry
//! on `unshare` logs `QRAB_DMA_COOKIE_MISSING` and returns silently
//! — soft-fail is preferable to a panic mid-DMA.

use alloc::collections::BTreeMap;
use core::cell::RefCell;
use core::ffi::c_void;
use core::ptr::NonNull;

use uefi_raw::protocol::pci::root_bridge::PciRootBridgeIoProtocolOperation;
use virtio_drivers::{BufferDirection, Hal, PhysAddr};

use crate::pci_dma;

/// UEFI page size. `PciRootBridgeIoProtocol::AllocateBuffer` always
/// returns page-aligned memory.
const PAGE_SIZE: usize = 4096;

/// Cookie table keyed on the host virtual address returned by
/// `pci_dma::allocate_buffer`. Used by `dma_dealloc` to recover the
/// `Map(CommonBuffer64)` cookie established during `dma_alloc`.
struct AllocMap(RefCell<BTreeMap<usize, *mut c_void>>);

// SAFETY: boot services run single-threaded; see the module-level
// "## State" block.
unsafe impl Sync for AllocMap {}

static ALLOC_MAP: AllocMap = AllocMap(RefCell::new(BTreeMap::new()));

/// Cookie table keyed on `(ptr, len)` of every buffer currently
/// shared with the device via `Hal::share`. Used by `unshare` to
/// recover the `Map(BusMaster*64)` cookie.
struct ShareMap(RefCell<BTreeMap<(usize, usize), *mut c_void>>);

// SAFETY: same as `AllocMap`.
unsafe impl Sync for ShareMap {}

static SHARE_MAP: ShareMap = ShareMap(RefCell::new(BTreeMap::new()));

/// `Hal` wrapper; has no instance state of its own — all state lives
/// in [`ALLOC_MAP`] / [`SHARE_MAP`] / [`pci_dma`].
#[allow(dead_code)]
pub struct UefiHal;

/// Map a virtio-drivers direction onto the
/// `EFI_PCI_ROOT_BRIDGE_IO_PROTOCOL_OPERATION` needed for a transient
/// bus-master mapping. The 64-bit variants are the right choice: the
/// buffer can live anywhere in memory, and the firmware's handling
/// of the 64-bit flag is "identical to the 32-bit operation plus
/// sets `RootBridge->DmaAbove4G`" per the UEFI spec.
fn share_op(direction: BufferDirection) -> PciRootBridgeIoProtocolOperation {
    match direction {
        BufferDirection::DriverToDevice => {
            // Device reads from the buffer (driver writes, then hands
            // over). IoMmuMap will copy plaintext into a shared
            // bounce buffer as part of Map on SNP.
            PciRootBridgeIoProtocolOperation::BUS_MASTER_READ64
        }
        BufferDirection::DeviceToDriver => {
            // Device writes into the buffer; Map allocates an empty
            // shared bounce and Unmap copies it back on release.
            PciRootBridgeIoProtocolOperation::BUS_MASTER_WRITE64
        }
        BufferDirection::Both => {
            // Common-buffer semantics: shared bi-directional region
            // with no bounce copies.
            PciRootBridgeIoProtocolOperation::BUS_MASTER_COMMON_BUFFER64
        }
    }
}

// SAFETY: every method delegates to `pci_dma` which wraps the
// firmware's `PciRootBridgeIoProtocol`. That protocol enforces the
// validity / alignment invariants the `Hal` trait demands; any
// error from the firmware is surfaced back to virtio-drivers by
// returning a dangling pointer from `dma_alloc` (which the Dma<H>
// constructor treats as a DMA error) or by logging + no-op for
// `unshare` (soft-fail — panicking mid-virtio-tx would strand the
// device half-setup).
unsafe impl Hal for UefiHal {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        // Allocate shared pages from the firmware's IoMmu pool. On
        // SNP this clears the C-bit on the returned pages; on TCG
        // it returns plain allocator memory.
        let host = match pci_dma::allocate_buffer(pages) {
            Ok(p) => p,
            Err(_) => return (0, NonNull::dangling()),
        };
        // Zero the pages; AllocateBuffer is not guaranteed to do so
        // (UEFI spec says "may return any memory"), and virtio's
        // `Dma::new` doc-comment promises zeroed output.
        //
        // SAFETY: `host` is the page-aligned start of a
        // `pages * PAGE_SIZE` shared region; no one else holds a
        // reference yet.
        unsafe {
            core::ptr::write_bytes(host, 0, pages * PAGE_SIZE);
        }

        // Establish a common-buffer mapping so the device can reach
        // the pages. On SNP this does nothing beyond recording the
        // mapping (the pages are already shared); on TCG the
        // device_addr equals host.
        let mapping = match pci_dma::map(
            PciRootBridgeIoProtocolOperation::BUS_MASTER_COMMON_BUFFER64,
            host as *const u8,
            pages * PAGE_SIZE,
        ) {
            Ok(m) => m,
            Err(_) => {
                // `pci_dma::free_buffer` on the partial allocation;
                // we cannot surface a richer error through the Hal
                // trait, so drop back into the dangling-pointer
                // signal that `Dma<H>::new` catches as `DmaError`.
                let _ = pci_dma::free_buffer(host, pages);
                return (0, NonNull::dangling());
            }
        };
        ALLOC_MAP
            .0
            .borrow_mut()
            .insert(host as usize, mapping.cookie);
        let vaddr = NonNull::new(host).unwrap_or(NonNull::dangling());
        (mapping.device_addr, vaddr)
    }

    unsafe fn dma_dealloc(paddr: PhysAddr, vaddr: NonNull<u8>, pages: usize) -> i32 {
        let host = vaddr.as_ptr();
        let cookie = ALLOC_MAP.0.borrow_mut().remove(&(host as usize));
        if let Some(cookie) = cookie {
            let _ = pci_dma::unmap(cookie);
        } else {
            // Missing cookie means we never mapped this allocation
            // (previous dma_alloc took the error branch). Log and
            // continue to free the underlying buffer if we can.
            log::warn!(
                "QRAB_DMA_COOKIE_MISSING op=dma_dealloc host={host:p} paddr={paddr:#x}",
            );
        }
        // SAFETY: `host` originated from `pci_dma::allocate_buffer`
        // and has not been freed yet. Matching `pages` must be
        // supplied by the caller per the `Hal` trait contract.
        if pci_dma::free_buffer(host, pages).is_err() {
            return -1;
        }
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        // MMIO BAR windows are left unencrypted by QEMU's CPU map
        // (SEV never encrypts device MMIO), and UEFI keeps them
        // identity-mapped, so physical == virtual here on every
        // target we support.
        NonNull::new(paddr as *mut u8).expect("mmio_phys_to_virt: null MMIO address")
    }

    unsafe fn share(buffer: NonNull<[u8]>, direction: BufferDirection) -> PhysAddr {
        let ptr = buffer.as_ptr() as *mut u8;
        let len = buffer.len();
        let op = share_op(direction);
        let mapping = match pci_dma::map(op, ptr, len) {
            Ok(m) => m,
            Err(_) => {
                // Returning 0 triggers virtio-drivers' `DmaError`
                // path on the descriptor submission side.
                return 0;
            }
        };
        SHARE_MAP
            .0
            .borrow_mut()
            .insert((ptr as usize, len), mapping.cookie);
        mapping.device_addr
    }

    unsafe fn unshare(_paddr: PhysAddr, buffer: NonNull<[u8]>, _direction: BufferDirection) {
        let ptr = buffer.as_ptr() as *mut u8;
        let len = buffer.len();
        let cookie = SHARE_MAP.0.borrow_mut().remove(&(ptr as usize, len));
        match cookie {
            Some(cookie) => {
                // `Unmap` on a BusMasterWrite64 mapping copies the
                // device-written bytes back into the caller's host
                // buffer; on BusMasterRead64 it just drops the
                // bounce copy. Errors are logged by `pci_dma` but
                // we cannot meaningfully propagate them through the
                // Hal trait.
                let _ = pci_dma::unmap(cookie);
            }
            None => {
                log::warn!(
                    "QRAB_DMA_COOKIE_MISSING op=unshare ptr={ptr:p} len={len}",
                );
            }
        }
    }
}
