//! Incremental request framing compatible with QEMU's JSON streamer.
//!
//! Requests are top-level JSON values; line breaks are optional. A `0xFF` byte,
//! which cannot occur in UTF-8 JSON, discards all partial input. Clients send it
//! before `guest-sync-delimited` to recover from a torn previous session.

#[derive(Debug, PartialEq, Eq)]
pub enum Frame {
    Message(Vec<u8>),
    /// The request exceeded the size bound. Input is discarded until the next
    /// line feed or `0xFF`.
    Overflow,
}

pub struct Framer {
    buf: Vec<u8>,
    limit: usize,
    depth: usize,
    in_string: bool,
    escaped: bool,
    discarding: bool,
}

impl Framer {
    pub fn new(limit: usize) -> Self {
        Self {
            buf: Vec::new(),
            limit,
            depth: 0,
            in_string: false,
            escaped: false,
            discarding: false,
        }
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.depth = 0;
        self.in_string = false;
        self.escaped = false;
        self.discarding = false;
    }

    pub fn push(&mut self, byte: u8) -> Option<Frame> {
        if byte == 0xff {
            self.reset();
            return None;
        }
        if self.discarding {
            if byte == b'\n' {
                self.reset();
            }
            return None;
        }
        let top = self.depth == 0 && !self.in_string;
        if top && byte.is_ascii_whitespace() {
            // A scalar top-level token ends at whitespace; QEMU rejects it.
            return (!self.buf.is_empty()).then(|| Frame::Message(std::mem::take(&mut self.buf)));
        }
        if self.buf.len() >= self.limit {
            self.reset();
            self.discarding = byte != b'\n';
            return Some(Frame::Overflow);
        }
        self.buf.push(byte);
        if self.in_string {
            if self.escaped {
                self.escaped = false;
            } else if byte == b'\\' {
                self.escaped = true;
            } else if byte == b'"' {
                self.in_string = false;
            }
            return None;
        }
        match byte {
            b'"' => self.in_string = true,
            b'{' | b'[' => self.depth += 1,
            b'}' | b']' => {
                self.depth = self.depth.saturating_sub(1);
                if self.depth == 0 {
                    return Some(Frame::Message(std::mem::take(&mut self.buf)));
                }
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(input: &[u8]) -> Vec<Frame> {
        let mut framer = Framer::new(64);
        input.iter().filter_map(|b| framer.push(*b)).collect()
    }

    #[test]
    fn objects_split_without_newlines_and_strings_hide_braces() {
        assert_eq!(
            frames(br#"{"a":"}{\""}  {"b":[1,{}]}"#),
            vec![
                Frame::Message(br#"{"a":"}{\""}"#.to_vec()),
                Frame::Message(br#"{"b":[1,{}]}"#.to_vec())
            ]
        );
    }

    #[test]
    fn sentinel_discards_partial_input_even_inside_strings() {
        assert_eq!(
            frames(b"{\"execute\":\"gue\xff{\"x\":1}\r\n"),
            vec![Frame::Message(br#"{"x":1}"#.to_vec())]
        );
        assert_eq!(frames(b"42 \xff"), vec![Frame::Message(b"42".to_vec())]);
    }

    #[test]
    fn oversized_input_is_reported_once_and_discarded_to_line_end() {
        let mut input = vec![b'{'];
        input.extend(std::iter::repeat_n(b'1', 100));
        input.extend(b"}\n{}");
        assert_eq!(
            frames(&input),
            vec![Frame::Overflow, Frame::Message(b"{}".to_vec())]
        );
    }
}
