//! Newtype wrapper around [`virtio_drivers::transport::pci::PciTransport`]
//! that forces the compiler to materialise every `config_generation`
//! read through a register before the next instruction sees it.
//!
//! ## Why
//!
//! Under SEV-ES / SEV-SNP the `#VC` handler in OVMF's
//! `CcExitLib::MmioExit` only decodes MOV-family opcodes (`0x88`-`0x8B`,
//! `0xA0`-`0xA3`, `0xC6`/`0xC7`, `0xB6`/`0xB7`, `0xBE`/`0xBF`). Any
//! other instruction that references an MMIO operand falls through the
//! switch, the handler returns cannot-decode, and the exception is
//! re-delivered to the guest as `#GP`.
//!
//! The default impl of [`Transport::read_consistent`] is:
//!
//! ```ignore
//! loop {
//!     let before = self.read_config_generation();
//!     let result = f();
//!     let after  = self.read_config_generation();
//!     if before == after { break result; }
//! }
//! ```
//!
//! `read_config_generation` goes through
//! `field_shared!(common_cfg, config_generation).read().into()` which
//! does bottom out in `core::ptr::read_volatile` via `safe-mmio`
//! 0.3's `volatile_mmio` module. Volatile reads cannot be elided, but
//! the x86_64 LLVM backend is allowed to fold a volatile load that is
//! immediately compared with another value into a single
//! `cmp r/m8, r8` instruction (opcode `0x3A`) — and that's the one
//! OVMF's `MmioExit` does not decode.
//!
//! Verified on 2026-04-18 against `qrab-boot` release binaries:
//! `llvm-objdump` showed the faulting RIP landed on
//! `41 3a 54 24 15` = `cmpb 0x15(%r12), %dl`, with `%r12` holding the
//! virtio common-cfg BAR and `%dl` carrying the value `before` had
//! stashed. The second `read_config_generation` had been folded.
//!
//! ## Fix
//!
//! This wrapper overrides [`Transport::read_consistent`] so every
//! `config_generation` sample passes through
//! [`core::hint::black_box`] before the equality test. `black_box`
//! lowers to `asm!("", inout(reg) x)`, which forces LLVM to spill the
//! value into a GPR and treat the subsequent compare as an unknown
//! register-vs-register compare (`cmp r8, r8`, opcode `0x38`/`0x39`
//! with reg modr/m) rather than a fused memory compare. The volatile
//! MMIO load is still emitted as a plain `MOV r, r/m` which OVMF's
//! `#VC` handler decodes just fine.
//!
//! Every other `Transport` method is delegated straight to the inner
//! [`PciTransport`]; none of them observed the same folding in the
//! faulting traces (writes compile to `MOV` / `MOV imm`, the single
//! `get_status` read goes via a plain `MOV` + bit test).
//!
//! ## Upstream fix to revisit
//!
//! This wrapper is a *workaround*, not a proper fix. The root cause
//! is OVMF's narrow MMIO-opcode decoder. Once
//! `CcExitLib::MmioExit` learns to decode CMP / SUB / AND / TEST with
//! a memory operand — tracked as option 1 in
//! `issues/qrab-boot-snp-high-mmio-gp.md` — this shim can be deleted
//! in favour of using `virtio_drivers::transport::pci::PciTransport`
//! directly again.

use core::hint::black_box;

use virtio_drivers::transport::pci::PciTransport;
use virtio_drivers::transport::{DeviceStatus, DeviceType, InterruptStatus, Transport};
use virtio_drivers::{PhysAddr, Result};
use zerocopy::{FromBytes, Immutable, IntoBytes};

/// Newtype wrapper over [`PciTransport`] whose
/// [`Transport::read_consistent`] is compiler-fence-hardened against
/// the CMP-with-memory instruction form that OVMF's `#VC` handler
/// cannot decode under SEV-SNP. See the module docs for the full
/// story.
pub struct VolatilePciTransport(PciTransport);

impl VolatilePciTransport {
    /// Wrap a ready-made [`PciTransport`]. The caller retains
    /// ownership via the returned wrapper; there is no un-wrap path
    /// because virtio-drivers consumes the transport by value.
    pub fn new(inner: PciTransport) -> Self {
        Self(inner)
    }
}

// SAFETY: `VolatilePciTransport` only forwards method calls to its
// `PciTransport`; the inner type already provides the
// `Send + Sync` invariants required by virtio-drivers' transport.
unsafe impl Send for VolatilePciTransport {}
// SAFETY: see above.
unsafe impl Sync for VolatilePciTransport {}

impl Transport for VolatilePciTransport {
    fn device_type(&self) -> DeviceType {
        self.0.device_type()
    }

    fn read_device_features(&mut self) -> u64 {
        self.0.read_device_features()
    }

