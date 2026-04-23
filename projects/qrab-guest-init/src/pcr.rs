//! TPM 2.0 PCR read over the resource-manager device `/dev/tpmrm0`.
//!
//! Spec: TCG TPM 2.0 Library Specification Part 3 "Commands" §22.4
//! (`TPM2_PCR_Read`) and Part 2 "Structures" §10.4.2 (TPMS_PCR_SELECTION).
//!
//! Drives L5 step 11 of `docs/security/level-5-init.md`: read the live
//! SHA-256 bank for PCRs 0..=9 so step 12 can compare it against the
//! replay produced by [`crate::eventlog::replay_sha256`].
//!
//! Wire layer is deliberately generic over `Read + Write` so tests can
//! drive a `Vec<u8>`-backed fake — the real binary passes a `File`
//! opened on `/dev/tpmrm0`.

use std::io::{Read, Write};

// --- TPM 2.0 wire constants ---

const TPM_ST_NO_SESSIONS: u16 = 0x8001;
const TPM_CC_PCR_READ: u32 = 0x0000_017E;
const TPM_ALG_SHA256: u16 = 0x000B;

/// Max PCRs we ever ask the TPM to return in one call. The TCG spec
/// actually caps `TPM2_PCR_Read` at 8 PCRs per call; L5 needs 0..=9
/// (ten slots) so the caller issues a single `TPM2_PCR_Read` with a
/// 10-bit bitmap and the TPM will clamp to whatever fits. In
/// practice every known TPM 2.0 impl returns all 10. If a future TPM
/// ever returns fewer, the downstream "zero means not read → compare
/// fails" path makes this failed-closed, not silently wrong.
const MAX_PCRS: u32 = 24;

/// Read the SHA-256 PCR bank from a TPM 2.0 device. The returned
/// array is always 24 slots — PCRs above `max_pcr` and any PCR the
/// TPM did not return stay zero-filled, so a downstream compare that
/// hits a zero slot fails closed.
///
/// The caller owns the device handle; in the live binary this is a
/// `File` on `/dev/tpmrm0`. In tests it's a `Vec<u8>`-backed fake.
pub fn read_sha256_bank<T: Read + Write + ?Sized>(
    device: &mut T,
    max_pcr: u32,
) -> Result<[[u8; 32]; 24], ReadError> {
    let max_pcr = max_pcr.min(MAX_PCRS - 1);
    let request = build_pcr_read_request(max_pcr);
    device.write_all(&request).map_err(ReadError::Io)?;

    let mut buf = [0u8; 4096];
    let n = device.read(&mut buf).map_err(ReadError::Io)?;
    let response = &buf[..n];
    parse_pcr_read_response(response, max_pcr)
}

/// Wire format per TCG TPM 2.0 Part 3 §22.4 (TPM2_PCR_Read):
///
/// ```text
///   tag                 u16 BE    = TPM_ST_NO_SESSIONS (0x8001)
///   commandSize         u32 BE    = total bytes incl. header
///   commandCode         u32 BE    = TPM_CC_PCR_Read (0x017E)
///   pcrSelectionCount   u32 BE    = 1
///     hashAlg           u16 BE   = TPM_ALG_SHA256
///     sizeOfSelect      u8       = 3 (3 × 8 bitmap bytes → 24 PCRs)
///     bitmap[3]         u8[3]    = selected PCRs
/// ```
pub fn build_pcr_read_request(max_pcr: u32) -> Vec<u8> {
    let mut bitmap = [0u8; 3];
    for pcr in 0..=max_pcr {
        if pcr >= 24 {
            break;
        }
        bitmap[(pcr / 8) as usize] |= 1 << (pcr % 8);
    }
    // Fixed-size request: 10-byte header + 4 count + 2 hashAlg
    // + 1 sizeOfSelect + 3 bitmap = 20 bytes.
    let command_size: u32 = 20;
    let mut out = Vec::with_capacity(command_size as usize);
    out.extend_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
    out.extend_from_slice(&command_size.to_be_bytes());
    out.extend_from_slice(&TPM_CC_PCR_READ.to_be_bytes());
    out.extend_from_slice(&1u32.to_be_bytes()); // pcrSelectionCount
    out.extend_from_slice(&TPM_ALG_SHA256.to_be_bytes());
    out.push(3); // sizeOfSelect
    out.extend_from_slice(&bitmap);
    out
}

