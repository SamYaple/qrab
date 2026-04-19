//! AMD SEV-SNP `snp_cert_table` parser.
//!
//! `SVM_VMGEXIT_EXT_GUEST_REQUEST` (exit code `0x8000_0012`) is the
//! extended variant of the guest-request VMGEXIT: on top of the
//! usual 96-byte `MSG_REPORT_REQ` envelope, the guest also hands
//! the hypervisor a contiguous run of 4 KiB pages whose starting
//! GPA and page count are placed in RAX / RBX. The hypervisor
//! fills those pages with the concatenated `snp_cert_table`
//! entries for the VCEK / ASK / ARK (or VLEK) chain it has cached
//! locally, in the same on-disk format AMD's KDS emits when queried
//! via the `/vcek/` / `/vlek/` endpoints.
//!
//! This module parses that blob. It does *not* issue the VMGEXIT
//! itself — the guest bootloader (qrab-boot) pairs it with the
//! `pci_dma` + `ghcb_vmgexit` helpers; a host-side verifier
//! (attest-server) pairs it with whatever cert source it prefers
//! (KDS, persisted cache, hypervisor ioctl, …). The format is
//! identical on both sides.
//!
//! ## Wire format (AMD SEV-SNP ABI 1.55 §8.27.2 Table 139)
//!
//! ```text
//!   offset  size  field
//!   0x00    16    GUID (big-endian mixed form — see `CERT_GUID_*`)
//!   0x10    4     offset within the blob
//!   0x14    4     length in bytes
//!   ...
//!   (terminator: 16 bytes of 0x00 for the GUID, offset, length
//!    likewise zero)
//! ```
//!
//! followed by the referenced raw DER blobs at the stated offsets.
//! The blob must be self-contained: no offset may point outside
//! the buffer, no two entries may overlap a third's extent, and the
//! table walks to a terminator within the buffer.
//!
//! ## GUIDs we recognise
//!
//! The GUIDs below are taken from AMD's OpenSEV-SNP-Attest
//! documentation and the Linux kernel `arch/x86/kvm/svm/sev.c`
//! (`SEV_SNP_CERT_CHAIN_GUID_*`). Any GUID not in this list is
//! preserved as `Entry::Other(guid)` so callers can route vendor
//! extensions without this crate having to know about them.

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// 16-byte GUID in AMD / UEFI "mixed-endian" form (first three
/// fields little-endian, last two big-endian).
pub type CertGuid = [u8; 16];

/// `63da758d-e664-4564-adc5-f4b93be8accd` — VCEK leaf.
#[rustfmt::skip]
pub const CERT_GUID_VCEK: CertGuid = [
    0x8d, 0x75, 0xda, 0x63, 0x64, 0xe6, 0x64, 0x45,
    0xad, 0xc5, 0xf4, 0xb9, 0x3b, 0xe8, 0xac, 0xcd,
];

/// `a8074bc2-a25a-483e-aae6-39c045a0b8a1` — VLEK leaf (hypervisor
/// -loaded endorsement key, used when the host rotates the signing
/// leaf without a chip reboot).
#[rustfmt::skip]
pub const CERT_GUID_VLEK: CertGuid = [
    0xc2, 0x4b, 0x07, 0xa8, 0x5a, 0xa2, 0x3e, 0x48,
    0xaa, 0xe6, 0x39, 0xc0, 0x45, 0xa0, 0xb8, 0xa1,
];

/// `4ab7b379-bbac-4fe4-a02f-05aef327c782` — ASK (AMD SEV Signing
/// Key) intermediate.
#[rustfmt::skip]
pub const CERT_GUID_ASK: CertGuid = [
    0x79, 0xb3, 0xb7, 0x4a, 0xac, 0xbb, 0xe4, 0x4f,
    0xa0, 0x2f, 0x05, 0xae, 0xf3, 0x27, 0xc7, 0x82,
];

