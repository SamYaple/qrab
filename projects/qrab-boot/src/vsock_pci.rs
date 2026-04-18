//! Build a `virtio_drivers::transport::pci::PciTransport` for the
//! virtio-vsock device discovered by [`crate::vsock::probe`].
//!
//! Phase 3a's probe tells us *where* the device is on the PCI bus.
//! This module goes the next step: it hands that address to
//! `virtio-drivers`, which then reads the device's PCI capabilities,
//! locates the virtio common/notify/isr/device config structures in
//! its BARs, and returns a ready-to-use [`PciTransport`].
//!
//! ## Finding the ECAM window
//!
//! `virtio-drivers` needs an `MmioCam` over the PCIe ECAM window; we
//! do **not** hard-code the address because OVMF's
//! `PcdPciExpressBaseAddress` varies per platform (0xB000_0000 for
//! plain OvmfPkgX64, 0xE000_0000 for AmdSev, potentially other
//! values under bhyve, etc). Instead we read the canonical source of
//! truth at runtime: the MCH's `PCIEXBAR` register
//! (bus=0 dev=0 fn=0 offset 0x60) via UEFI's
//! `EFI_PCI_ROOT_BRIDGE_IO_PROTOCOL`. That protocol is identity-
//! mapped and available throughout boot services.
//!
//! [`PciTransport`]: virtio_drivers::transport::pci::PciTransport
//! [`MmioCam`]: virtio_drivers::transport::pci::bus::MmioCam

use uefi::boot;
use uefi::proto::pci::PciIoAddress;
use uefi::proto::pci::root_bridge::PciRootBridgeIo;
use virtio_drivers::transport::pci::PciTransport;
use virtio_drivers::transport::pci::bus::{Cam, DeviceFunction, MmioCam, PciRoot};

use crate::vsock_hal::UefiHal;

/// Size of a single PCIe ECAM window: 256 MiB, covering 256 buses.
const ECAM_SIZE: u64 = 256 * 1024 * 1024;

/// MCH PCIEXBAR register offset (see Intel q35 datasheet, MCH
/// configuration space). The PCIEXBAR is a 64-bit register split
/// across offsets 0x60 (low dword) and 0x64 (high dword); bits [1:0]
/// are the enable + length fields.
const MCH_PCIEXBAR_OFF: u8 = 0x60;

/// Read the low+high halves of the MCH PCIEXBAR and return the
/// decoded ECAM base address, or `None` if the register looks
/// uninitialised.
fn pciexbar_base() -> Option<u64> {
    log::info!("QRAB_VSOCK_PCIEXBAR_PROBE start");
    let handles = match boot::locate_handle_buffer(
        boot::SearchType::from_proto::<PciRootBridgeIo>(),
    ) {
        Ok(h) => h,
        Err(e) => {
            log::warn!("QRAB_VSOCK_PCIEXBAR locate_handle_buffer err={e:?}");
            return None;
        }
    };
    let Some(&handle) = handles.first() else {
        log::warn!("QRAB_VSOCK_PCIEXBAR no_handles");
        return None;
    };
    // Non-exclusive open matches what `crate::vsock::probe` uses; an
    // `open_protocol_exclusive` here blocks because `PciBusDxe` is
    // already holding the same protocol handle.
    let mut bridge = match unsafe {
        boot::open_protocol::<PciRootBridgeIo>(
            boot::OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            },
            boot::OpenProtocolAttributes::GetProtocol,
        )
    } {
        Ok(b) => b,
        Err(e) => {
            log::warn!("QRAB_VSOCK_PCIEXBAR open_protocol err={e:?}");
            return None;
        }
    };
    let pci = bridge.pci();
    let addr_lo = PciIoAddress::new(0, 0, 0).with_register(MCH_PCIEXBAR_OFF);
    let addr_hi = PciIoAddress::new(0, 0, 0).with_register(MCH_PCIEXBAR_OFF + 4);
    let lo: u32 = pci.read_one(addr_lo).ok()?;
    let hi: u32 = pci.read_one(addr_hi).ok()?;
    let raw = ((hi as u64) << 32) | (lo as u64);
    log::info!("QRAB_VSOCK_PCIEXBAR raw={raw:#x}");
    // Bit 0 is the enable flag; bail if the MCH hasn't lit the window.
    if raw & 1 == 0 {
        return None;
    }
    // Bits [1:0] are control, bits [27:4] are length/address mix
    // depending on LENGTH. For the 256-MiB window the base is the
    // high 36 bits (>>28 << 28). Mask everything below the 28th bit.
    Some(raw & !0x0FFF_FFFFu64)
}

/// Distinguished errors from [`build_transport`].
#[allow(dead_code)]
#[derive(Debug)]
pub enum BuildError {
    /// MCH PCIEXBAR register was not readable or not enabled.
    PciexbarUnavailable,
    /// `virtio-drivers` rejected the device.
    Virtio(virtio_drivers::transport::pci::VirtioPciError),
}

/// Build a virtio-vsock PCI transport for the device at
/// `(bus, device, function)`. The caller is expected to have obtained
/// those coordinates from [`crate::vsock::probe`].
#[allow(dead_code)]
pub fn build_transport(
    bus: u8,
    device: u8,
    function: u8,
) -> Result<PciTransport, BuildError> {
    let ecam_base = pciexbar_base().ok_or(BuildError::PciexbarUnavailable)?;
    log::info!("QRAB_VSOCK_ECAM base={ecam_base:#x} size={ECAM_SIZE:#x}");

    // SAFETY: the MCH reports an enabled 256 MiB ECAM window at
    // `ecam_base`, identity-mapped by UEFI. No Rust references
    // overlap; all access goes through the MMIO wrapper.
    let cam = unsafe { MmioCam::new(ecam_base as *mut u8, Cam::Ecam) };
    let mut root = PciRoot::new(cam);

    let df = DeviceFunction {
        bus,
        device,
        function,
    };

    PciTransport::new::<UefiHal, _>(&mut root, df).map_err(BuildError::Virtio)
}