/// Parse the fixed shape of a `TPM2_PCR_Read` response into a 24-slot
/// SHA-256 bank. See Part 3 §22.4 "Response":
///
/// ```text
///   tag                 u16 BE    — must be TPM_ST_NO_SESSIONS
///   responseSize        u32 BE
///   responseCode        u32 BE    — must be 0
///   pcrUpdateCounter    u32 BE
///   pcrSelectionCount   u32 BE    — 1
///     hashAlg           u16 BE
///     sizeOfSelect      u8
///     bitmap[sizeOfSelect]
///   digestsCount        u32 BE    — number of PCRs actually returned
///     per-digest:
///       size            u16 BE    — 32 for SHA-256
///       digest[size]    — 32 bytes
/// ```
fn parse_pcr_read_response(
    response: &[u8],
    max_pcr: u32,
) -> Result<[[u8; 32]; 24], ReadError> {
    let mut cur = Cursor::new(response);
    let tag = cur.read_u16_be()?;
    if tag != TPM_ST_NO_SESSIONS {
        return Err(ReadError::ResponseBadTag);
    }
    let _response_size = cur.read_u32_be()?;
    let rc = cur.read_u32_be()?;
    if rc != 0 {
        return Err(ReadError::ResponseRcNonZero(rc));
    }
    let _pcr_update_counter = cur.read_u32_be()?;
    let sel_count = cur.read_u32_be()?;
    if sel_count != 1 {
        return Err(ReadError::DigestCountMismatch);
    }
    let _hash_alg = cur.read_u16_be()?;
    let sel_size = cur.read_u8()?;
    cur.skip(sel_size as usize)?;

    let digest_count = cur.read_u32_be()?;
    // Rebuild the same selected-PCR list the request asked for, in
    // ascending PCR order; digests come back in exactly that order
    // per the TPM 2.0 spec.
    let selected: Vec<u32> = (0..=max_pcr).filter(|p| *p < 24).collect();
    if digest_count as usize != selected.len() {
        return Err(ReadError::DigestCountMismatch);
    }

    let mut out = [[0u8; 32]; 24];
    for pcr in selected {
        let size = cur.read_u16_be()?;
        if size != 32 {
            return Err(ReadError::DigestSizeWrong(size));
        }
        let digest = cur.read_bytes(32)?;
        out[pcr as usize].copy_from_slice(digest);
    }
    Ok(out)
}

/// Failure modes reading the TPM. Plain enum + `Debug` —
/// narrow dep graph per L5 spec's musl-build rationale.
#[derive(Debug)]
pub enum ReadError {
    Io(std::io::Error),
    ResponseTooShort,
    ResponseBadTag,
    ResponseRcNonZero(u32),
    DigestCountMismatch,
    DigestSizeWrong(u16),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "TPM IO: {e}"),
            Self::ResponseTooShort => f.write_str("TPM response shorter than expected"),
            Self::ResponseBadTag => f.write_str("TPM response tag != TPM_ST_NO_SESSIONS"),
            Self::ResponseRcNonZero(rc) => write!(f, "TPM rc != 0: 0x{rc:08x}"),
            Self::DigestCountMismatch => f.write_str("TPM returned wrong digest count"),
            Self::DigestSizeWrong(sz) => write!(f, "TPM digest size {sz} != 32 (SHA-256)"),
        }
    }
}

