//! The browser-to-VNC half of the console bridge, parsed message by message.
//! It is the enforcement point for admin-bucket audio (docs/bmc.md#admin-bucket):
//! unless the connection's grants admit audio, the VNC audio pseudo-encoding
//! (-259) is removed from SetEncodings and QEMU audio messages (255/1) are
//! dropped, so QEMU never acknowledges audio or sends any on that connection.
//! Every other message noVNC sends passes unchanged; anything else closes the
//! console.
//!
//! The handshake follows QEMU's VNC server on the private socket, which offers
//! only the None security type: a 12-byte ProtocolVersion (3.3, 3.7 or 3.8),
//! the one-byte security choice (3.7 and 3.8 only) and the one-byte ClientInit.
//! Input may be split or coalesced arbitrarily across WebSocket messages.
//! Buffering is bounded by the largest message the filter holds whole (a
//! SetEncodings of [`MAX_ENCODINGS`]); cut-text payloads stream through.
//!
//! ```
//! use bmc::rfb_filter::{RfbFilter, AUDIO_ENCODING};
//! let mut handshake = b"RFB 003.008\n\x01\x01".to_vec();
//! // SetEncodings [Raw, audio], then audio enable (255/1/0).
//! handshake.extend([2, 0, 0, 2, 0, 0, 0, 0]);
//! handshake.extend(AUDIO_ENCODING.to_be_bytes());
//! handshake.extend([255, 1, 0, 0]);
//! let mut out = Vec::new();
//! RfbFilter::new().feed(&handshake, false, &mut out).unwrap();
//! assert_eq!(out, b"RFB 003.008\n\x01\x01\x02\x00\x00\x01\x00\x00\x00\x00");
//! out.clear();
//! RfbFilter::new().feed(&handshake, true, &mut out).unwrap();
//! assert_eq!(out, handshake);
//! assert!(RfbFilter::new().feed(b"RFB 003.008\n\x01\x01\xfd", true, &mut out).is_err());
//! ```

/// QEMU's VNC audio pseudo-encoding.
pub const AUDIO_ENCODING: i32 = -259;
/// The longest SetEncodings accepted; noVNC sends about twenty.
pub const MAX_ENCODINGS: usize = 1024;
/// ClientFence payloads are at most 64 bytes (RFB community extension).
const MAX_FENCE_PAYLOAD: usize = 64;

/// Why the filter closed the console.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Violation(pub &'static str);

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Version,
    Security,
    ClientInit,
    Messages,
}

/// One console connection's client-to-server parser. After a [`Violation`]
/// every later call fails too.
#[derive(Debug)]
pub struct RfbFilter {
    phase: Phase,
    /// The current unit, until it is complete and decided.
    pending: Vec<u8>,
    /// Cut-text payload bytes still to copy through unchanged.
    passthrough: usize,
    failed: Option<Violation>,
}

impl Default for RfbFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl RfbFilter {
    pub fn new() -> Self {
        Self {
            phase: Phase::Version,
            pending: Vec::new(),
            passthrough: 0,
            failed: None,
        }
    }

    /// Filters `input`, appending what may reach the VNC server to `out`.
    /// `audio` is whether the connection's grants admit audio right now.
    pub fn feed(
        &mut self,
        mut input: &[u8],
        audio: bool,
        out: &mut Vec<u8>,
    ) -> Result<(), Violation> {
        if let Some(violation) = self.failed {
            return Err(violation);
        }
        let result = (|| loop {
            if self.passthrough > 0 {
                if input.is_empty() {
                    return Ok(());
                }
                let n = self.passthrough.min(input.len());
                out.extend_from_slice(&input[..n]);
                input = &input[n..];
                self.passthrough -= n;
                continue;
            }
            let need = self.required()?;
            if self.pending.len() < need {
                if input.is_empty() {
                    return Ok(());
                }
                let n = (need - self.pending.len()).min(input.len());
                self.pending.extend_from_slice(&input[..n]);
                input = &input[n..];
                continue;
            }
            self.complete(audio, out)?;
        })();
        if let Err(violation) = result {
            self.failed = Some(violation);
            self.pending = Vec::new();
        }
        result
    }

