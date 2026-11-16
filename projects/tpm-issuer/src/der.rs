//! Minimal DER encoders for the TCG extensions rcgen does not model.

fn length(len: usize, out: &mut Vec<u8>) {
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes = len.to_be_bytes();
        let skip = bytes.iter().take_while(|b| **b == 0).count();
        out.push(0x80 | (bytes.len() - skip) as u8);
        out.extend_from_slice(&bytes[skip..]);
    }
}

pub fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    length(content.len(), &mut out);
    out.extend_from_slice(content);
    out
}

pub fn sequence(items: &[Vec<u8>]) -> Vec<u8> {
    tlv(0x30, &items.concat())
}

pub fn set(items: &[Vec<u8>]) -> Vec<u8> {
    tlv(0x31, &items.concat())
}

pub fn utf8(text: &str) -> Vec<u8> {
    tlv(0x0c, text.as_bytes())
}

/// A non-negative INTEGER from big-endian magnitude bytes.
pub fn unsigned_integer(magnitude: &[u8]) -> Vec<u8> {
    let start = magnitude
        .iter()
        .position(|b| *b != 0)
        .unwrap_or(magnitude.len());
    let trimmed = &magnitude[start..];
    let mut content = Vec::with_capacity(trimmed.len() + 1);
    if trimmed.first().is_none_or(|b| b & 0x80 != 0) {
        content.push(0);
    }
    content.extend_from_slice(trimmed);
    tlv(0x02, &content)
}

pub fn oid(arcs: &[u64]) -> Vec<u8> {
    let mut content = Vec::new();
    let mut base128 = |mut v: u64| {
        let mut tmp = vec![(v & 0x7f) as u8];
        v >>= 7;
        while v > 0 {
            tmp.push(0x80 | (v & 0x7f) as u8);
            v >>= 7;
        }
        tmp.reverse();
        content.extend_from_slice(&tmp);
    };
    base128(arcs[0] * 40 + arcs[1]);
    for arc in &arcs[2..] {
        base128(*arc);
    }
    tlv(0x06, &content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings_match_x690() {
        assert_eq!(oid(&[2, 5, 29, 17]), [0x06, 0x03, 0x55, 0x1d, 0x11]);
        assert_eq!(
            oid(&[2, 23, 133, 8, 1]),
            [0x06, 0x05, 0x67, 0x81, 0x05, 0x08, 0x01]
        );
        assert_eq!(unsigned_integer(&[0]), [0x02, 0x01, 0x00]);
        assert_eq!(unsigned_integer(&[0, 0, 0x7f]), [0x02, 0x01, 0x7f]);
        assert_eq!(unsigned_integer(&[0x80]), [0x02, 0x02, 0x00, 0x80]);
        assert_eq!(
            unsigned_integer(&164u32.to_be_bytes()),
            [0x02, 0x02, 0x00, 0xa4]
        );
        let long = tlv(0x04, &[0; 300]);
        assert_eq!(&long[..4], &[0x04, 0x82, 0x01, 0x2c]);
    }
}
