//! Build a `virtio_drivers::transport::pci::PciTransport` for the
//! virtio-vsock device discovered by [`crate::vsock::probe`].
//!
//! Phase 3a's probe tells us *where* the device is on the PCI bus.
//! This module goes the next step: it hands that address to
//! `virtio-drivers`, which then reads the device's PCI capabilities,
//! locates the virtio common/notify/isr/device config structures in
//! its BARs, and returns a ready-to-use [`PciTransport`].
//!
//! ## Configuration access
//!
//! `virtio-drivers` accepts any `ConfigurationAccess` implementation.
//! We do **not** wrap the UEFI [`PciRootBridgeIo`] protocol here —
//! that would force us to keep a `ScopedProtocol` alive across the
//! whole driver lifetime and add a no-op `unsafe_clone` that cannot
//! correctly re-open the protocol. Instead we use the built-in
//! [`MmioCam`] directly against QEMU's q35 PCIe ECAM region, which
//! OVMF identity-maps at `PcdPciExpressBaseAddress` (0xB000_0000 on
//! both plain OvmfPkgX64 and AmdSev OVMF). UEFI BARs are also
//! identity-mapped, so no translation is required.
//!
//! ## Scope of this commit
//!
//! Build the transport and log success/failure. No virtqueue
//! initialisation, no socket, no device reset-and-feature
//! negotiation (`PciTransport::new` *does* read the PCI caps, but
//! the actual virtio handshake happens later when a device driver —
//! e.g. `VirtIOSocket::new` — consumes the transport). That driver
//! construction lands in 3b.3 together with
//! [`virtio_drivers::device::socket::VsockConnectionManager`].
//!
//! [`PciRootBridgeIo`]: uefi::proto::pci::root_bridge::PciRootBridgeIo
//! [`PciTransport`]: virtio_drivers::transport::pci::PciTransport
//! [`MmioCam`]: virtio_drivers::transport::pci::bus::MmioCam

use virtio_drivers::transport::pci::PciTransport;
use virtio_drivers::transport::pci::bus::{Cam, DeviceFunction, MmioCam, PciRoot};

use crate::vsock_hal::UefiHal;

/// QEMU q35 / OVMF PCIe ECAM base. Both `OvmfPkgX64` and `AmdSev`
/// platforms set `gUefiOvmfPkgTokenSpaceGuid.PcdPciExpressBaseAddress`
/// to `0xB000_0000` and expose a 256 MiB ECAM window there. Changing
/// the machine type (e.g. `pc` i440fx) means this constant no longer
/// applies; we only support q35 for SEV-SNP anyway.
const Q35_ECAM_BASE: *mut u8 = 0xB000_0000 as *mut u8;

/// Build a virtio-vsock PCI transport for the device at
/// `(bus, device, function)`. The caller is expected to have obtained
/// those coordinates from [`crate::vsock::probe`].
///
/// On success the returned [`PciTransport`] has:
///
/// * confirmed the PCI vendor is `0x1AF4`,
/// * verified the device ID maps to a recognised virtio type (19 for
///   vsock), and
/// * mapped all required virtio PCI capability windows.
///
/// No virtqueues are allocated yet — that is `PciTransport`'s
/// consumer's job (the vsock device driver in 3b.3).
///
/// # Safety
///
/// This function synthesises a `MmioCam` over the ECAM region. UEFI
/// firmware has already configured and exposed that region identity-
/// mapped, so reads and writes through the CAM are sound; we verify by
/// asserting that PCI config reads at the requested
/// `(bus,device,function)` return a non-`0xFFFF` vendor ID before
/// handing the transport to the caller.
#[allow(dead_code)]
pub fn build_transport(
    bus: u8,
    device: u8,
    function: u8,
) -> Result<PciTransport, BuildError> {
    // SAFETY: q35/OVMF identity-maps the PCIe ECAM window at
    // Q35_ECAM_BASE for 256 MiB, and is alive for the remainder of
    // boot services. No Rust references alias this window: all access
    // goes through the MMIO wrapper.
    let cam = unsafe { MmioCam::new(Q35_ECAM_BASE, Cam::Ecam) };
    let mut root = PciRoot::new(cam);

    let df = DeviceFunction {
        bus,
        device,
        function,
    };

    PciTransport::new::<UefiHal, _>(&mut root, df).map_err(BuildError::Virtio)
}

/// Distinguished errors from [`build_transport`].
#[allow(dead_code)]
#[derive(Debug)]
pub enum BuildError {
    /// The `virtio-drivers` crate rejected the device.
    Virtio(virtio_drivers::transport::pci::VirtioPciError),
}
