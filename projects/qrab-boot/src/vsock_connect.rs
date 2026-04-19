//! Blocking connect + framed-stream helpers for the virtio-vsock
//! connection manager.
//!
//! [`VsockConnectionManager::connect`] only kicks off the handshake:
//! it sends a `VIRTIO_VSOCK_OP_REQUEST` on the driver's TX queue and
//! returns immediately. The guest then has to pump the RX queue via
//! [`poll`] until the device emits the matching `Connected` event.
//! This module wraps that loop in a bounded-retry helper so callers
//! can treat a successful connect as a straight-line operation.
//!
//! Phase 5 lifted the single-shot `send_and_recv` helper with a
//! proper [`FramedStream`] that carries a byte buffer between
//! successive [`FramedStream::recv_frame`] calls. The attestation
//! server writes two length-prefixed payloads back-to-back
//! (`AttestResponse` + UKI bytes); virtio-vsock is a stream, so both
//! arrive as one or more `Received` events with no guarantee on
//! split points. The framed reader reassembles them transparently.
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

use alloc::vec::Vec;
use core::arch::asm;

use virtio_drivers::device::socket::{VsockAddr, VsockEventType};

use crate::vsock_driver::VsockDriver;

/// How many poll iterations to spend waiting for the host to accept
/// the connection. Each iteration is one `VsockConnectionManager::poll`
/// + a `pause`; under TCG that's a few tens of microseconds. 80 000
/// iterations caps the wait at roughly 2.5 s, which is long enough
/// for the `recv_frame` path to drain a 64 KiB payload off the
/// virtio-vsock ring (Plan §scenario 11). The connect-and-wait +
/// server-unreachable scenarios still fail fast because the bailout
/// path hits the first few hundred polls; the extra ceiling only
/// applies to payloads that genuinely take longer to arrive.
const MAX_POLL_ITERATIONS: usize = 80_000;

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

    log::info!("QRAB_VSOCK_CONNECT_ENTER cid={cid} port={port} src_port={src_port}");
    driver.manager.connect(dst, src_port).map_err(|e| {
        log::warn!("QRAB_VSOCK_CONNECT_REQ_ERR {e:?}");
        ConnectError::Request(e)
    })?;
    log::info!("QRAB_VSOCK_CONNECT_REQ_SENT cid={cid} port={port}");

    for i in 0..MAX_POLL_ITERATIONS {
        // Heartbeat every 1000 polls so the smoke harness can tell
        // a hung polling loop apart from a silent crash.
        if i % 1000 == 0 {
            log::info!("QRAB_VSOCK_CONNECT_POLL i={i}");
        }
        match driver.manager.poll() {
            Ok(Some(event)) => {
                log::info!(
                    "QRAB_VSOCK_CONNECT_EVENT i={i} src_cid={} src_port={} dst_port={} type={:?}",
                    event.source.cid,
                    event.source.port,
                    event.destination.port,
                    event.event_type,
                );
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

/// How many *idle* poll iterations (no bytes received) to tolerate
/// between chunks of a framed payload before giving up. The counter
/// resets to zero every time bytes actually arrive, so this limits
/// the gap between host writes, not total transfer time. TCG under
/// heavy load can stall for ~1 s between virtio-vsock rx bursts on
/// large payloads (scenario 11's 64 KiB UKI), so 320 000 iterations
/// buys ~10 s of tolerance while still failing fast when no payload
/// is coming at all (scenarios 6 / attest-server absent).
const MAX_RECV_ITERATIONS: usize = 320_000;

/// Hard ceiling on a single framed payload. Matches
/// `qrab_boot_proto::MAX_FRAMED_PAYLOAD` so an attack that lies
/// about the length prefix cannot convince us to allocate a hostile
/// amount of memory.
const MAX_FRAMED_PAYLOAD: usize = 128 * 1024;

/// Size of the staging buffer for individual `recv` calls. Matches the
/// driver's RX ring entry size, so every `recv` returns the full
/// contents of one Received event without needing multiple trips.
const RECV_CHUNK_BYTES: usize = 1024;

/// Distinguished errors from [`send_one`] / [`FramedStream::recv_frame`].
#[allow(dead_code)]
#[derive(Debug)]
pub enum IoError {
    /// `VsockConnectionManager::send` rejected the write.
    Send(virtio_drivers::Error),
    /// `VsockConnectionManager::recv` rejected the read.
    Recv(virtio_drivers::Error),
    /// `poll` surfaced an error while waiting for the reply.
    Poll(virtio_drivers::Error),
    /// Nothing came back within [`MAX_RECV_ITERATIONS`].
    Timeout,
    /// The peer reported `VsockEventType::Disconnected` instead of
    /// a data event.
    Disconnected,
    /// A frame length prefix exceeded [`MAX_FRAMED_PAYLOAD`].
    FrameTooLarge(usize),
}

/// Transmit `payload` on the established stream. Emits
/// `QRAB_VSOCK_SENT bytes=<n>` on success so the smoke harness can
/// grep for the specific byte count.
///
/// The caller is responsible for having run [`connect_and_wait`]
/// against the same `(cid, port, src_port)` triple first; we do not
/// re-validate here to keep the helper composable.
#[allow(dead_code)]
pub fn send_one(
    driver: &mut VsockDriver,
    cid: u64,
    port: u32,
    src_port: u32,
    payload: &[u8],
) -> Result<(), IoError> {
    let dst = VsockAddr { cid, port };
    driver.manager.send(dst, src_port, payload).map_err(|e| {
        log::warn!("QRAB_VSOCK_SEND_ERR {e:?}");
        IoError::Send(e)
    })?;
    log::info!("QRAB_VSOCK_SENT bytes={}", payload.len());
    Ok(())
}

/// Reassembles length-prefixed frames across multiple `Received`
/// events. A single `FramedStream` is owned by the caller and
/// threaded through successive [`FramedStream::recv_frame`] calls
/// on the same vsock connection, which keeps any bytes a preceding
/// chunk over-delivered (e.g. when the server writes two frames
/// back-to-back and the device coalesces them into one ring slot).
///
/// The reader's frame format matches `qrab_boot_proto::encode_framed`:
/// a 4-byte little-endian `u32` length prefix followed by that many
/// bytes of payload.
#[allow(dead_code)]
pub struct FramedStream {
    carry: Vec<u8>,
}

impl FramedStream {
    /// Create a fresh reader with an empty carry buffer.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { carry: Vec::new() }
    }

    /// Read the next length-prefixed frame, reassembling across
    /// multiple `Received` events. Returns the frame body (prefix
    /// stripped). Stops with [`IoError::Timeout`] if no new bytes
    /// arrive for [`MAX_RECV_ITERATIONS`] iterations.
    ///
    /// Logs `QRAB_VSOCK_FRAME bytes=<n>` for every successful frame,
    /// and mirrors the legacy `send_and_recv` error tokens for
    /// failures so the smoke harness stays simple.
    #[allow(dead_code)]
    pub fn recv_frame(
        &mut self,
        driver: &mut VsockDriver,
        cid: u64,
        port: u32,
        src_port: u32,
    ) -> Result<Vec<u8>, IoError> {
        let dst = VsockAddr { cid, port };
        let mut idle = 0usize;

        // Fast-path: a prior call may have over-delivered a full
        // second frame into `carry`. Try to extract one before
        // spinning on the ring again.
        if let Some(frame) = self.try_extract()? {
            log::info!("QRAB_VSOCK_FRAME bytes={}", frame.len());
            return Ok(frame);
        }

        while idle < MAX_RECV_ITERATIONS {
            match driver.manager.poll() {
                Ok(Some(event)) if event.source == dst => {
                    match event.event_type {
                        VsockEventType::Received { length: _ } => {
                            let mut chunk = [0u8; RECV_CHUNK_BYTES];
                            let n = driver
                                .manager
                                .recv(dst, src_port, &mut chunk)
                                .map_err(|e| {
                                    log::warn!("QRAB_VSOCK_RECV_ERR {e:?}");
                                    IoError::Recv(e)
                                })?;
                            self.carry.extend_from_slice(&chunk[..n]);
                            if let Some(frame) = self.try_extract()? {
                                log::info!("QRAB_VSOCK_FRAME bytes={}", frame.len());
                                return Ok(frame);
                            }
                            // Reset the idle counter only when we actually
                            // got bytes. A hostile/broken host that streams
                            // zero-byte `Received` events must not pin this
                            // loop forever — those still count as idle.
                            if n > 0 {
                                idle = 0;
                            }
                        }
                        VsockEventType::Disconnected { reason } => {
                            log::warn!("QRAB_VSOCK_RECV_DISCONNECT reason={reason:?}");
                            return Err(IoError::Disconnected);
                        }
                        _ => {}
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!("QRAB_VSOCK_RECV_POLL_ERR {e:?}");
                    return Err(IoError::Poll(e));
                }
            }
            idle += 1;
            // SAFETY: `pause` is always legal at any privilege level.
            unsafe {
                asm!("pause", options(nomem, nostack, preserves_flags));
            }
        }

        log::warn!("QRAB_VSOCK_RECV_TIMEOUT cid={cid} port={port}");
        Err(IoError::Timeout)
    }

    /// If `carry` already holds at least one full length-prefixed
    /// frame, consume it and return the body. Returns `Ok(None)` when
    /// more bytes are needed (the common "carry too short" case), and
    /// `Err(FrameTooLarge)` when the declared length would force us
    /// to allocate past `MAX_FRAMED_PAYLOAD`.
    fn try_extract(&mut self) -> Result<Option<Vec<u8>>, IoError> {
        if self.carry.len() < 4 {
            return Ok(None);
        }
        let len_bytes: [u8; 4] = self.carry[..4]
            .try_into()
            .expect("slice length matches array");
        let len = u32::from_le_bytes(len_bytes) as usize;
        if len > MAX_FRAMED_PAYLOAD {
            return Err(IoError::FrameTooLarge(len));
        }
        if self.carry.len() < 4 + len {
            return Ok(None);
        }
        let body: Vec<u8> = self.carry[4..4 + len].to_vec();
        // Shift any remaining bytes down for the next call.
        self.carry.drain(..4 + len);
        Ok(Some(body))
    }
}

impl Default for FramedStream {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy single-frame wrapper: sends `payload`, receives one full
/// framed reply and copies it into `reply`. Kept for callers that
/// only need one round-trip; new call sites should prefer [`send_one`]
/// + [`FramedStream::recv_frame`].
#[allow(dead_code)]
pub fn send_and_recv_framed(
    driver: &mut VsockDriver,
    cid: u64,
    port: u32,
    src_port: u32,
    payload: &[u8],
    reply: &mut [u8],
) -> Result<usize, IoError> {
    send_one(driver, cid, port, src_port, payload)?;
    let mut stream = FramedStream::new();
    let body = stream.recv_frame(driver, cid, port, src_port)?;
    let n = body.len().min(reply.len());
    reply[..n].copy_from_slice(&body[..n]);
    Ok(n)
}