    /// The length of the current unit, as far as its pending header tells.
    fn required(&self) -> Result<usize, Violation> {
        let p = &self.pending;
        let u16_at = |i: usize| usize::from(u16::from_be_bytes([p[i], p[i + 1]]));
        Ok(match self.phase {
            Phase::Version => 12,
            Phase::Security | Phase::ClientInit => 1,
            Phase::Messages => match p.first() {
                None => 1,
                // SetPixelFormat
                Some(0) => 20,
                // SetEncodings
                Some(2) if p.len() < 4 => 4,
                Some(2) => {
                    let count = u16_at(2);
                    if count > MAX_ENCODINGS {
                        return Err(Violation("too many encodings"));
                    }
                    4 + 4 * count
                }
                // FramebufferUpdateRequest
                Some(3) => 10,
                // KeyEvent
                Some(4) => 8,
                // PointerEvent
                Some(5) => 6,
                // ClientCutText header; the payload streams through.
                Some(6) => 8,
                // EnableContinuousUpdates
                Some(150) => 10,
                // ClientFence
                Some(248) if p.len() < 9 => 9,
                Some(248) => {
                    let length = usize::from(p[8]);
                    if length > MAX_FENCE_PAYLOAD {
                        return Err(Violation("fence payload too long"));
                    }
                    9 + length
                }
                // xvp
                Some(250) => 4,
                // SetDesktopSize
                Some(251) if p.len() < 8 => 8,
                Some(251) => 8 + 16 * usize::from(p[6]),
                // QEMU client messages
                Some(255) if p.len() < 2 => 2,
                Some(255) => match p[1] {
                    // Extended key event
                    0 => 12,
                    1 if p.len() < 4 => 4,
                    // Audio enable, disable, and set-format
                    1 => match u16_at(2) {
                        0 | 1 => 4,
                        2 => 10,
                        _ => return Err(Violation("unknown QEMU audio operation")),
                    },
                    _ => return Err(Violation("unknown QEMU client message")),
                },
                Some(_) => return Err(Violation("unknown client message type")),
            },
        })
    }