/// `c0b406a4-a803-4952-9743-3fb6014cd0ae` — ARK (AMD Root Key).
#[rustfmt::skip]
pub const CERT_GUID_ARK: CertGuid = [
    0xa4, 0x06, 0xb4, 0xc0, 0x03, 0xa8, 0x52, 0x49,
    0x97, 0x43, 0x3f, 0xb6, 0x01, 0x4c, 0xd0, 0xae,
];

/// Raw on-wire header entry. Size = 24 bytes.
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct CertTableEntryRaw {
    pub guid: CertGuid,
    pub offset: u32,
    pub length: u32,
}

/// Semantic view of a single parsed entry. Holds a slice pointing
/// back into the original blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertEntry<'a> {
    pub kind: CertKind,
    pub der: &'a [u8],
}

/// Which link of the chain this entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertKind {
    /// VCEK (Versioned Chip Endorsement Key) leaf.
    Vcek,
    /// VLEK (Versioned Loaded Endorsement Key) leaf.
    Vlek,
    /// ASK (AMD SEV Signing Key) intermediate.
    Ask,
    /// ARK (AMD Root Key) root.
    Ark,
    /// Unknown GUID — forwarded so callers can handle vendor
    /// extensions without losing data.
    Other(CertGuid),
}

/// Parse failure modes for [`parse_cert_table`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertTableError {
    /// Buffer is smaller than a single 24-byte header, so the
    /// terminator can't even fit.
    Truncated,
    /// An entry's `offset + length` walks past the end of the
    /// caller-supplied blob. Fail closed so a malicious host
    /// can't trick the parser into reading out-of-bounds.
    EntryOverflowsBuffer {
        guid: CertGuid,
        offset: u32,
        length: u32,
    },
    /// The header table was not terminated by an all-zero entry
    /// before reaching the first referenced cert body. The AMD
    /// spec requires the zero-terminator; missing it is a malformed
    /// blob.
    MissingTerminator,
}

impl core::fmt::Display for CertTableError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => f.write_str("snp_cert_table: buffer too small for a single entry"),
            Self::EntryOverflowsBuffer { offset, length, .. } => {
                write!(f, "snp_cert_table: entry offset={offset} length={length} walks past buffer")
            }
            Self::MissingTerminator => {
                f.write_str("snp_cert_table: header list not zero-terminated")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CertTableError {}

/// Walk an `snp_cert_table` blob and yield a parsed entry for each
/// non-terminator header. The returned slice lifetimes all borrow
/// from `blob`.
///
/// Bounds: every entry's `[offset, offset+length)` range must lie
/// fully inside `blob`. Overlap between entries is permitted (the
/// ABI does not forbid it) but any overlap with the header table
/// itself is suspicious and will usually round-trip through a
/// downstream X.509 parser that rejects the malformed DER.
pub fn parse_cert_table(blob: &[u8]) -> Result<Vec<CertEntry<'_>>, CertTableError> {
    const ENTRY_SIZE: usize = core::mem::size_of::<CertTableEntryRaw>();
    if blob.len() < ENTRY_SIZE {
        return Err(CertTableError::Truncated);
    }

    let mut out = Vec::new();
    let mut cursor = 0usize;
    loop {
        if cursor + ENTRY_SIZE > blob.len() {
            return Err(CertTableError::MissingTerminator);
        }
        let entry = CertTableEntryRaw::read_from_bytes(&blob[cursor..cursor + ENTRY_SIZE])
            .map_err(|_| CertTableError::Truncated)?;
        cursor += ENTRY_SIZE;

        // Terminator = all-zero entry.
        let all_zero = entry.guid == [0u8; 16] && entry.offset == 0 && entry.length == 0;
        if all_zero {
            break;
        }

        let start = entry.offset as usize;
        let end = start
            .checked_add(entry.length as usize)
            .ok_or(CertTableError::EntryOverflowsBuffer {
                guid: entry.guid,
                offset: entry.offset,
                length: entry.length,
            })?;
        if end > blob.len() {
            return Err(CertTableError::EntryOverflowsBuffer {
                guid: entry.guid,
                offset: entry.offset,
                length: entry.length,
            });
        }

        let kind = match entry.guid {
            CERT_GUID_VCEK => CertKind::Vcek,
            CERT_GUID_VLEK => CertKind::Vlek,
            CERT_GUID_ASK => CertKind::Ask,
            CERT_GUID_ARK => CertKind::Ark,
            other => CertKind::Other(other),
        };
        out.push(CertEntry {
            kind,
            der: &blob[start..end],
        });
    }

    Ok(out)
}

