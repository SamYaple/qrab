//! Glue between the phase-3a PCI probe and the phase-3b PCI
//! transport + VirtIO vsock driver.
//!
//! This module owns the one thing `virtio-drivers` asks of us at the
//! driver layer: take a ready-made [`PciTransport`], wrap it in a
//! [`VirtIOSocket`], and hand the result to a
//! [`VsockConnectionManager`] that keeps track of open connections.
//!
//! The `VirtIOSocket::new` call is where the actual virtio handshake
//! happens (feature negotiation, virtqueue allocation via our
//! [`crate::vsock_hal::UefiHal`], device `DRIVER_OK`). After it
//! returns, the guest's CID is known and the manager is ready to
//! [`connect`] — which we defer to the next commit.
//!
//! Failure modes here log a distinct token and bubble up so callers
//! can branch cleanly (attestation mode vs dev-bypass mode).
//!
//! [`PciTransport`]: virtio_drivers::transport::pci::PciTransport
//! [`VirtIOSocket`]: virtio_drivers::device::socket::VirtIOSocket
//! [`VsockConnectionManager`]: virtio_drivers::device::socket::VsockConnectionManager
//! [`connect`]: virtio_drivers::device::socket::VsockConnectionManager::connect

use virtio_drivers::device::socket::{VirtIOSocket, VsockConnectionManager};
use virtio_drivers::transport::pci::PciTransport;

use crate::vsock::Status;
use crate::vsock_hal::UefiHal;
use crate::vsock_pci::build_transport;

/// Receive-ring buffer size for a single virtio-vsock packet. Matches
/// the default used by `virtio-drivers` examples; big enough for the
/// 40-byte VirtioVsockHdr + our small attestation payloads.
const RX_BUFFER_SIZE: usize = 1024;

/// Handle returned by [`init`] — owns the transport, driver, and
/// connection bookkeeping. Dropping it returns the virtio device to a
/// clean state; for our fail-closed boot we just keep it alive until
/// `hlt`.
#[allow(dead_code)]
pub struct VsockDriver {
    /// Guest CID reported by the device after feature negotiation.
    pub guest_cid: u64,
    /// The connection manager. Phase 3b.4+ will `connect` / `send` /
    /// `poll` / `recv` through this to reach the host attestation
    /// endpoint.
    pub manager: VsockConnectionManager<UefiHal, PciTransport, RX_BUFFER_SIZE>,
}

/// Distinguished errors from [`init`]. The log tokens at call sites
/// should mention these so the smoke harness can grep the cause.
#[allow(dead_code)]
#[derive(Debug)]
pub enum InitError {
    /// Phase-3a probe did not find a virtio-vsock device.
    DeviceMissing,
    /// `virtio-drivers` failed to build a PCI transport (no caps,
    /// unknown device type, etc).
    Transport(crate::vsock_pci::BuildError),
    /// `VirtIOSocket::new` rejected the device during feature
    /// negotiation or virtqueue setup.
    Driver(virtio_drivers::Error),
}

/// Turn a phase-3a probe result into a live [`VsockDriver`], logging
/// tokens along the way:
///
/// * `QRAB_VSOCK_DRIVER_OK cid=<n>` on success.
/// * `QRAB_VSOCK_DRIVER_DEVICE_MISSING` when the probe said so.
/// * `QRAB_VSOCK_DRIVER_TRANSPORT_ERR <err>` on [`BuildError`].
/// * `QRAB_VSOCK_DRIVER_SOCKET_ERR <err>` on virtio handshake
///   failure.
///
/// The function is `#[allow(dead_code)]` only until `efi_main` calls
/// it in a subsequent tiny commit; the build currently still does not
/// exercise the virtio handshake at runtime.
#[allow(dead_code)]
pub fn init(status: Status) -> Result<VsockDriver, InitError> {
    let (bus, device, function) = match status {
        Status::Found { bus, dev, fun } => (bus, dev, fun),
        Status::Missing => {
            log::warn!("QRAB_VSOCK_DRIVER_DEVICE_MISSING");
            return Err(InitError::DeviceMissing);
        }
    };

    let transport = build_transport(bus, device, function).map_err(|e| {
        log::warn!("QRAB_VSOCK_DRIVER_TRANSPORT_ERR {e:?}");
        InitError::Transport(e)
    })?;

    let driver: VirtIOSocket<UefiHal, PciTransport, RX_BUFFER_SIZE> =
        VirtIOSocket::new(transport).map_err(|e| {
            log::warn!("QRAB_VSOCK_DRIVER_SOCKET_ERR {e:?}");
            InitError::Driver(e)
        })?;

    let guest_cid = driver.guest_cid();
    let manager = VsockConnectionManager::new(driver);

    log::info!("QRAB_VSOCK_DRIVER_OK cid={guest_cid}");

    Ok(VsockDriver {
        guest_cid,
        manager,
    })
}
