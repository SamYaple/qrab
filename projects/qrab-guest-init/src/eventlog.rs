//! TCG 2.0 crypto-agile event-log parser + SHA-256 replay.
//!
//! Spec: TCG PC Client Platform Firmware Profile Specification 1.06
//! rev 52 §8 "Event Logging"; TCG EFI Protocol Specification rev 00.13
//! §7 "Crypto Agile Log Entry Format".
//!
//! Drives L5 steps 9 and 10 of `docs/security/level-5-init.md`:
//!
//! * **Step 9** — parse the binary TCG log (first event legacy
//!   `TCG_PCR_EVENT` carrying the `TCG_EfiSpecIdEvent` header;
//!   subsequent events `TCG_PCR_EVENT2` with a `TPML_DIGEST_VALUES`
//!   digest list).
//! * **Step 10** — replay the SHA-256 bank across PCRs 0..=9 so
//!   step 12 can compare against the live TPM read from step 11.
//!
//! Only SHA-256 is consumed for replay — other banks (SHA-1, SHA-384,
//! SHA-512) are parsed (so the stream is consumed correctly) but
//! ignored. PCR entries for indices above `max_pcr` are dropped during
//! replay; an event-log attacker can list nonexistent PCRs all day and
//! the check at step 12 still catches the tampering via the 0..=9
//! window.
//!
//! No `anyhow` / `thiserror` — plain enum + `impl Display`, to keep
//! the static musl build narrow as the L5 spec dictates.

use sha2::{Digest, Sha256};

/// One parsed event. Retained verbatim (all banks) so future steps
/// can cross-check non-SHA-256 digests if policy ever demands it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventLogEntry {
    pub pcr_index: u32,
    pub event_type: u32,
    pub digests: Vec<(AlgId, Vec<u8>)>,
    pub event_data: Vec<u8>,
}

/// TCG TPM 2.0 Algorithm ID (see TPM 2.0 Library Part 2 §6.3).
///
/// We explicitly handle four: SHA-1 (0x0004), SHA-256 (0x000B),
/// SHA-384 (0x000C), SHA-512 (0x000D). Anything else must be
/// advertised with a size in the spec-id header's `DigestSizes[]`
/// table, otherwise the stream isn't self-describing and parsing
/// bails with `UnknownAlgTruncated`.
pub type AlgId = u16;

pub const TPM_ALG_SHA1: AlgId = 0x0004;
pub const TPM_ALG_SHA256: AlgId = 0x000B;
pub const TPM_ALG_SHA384: AlgId = 0x000C;
pub const TPM_ALG_SHA512: AlgId = 0x000D;

/// Signature bytes of the `TCG_EfiSpecIdEvent03` header (16 bytes,
/// NUL-terminated). Must appear at the start of the first event's
/// event-data.
pub const SPEC_ID_SIGNATURE: &[u8; 16] = b"Spec ID Event03\0";

/// Defensive upper bound on per-event digest count. The spec allows
/// up to 16 algorithm IDs but real firmware emits 2-4; anything
/// higher is almost certainly a malformed/attacker-crafted log.
const MAX_DIGESTS_PER_EVENT: u32 = 16;