    fn write_driver_features(&mut self, driver_features: u64) {
        // virtio 1.1 §3.1.1 step 6: when the device offers
        // `VIRTIO_F_ACCESS_PLATFORM` (bit 33), the driver is
        // required to acknowledge it by writing the bit back
        // verbatim as part of the negotiated feature set. QEMU turns
        // ACCESS_PLATFORM on implicitly for every virtio-pci device
        // behind a SEV-SNP guest (and for iommu-platform=on
        // generally); if the driver clears the bit, the device may
        // stall silently without setting FAILED.
        //
        // `virtio-drivers` 0.13's `SUPPORTED_FEATURES` for vsock
        // does not list `ACCESS_PLATFORM`, so the
        // `negotiated_features` handed to `begin_init` never carries
        // it. We OR the bit back in at the write boundary. Our
        // `UefiHal` always goes through `pci_dma::map` (i.e.
        // `PciRootBridgeIo::Map`) so the platform-mediated DMA
        // contract is honoured regardless of whether virtio-drivers
        // knows we accepted the bit.
        const VIRTIO_F_ACCESS_PLATFORM: u64 = 1 << 33;
        self.0
            .write_driver_features(driver_features | VIRTIO_F_ACCESS_PLATFORM);
    }

    fn max_queue_size(&mut self, queue: u16) -> u32 {
        self.0.max_queue_size(queue)
    }

    fn notify(&mut self, queue: u16) {
        self.0.notify(queue);
    }

    fn get_status(&self) -> DeviceStatus {
        self.0.get_status()
    }

    fn set_status(&mut self, status: DeviceStatus) {
        self.0.set_status(status);
    }

    fn set_guest_page_size(&mut self, guest_page_size: u32) {
        self.0.set_guest_page_size(guest_page_size);
    }

    fn requires_legacy_layout(&self) -> bool {
        self.0.requires_legacy_layout()
    }

    fn queue_set(
        &mut self,
        queue: u16,
        size: u32,
        descriptors: PhysAddr,
        driver_area: PhysAddr,
        device_area: PhysAddr,
    ) {
        self.0
            .queue_set(queue, size, descriptors, driver_area, device_area);
    }

    fn queue_unset(&mut self, queue: u16) {
        self.0.queue_unset(queue);
    }

    fn queue_used(&mut self, queue: u16) -> bool {
        self.0.queue_used(queue)
    }

    fn ack_interrupt(&mut self) -> InterruptStatus {
        self.0.ack_interrupt()
    }

    fn read_config_generation(&self) -> u32 {
        // `black_box` forces the returned value through a GPR so any
        // subsequent comparison happens on a register operand and
        // cannot be folded into a `cmp r/m, r` instruction by LLVM.
        // The inner `PciTransport::read_config_generation` still
        // emits a `volatile_load` (a plain `MOV r, r/m` at the x86
        // level, which OVMF's `#VC` handler decodes correctly).
        black_box(self.0.read_config_generation())
    }

    fn read_config_space<T: FromBytes + IntoBytes>(&self, offset: usize) -> Result<T> {
        // Same fence rationale as `read_config_generation` above. The
        // inner `PciTransport::read_config_space` bottoms out in
        // `safe_mmio::Field::<T>::read_unsafe` → `read_volatile`, but
        // LLVM's x86 backend is allowed to fuse a volatile load into
        // whatever follows; observed on SEV-SNP with `read_config!`
        // macro-sites where an inlined u32 load was mis-decoded by
        // OVMF's `#VC` handler and returned silent garbage for the
        // vhost-vsock `guest_cid` field. Feeding the value through
        // `black_box` forces it to be materialised in a GPR before
        // any subsequent use, so the volatile read stays a plain
        // `MOV r, r/m` that OVMF does decode correctly.
        let value = self.0.read_config_space::<T>(offset)?;
        Ok(black_box(value))
    }

    fn write_config_space<T: IntoBytes + Immutable>(
        &mut self,
        offset: usize,
        value: T,
    ) -> Result<()> {
        self.0.write_config_space(offset, value)
    }

    // Override `read_consistent` as belt-and-suspenders: even if some
    // future virtio-drivers version changes the default impl to
    // something that sidesteps our `read_config_generation` hook
    // above, this local copy keeps the explicit `black_box` fences
    // around both samples of the generation counter.
    fn read_consistent<R>(&self, f: impl Fn() -> Result<R>) -> Result<R> {
        loop {
            let before = black_box(self.read_config_generation());
            let result = f();
            let after = black_box(self.read_config_generation());
            if before == after {
                break result;
            }
        }
    }

    // `begin_init` and `finish_init` are default-impl methods on
    // the trait; we do not override them. The default `begin_init`
    // drives writes via `write_driver_features` / `set_status`
    // (which we delegate), so it ends up exercising exactly the
    // same MOV + MOV-imm opcodes the inner `PciTransport` does —
    // no CMP-with-memory folding observed on any of the
    // faulting-run disassemblies.
}
