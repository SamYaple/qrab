//! Frames multiplexing guest TCP connections over one virtio-serial stream.
//!
//! Each frame is `kind u8 | id u32 BE | len u16 BE | payload`, COBS-encoded
//! and terminated by a `0x00` byte, so a receiver resynchronizes at the next
//! delimiter after torn input. Session frames are also preceded by `0x00`,
//! which terminates any partial frame a previous peer left in the channel.
//!
//! The host starts every chardev connection with [`Frame::Hello`] and ignores
//! guest frames until the matching [`Frame::Ack`]. The guest discards all of
//! its connections when it reads a `Hello`. A restarted guest relay sends
//! [`Frame::Reset`] to make the host start a new session. Connection ids are
//! chosen by the guest and never reused within one guest relay process.
//!
//! Data is credit-based: each direction of a connection starts with
//! [`WINDOW`] bytes of credit, a sender never has more than that in flight,
//! and the receiver returns [`Frame::Credit`] once it wrote bytes onward. A
//! slow reader therefore stalls its sender instead of growing any buffer.
//! [`Frame::Close`] ends the whole connection; there is no half-close.
//!
//! ```
//! use qrab_spire_relay::wire::{Decoder, Frame};
//! let mut decoder = Decoder::default();
//! let mut bytes = Frame::Data(7, b"\0spire\0".to_vec()).encode();
//! bytes.extend(Frame::Close(7).encode());
//! let frames: Vec<_> = decoder.feed(&bytes).collect();
//! assert_eq!(frames, [Ok(Frame::Data(7, b"\0spire\0".to_vec())), Ok(Frame::Close(7))]);
//! ```

/// Name of the virtio-serial port the manager attaches.
pub const PORT_NAME: &str = "org.yapplesauce.spire.0";
/// Largest data payload in one frame.
pub const MAX_PAYLOAD: usize = 16 * 1024;
/// Bytes one direction of a connection may have outstanding.
pub const WINDOW: u32 = 64 * 1024;
/// Payload of the session frames; a different value is a different protocol.
pub const MAGIC: [u8; 4] = *b"QSR1";

const HEADER: usize = 7;
const MAX_RAW: usize = HEADER + MAX_PAYLOAD;
const MAX_ENCODED: usize = MAX_RAW + MAX_RAW / 254 + 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// Host to guest: a new session; the guest discards every connection.
    Hello(u32),
    /// Guest to host: the guest read `Hello` with this session number.
    Ack(u32),
    /// Guest to host: the guest relay restarted; start a new session.
    Reset,
    /// Guest to host: a new connection with this id.
    Open(u32),
    Data(u32, Vec<u8>),
    /// Either direction: the connection is gone.
    Close(u32),
    /// The receiver wrote this many bytes of the connection onward.
    Credit(u32, u32),
}

impl Frame {
    fn kind(&self) -> u8 {
        match self {
            Self::Hello(_) => 1,
            Self::Ack(_) => 2,
            Self::Reset => 3,
            Self::Open(_) => 4,
            Self::Data(..) => 5,
            Self::Close(_) => 6,
            Self::Credit(..) => 7,
        }
    }

    /// The delimited wire bytes. Panics if `Data` exceeds [`MAX_PAYLOAD`].
    pub fn encode(&self) -> Vec<u8> {
        let (id, payload): (u32, &[u8]) = match self {
            Self::Hello(session) | Self::Ack(session) => (*session, &MAGIC),
            Self::Reset => (0, &MAGIC),
            Self::Open(id) | Self::Close(id) => (*id, &[]),
            Self::Data(id, data) => (*id, data),
            Self::Credit(id, bytes) => (*id, &bytes.to_be_bytes()),
        };
        assert!(
            payload.len() <= MAX_PAYLOAD,
            "relay frame payload too large"
        );
        let mut raw = Vec::with_capacity(HEADER + payload.len());
        raw.push(self.kind());
        raw.extend(id.to_be_bytes());
        raw.extend((payload.len() as u16).to_be_bytes());
        raw.extend(payload);
        let mut out = Vec::with_capacity(raw.len() + raw.len() / 254 + 3);
        if matches!(self, Self::Hello(_) | Self::Ack(_) | Self::Reset) {
            out.push(0);
        }
        cobs_encode(&raw, &mut out);
        out.push(0);
        out
    }

    fn parse(raw: &[u8]) -> Option<Self> {
        let (header, payload) = raw.split_at_checked(HEADER)?;
        let id = u32::from_be_bytes(header[1..5].try_into().ok()?);
        let len = u16::from_be_bytes(header[5..7].try_into().ok()?);
        if usize::from(len) != payload.len() || payload.len() > MAX_PAYLOAD {
            return None;
        }
        let session = payload == MAGIC;
        let empty = payload.is_empty();
        let frame = match header[0] {
            1 if session => Self::Hello(id),
            2 if session => Self::Ack(id),
            3 if session && id == 0 => Self::Reset,
            4 if empty && id != 0 => Self::Open(id),
            5 if !empty && id != 0 => Self::Data(id, payload.to_vec()),
            6 if empty && id != 0 => Self::Close(id),
            7 if id != 0 => {
                let bytes = u32::from_be_bytes(payload.try_into().ok()?);
                if bytes == 0 || bytes > WINDOW {
                    return None;
                }
                Self::Credit(id, bytes)
            }
            _ => return None,
        };
        Some(frame)
    }
}

/// A delimited frame that did not decode; the stream continues after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invalid;

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("malformed SPIRE relay frame")
    }
}

impl std::error::Error for Invalid {}

/// Incremental decoder holding at most one encoded frame.
#[derive(Debug, Default)]
pub struct Decoder {
    buf: Vec<u8>,
    overflow: bool,
}