/// Parse a full TCG 2.0 crypto-agile log into a linear event vector.
///
/// The spec mandates that `bytes[0..N]` is one legacy `TCG_PCR_EVENT`
/// carrying the `TCG_EfiSpecIdEvent` header; we parse that header to
/// learn the per-algorithm digest sizes, then walk the remainder as
/// `TCG_PCR_EVENT2` entries using those sizes to advance the cursor.
pub fn parse(bytes: &[u8]) -> Result<Vec<EventLogEntry>, ParseError> {
    let mut cur = Cursor::new(bytes);

    // --- Legacy first event: TCG_PCR_EVENT ---
    let pcr_index = cur.read_u32_le()?;
    let event_type = cur.read_u32_le()?;
    let sha1_digest = cur.read_bytes(20)?.to_vec();
    let event_size = cur.read_u32_le()? as usize;
    let event_data = cur.read_bytes(event_size)?.to_vec();

    // The first event's data MUST carry the TCG_EfiSpecIdEvent so the
    // rest of the log is decodable. Reject anything else — we cannot
    // walk subsequent events without the DigestSizes[] table.
    let alg_sizes = parse_spec_id_event(&event_data)?;

    let mut events = Vec::with_capacity(8);
    events.push(EventLogEntry {
        pcr_index,
        event_type,
        digests: vec![(TPM_ALG_SHA1, sha1_digest)],
        event_data,
    });

    // --- Subsequent events: TCG_PCR_EVENT2 ---
    while cur.remaining() > 0 {
        let pcr_index = cur.read_u32_le()?;
        let event_type = cur.read_u32_le()?;

        // TPML_DIGEST_VALUES: u32 count, then count × TPMT_HA
        // (u16 algId + digest bytes determined by the spec-id table).
        let count = cur.read_u32_le()?;
        if count > MAX_DIGESTS_PER_EVENT {
            return Err(ParseError::DigestCountOverflow);
        }
        let mut digests = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let alg = cur.read_u16_le()?;
            let size = digest_size(alg, &alg_sizes)
                .ok_or(ParseError::UnknownAlgTruncated)?;
            let bytes = cur.read_bytes(size)?.to_vec();
            digests.push((alg, bytes));
        }

        let event_size = cur.read_u32_le()? as usize;
        let event_data = cur.read_bytes(event_size)?.to_vec();

        events.push(EventLogEntry {
            pcr_index,
            event_type,
            digests,
            event_data,
        });
    }

    Ok(events)
}

/// Find the single SHA-256 digest of the PCR-`n` event. Returns
/// `None` if zero or multiple events target that PCR.
///
/// Used by L5 step 18 (`docs/security/level-5-init.md`) to extract
/// `uki_sha256` from the PCR-8 event for the `report_data`
/// reconstruct. The spec pins PCR 8 as a single-event bank (qrab-boot
/// extends exactly `sha256(uki_bytes)` once); multiple events there
/// mean the log is either malformed or an attacker planted a second
/// extension to dilute the binding. Zero events means qrab-boot
/// never extended the UKI digest, which the L5 chain cannot tolerate
/// either. Both cases must fail step 18 — the caller maps `None` to
/// `Token::FailReportData`.
pub fn unique_sha256_digest_for_pcr(log: &[EventLogEntry], pcr_index: u32) -> Option<[u8; 32]> {
    let mut found: Option<[u8; 32]> = None;
    for event in log {
        if event.pcr_index != pcr_index {
            continue;
        }
        let Some(sha256_bytes) = event
            .digests
            .iter()
            .find_map(|(alg, d)| (*alg == TPM_ALG_SHA256).then_some(d))
        else {
            // Event targets our PCR but carries no SHA-256 digest
            // (e.g. a SHA-1-only legacy event somehow landed in
            // crypto-agile territory). Count it as "this PCR saw a
            // non-unique event" — bail.
            return None;
        };
        if sha256_bytes.len() != 32 {
            return None;
        }
        if found.is_some() {
            // Second hit on the same PCR — not unique.
            return None;
        }
        let mut d = [0u8; 32];
        d.copy_from_slice(sha256_bytes);
        found = Some(d);
    }
    found
}

