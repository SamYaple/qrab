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

use uefi::boot;
use uefi::proto::pci::root_bridge::PciRootBridgeIo;
use uefi_raw::protocol::pci::root_bridge::PciRootBridgeIoProtocol;
use virtio_drivers::device::socket::{VirtIOSocket, VsockConnectionManager};

use crate::pci_dma;
use crate::volatile_transport::VolatilePciTransport;
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
    pub manager: VsockConnectionManager<UefiHal, VolatilePciTransport, RX_BUFFER_SIZE>,
}

/// Distinguished errors from [`init`]. The log tokens at call sites
/// should mention these so the smoke harness can grep the cause.
#[allow(dead_code)]
#[derive(Debug)]
pub enum InitError {
    /// Phase-3a probe did not find a virtio-vsock device.
    DeviceMissing,
    /// Firmware did not surface any `PciRootBridgeIo` handles, or
    /// the open-protocol call failed. Without the bridge the DMA
    /// HAL cannot reach `IoMmuDxe`'s Map/Unmap entry points under
    /// SNP, so we bail before starting the virtio handshake.
    NoBridge,
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

    // Open PciRootBridgeIo for the DMA HAL's lifetime. Under AmdSev
    // OVMF this protocol routes `Map`/`Unmap`/`AllocateBuffer`/
    // `FreeBuffer` through `IoMmuDxe`, which clears the C-bit on
    // allocated pages and bounce-buffers plaintext through shared
    // memory. On plain TCG OVMF the same calls short-circuit to
    // identity-mapping inside `PciHostBridgeDxe`, so TCG and SNP
    // share a single code path downstream.
    //
    // We deliberately leak the `ScopedProtocol` once we have the raw
    // pointer: `close_protocol` at Drop would invalidate the HAL's
    // stashed pointer, and qrab-boot never returns to firmware
    // (halt_forever on all paths), so the "leak" is the correct
    // choice here. `build_transport` still opens its own short-lived
    // handle for the MCH PCIEXBAR read; concurrent `GetProtocol`
    // opens are ref-counted by firmware and incur no interference.
    hal_bridge_init()?;

    let transport = build_transport(bus, device, function).map_err(|e| {
        log::warn!("QRAB_VSOCK_DRIVER_TRANSPORT_ERR {e:?}");
        InitError::Transport(e)
    })?;
    // Wrap the raw `PciTransport` in `VolatilePciTransport` so every
    // `config_generation` sample is fenced through `black_box` before
    // LLVM can fold it into a `cmp r/m, r` instruction that OVMF's
    // #VC MMIO decoder cannot handle under SEV-SNP. See
    // `volatile_transport` module docs for the full story and
    // `issues/qrab-boot-snp-high-mmio-gp.md` for the upstream
    // tracking ticket.
    let transport = VolatilePciTransport::new(transport);

    let driver: VirtIOSocket<UefiHal, VolatilePciTransport, RX_BUFFER_SIZE> =
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

/// Open the first available `PciRootBridgeIo` handle, hand its raw
/// FFI pointer to [`pci_dma::init`], and `mem::forget` the
/// `ScopedProtocol` so the firmware keeps the interface alive for
/// the remainder of boot.
///
/// The `PciRootBridgeIo` wrapper is `#[repr(transparent)]` around
/// `PciRootBridgeIoProtocol` (see
/// `uefi-0.36.1/src/proto/pci/root_bridge.rs`), so casting
/// `&mut PciRootBridgeIo` to `*mut PciRootBridgeIoProtocol` is
/// layout-stable. After casting, the `ScopedProtocol` is leaked with
/// `mem::forget` so its `Drop` impl (which would call
/// `close_protocol` and invalidate the raw pointer) never runs.
///
/// `pci_dma::init` must precede `build_transport` *and*
/// `VirtIOSocket::new`, because `virtio-drivers` calls `dma_alloc` /
/// `share` during virtqueue setup and those will fault with
/// `QRAB_DMA_NOT_READY` if the bridge pointer is still null.
fn hal_bridge_init() -> Result<(), InitError> {
    let handles =
        boot::locate_handle_buffer(boot::SearchType::from_proto::<PciRootBridgeIo>())
            .map_err(|e| {
                log::warn!("QRAB_VSOCK_DRIVER_NO_BRIDGE locate={e:?}");
                InitError::NoBridge
            })?;
    let Some(&handle) = handles.first() else {
        log::warn!("QRAB_VSOCK_DRIVER_NO_BRIDGE handles=empty");
        return Err(InitError::NoBridge);
    };
    // SAFETY: `handle` was returned by `locate_handle_buffer` for
    // `PciRootBridgeIo`; `image_handle` is qrab-boot's own loaded
    // image; `GetProtocol` attribute asks for a non-exclusive
    // handle that does not perturb the firmware-side PCI bus
    // driver's hold.
    let mut bridge = unsafe {
        boot::open_protocol::<PciRootBridgeIo>(
            boot::OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            },
            boot::OpenProtocolAttributes::GetProtocol,
        )
    }
    .map_err(|e| {
        log::warn!("QRAB_VSOCK_DRIVER_NO_BRIDGE open={e:?}");
        InitError::NoBridge
    })?;

    // `PciRootBridgeIo` is `#[repr(transparent)]` around
    // `PciRootBridgeIoProtocol`, so `&mut PciRootBridgeIo` (obtained
    // here via `ScopedProtocol`'s `DerefMut`) is layout-compatible
    // with `*mut PciRootBridgeIoProtocol`.
    let raw: *mut PciRootBridgeIoProtocol =
        core::ptr::from_mut::<PciRootBridgeIo>(&mut *bridge).cast();

    // Hand the pointer to the HAL *before* leaking the
    // ScopedProtocol: once leaked we no longer have a Rust-visible
    // alias, which sidesteps any further aliasing concern with
    // `pci_dma`'s static.
    pci_dma::init(raw);

    // Leak the ScopedProtocol: its Drop would close_protocol, which
    // would invalidate the pointer `pci_dma` just stashed. qrab-boot
    // never exits boot services, so no resource leaks in practice.
    core::mem::forget(bridge);
    Ok(())
}