impl Decoder {
    pub fn push(&mut self, byte: u8) -> Option<Result<Frame, Invalid>> {
        if byte == 0 {
            let overflow = std::mem::take(&mut self.overflow);
            if self.buf.is_empty() && !overflow {
                return None;
            }
            let frame = (!overflow)
                .then(|| cobs_decode(&self.buf).and_then(|raw| Frame::parse(&raw)))
                .flatten()
                .ok_or(Invalid);
            self.buf.clear();
            return Some(frame);
        }
        if self.overflow {
            return None;
        }
        if self.buf.len() == MAX_ENCODED {
            self.buf.clear();
            self.overflow = true;
            return None;
        }
        self.buf.push(byte);
        None
    }

    pub fn feed<'a>(
        &'a mut self,
        bytes: &'a [u8],
    ) -> impl Iterator<Item = Result<Frame, Invalid>> + 'a {
        bytes.iter().filter_map(|byte| self.push(*byte))
    }
}

fn cobs_encode(input: &[u8], out: &mut Vec<u8>) {
    let mut code_at = out.len();
    out.push(0);
    let mut code = 1u8;
    for &byte in input {
        if byte != 0 {
            out.push(byte);
            code += 1;
        }
        if byte == 0 || code == 0xff {
            out[code_at] = code;
            code_at = out.len();
            out.push(0);
            code = 1;
        }
    }
    out[code_at] = code;
}

fn cobs_decode(input: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len());
    let mut at = 0;
    while at < input.len() {
        let code = usize::from(input[at]);
        let end = at + code;
        if code == 0 || end > input.len() {
            return None;
        }
        out.extend_from_slice(&input[at + 1..end]);
        at = end;
        if code < 0xff && at < input.len() {
            out.push(0);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_all(bytes: &[u8]) -> Vec<Result<Frame, Invalid>> {
        Decoder::default().feed(bytes).collect()
    }

    #[test]
    fn every_frame_round_trips_including_zero_and_long_runs() {
        let mut max = vec![0u8; MAX_PAYLOAD];
        max[300..600].fill(7);
        let frames = [
            Frame::Hello(0),
            Frame::Hello(u32::MAX),
            Frame::Ack(42),
            Frame::Reset,
            Frame::Open(1),
            Frame::Data(1, vec![0]),
            Frame::Data(2, vec![0xff; 254]),
            Frame::Data(2, vec![0xff; 255]),
            Frame::Data(3, (0..=255).cycle().take(5000).collect()),
            Frame::Data(u32::MAX, max),
            Frame::Close(9),
            Frame::Credit(9, WINDOW),
            Frame::Credit(9, 1),
        ];
        let mut bytes = Vec::new();
        for frame in &frames {
            let encoded = frame.encode();
            assert_eq!(*encoded.last().unwrap(), 0);
            assert!(!encoded[1..encoded.len() - 1].contains(&0));
            bytes.extend(encoded);
        }
        let decoded: Vec<_> = decode_all(&bytes).into_iter().map(Result::unwrap).collect();
        assert_eq!(decoded, frames);
    }

    #[test]
    fn malformed_frames_are_rejected() {
        let raw = |kind: u8, id: u32, payload: &[u8], len: u16| {
            let mut raw = vec![kind];
            raw.extend(id.to_be_bytes());
            raw.extend(len.to_be_bytes());
            raw.extend(payload);
            let mut out = Vec::new();
            cobs_encode(&raw, &mut out);
            out.push(0);
            out
        };
        for bytes in [
            raw(4, 0, &[], 0),                 // Open needs an id
            raw(5, 1, &[], 0),                 // empty Data
            raw(5, 1, b"ab", 3),               // length mismatch
            raw(1, 1, b"QSR2", 4),             // another protocol
            raw(3, 5, &MAGIC, 4),              // Reset carries no id
            raw(6, 1, b"x", 1),                // Close has no payload
            raw(7, 1, &0u32.to_be_bytes(), 4), // zero credit
            raw(7, 1, &(WINDOW + 1).to_be_bytes(), 4),
            raw(8, 1, &[], 0), // unknown kind
            raw(5, 1, &vec![1; MAX_PAYLOAD + 1], (MAX_PAYLOAD + 1) as u16),
            vec![1, 2, 0], // short header
            vec![9, 1, 0], // COBS code past the end
        ] {
            assert_eq!(decode_all(&bytes), [Err(Invalid)], "{bytes:?}");
        }
    }

    #[test]
    fn torn_input_resynchronizes_at_the_next_session_frame() {
        // A peer died mid-frame; the next session frame's leading delimiter
        // ends the torn bytes, so the frame after it decodes intact.
        let torn = Frame::Data(1, vec![5; 1000]).encode();
        let mut bytes = torn[..torn.len() / 2].to_vec();
        bytes.extend(Frame::Hello(3).encode());
        bytes.extend(Frame::Open(2).encode());
        assert_eq!(
            decode_all(&bytes),
            [Err(Invalid), Ok(Frame::Hello(3)), Ok(Frame::Open(2))]
        );
    }

    #[test]
    fn an_oversized_frame_is_discarded_with_bounded_memory() {
        let mut decoder = Decoder::default();
        let garbage = vec![0x55; MAX_ENCODED * 3];
        assert_eq!(decoder.feed(&garbage).count(), 0);
        assert!(decoder.buf.capacity() <= MAX_ENCODED * 2);
        let mut bytes = vec![0];
        bytes.extend(Frame::Close(4).encode());
        assert_eq!(
            decoder.feed(&bytes).collect::<Vec<_>>(),
            [Err(Invalid), Ok(Frame::Close(4))]
        );
    }
}