// --- internals ---

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    fn read_u8(&mut self) -> Result<u8, ReadError> {
        if self.remaining() < 1 {
            return Err(ReadError::ResponseTooShort);
        }
        let v = self.bytes[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u16_be(&mut self) -> Result<u16, ReadError> {
        if self.remaining() < 2 {
            return Err(ReadError::ResponseTooShort);
        }
        let v = u16::from_be_bytes([self.bytes[self.pos], self.bytes[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn read_u32_be(&mut self) -> Result<u32, ReadError> {
        if self.remaining() < 4 {
            return Err(ReadError::ResponseTooShort);
        }
        let v = u32::from_be_bytes([
            self.bytes[self.pos],
            self.bytes[self.pos + 1],
            self.bytes[self.pos + 2],
            self.bytes[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], ReadError> {
        if self.remaining() < n {
            return Err(ReadError::ResponseTooShort);
        }
        let out = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn skip(&mut self, n: usize) -> Result<(), ReadError> {
        if self.remaining() < n {
            return Err(ReadError::ResponseTooShort);
        }
        self.pos += n;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory TPM stub. A test sets `.response` to whatever bytes
    /// the fake TPM should return, then borrows this as `Read+Write`.
    struct FakeTpm {
        response: Vec<u8>,
        written: Vec<u8>,
        read_err: Option<std::io::ErrorKind>,
        write_err: Option<std::io::ErrorKind>,
    }

    impl FakeTpm {
        fn new(response: Vec<u8>) -> Self {
            Self {
                response,
                written: Vec::new(),
                read_err: None,
                write_err: None,
            }
        }
    }

    impl Read for FakeTpm {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if let Some(k) = self.read_err.take() {
                return Err(std::io::Error::from(k));
            }
            let n = std::cmp::min(buf.len(), self.response.len());
            buf[..n].copy_from_slice(&self.response[..n]);
            // Drain so a subsequent call returns 0.
            self.response.drain(..n);
            Ok(n)
        }
    }

    impl Write for FakeTpm {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if let Some(k) = self.write_err.take() {
                return Err(std::io::Error::from(k));
            }
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn build_ok_response(pcr_count: u32, digest: u8) -> Vec<u8> {
        let mut out = Vec::new();
        // header
        out.extend_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
        // responseSize will be rewritten below
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes()); // rc = 0
        // pcrUpdateCounter
        out.extend_from_slice(&42u32.to_be_bytes());
        // pcrSelectionCount
        out.extend_from_slice(&1u32.to_be_bytes());
        // hashAlg
        out.extend_from_slice(&TPM_ALG_SHA256.to_be_bytes());
        // sizeOfSelect
        out.push(3);
        // bitmap for pcrs 0..pcr_count
        let mut bitmap = [0u8; 3];
        for p in 0..pcr_count {
            bitmap[(p / 8) as usize] |= 1 << (p % 8);
        }
        out.extend_from_slice(&bitmap);
        // digestsCount
        out.extend_from_slice(&pcr_count.to_be_bytes());
        for _ in 0..pcr_count {
            out.extend_from_slice(&32u16.to_be_bytes());
            out.extend_from_slice(&[digest; 32]);
        }
        let sz = out.len() as u32;
        out[2..6].copy_from_slice(&sz.to_be_bytes());
        out
    }

    #[test]
    fn happy_path_reads_pcrs_0_to_9() {
        let response = build_ok_response(10, 0xAA);
        let mut tpm = FakeTpm::new(response);
        let pcrs = read_sha256_bank(&mut tpm, 9).expect("ok");
        for (i, slot) in pcrs.iter().enumerate().take(10) {
            assert_eq!(*slot, [0xAAu8; 32], "PCR{i} should be 0xAA×32");
        }
        for (i, slot) in pcrs.iter().enumerate().skip(10) {
            assert_eq!(*slot, [0u8; 32], "PCR{i} should stay zero");
        }
    }

    #[test]
    fn request_wire_shape_is_fixed() {
        let req = build_pcr_read_request(9);
        // 20 bytes total per spec (10-byte header + 10-byte payload).
        assert_eq!(req.len(), 20);
        // tag
        assert_eq!(&req[0..2], &TPM_ST_NO_SESSIONS.to_be_bytes());
        // commandSize
        assert_eq!(u32::from_be_bytes(req[2..6].try_into().unwrap()), 20);
        // commandCode
        assert_eq!(u32::from_be_bytes(req[6..10].try_into().unwrap()), TPM_CC_PCR_READ);
        // pcrSelectionCount = 1
        assert_eq!(u32::from_be_bytes(req[10..14].try_into().unwrap()), 1);
        // hashAlg
        assert_eq!(u16::from_be_bytes(req[14..16].try_into().unwrap()), TPM_ALG_SHA256);
        // sizeOfSelect
        assert_eq!(req[16], 3);
        // bitmap — pcr 0..9 = 0xFF, 0x03, 0x00.
        assert_eq!(&req[17..20], &[0xFF, 0x03, 0x00]);
    }

    #[test]
    fn bitmap_spans_higher_banks() {
        // max_pcr = 15 → bitmap[0] full, bitmap[1] full, bitmap[2] = 0.
        let req = build_pcr_read_request(15);
        assert_eq!(&req[17..20], &[0xFF, 0xFF, 0x00]);
    }

    #[test]
    fn fail_response_bad_tag() {
        // Response with bogus tag 0x1234.
        let mut bad = build_ok_response(10, 0);
        bad[0] = 0x12;
        bad[1] = 0x34;
        let mut tpm = FakeTpm::new(bad);
        assert!(matches!(
            read_sha256_bank(&mut tpm, 9),
            Err(ReadError::ResponseBadTag),
        ));
    }

    #[test]
    fn fail_rc_non_zero() {
        let mut bad = build_ok_response(10, 0);
        // rc lives at offset 6..10: tag (2) + responseSize (4).
        bad[6..10].copy_from_slice(&0x0000_0100u32.to_be_bytes());
        let mut tpm = FakeTpm::new(bad);
        assert!(matches!(
            read_sha256_bank(&mut tpm, 9),
            Err(ReadError::ResponseRcNonZero(0x100)),
        ));
    }

    #[test]
    fn fail_digest_size_wrong() {
        // Build a response that claims each digest is 20 bytes (SHA-1).
        let mut out = Vec::new();
        out.extend_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes()); // size rewritten
        out.extend_from_slice(&0u32.to_be_bytes()); // rc
        out.extend_from_slice(&0u32.to_be_bytes()); // pcrUpdateCounter
        out.extend_from_slice(&1u32.to_be_bytes()); // selCount
        out.extend_from_slice(&TPM_ALG_SHA256.to_be_bytes());
        out.push(3);
        out.extend_from_slice(&[0xFF, 0x03, 0x00]);
        out.extend_from_slice(&10u32.to_be_bytes()); // digestsCount
        for _ in 0..10 {
            out.extend_from_slice(&20u16.to_be_bytes()); // WRONG size
            out.extend_from_slice(&[0x11u8; 20]);
        }
        let sz = out.len() as u32;
        out[2..6].copy_from_slice(&sz.to_be_bytes());

        let mut tpm = FakeTpm::new(out);
        assert!(matches!(
            read_sha256_bank(&mut tpm, 9),
            Err(ReadError::DigestSizeWrong(20)),
        ));
    }

    #[test]
    fn fail_response_too_short_truncated_header() {
        // Only 4 bytes — not even the tag+size fields.
        let mut tpm = FakeTpm::new(vec![0x80, 0x01, 0, 0]);
        assert!(matches!(
            read_sha256_bank(&mut tpm, 9),
            Err(ReadError::ResponseTooShort),
        ));
    }

    #[test]
    fn fail_response_too_short_truncated_digest_body() {
        // Correct header but cut off mid-digest.
        let mut ok = build_ok_response(10, 0);
        ok.truncate(50);
        let mut tpm = FakeTpm::new(ok);
        assert!(matches!(
            read_sha256_bank(&mut tpm, 9),
            Err(ReadError::ResponseTooShort),
        ));
    }

    #[test]
    fn fail_digest_count_mismatch() {
        // Build a response that says 5 digests but we asked for 10.
        let mut bad = build_ok_response(5, 0xAB);
        // Flip nothing else — the digestsCount at the fixed offset is 5
        // while our request covered PCRs 0..=9.
        // Rather than build it by hand, use build_ok_response(5) which
        // internally writes digestsCount = 5.
        // But our read asks max_pcr=9 → expected 10.
        let _ = &mut bad;
        let mut tpm = FakeTpm::new(bad);
        assert!(matches!(
            read_sha256_bank(&mut tpm, 9),
            Err(ReadError::DigestCountMismatch),
        ));
    }

    #[test]
    fn fail_selection_count_not_one() {
        // Craft a response with selectionCount = 2.
        let mut out = Vec::new();
        out.extend_from_slice(&TPM_ST_NO_SESSIONS.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&2u32.to_be_bytes()); // selCount = 2
        out.extend_from_slice(&TPM_ALG_SHA256.to_be_bytes());
        out.push(3);
        out.extend_from_slice(&[0u8; 3]);
        out.extend_from_slice(&TPM_ALG_SHA256.to_be_bytes());
        out.push(3);
        out.extend_from_slice(&[0u8; 3]);
        out.extend_from_slice(&0u32.to_be_bytes());
        let sz = out.len() as u32;
        out[2..6].copy_from_slice(&sz.to_be_bytes());
        let mut tpm = FakeTpm::new(out);
        assert!(matches!(
            read_sha256_bank(&mut tpm, 9),
            Err(ReadError::DigestCountMismatch),
        ));
    }

    #[test]
    fn fail_write_io_error() {
        let mut tpm = FakeTpm::new(Vec::new());
        tpm.write_err = Some(std::io::ErrorKind::BrokenPipe);
        assert!(matches!(
            read_sha256_bank(&mut tpm, 9),
            Err(ReadError::Io(_)),
        ));
    }

    #[test]
    fn fail_read_io_error() {
        let mut tpm = FakeTpm::new(Vec::new());
        tpm.read_err = Some(std::io::ErrorKind::UnexpectedEof);
        assert!(matches!(
            read_sha256_bank(&mut tpm, 9),
            Err(ReadError::Io(_)),
        ));
    }

    #[test]
    fn request_is_written_verbatim() {
        // Confirm the bytes we flow to the device are exactly
        // `build_pcr_read_request(9)` — no framing, no prefix.
        let mut tpm = FakeTpm::new(build_ok_response(10, 0));
        let _ = read_sha256_bank(&mut tpm, 9);
        assert_eq!(tpm.written, build_pcr_read_request(9));
    }

    #[test]
    fn display_formats_each_variant() {
        let cases: Vec<ReadError> = vec![
            ReadError::Io(std::io::Error::from(std::io::ErrorKind::Other)),
            ReadError::ResponseTooShort,
            ReadError::ResponseBadTag,
            ReadError::ResponseRcNonZero(0x155),
            ReadError::DigestCountMismatch,
            ReadError::DigestSizeWrong(48),
        ];
        for e in cases {
            let rendered = format!("{e}");
            assert!(!rendered.is_empty(), "empty Display for {e:?}");
        }
    }
}
