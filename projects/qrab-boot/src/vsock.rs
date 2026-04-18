//! Virtio-vsock device discovery (phase 3a).
//!
//! This module walks every PCI root bridge exposed by the firmware
//! (`EFI_PCI_ROOT_BRIDGE_IO_PROTOCOL`) and scans its [bus 0..=255, dev
//! 0..=31, fn 0..=7] space for a vsock-capable virtio device. The
//! presence or absence of the device is logged via the serial console
//! so the integration test harness can gate on it.
//!
//! Phase 3a stops at discovery: no BAR mapping, no virtqueue
//! allocation, no [`virtio_drivers::transport::pci::PciTransport`]
//! construction. Phase 3b picks those up and plugs in
//! [`virtio_drivers::device::socket::VsockConnectionManager`].
//!
//! ## Why walk PCI ourselves?
//!
//! `virtio-drivers` does offer a PCI enumeration helper, but it
//! requires a `ConfigurationAccess` backed by raw ECAM/PIO. UEFI gives
//! us a paved path through `EFI_PCI_ROOT_BRIDGE_IO_PROTOCOL` that
//! already knows the correct root bridge topology, SEV-SNP-aware DMA
//! attributes, and IOMMU policy. Using it keeps phase 3a tiny and lets
//! phase 3b build its `ConfigurationAccess` around the same protocol
//! rather than probing memory directly.

use uefi::boot::{self, ScopedProtocol};
use uefi::proto::pci::PciIoAddress;
use uefi::proto::pci::root_bridge::PciRootBridgeIo;

/// Red Hat / virtio PCI vendor ID.
const VIRTIO_VENDOR_ID: u16 = 0x1AF4;

/// Modern (virtio 1.0+) device ID for `virtio-vsock`.
/// Legacy / transitional devices are intentionally unsupported; they
/// do not expose the modern PCI capabilities layout that
/// `virtio-drivers` needs.
const VIRTIO_VSOCK_DEVICE_ID: u16 = 0x1053;

/// Result of [`probe`].
//
// Phase 3a only reads these via the `log` call; phase 3b will consume
// them when building the `PciTransport`. Silence the dead-code lint in
// the meantime rather than changing the API twice.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Status {
    /// At least one virtio-vsock device was located.
    Found {
        /// PCI bus/device/function where the first hit lives.
        bus: u8,
        dev: u8,
        fun: u8,
    },
    /// The bus was walked without finding a virtio-vsock device.
    Missing,
}

/// Scan every root bridge for virtio-vsock. Emits one serial log line
/// describing the outcome; returns the parsed status for programmatic
/// use.
pub fn probe() -> Status {
    let handles = match boot::locate_handle_buffer(boot::SearchType::from_proto::<PciRootBridgeIo>(
    )) {
        Ok(h) => h,
        Err(e) => {
            log::warn!("QRAB_VSOCK_NO_PCI_ROOT_BRIDGE_IO: {e:?}");
            return Status::Missing;
        }
    };

    for &handle in handles.iter() {
        // `open_protocol_exclusive` would disconnect the firmware-side
        // PCI bus driver that is probably already holding this
        // protocol open; we only want a read handle, so use the
        // non-disruptive `GetProtocol` attribute.
        let bridge = unsafe {
            boot::open_protocol::<PciRootBridgeIo>(
                boot::OpenProtocolParams {
                    handle,
                    agent: boot::image_handle(),
                    controller: None,
                },
                boot::OpenProtocolAttributes::GetProtocol,
            )
        };
        let bridge = match bridge {
            Ok(b) => b,
            Err(e) => {
                log::warn!("QRAB_VSOCK_BRIDGE_OPEN_FAIL: {e:?}");
                continue;
            }
        };
        if let Some(hit) = scan_bridge(bridge) {
            log::info!(
                "QRAB_VSOCK_FOUND bus={} dev={} fun={}",
                hit.0,
                hit.1,
                hit.2,
            );
            return Status::Found {
                bus: hit.0,
                dev: hit.1,
                fun: hit.2,
            };
        }
    }

    log::info!("QRAB_VSOCK_MISSING");
    Status::Missing
}

/// Return `(bus, dev, fun)` of the first virtio-vsock device on this
/// bridge, or `None` if absent.
fn scan_bridge(mut bridge: ScopedProtocol<PciRootBridgeIo>) -> Option<(u8, u8, u8)> {
    // `PciRootBridgeIo::pci()` returns a handle that accepts full
    // `PciIoAddress`es without first restricting us to a specific
    // segment; we use it as the uniform config-space accessor. On
    // firmware with multiple segments the protocol is instantiated per
    // segment by the firmware, so iterating `handles` already covers
    // the multi-segment case.
    let pci = bridge.pci();

    // PCI spec allows 256 buses × 32 devices × 8 functions, but that
    // is ~65k firmware calls per bus and under QEMU+TCG a naive scan
    // takes tens of seconds. Two cheap tricks bring the hot path down
    // to a few hundred reads:
    //
    //   1. Skip the remaining 7 functions on a device as soon as
    //      function 0 answers with vendor ID 0xFFFF (standard "no
    //      device" sentinel). Multi-function devices always populate
    //      function 0, so `f0=0xFFFF` guarantees no siblings.
    //
    //   2. Stop walking buses as soon as a bus is entirely empty
    //      (every dev/fun returned 0xFFFF). q35 only ever populates
    //      bus 0 + a small number of PCIe root-port downstream buses,
    //      and early-out after the first empty bus cuts runtime to
    //      the minimum without hard-coding a bus limit.
    let mut empty_buses = 0u8;
    for bus in 0u8..=255 {
        let mut any_on_bus = false;
        for dev in 0u8..32 {
            for fun in 0u8..8 {
                let addr = PciIoAddress::new(bus, dev, fun);
                let vendor: u16 = match pci.read_one(addr) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if vendor == 0xFFFF {
                    // If function 0 is empty the whole device slot is
                    // empty; short-circuit the remaining 7 functions.
                    if fun == 0 {
                        break;
                    }
                    continue;
                }
                any_on_bus = true;
                if vendor != VIRTIO_VENDOR_ID {
                    continue;
                }
                let device: u16 = match pci.read_one(addr.with_register(0x2)) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if device == VIRTIO_VSOCK_DEVICE_ID {
                    return Some((bus, dev, fun));
                }
            }
        }
        if any_on_bus {
            empty_buses = 0;
        } else {
            empty_buses = empty_buses.saturating_add(1);
            // Two consecutive empty buses is our stop condition: any
            // legitimate bridge tops out well before that gap.
            if empty_buses >= 2 {
                break;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    // Host-side tests cannot exercise `probe` (it depends on
    // UEFI boot services); phase 3b will add integration coverage via
    // QEMU + a mock vhost-vsock server.
    //
    // This block exists only to pin the public surface so accidental
    // removals are caught at `cargo check` time on the UEFI target.
    #[allow(dead_code)]
    fn _surface_check() {
        let _v: &[super::Status] = &[
            super::Status::Missing,
            super::Status::Found {
                bus: 0,
                dev: 0,
                fun: 0,
            },
        ];
    }
}