/// Replay the SHA-256 digests from `log` across PCRs `0..=max_pcr`.
///
/// Each PCR starts at 32 zero bytes. For every event whose
/// `pcr_index <= max_pcr`, the matching SHA-256 digest (if any) is
/// folded via `pcr = sha256(pcr || digest)` — the canonical TPM 2.0
/// extend operation (TPM 2.0 Library Part 1 §17). Events with no
/// SHA-256 digest, or whose PCR index exceeds `max_pcr`, are ignored.
///
/// Returns the full 24-slot bank so the caller can size-agnostically
/// compare against a 24-slot TPM read. Slots above `max_pcr` remain
/// at zero.
pub fn replay_sha256(log: &[EventLogEntry], max_pcr: u32) -> [[u8; 32]; 24] {
    let mut pcrs = [[0u8; 32]; 24];
    for event in log {
        if event.pcr_index > max_pcr || event.pcr_index >= 24 {
            continue;
        }
        let Some(sha256) = event
            .digests
            .iter()
            .find_map(|(alg, d)| (*alg == TPM_ALG_SHA256).then_some(d))
        else {
            continue;
        };
        if sha256.len() != 32 {
            // A SHA-256 entry must be exactly 32 bytes by spec. If the
            // parser somehow admitted a wrong-sized digest, skip rather
            // than panicking — the downstream PCR compare will catch the
            // resulting mismatch.
            continue;
        }
        let idx = event.pcr_index as usize;
        let mut hasher = Sha256::new();
        hasher.update(pcrs[idx]);
        hasher.update(sha256);
        let out = hasher.finalize();
        pcrs[idx].copy_from_slice(&out);
    }
    pcrs
}

/// Errors returned by [`parse`]. Plain enum with no `thiserror` dep —
/// L5 wants a minimal dep graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Fewer bytes than the fixed TCG_PCR_EVENT header requires.
    HeaderTooShort,
    /// First event's data does not carry a recognizable
    /// `TCG_EfiSpecIdEvent` (bad signature or wrong version).
    SpecIdInvalid,
    /// TPML_DIGEST_VALUES truncated mid-digest.
    DigestListTruncated,
    /// Event-data length field points past end of buffer.
    EventDataTruncated,
    /// Per-event digest count exceeds the 16-alg sanity ceiling.
    DigestCountOverflow,
    /// Encountered a TPM_ALG_ID we don't know the size of, and the
    /// spec-id header's DigestSizes[] table didn't advertise it.
    UnknownAlgTruncated,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::HeaderTooShort => "TCG event-log shorter than legacy PCR_EVENT header",
            Self::SpecIdInvalid => "first event's data is not a TCG_EfiSpecIdEvent",
            Self::DigestListTruncated => "TPML_DIGEST_VALUES truncated",
            Self::EventDataTruncated => "event data length overruns buffer",
            Self::DigestCountOverflow => "digest count exceeds 16-alg ceiling",
            Self::UnknownAlgTruncated => "unknown TPM_ALG_ID with no advertised size",
        };
        f.write_str(s)
    }
}

// --- internals ---

/// Parse the `TCG_EfiSpecIdEvent` carried in the first event's data.
/// Layout per TCG EFI Protocol Spec §9.4.5.1:
///
/// ```text
///  Signature           u8[16]  = "Spec ID Event03\0"
///  platformClass       u32
///  specVersionMinor    u8
///  specVersionMajor    u8
///  specErrata          u8
///  uintnSize           u8
///  numberOfAlgorithms  u32
///  digestSizes[]       numberOfAlgorithms × { u16 algId, u16 digestSize }
///  vendorInfoSize      u8
///  vendorInfo[]        vendorInfoSize bytes
/// ```
fn parse_spec_id_event(data: &[u8]) -> Result<Vec<(AlgId, u16)>, ParseError> {
    if data.len() < 16 + 4 + 4 + 4 {
        return Err(ParseError::SpecIdInvalid);
    }
    if &data[..16] != SPEC_ID_SIGNATURE {
        return Err(ParseError::SpecIdInvalid);
    }
    // platformClass (4) + versionMinor (1) + versionMajor (1)
    //   + errata (1) + uintnSize (1) = 8 bytes
    let major = data[16 + 4 + 1];
    if major != 2 {
        return Err(ParseError::SpecIdInvalid);
    }
    let num_algs_off = 16 + 4 + 4;
    let num_algs = u32::from_le_bytes(
        data[num_algs_off..num_algs_off + 4]
            .try_into()
            .map_err(|_| ParseError::SpecIdInvalid)?,
    );
    if num_algs > MAX_DIGESTS_PER_EVENT {
        return Err(ParseError::SpecIdInvalid);
    }
    let mut off = num_algs_off + 4;
    let mut algs = Vec::with_capacity(num_algs as usize);
    for _ in 0..num_algs {
        if data.len() < off + 4 {
            return Err(ParseError::SpecIdInvalid);
        }
        let alg = u16::from_le_bytes([data[off], data[off + 1]]);
        let size = u16::from_le_bytes([data[off + 2], data[off + 3]]);
        algs.push((alg, size));
        off += 4;
    }
    // vendorInfoSize is the next byte; we don't care about its body but
    // confirm it doesn't claim to overrun.
    if data.len() < off + 1 {
        return Err(ParseError::SpecIdInvalid);
    }
    let vendor_len = data[off] as usize;
    off += 1;
    if data.len() < off + vendor_len {
        return Err(ParseError::SpecIdInvalid);
    }
    Ok(algs)
}

