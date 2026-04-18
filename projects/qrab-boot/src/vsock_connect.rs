//! Blocking connect helper for the virtio-vsock connection manager.
//!
//! [`VsockConnectionManager::connect`] only kicks off the handshake:
//! it sends a `VIRTIO_VSOCK_OP_REQUEST` on the driver's TX queue and
//! returns immediately. The guest then has to pump the RX queue via
//! [`poll`] until the device emits the matching `Connected` event.
//! This module wraps that loop in a bounded-retry helper so callers
//! can treat a successful connect as a straight-line operation.
//!
//! No async runtime required: we spin, yield the CPU via `pause`
//! inside each iteration, and give up after `MAX_POLL_ITERATIONS` of
//! no `Connected` event. The limit is tuned for QEMU TCG — fast
//! enough to be unobtrusive on KVM, long enough that a backgrounded
//! host-side server has time to `accept`.
//!
//! [`VsockConnectionManager::connect`]:
//!     virtio_drivers::device::socket::VsockConnectionManager::connect
//! [`poll`]:
//!     virtio_drivers::device::socket::VsockConnectionManager::poll

use core::arch::asm;

use virtio_drivers::device::socket::{VsockAddr, VsockEventType};

use crate::vsock_driver::VsockDriver;

/// How many poll iterations to spend waiting for the host to accept
/// the connection. Each iteration is one `VsockConnectionManager::poll`
/// + a `pause`; under TCG that's a few tens of microseconds, so ~8000
/// iterations caps us at roughly a quarter-second before giving up.
const MAX_POLL_ITERATIONS: usize = 8_000;

/// Distinguished errors from [`connect_and_wait`].
#[allow(dead_code)]
#[derive(Debug)]
pub enum ConnectError {
    /// `VsockConnectionManager::connect` itself failed (resource
    /// exhaustion, buggy device, duplicate src_port, ...).
    Request(virtio_drivers::Error),
    /// We drained the poll budget without seeing the matching
    /// `Connected` event.
    Timeout,
    /// `poll` surfaced an error mid-wait.
    Poll(virtio_drivers::Error),
}

/// Open a vsock stream to `(cid, port)` using `src_port` as the
/// guest-side source port. Blocks until the host accepts the
/// connection or [`MAX_POLL_ITERATIONS`] elapses.
///
/// Logs one of:
///
///   QRAB_VSOCK_CONNECT_OK cid=<cid> port=<port>
///   QRAB_VSOCK_CONNECT_TIMEOUT cid=<cid> port=<port>
///   QRAB_VSOCK_CONNECT_REQ_ERR <err>
///   QRAB_VSOCK_CONNECT_POLL_ERR <err>
///
/// so the smoke harness can pinpoint the failure mode.
#[allow(dead_code)]
pub fn connect_and_wait(
    driver: &mut VsockDriver,
    cid: u64,
    port: u32,
    src_port: u32,
) -> Result<(), ConnectError> {
    let dst = VsockAddr { cid, port };

    driver.manager.connect(dst, src_port).map_err(|e| {
        log::warn!("QRAB_VSOCK_CONNECT_REQ_ERR {e:?}");
        ConnectError::Request(e)
    })?;

    for _ in 0..MAX_POLL_ITERATIONS {
        match driver.manager.poll() {
            Ok(Some(event)) => {
                if event.source == dst
                    && event.destination.port == src_port
                    && matches!(event.event_type, VsockEventType::Connected)
                {
                    log::info!("QRAB_VSOCK_CONNECT_OK cid={cid} port={port}");
                    return Ok(());
                }
            }
            Ok(None) => {}
            Err(e) => {
                log::warn!("QRAB_VSOCK_CONNECT_POLL_ERR {e:?}");
                return Err(ConnectError::Poll(e));
            }
        }
        // SAFETY: `pause` is an unprivileged hint that is always
        // safe to execute at any x86 privilege level.
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }

    log::warn!("QRAB_VSOCK_CONNECT_TIMEOUT cid={cid} port={port}");
    Err(ConnectError::Timeout)
}