    /// Decides the complete unit in `pending`.
    fn complete(&mut self, audio: bool, out: &mut Vec<u8>) -> Result<(), Violation> {
        let unit = std::mem::take(&mut self.pending);
        let mut forward = true;
        match self.phase {
            Phase::Version => {
                self.phase = match &unit[..] {
                    b"RFB 003.003\n" => Phase::ClientInit,
                    b"RFB 003.007\n" | b"RFB 003.008\n" => Phase::Security,
                    _ => return Err(Violation("unsupported protocol version")),
                };
            }
            Phase::Security => {
                if unit[0] != 1 {
                    return Err(Violation("only the None security type is offered"));
                }
                self.phase = Phase::ClientInit;
            }
            Phase::ClientInit => self.phase = Phase::Messages,
            Phase::Messages => match unit[0] {
                2 if !audio => {
                    let kept: Vec<&[u8]> = unit[4..]
                        .chunks_exact(4)
                        .filter(|e| **e != AUDIO_ENCODING.to_be_bytes())
                        .collect();
                    out.extend_from_slice(&[2, unit[1]]);
                    out.extend_from_slice(&(kept.len() as u16).to_be_bytes());
                    kept.into_iter().for_each(|e| out.extend_from_slice(e));
                    forward = false;
                }
                255 if unit[1] == 1 && !audio => forward = false,
                6 => {
                    let length = i32::from_be_bytes([unit[4], unit[5], unit[6], unit[7]]);
                    self.passthrough = length.unsigned_abs() as usize;
                }
                _ => {}
            },
        }
        if forward {
            out.extend_from_slice(&unit);
        }
        // Reuse the allocation for the next unit.
        self.pending = unit;
        self.pending.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HANDSHAKE: &[u8] = b"RFB 003.008\n\x01\x01";

    fn encodings(list: &[i32]) -> Vec<u8> {
        let mut message = vec![2, 0];
        message.extend((list.len() as u16).to_be_bytes());
        list.iter().for_each(|e| message.extend(e.to_be_bytes()));
        message
    }

    /// Every message type noVNC sends, with QEMU's audio negotiation.
    fn session() -> Vec<u8> {
        let mut bytes = HANDSHAKE.to_vec();
        bytes.extend([
            0, 0, 0, 0, 32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 16, 8, 0, 0, 0, 0,
        ]);
        bytes.extend(encodings(&[
            1,
            0,
            -258,
            AUDIO_ENCODING,
            -313,
            0xc0a1e5ceu32 as i32,
        ]));
        bytes.extend([3, 0, 0, 0, 0, 0, 2, 208, 1, 144]);
        bytes.extend([4, 1, 0, 0, 0, 0, 0, 0x61]);
        bytes.extend([5, 1, 0, 10, 0, 20]);
        bytes.extend([6, 0, 0, 0, 0, 0, 0, 5]);
        bytes.extend(b"hello");
        // Extended clipboard: negative length.
        bytes.extend([6, 0, 0, 0]);
        bytes.extend((-6i32).to_be_bytes());
        bytes.extend([0, 0, 0, 1, 0xfd, 0xfe]);
        bytes.extend([150, 1, 0, 0, 0, 0, 2, 208, 1, 144]);
        bytes.extend([248, 0, 0, 0, 0x80, 0, 0, 0, 3, 1, 2, 3]);
        bytes.extend([250, 0, 1, 4]);
        bytes.extend([251, 0, 2, 208, 1, 144, 1, 0]);
        bytes.extend([0; 16]);
        // QEMU extended key event.
        bytes.extend([255, 0, 0, 1, 0, 0, 0, 0x61, 0, 0, 0, 0x1e]);
        // Audio set-format S16 stereo 44.1 kHz, enable, disable.
        bytes.extend([255, 1, 0, 2, 3, 2, 0, 0, 0xac, 0x44]);
        bytes.extend([255, 1, 0, 0]);
        bytes.extend([255, 1, 0, 1]);
        bytes
    }

    /// `session()` as an unadmitted connection must forward it.
    fn stripped() -> Vec<u8> {
        let full = session();
        let start = HANDSHAKE.len() + 20;
        let mut bytes = full[..start].to_vec();
        bytes.extend(encodings(&[1, 0, -258, -313, 0xc0a1e5ceu32 as i32]));
        let rest = start + 4 + 6 * 4;
        let audio = full.len() - 18;
        bytes.extend(&full[rest..audio]);
        bytes
    }

    fn filter(chunks: &[&[u8]], audio: bool) -> Result<Vec<u8>, Violation> {
        let mut filter = RfbFilter::new();
        let mut out = Vec::new();
        for chunk in chunks {
            filter.feed(chunk, audio, &mut out)?;
        }
        Ok(out)
    }

    #[test]
    fn unadmitted_connections_lose_the_audio_encoding_and_audio_messages() {
        let session = session();
        assert_eq!(filter(&[&session], false).unwrap(), stripped());
        let only_audio = [HANDSHAKE, &encodings(&[AUDIO_ENCODING])[..]].concat();
        assert_eq!(
            filter(&[&only_audio], false).unwrap(),
            [HANDSHAKE, &[2, 0, 0, 0][..]].concat()
        );
    }

    #[test]
    fn admitted_connections_pass_everything_unchanged() {
        let session = session();
        assert_eq!(filter(&[&session], true).unwrap(), session);
    }

    #[test]
    fn split_and_coalesced_input_gives_the_same_stream() {
        let session = session();
        for audio in [false, true] {
            let whole = filter(&[&session], audio).unwrap();
            let bytes: Vec<&[u8]> = session.chunks(1).collect();
            assert_eq!(filter(&bytes, audio).unwrap(), whole);
            for size in [2, 3, 5, 7, 13, 64] {
                let chunks: Vec<&[u8]> = session.chunks(size).collect();
                assert_eq!(filter(&chunks, audio).unwrap(), whole, "{size}");
            }
            // Empty reads change nothing.
            assert_eq!(filter(&[&[], &session, &[]], audio).unwrap(), whole);
        }
    }

    #[test]
    fn extended_key_events_always_pass() {
        let key = [255, 0, 0, 1, 0, 0, 0, 0x61, 0, 0, 0, 0x1e];
        let session = [HANDSHAKE, &key[..]].concat();
        for audio in [false, true] {
            assert_eq!(filter(&[&session], audio).unwrap(), session);
        }
    }

    #[test]
    fn unknown_or_malformed_messages_close_the_console() {
        let after = |tail: &[u8]| [HANDSHAKE, tail].concat();
        for (bytes, reason) in [
            (after(&[1]), "unknown client message type"),
            (after(&[7]), "unknown client message type"),
            (after(&[253]), "unknown client message type"),
            (after(&[255, 2]), "unknown QEMU client message"),
            (after(&[255, 1, 0, 3]), "unknown QEMU audio operation"),
            (after(&[2, 0, 4, 1]), "too many encodings"),
            (
                after(&[248, 0, 0, 0, 0, 0, 0, 0, 65]),
                "fence payload too long",
            ),
            (b"RFB 003.889\n".to_vec(), "unsupported protocol version"),
            (b"RFB 003.005\n".to_vec(), "unsupported protocol version"),
            (
                b"RFB 003.008\n\x02".to_vec(),
                "only the None security type is offered",
            ),
        ] {
            for audio in [false, true] {
                assert_eq!(filter(&[&bytes], audio), Err(Violation(reason)));
            }
        }
        // A violation is final, even for valid bytes afterwards.
        let mut filter = RfbFilter::new();
        let mut out = Vec::new();
        assert!(filter.feed(&after(&[9]), true, &mut out).is_err());
        assert!(filter.feed(&[3; 10], true, &mut out).is_err());
        assert_eq!(out, HANDSHAKE);
    }

    #[test]
    fn rfb_3_3_has_no_security_choice() {
        let mut bytes = b"RFB 003.003\n\x01".to_vec();
        bytes.extend(encodings(&[0, AUDIO_ENCODING]));
        let mut expected = b"RFB 003.003\n\x01".to_vec();
        expected.extend(encodings(&[0]));
        assert_eq!(filter(&[&bytes], false).unwrap(), expected);
    }

    #[test]
    fn cut_text_payloads_stream_without_being_parsed() {
        // A payload that looks like audio messages is still payload.
        let mut bytes = HANDSHAKE.to_vec();
        bytes.extend([6, 0, 0, 0, 0, 0, 0, 8, 255, 1, 0, 0, 255, 1, 0, 0]);
        bytes.extend([255, 1, 0, 0]);
        let out = filter(&[&bytes[..20], &bytes[20..]], false).unwrap();
        assert_eq!(out, &bytes[..bytes.len() - 4]);
        // Only the header is held: a large payload is not buffered.
        let mut filter = RfbFilter::new();
        let mut out = Vec::new();
        let header = [HANDSHAKE, &[6, 0, 0, 0, 0, 0x10, 0, 0][..]].concat();
        filter.feed(&header, false, &mut out).unwrap();
        filter.feed(&[b'x'; 4096], false, &mut out).unwrap();
        assert!(filter.pending.is_empty() && filter.passthrough == 0x100000 - 4096);
    }
}
