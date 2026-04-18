//! `virtio_drivers::Hal` implementation backed by UEFI boot services.
//!
//! `virtio-drivers` requires the driver crate to supply a platform
//! layer that can (a) allocate physically-contiguous, page-aligned
//! zeroed DMA memory, (b) deallocate it, (c) translate MMIO physical
//! addresses to accessible virtual addresses, and (d) share buffers
//! with the device for single transactions.
//!
//! During UEFI boot services every allocation made through
//! `BootServices::allocate_pages` is identity-mapped (virtual address
//! equals physical address) and naturally page-aligned, which
//! collapses most of this surface into a one-liner.
//!
//! SEV-SNP note: guest-to-device DMA on SNP must use memory that the
//! host has been granted access to. Under UEFI + AmdSev OVMF the
//! `AmdSevDxe` / `IoMmuDxe` drivers manage the grant tables, and the
//! `PCI Root Bridge I/O` protocol's `map`/`unmap` methods ride those
//! drivers. Phase 3b only implements the non-IOMMU fast path; the
//! SEV-SNP DMA mapping is deferred to a future commit that plugs
//! `PciRootBridgeIo` `map`/`unmap` into `share` / `unshare`.

use core::ptr::NonNull;

use uefi::boot::{self, AllocateType, MemoryType};
use virtio_drivers::{BufferDirection, Hal, PhysAddr};

/// Page size used by `virtio-drivers` and by UEFI `AllocatePages`.
const PAGE_SIZE: usize = 4096;

/// Memory type for DMA buffers. Using `LoaderData` keeps the
/// allocation visible to firmware tracking and matches what other
/// UEFI-resident DMA drivers do; we free everything before
/// `ExitBootServices`, so the post-boot memory map is untouched.
const DMA_MEM_TYPE: MemoryType = MemoryType::LOADER_DATA;

/// `Hal` wrapper; has no state of its own.
//
// Phase 3b instantiates `virtio_drivers::transport::pci::PciTransport`
// with this Hal; until that commit lands, the type is declared but
// never constructed and the normal dead-code lint would fire. Allow it
// locally rather than sprinkling `#[allow]` on the type's eventual
// consumer.
#[allow(dead_code)]
pub struct UefiHal;

// SAFETY: UEFI boot services are single-threaded during pre-boot, and
// all memory returned by `allocate_pages` is identity-mapped with
// exclusive access. `share`/`unshare` operate on caller-provided
// buffers and add no aliasing.
unsafe impl Hal for UefiHal {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let Ok(phys) = boot::allocate_pages(AllocateType::AnyPages, DMA_MEM_TYPE, pages) else {
            return (0, NonNull::dangling());
        };
        let ptr = phys.as_ptr();
        // SAFETY: AllocatePages returned success, so the region is
        // valid for `pages * PAGE_SIZE` bytes and properly aligned.
        unsafe {
            core::ptr::write_bytes(ptr, 0, pages * PAGE_SIZE);
        }
        (ptr as u64, phys)
    }

    unsafe fn dma_dealloc(paddr: PhysAddr, _vaddr: NonNull<u8>, pages: usize) -> i32 {
        let Some(ptr) = NonNull::new(paddr as *mut u8) else {
            return -1;
        };
        // SAFETY: `paddr` was returned by our own `dma_alloc` and has
        // not yet been freed; `pages` matches the allocation size.
        unsafe {
            if boot::free_pages(ptr, pages).is_err() {
                return -1;
            }
        }
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        // UEFI keeps BAR windows identity-mapped, so the physical
        // address is already a valid virtual one.
        NonNull::new(paddr as *mut u8).expect("mmio_phys_to_virt: null MMIO address")
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        // Identity mapping under UEFI; the buffer's virtual address
        // *is* its physical address.
        buffer.as_ptr() as *mut u8 as u64
    }

    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {
        // No-op for the identity-mapped fast path. SEV-SNP DMA unmap
        // will land here when we switch to PciRootBridgeIo map/unmap.
    }
}