/// Return the digest size in bytes for `alg`. Known-size algs
/// short-circuit to hard-coded values so a spoofed spec-id header
/// can't trick us into reading a 1-byte SHA-256. Unknown algs fall
/// back to whatever size the header advertised.
fn digest_size(alg: AlgId, advertised: &[(AlgId, u16)]) -> Option<usize> {
    match alg {
        TPM_ALG_SHA1 => Some(20),
        TPM_ALG_SHA256 => Some(32),
        TPM_ALG_SHA384 => Some(48),
        TPM_ALG_SHA512 => Some(64),
        other => advertised.iter().find_map(|(a, s)| {
            (*a == other).then_some(*s as usize)
        }),
    }
}

/// Minimal bounded cursor that maps every overrun into a
/// spec-appropriate `ParseError`. Keeps the parser free of
/// explicit boundary-check noise at each field.
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

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        if self.remaining() < n {
            return Err(if self.pos < 32 {
                // Inside the first (legacy) header.
                ParseError::HeaderTooShort
            } else {
                // We've walked into event data or the digest list.
                // Exact shape of the failure is decided by which field
                // we're reading; callers that need DigestListTruncated
                // call through read_u16_le/read_u32_le wrappers below
                // and translate from there.
                ParseError::EventDataTruncated
            });
        }
        let out = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    fn read_u16_le(&mut self) -> Result<u16, ParseError> {
        if self.remaining() < 2 {
            return Err(ParseError::DigestListTruncated);
        }
        let v = u16::from_le_bytes([self.bytes[self.pos], self.bytes[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn read_u32_le(&mut self) -> Result<u32, ParseError> {
        if self.remaining() < 4 {
            return Err(if self.pos < 32 {
                ParseError::HeaderTooShort
            } else {
                ParseError::DigestListTruncated
            });
        }
        let v = u32::from_le_bytes([
            self.bytes[self.pos],
            self.bytes[self.pos + 1],
            self.bytes[self.pos + 2],
            self.bytes[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Byte-level builder for a TCG 2.0 crypto-agile log. Keeps
    /// individual tests readable — each appends whatever malformed/
    /// well-formed chunk it needs and hands off to `parse`.
    struct LogBuilder {
        bytes: Vec<u8>,
    }

    impl LogBuilder {
        fn new() -> Self {
            Self { bytes: Vec::new() }
        }

        fn push_u16(&mut self, v: u16) {
            self.bytes.extend_from_slice(&v.to_le_bytes());
        }

        fn push_u32(&mut self, v: u32) {
            self.bytes.extend_from_slice(&v.to_le_bytes());
        }

        fn push_bytes(&mut self, b: &[u8]) {
            self.bytes.extend_from_slice(b);
        }

        fn push_legacy_event(&mut self, pcr: u32, event_type: u32, data: &[u8]) {
            self.push_u32(pcr);
            self.push_u32(event_type);
            self.push_bytes(&[0u8; 20]); // SHA1 digest
            self.push_u32(data.len() as u32);
            self.push_bytes(data);
        }

        fn push_spec_id_header(&mut self, algs: &[(AlgId, u16)]) {
            let mut data = Vec::new();
            data.extend_from_slice(SPEC_ID_SIGNATURE);
            data.extend_from_slice(&0u32.to_le_bytes()); // platformClass
            data.push(0); // specVersionMinor
            data.push(2); // specVersionMajor
            data.push(0); // specErrata
            data.push(2); // uintnSize (0x02 = 8-byte)
            data.extend_from_slice(&(algs.len() as u32).to_le_bytes());
            for (alg, size) in algs {
                data.extend_from_slice(&alg.to_le_bytes());
                data.extend_from_slice(&size.to_le_bytes());
            }
            data.push(0); // vendorInfoSize
            self.push_legacy_event(0, 0x0000_0003, &data);
        }

        fn push_event2(
            &mut self,
            pcr: u32,
            event_type: u32,
            digests: &[(AlgId, Vec<u8>)],
            event_data: &[u8],
        ) {
            self.push_u32(pcr);
            self.push_u32(event_type);
            self.push_u32(digests.len() as u32);
            for (alg, bytes) in digests {
                self.push_u16(*alg);
                self.push_bytes(bytes);
            }
            self.push_u32(event_data.len() as u32);
            self.push_bytes(event_data);
        }
    }

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(bytes);
        h.finalize().into()
    }

    fn extend(pcr: [u8; 32], digest: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(pcr);
        h.update(digest);
        h.finalize().into()
    }

    #[test]
    fn parse_accepts_spec_id_only_log() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA1, 20), (TPM_ALG_SHA256, 32)]);
        let log = parse(&b.bytes).expect("parse ok");
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].pcr_index, 0);
        assert_eq!(log[0].event_type, 0x0000_0003);
    }

    #[test]
    fn parse_reads_two_extend_events() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA1, 20), (TPM_ALG_SHA256, 32)]);
        let d1 = sha256(b"event1");
        let d2 = sha256(b"event2");
        b.push_event2(
            4,
            0x0000_0005,
            &[(TPM_ALG_SHA1, vec![0; 20]), (TPM_ALG_SHA256, d1.to_vec())],
            b"payload1",
        );
        b.push_event2(
            7,
            0x0000_0006,
            &[(TPM_ALG_SHA1, vec![0; 20]), (TPM_ALG_SHA256, d2.to_vec())],
            b"payload2",
        );
        let log = parse(&b.bytes).expect("parse ok");
        assert_eq!(log.len(), 3);
        assert_eq!(log[1].pcr_index, 4);
        assert_eq!(log[1].event_data, b"payload1");
        assert_eq!(log[2].pcr_index, 7);
        // SHA-256 digest matches what we planted.
        assert_eq!(log[1].digests[1], (TPM_ALG_SHA256, d1.to_vec()));
        assert_eq!(log[2].digests[1], (TPM_ALG_SHA256, d2.to_vec()));
    }

    #[test]
    fn replay_folds_single_event_into_one_pcr() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        let d = sha256(b"some-event");
        b.push_event2(4, 0x0000_0005, &[(TPM_ALG_SHA256, d.to_vec())], b"");
        let log = parse(&b.bytes).unwrap();
        let pcrs = replay_sha256(&log, 9);
        // PCR 4 should be sha256(0||d). Others stay zero.
        let mut expected_pcr4 = [0u8; 32];
        let extended = extend(expected_pcr4, &d);
        expected_pcr4.copy_from_slice(&extended);
        assert_eq!(pcrs[4], expected_pcr4);
        for i in [0, 1, 2, 3, 5, 6, 7, 8, 9] {
            assert_eq!(pcrs[i], [0u8; 32], "PCR{i} unexpectedly non-zero");
        }
    }

    #[test]
    fn replay_chains_multiple_events_to_same_pcr() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        let d1 = sha256(b"a");
        let d2 = sha256(b"b");
        b.push_event2(8, 0, &[(TPM_ALG_SHA256, d1.to_vec())], b"");
        b.push_event2(8, 0, &[(TPM_ALG_SHA256, d2.to_vec())], b"");
        let log = parse(&b.bytes).unwrap();
        let pcrs = replay_sha256(&log, 9);
        let expected = extend(extend([0u8; 32], &d1), &d2);
        assert_eq!(pcrs[8], expected);
    }

    #[test]
    fn replay_covers_pcrs_0_4_7_8() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        let digests = [
            (0, sha256(b"boot-services-driver")),
            (4, sha256(b"bootloader-hash")),
            (7, sha256(b"secure-boot-policy")),
            (8, sha256(b"kernel-cmdline")),
        ];
        for (pcr, d) in &digests {
            b.push_event2(*pcr, 0, &[(TPM_ALG_SHA256, d.to_vec())], b"");
        }
        let log = parse(&b.bytes).unwrap();
        let pcrs = replay_sha256(&log, 9);
        for (pcr, d) in &digests {
            let expected = extend([0u8; 32], d);
            assert_eq!(
                pcrs[*pcr as usize], expected,
                "PCR{pcr} digest mismatch",
            );
        }
    }

    #[test]
    fn replay_ignores_events_above_max_pcr() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        let d = sha256(b"x");
        // max_pcr will be 9; event at PCR 10 must be skipped.
        b.push_event2(10, 0, &[(TPM_ALG_SHA256, d.to_vec())], b"");
        let log = parse(&b.bytes).unwrap();
        let pcrs = replay_sha256(&log, 9);
        for slot in pcrs.iter() {
            assert_eq!(*slot, [0u8; 32]);
        }
    }

    #[test]
    fn replay_ignores_events_at_or_above_24() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        let d = sha256(b"x");
        // PCR 24 is outside the array — must be silently dropped even
        // if max_pcr is cranked high enough to include it in the mask.
        b.push_event2(24, 0, &[(TPM_ALG_SHA256, d.to_vec())], b"");
        let log = parse(&b.bytes).unwrap();
        let pcrs = replay_sha256(&log, 31);
        for slot in pcrs.iter() {
            assert_eq!(*slot, [0u8; 32]);
        }
    }

    #[test]
    fn replay_ignores_sha1_only_events() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA1, 20), (TPM_ALG_SHA256, 32)]);
        // Event carries only a SHA-1 digest (no SHA-256 entry).
        b.push_event2(4, 0, &[(TPM_ALG_SHA1, vec![0xab; 20])], b"");
        let log = parse(&b.bytes).unwrap();
        let pcrs = replay_sha256(&log, 9);
        assert_eq!(pcrs[4], [0u8; 32], "SHA-1-only event must not affect SHA-256 bank");
    }

    #[test]
    fn parse_fails_on_truncated_legacy_header() {
        // 10 bytes is well short of the 32-byte TCG_PCR_EVENT header.
        let short = vec![0u8; 10];
        assert_eq!(parse(&short), Err(ParseError::HeaderTooShort));
    }

    #[test]
    fn parse_fails_on_empty_buffer() {
        assert_eq!(parse(&[]), Err(ParseError::HeaderTooShort));
    }

    #[test]
    fn parse_fails_on_bad_spec_id_signature() {
        let mut b = LogBuilder::new();
        // Legacy header with 32 bytes of data that LOOK like a spec-id
        // event but have the wrong signature.
        let mut data = vec![0u8; 32];
        data[..16].copy_from_slice(b"NOT A SPEC EVT!!");
        b.push_legacy_event(0, 0x0000_0003, &data);
        assert_eq!(parse(&b.bytes), Err(ParseError::SpecIdInvalid));
    }

    #[test]
    fn parse_fails_on_wrong_spec_version() {
        // Fabricate a legacy event whose spec-id header claims major
        // version 3 instead of 2.
        let mut data = Vec::new();
        data.extend_from_slice(SPEC_ID_SIGNATURE);
        data.extend_from_slice(&0u32.to_le_bytes());
        data.push(0); // minor
        data.push(3); // major (WRONG)
        data.push(0);
        data.push(2);
        data.extend_from_slice(&0u32.to_le_bytes()); // zero algs
        data.push(0); // vendorInfoSize
        let mut b = LogBuilder::new();
        b.push_legacy_event(0, 0x0000_0003, &data);
        assert_eq!(parse(&b.bytes), Err(ParseError::SpecIdInvalid));
    }

    #[test]
    fn parse_fails_on_spec_id_num_algs_overflow() {
        // 17 > MAX_DIGESTS_PER_EVENT (16).
        let mut data = Vec::new();
        data.extend_from_slice(SPEC_ID_SIGNATURE);
        data.extend_from_slice(&0u32.to_le_bytes());
        data.push(0); // minor
        data.push(2); // major
        data.push(0);
        data.push(2);
        data.extend_from_slice(&17u32.to_le_bytes());
        let mut b = LogBuilder::new();
        b.push_legacy_event(0, 0x0000_0003, &data);
        assert_eq!(parse(&b.bytes), Err(ParseError::SpecIdInvalid));
    }

    #[test]
    fn parse_fails_on_truncated_digest_list() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        // Start an event2 header and claim 1 digest, then cut off
        // before even the alg id lands.
        b.push_u32(4); // pcr
        b.push_u32(0); // event type
        b.push_u32(1); // count
        // deliberately NO algId
        assert_eq!(parse(&b.bytes), Err(ParseError::DigestListTruncated));
    }

    #[test]
    fn parse_fails_on_truncated_digest_bytes() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        b.push_u32(4);
        b.push_u32(0);
        b.push_u32(1);
        b.push_u16(TPM_ALG_SHA256);
        // Only 10 bytes where SHA-256 needs 32.
        b.push_bytes(&[0u8; 10]);
        // Cursor is now deep past position 32, so underflow surfaces as
        // EventDataTruncated (read_bytes' post-header branch).
        assert_eq!(parse(&b.bytes), Err(ParseError::EventDataTruncated));
    }

    #[test]
    fn parse_fails_on_event_data_overrun() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        b.push_u32(4);
        b.push_u32(0);
        b.push_u32(1);
        b.push_u16(TPM_ALG_SHA256);
        b.push_bytes(&[0u8; 32]);
        // Claim 1000 bytes of event data but append none.
        b.push_u32(1000);
        assert_eq!(parse(&b.bytes), Err(ParseError::EventDataTruncated));
    }

    #[test]
    fn parse_fails_on_digest_count_overflow() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        b.push_u32(4);
        b.push_u32(0);
        // 1_000 digests — well above MAX_DIGESTS_PER_EVENT.
        b.push_u32(1000);
        assert_eq!(parse(&b.bytes), Err(ParseError::DigestCountOverflow));
    }

    #[test]
    fn parse_fails_on_unknown_alg_without_size() {
        // Spec-id header doesn't advertise 0x0099, and 0x0099 isn't a
        // hard-coded known alg → UnknownAlgTruncated.
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        b.push_u32(4);
        b.push_u32(0);
        b.push_u32(1);
        b.push_u16(0x0099);
        // Whatever bytes follow don't matter — we bail before reading them.
        b.push_bytes(&[0u8; 16]);
        assert_eq!(parse(&b.bytes), Err(ParseError::UnknownAlgTruncated));
    }

    #[test]
    fn parse_accepts_unknown_alg_if_spec_id_advertises_size() {
        // Vendor alg 0x0099 advertised as 8 bytes in spec-id ⇒ parse
        // succeeds; replay then ignores it (we only fold SHA-256).
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32), (0x0099, 8)]);
        b.push_event2(
            4,
            0,
            &[
                (0x0099, vec![0xde; 8]),
                (TPM_ALG_SHA256, sha256(b"seed").to_vec()),
            ],
            b"",
        );
        let log = parse(&b.bytes).unwrap();
        assert_eq!(log[1].digests.len(), 2);
        assert_eq!(log[1].digests[0], (0x0099, vec![0xde; 8]));
        // Replay still folds in only the SHA-256 entry.
        let pcrs = replay_sha256(&log, 9);
        assert_eq!(pcrs[4], extend([0u8; 32], &sha256(b"seed")));
    }

    #[test]
    fn parse_handles_sha384_and_sha512_banks() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[
            (TPM_ALG_SHA1, 20),
            (TPM_ALG_SHA256, 32),
            (TPM_ALG_SHA384, 48),
            (TPM_ALG_SHA512, 64),
        ]);
        b.push_event2(
            4,
            0,
            &[
                (TPM_ALG_SHA1, vec![0; 20]),
                (TPM_ALG_SHA256, vec![1; 32]),
                (TPM_ALG_SHA384, vec![2; 48]),
                (TPM_ALG_SHA512, vec![3; 64]),
            ],
            b"",
        );
        let log = parse(&b.bytes).unwrap();
        assert_eq!(log[1].digests.len(), 4);
        assert_eq!(log[1].digests[2].1.len(), 48);
        assert_eq!(log[1].digests[3].1.len(), 64);
    }

    #[test]
    fn unique_digest_returns_lone_event() {
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        let d = sha256(b"only-one");
        b.push_event2(8, 0, &[(TPM_ALG_SHA256, d.to_vec())], b"");
        let log = parse(&b.bytes).unwrap();
        assert_eq!(unique_sha256_digest_for_pcr(&log, 8), Some(d));
    }

    #[test]
    fn unique_digest_returns_none_when_zero_events() {
        // Spec-id header only, no events on PCR 8 → None.
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        let log = parse(&b.bytes).unwrap();
        assert!(unique_sha256_digest_for_pcr(&log, 8).is_none());
    }

    #[test]
    fn unique_digest_returns_none_when_multiple_events() {
        // Two events extend PCR 8 → ambiguous for step 18's purposes.
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        b.push_event2(8, 0, &[(TPM_ALG_SHA256, sha256(b"a").to_vec())], b"");
        b.push_event2(8, 0, &[(TPM_ALG_SHA256, sha256(b"b").to_vec())], b"");
        let log = parse(&b.bytes).unwrap();
        assert!(unique_sha256_digest_for_pcr(&log, 8).is_none());
    }

    #[test]
    fn unique_digest_ignores_other_pcrs() {
        // Events on PCR 4 must NOT be counted when we ask about PCR 8.
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA256, 32)]);
        b.push_event2(4, 0, &[(TPM_ALG_SHA256, sha256(b"a").to_vec())], b"");
        b.push_event2(4, 0, &[(TPM_ALG_SHA256, sha256(b"b").to_vec())], b"");
        let pcr8 = sha256(b"c");
        b.push_event2(8, 0, &[(TPM_ALG_SHA256, pcr8.to_vec())], b"");
        let log = parse(&b.bytes).unwrap();
        assert_eq!(unique_sha256_digest_for_pcr(&log, 8), Some(pcr8));
    }

    #[test]
    fn unique_digest_returns_none_for_sha1_only_event() {
        // PCR 8 event carries only SHA-1 — no SHA-256 digest to
        // return. Step 18 must bail rather than fall through.
        let mut b = LogBuilder::new();
        b.push_spec_id_header(&[(TPM_ALG_SHA1, 20), (TPM_ALG_SHA256, 32)]);
        b.push_event2(8, 0, &[(TPM_ALG_SHA1, vec![0xAB; 20])], b"");
        let log = parse(&b.bytes).unwrap();
        assert!(unique_sha256_digest_for_pcr(&log, 8).is_none());
    }

    #[test]
    fn display_for_parse_error_covers_each_variant() {
        use ParseError::*;
        for e in [
            HeaderTooShort,
            SpecIdInvalid,
            DigestListTruncated,
            EventDataTruncated,
            DigestCountOverflow,
            UnknownAlgTruncated,
        ] {
            let rendered = format!("{e}");
            assert!(!rendered.is_empty(), "empty Display for {e:?}");
        }
    }
}