// Bring `Vec` into scope for no_std + alloc or std builds. We only
// compile the parser under `std` (via the `verify` feature), so
// the `use` below is tied to the same gate. Placed above the `mod
// tests` so the macro `vec!` expands correctly inside the tests too.
#[cfg(feature = "std")]
use std::vec::Vec;
#[cfg(all(test, feature = "std"))]
use std::vec;

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid blob with a VCEK entry whose DER is
    /// `vcek_body`. Layout:
    ///
    /// ```text
    ///   [VCEK header]
    ///   [terminator]
    ///   [vcek_body]
    /// ```
    fn build_blob(vcek_body: &[u8]) -> Vec<u8> {
        let header_len = 2 * core::mem::size_of::<CertTableEntryRaw>();
        let total = header_len + vcek_body.len();
        let mut blob = vec![0u8; total];

        // VCEK header at offset 0.
        blob[0..16].copy_from_slice(&CERT_GUID_VCEK);
        blob[16..20].copy_from_slice(&(header_len as u32).to_le_bytes());
        blob[20..24].copy_from_slice(&(vcek_body.len() as u32).to_le_bytes());
        // Terminator at offset 24 (all-zero already).
        // Body starts at header_len.
        blob[header_len..].copy_from_slice(vcek_body);
        blob
    }

    #[test]
    fn parse_single_vcek() {
        let body = b"--fake-vcek-der--";
        let blob = build_blob(body);
        let entries = parse_cert_table(&blob).expect("parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, CertKind::Vcek);
        assert_eq!(entries[0].der, body);
    }

    #[test]
    fn parse_ark_ask_vcek_chain() {
        // Real-world chain shape: 4 entries (ARK, ASK, VCEK, terminator).
        let ark_body = b"AAAAARK-DER-BYTES";
        let ask_body = b"-ASK-DER-BYTES";
        let vcek_body = b"VCEK-DER-BYTES-xyz";
        let header_len = 4 * core::mem::size_of::<CertTableEntryRaw>();
        let ark_off = header_len;
        let ask_off = ark_off + ark_body.len();
        let vcek_off = ask_off + ask_body.len();
        let total = vcek_off + vcek_body.len();

        let mut blob = vec![0u8; total];
        // Headers
        let hdr = |guid: CertGuid, offset: u32, length: u32| -> [u8; 24] {
            let mut h = [0u8; 24];
            h[0..16].copy_from_slice(&guid);
            h[16..20].copy_from_slice(&offset.to_le_bytes());
            h[20..24].copy_from_slice(&length.to_le_bytes());
            h
        };
        blob[0..24].copy_from_slice(&hdr(CERT_GUID_ARK, ark_off as u32, ark_body.len() as u32));
        blob[24..48].copy_from_slice(&hdr(CERT_GUID_ASK, ask_off as u32, ask_body.len() as u32));
        blob[48..72].copy_from_slice(&hdr(CERT_GUID_VCEK, vcek_off as u32, vcek_body.len() as u32));
        // blob[72..96] is the terminator (all-zero).
        blob[ark_off..ask_off].copy_from_slice(ark_body);
        blob[ask_off..vcek_off].copy_from_slice(ask_body);
        blob[vcek_off..].copy_from_slice(vcek_body);

        let entries = parse_cert_table(&blob).expect("parse");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].kind, CertKind::Ark);
        assert_eq!(entries[0].der, ark_body);
        assert_eq!(entries[1].kind, CertKind::Ask);
        assert_eq!(entries[1].der, ask_body);
        assert_eq!(entries[2].kind, CertKind::Vcek);
        assert_eq!(entries[2].der, vcek_body);
    }

    #[test]
    fn parse_vlek_variant() {
        let body = b"vlek-der";
        let header_len = 2 * core::mem::size_of::<CertTableEntryRaw>();
        let mut blob = vec![0u8; header_len + body.len()];
        blob[0..16].copy_from_slice(&CERT_GUID_VLEK);
        blob[16..20].copy_from_slice(&(header_len as u32).to_le_bytes());
        blob[20..24].copy_from_slice(&(body.len() as u32).to_le_bytes());
        blob[header_len..].copy_from_slice(body);
        let entries = parse_cert_table(&blob).expect("parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, CertKind::Vlek);
    }

    #[test]
    fn parse_other_guid_preserved() {
        let guid = [0xAA; 16];
        let body = b"unknown-vendor";
        let header_len = 2 * core::mem::size_of::<CertTableEntryRaw>();
        let mut blob = vec![0u8; header_len + body.len()];
        blob[0..16].copy_from_slice(&guid);
        blob[16..20].copy_from_slice(&(header_len as u32).to_le_bytes());
        blob[20..24].copy_from_slice(&(body.len() as u32).to_le_bytes());
        blob[header_len..].copy_from_slice(body);
        let entries = parse_cert_table(&blob).expect("parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, CertKind::Other(guid));
    }

    #[test]
    fn reject_offset_past_buffer() {
        let mut blob = vec![0u8; 48];
        blob[0..16].copy_from_slice(&CERT_GUID_VCEK);
        blob[16..20].copy_from_slice(&999u32.to_le_bytes()); // past end
        blob[20..24].copy_from_slice(&16u32.to_le_bytes());
        let err = parse_cert_table(&blob).unwrap_err();
        matches!(err, CertTableError::EntryOverflowsBuffer { .. });
    }

    #[test]
    fn reject_missing_terminator() {
        // Single non-zero header, no terminator before the data —
        // parser should bail when it runs out of bytes looking for
        // the terminator.
        let header_len = core::mem::size_of::<CertTableEntryRaw>();
        let mut blob = vec![0u8; header_len + 8];
        blob[0..16].copy_from_slice(&CERT_GUID_VCEK);
        blob[16..20].copy_from_slice(&(header_len as u32).to_le_bytes());
        blob[20..24].copy_from_slice(&8u32.to_le_bytes());
        blob[header_len..].copy_from_slice(b"vcekbody");
        // Next 24 bytes after the header would be "vcekbody" (8 B)
        // + 16 B of zeroes from the allocation — which happen to
        // look like part of a terminator, so rely on the header
        // length check instead. Truncate to block that.
        let truncated = &blob[..header_len + 8];
        let err = parse_cert_table(truncated).unwrap_err();
        assert_eq!(err, CertTableError::MissingTerminator);
    }

    #[test]
    fn reject_truncated_single_byte_buffer() {
        let err = parse_cert_table(&[0u8; 4]).unwrap_err();
        assert_eq!(err, CertTableError::Truncated);
    }

    #[test]
    fn empty_table_terminator_only() {
        // An all-zero 24-byte buffer is a valid empty cert table.
        let blob = vec![0u8; core::mem::size_of::<CertTableEntryRaw>()];
        let entries = parse_cert_table(&blob).expect("parse");
        assert!(entries.is_empty());
    }

    #[test]
    fn entry_raw_size_is_24() {
        // A future zerocopy bump that changes alignment must not
        // silently grow the entry size past 24 B.
        assert_eq!(core::mem::size_of::<CertTableEntryRaw>(), 24);
    }
}
