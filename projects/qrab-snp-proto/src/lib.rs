//! On-wire types + AES-256-GCM envelope helpers for the SEV-SNP
//! `SNP_GUEST_REQUEST MSG_REPORT_REQ` exchange.
//!
//! Shared between qrab-boot (UEFI, `no_std`) and host-side test /
//! verifier code (`cargo test` / attest-server). Every `#[repr(C)]`
//! type here mirrors a table in AMD's SEV-SNP ABI Specification;
//! sizes are asserted at compile time against the values the spec
//! quotes in hex.
//!
//! ## Scope
//!
//! Phase 5a (this file) only lays out the types. AES-GCM encrypt /
//! decrypt + IV/AAD helpers live alongside these definitions so
//! both the UEFI bootloader and a host-side round-trip test can
//! exercise the same code path. Phase 5c wires the actual VMGEXIT.
//!
//! ## References (AMD SEV-SNP ABI Spec v1.55 / rev 2.03)
//!
//!   * Table 19 — MSG_REPORT_REQ (96 B)
//!   * Table 21 — MSG_REPORT_RESP (1280 B)
//!   * Table 23 — Attestation Report (1184 B)
//!   * Table 98 — SNP_GUEST_MSG header (96 B)
//!   * Table 99 — SNP_GUEST_MSG envelope (4096 B)
//!   * §5.4.2  — AEAD envelope (AAD = hdr[0x30..0x60],
//!               IV = MsgSeqno (LE) || 4 zero bytes)
//!   * §5.4.3  — SNP Secrets page layout (VMPCK0..3 location)

#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(any(feature = "std", test))]
extern crate std;

mod envelope;
pub use envelope::{
    EnvelopeError, REPORT_REQ_PLAINTEXT_SIZE, decrypt_guest_msg, encrypt_guest_msg,
};

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// Message type octet that appears in `SnpGuestMsgHdr::msg_type`.
/// Only the variants we exercise are listed.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnpMsgType {
    /// `MSG_REPORT_REQ` — guest asks PSP for a fresh attestation
    /// report over the 64-byte `report_data`.
    ReportReq = 5,
    /// `MSG_REPORT_RESP` — PSP returns the signed 1184-byte
    /// attestation report.
    ReportResp = 6,
}

/// `AEAD_AES_256_GCM` identifier for `SnpGuestMsgHdr::algo_id`.
pub const SNP_AEAD_AES_256_GCM: u8 = 1;

/// `HdrVersion` octet written into `SnpGuestMsgHdr::hdr_version`.
/// Currently `1` per spec; if AMD ever bumps this we want to notice.
pub const SNP_GUEST_MSG_HDR_VERSION: u8 = 1;

/// `MsgVersion` octet written into `SnpGuestMsgHdr::msg_version`
/// for the REPORT_REQ / REPORT_RESP pair.
pub const SNP_REPORT_MSG_VERSION: u8 = 1;

/// Total envelope size — PSP expects the guest-request page to be
/// exactly one 4 KiB page.
pub const SNP_GUEST_MSG_SIZE: usize = 4096;

/// Size of the plaintext `MSG_REPORT_REQ` payload (Table 19).
pub const MSG_REPORT_REQ_SIZE: usize = 96;

/// Size of the plaintext `MSG_REPORT_RESP` payload (Table 21).
pub const MSG_REPORT_RESP_SIZE: usize = 1280;

/// Size of the attestation report proper (Table 23).
pub const SNP_ATTESTATION_REPORT_SIZE: usize = 1184;

/// Length of a single VMPCK (AES-256 key, 32 bytes).
pub const VMPCK_LEN: usize = 32;

/// Length of the AES-GCM authentication tag actually used (16 B).
/// `SnpGuestMsgHdr::auth_tag` is 32 bytes so AMD reserved room for a
/// longer tag; only the first 16 are populated for AES-256-GCM.
pub const AEAD_TAG_LEN: usize = 16;

/// Length of the AES-GCM nonce / IV (12 B).
pub const AEAD_IV_LEN: usize = 12;

/// Table 19 — `MSG_REPORT_REQ` plaintext payload.
///
/// Wire layout (96 B, little-endian):
///
/// ```text
///   0x000  report_data[64]   SHA-256(SIG_DOMAIN_TAG || nonces || ...)
///   0x040  vmpl              u32, 0..3 — must match VMPCK index
///   0x044  reserved[28]      must be zero
/// ```
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct MsgReportReq {
    pub report_data: [u8; 64],
    pub vmpl: u32,
    pub reserved: [u8; 28],
}

impl MsgReportReq {
    /// Build a fresh request with the given `report_data` and VMPL.
    /// VMPL0 is the only level qrab-boot runs at; keep the argument
    /// explicit so that future multi-VMPL experiments have to be
    /// deliberate.
    pub const fn new(report_data: [u8; 64], vmpl: u32) -> Self {
        Self {
            report_data,
            vmpl,
            reserved: [0; 28],
        }
    }
}

/// Table 21 — `MSG_REPORT_RESP` plaintext payload. The inner
/// [`SnpAttestationReport`] holds the signed evidence; the
/// surrounding `status` / `report_size` fields let the guest detect
/// PSP-level failures (`status != 0`) before parsing.
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct MsgReportResp {
    pub status: u32,
    pub report_size: u32,
    pub reserved: [u8; 24],
    pub report: SnpAttestationReport,
    /// Per Table 21 the payload is padded to 1280 B so AEAD blocks
    /// align cleanly; we keep the tail explicit rather than hiding
    /// it behind a slice so `size_of::<MsgReportResp>() == 1280`.
    pub padding: [u8; MSG_REPORT_RESP_SIZE
        - 4 // status
        - 4 // report_size
        - 24 // reserved
        - SNP_ATTESTATION_REPORT_SIZE],
}

/// Status codes the PSP returns in `MsgReportResp::status`.
///
/// This is not exhaustive — AMD reserves the full u32 space — but
/// the common ones are worth naming so the caller's error surface
/// is specific.
pub mod report_status {
    pub const SUCCESS: u32 = 0x0000_0000;
    pub const INVALID_PARAMETER: u32 = 0x0000_0016;
    pub const INVALID_KEY: u32 = 0x0000_0027;
}

/// Table 23 — SNP Attestation Report. Layout is position-sensitive;
/// field-order changes break the signature check against ARK/ASK/VCEK.
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct SnpAttestationReport {
    pub version: u32,
    pub guest_svn: u32,
    pub policy: u64,
    pub family_id: [u8; 16],
    pub image_id: [u8; 16],
    pub vmpl: u32,
    pub signature_algo: u32,
    pub current_tcb: u64,
    pub platform_info: u64,
    pub flags: u32,
    pub reserved0: u32,
    pub report_data: [u8; 64],
    pub measurement: [u8; 48],
    pub host_data: [u8; 32],
    pub id_key_digest: [u8; 48],
    pub author_key_digest: [u8; 48],
    pub report_id: [u8; 32],
    pub report_id_ma: [u8; 32],
    pub reported_tcb: u64,
    pub reserved1: [u8; 24],
    pub chip_id: [u8; 64],
    pub committed_tcb: u64,
    pub current_build: u8,
    pub current_minor: u8,
    pub current_major: u8,
    pub reserved2: u8,
    pub committed_build: u8,
    pub committed_minor: u8,
    pub committed_major: u8,
    pub reserved3: u8,
    pub launch_tcb: u64,
    pub reserved4: [u8; 168],
    pub signature: [u8; 512],
}

/// Table 98 — `SNP_GUEST_MSG` header. Exactly 96 bytes; first 32
/// hold the AES-256-GCM auth tag (16 used + 16 reserved), the next
/// 16 hold `msg_seqno` + a reserved quadword, and the last 48 are
/// the **AAD** region (`algo_id` .. `reserved3`).
///
/// AAD boundary is at offset 0x30 — see §5.4.2.
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct SnpGuestMsgHdr {
    pub auth_tag: [u8; 32],
    pub msg_seqno: u64,
    pub reserved1: [u8; 8],
    pub algo_id: u8,
    pub hdr_version: u8,
    pub hdr_size: u16,
    pub msg_type: u8,
    pub msg_version: u8,
    pub msg_size: u16,
    pub reserved2: u32,
    pub msg_vmpck: u8,
    pub reserved3: [u8; 35],
}

/// Byte offset where the AAD region begins inside
/// [`SnpGuestMsgHdr`]. Hard-coded because it's load-bearing for the
/// AES-GCM construction and callers will reference it directly.
pub const SNP_GUEST_MSG_AAD_OFFSET: usize = 0x30;

/// Length of the AAD region inside [`SnpGuestMsgHdr`] (from
/// `SNP_GUEST_MSG_AAD_OFFSET` through the end of the header).
pub const SNP_GUEST_MSG_AAD_LEN: usize = 0x60 - SNP_GUEST_MSG_AAD_OFFSET;

/// Table 99 — the full 4 KiB `SNP_GUEST_MSG` envelope: 96-byte
/// header followed by a 4000-byte payload. On the wire the payload
/// is always AES-256-GCM ciphertext; the plaintext decrypts into a
/// [`MsgReportReq`] or [`MsgReportResp`] (zero-padded to 4000 B).
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct SnpGuestMsg {
    pub hdr: SnpGuestMsgHdr,
    pub payload: [u8; SNP_GUEST_MSG_SIZE - 96],
}

impl SnpGuestMsg {
    /// Extract the AAD slice that the AES-GCM construction demands
    /// from a raw 4 KiB envelope buffer. Mirrors AMD SEV-SNP ABI
    /// Spec §5.4.2.
    pub fn aad_slice(bytes: &[u8; SNP_GUEST_MSG_SIZE]) -> &[u8] {
        &bytes[SNP_GUEST_MSG_AAD_OFFSET..0x60]
    }

    /// Build the 12-byte AES-GCM IV from a 64-bit message sequence
    /// number. Per §5.4.2 the IV is the LE encoding of `msg_seqno`
    /// followed by four zero bytes.
    pub fn iv_from_seqno(seqno: u64) -> [u8; AEAD_IV_LEN] {
        let mut iv = [0u8; AEAD_IV_LEN];
        iv[..8].copy_from_slice(&seqno.to_le_bytes());
        iv
    }
}

/// Subset of the SNP Secrets page layout that we actually read. The
/// spec-defined on-wire layout is 4 KiB; we give names to the
/// fields up through VMPCK3 + ignore the rest. Offsets match the
/// AMD SEV-SNP ABI Spec §5.4.3 and edk2's
/// `OvmfPkg/Include/Library/PlatformInitLib.h`.
///
/// `reserved_tail` absorbs everything past the four VMPCKs so the
/// struct still totals 4096 B — keeps `size_of::<SnpSecretsPage>()
/// == 4096`. Size = 4096 − 0xa0 (end of VMPCK3) = 3936.
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct SnpSecretsPage {
    pub version: u32,
    pub imien: u32,
    pub fms: u32,
    pub reserved0: [u8; 4],
    pub gosv: [u8; 16],
    pub vmpck0: [u8; VMPCK_LEN],
    pub vmpck1: [u8; VMPCK_LEN],
    pub vmpck2: [u8; VMPCK_LEN],
    pub vmpck3: [u8; VMPCK_LEN],
    pub reserved_tail: [u8; 4096 - 0xa0],
}

/// Failure modes from [`validate_secrets_page`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretsValidateError {
    /// The `SnpSecretsPage::version` field is zero, which the spec
    /// disallows — the page has either not been populated by the
    /// PSP or the caller is reading the wrong PA.
    VersionZero,
    /// All four VMPCK entries are zero. On a real SNP launch every
    /// VMPCK is randomly generated, so seeing all-zero means the
    /// page was either not populated or has already been wiped.
    AllVmpcksZero,
}

/// Check that a [`SnpSecretsPage`] looks plausibly populated by the
/// PSP. This is a cheap sanity check that catches the common
/// "wrong PA" failure mode without being a substitute for the real
/// RMP / attestation-report validation downstream of
/// `MSG_REPORT_REQ`.
pub fn validate_secrets_page(
    page: &SnpSecretsPage,
) -> Result<(), SecretsValidateError> {
    if { page.version } == 0 {
        return Err(SecretsValidateError::VersionZero);
    }
    let any_nonzero = page.vmpck0.iter().any(|&b| b != 0)
        || page.vmpck1.iter().any(|&b| b != 0)
        || page.vmpck2.iter().any(|&b| b != 0)
        || page.vmpck3.iter().any(|&b| b != 0);
    if !any_nonzero {
        return Err(SecretsValidateError::AllVmpcksZero);
    }
    Ok(())
}

/// `true` if `vmpck` contains at least one non-zero byte. Exposed
/// so callers can log a non-zeroness token without copying the
/// VMPCK bytes themselves.
#[inline]
pub fn is_vmpck_nonzero(vmpck: &[u8]) -> bool {
    vmpck.iter().any(|&b| b != 0)
}

// ---- compile-time size assertions ------------------------------------------
//
// These catch the kind of off-by-one layout bugs that would only
// surface as "report signature does not verify" in a live SNP
// smoke. Assert against the hex constants AMD publishes so the
// numbers are easy to cross-reference with the spec.

const _: () = {
    // SIZES — these are spec-quoted constants, not magic numbers.
    assert!(core::mem::size_of::<MsgReportReq>() == MSG_REPORT_REQ_SIZE);
    assert!(core::mem::size_of::<MsgReportReq>() == 96);
    assert!(core::mem::size_of::<SnpAttestationReport>() == SNP_ATTESTATION_REPORT_SIZE);
    assert!(core::mem::size_of::<SnpAttestationReport>() == 1184);
    assert!(core::mem::size_of::<MsgReportResp>() == MSG_REPORT_RESP_SIZE);
    assert!(core::mem::size_of::<MsgReportResp>() == 1280);
    assert!(core::mem::size_of::<SnpGuestMsgHdr>() == 96);
    assert!(core::mem::size_of::<SnpGuestMsg>() == SNP_GUEST_MSG_SIZE);
    assert!(core::mem::size_of::<SnpGuestMsg>() == 4096);
    assert!(core::mem::size_of::<SnpSecretsPage>() == 4096);

    // AAD boundary — §5.4.2. If this slips, AES-GCM tag verification
    // breaks without any obvious diagnostic.
    assert!(SNP_GUEST_MSG_AAD_OFFSET == 0x30);
    assert!(SNP_GUEST_MSG_AAD_LEN == 0x30);
};

// ---- host-side unit tests --------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::offset_of;

    #[test]
    fn msg_report_req_layout() {
        assert_eq!(offset_of!(MsgReportReq, report_data), 0x00);
        assert_eq!(offset_of!(MsgReportReq, vmpl), 0x40);
        assert_eq!(offset_of!(MsgReportReq, reserved), 0x44);
    }

    #[test]
    fn attestation_report_layout() {
        assert_eq!(offset_of!(SnpAttestationReport, version), 0x000);
        assert_eq!(offset_of!(SnpAttestationReport, guest_svn), 0x004);
        assert_eq!(offset_of!(SnpAttestationReport, policy), 0x008);
        assert_eq!(offset_of!(SnpAttestationReport, report_data), 0x050);
        assert_eq!(offset_of!(SnpAttestationReport, measurement), 0x090);
        assert_eq!(offset_of!(SnpAttestationReport, host_data), 0x0c0);
        assert_eq!(offset_of!(SnpAttestationReport, id_key_digest), 0x0e0);
        assert_eq!(offset_of!(SnpAttestationReport, report_id), 0x140);
        assert_eq!(offset_of!(SnpAttestationReport, chip_id), 0x1a0);
        assert_eq!(offset_of!(SnpAttestationReport, launch_tcb), 0x1f0);
        assert_eq!(offset_of!(SnpAttestationReport, reserved4), 0x1f8);
        assert_eq!(offset_of!(SnpAttestationReport, signature), 0x2a0);
    }

    #[test]
    fn msg_report_resp_layout() {
        assert_eq!(offset_of!(MsgReportResp, status), 0x00);
        assert_eq!(offset_of!(MsgReportResp, report_size), 0x04);
        assert_eq!(offset_of!(MsgReportResp, reserved), 0x08);
        assert_eq!(offset_of!(MsgReportResp, report), 0x20);
    }

    #[test]
    fn snp_guest_msg_hdr_layout() {
        assert_eq!(offset_of!(SnpGuestMsgHdr, auth_tag), 0x00);
        assert_eq!(offset_of!(SnpGuestMsgHdr, msg_seqno), 0x20);
        assert_eq!(offset_of!(SnpGuestMsgHdr, reserved1), 0x28);
        assert_eq!(offset_of!(SnpGuestMsgHdr, algo_id), 0x30);
        assert_eq!(offset_of!(SnpGuestMsgHdr, hdr_version), 0x31);
        assert_eq!(offset_of!(SnpGuestMsgHdr, hdr_size), 0x32);
        assert_eq!(offset_of!(SnpGuestMsgHdr, msg_type), 0x34);
        assert_eq!(offset_of!(SnpGuestMsgHdr, msg_version), 0x35);
        assert_eq!(offset_of!(SnpGuestMsgHdr, msg_size), 0x36);
        assert_eq!(offset_of!(SnpGuestMsgHdr, msg_vmpck), 0x3c);
        assert_eq!(offset_of!(SnpGuestMsgHdr, reserved3), 0x3d);
    }

    #[test]
    fn secrets_page_layout() {
        assert_eq!(offset_of!(SnpSecretsPage, version), 0x000);
        assert_eq!(offset_of!(SnpSecretsPage, gosv), 0x010);
        assert_eq!(offset_of!(SnpSecretsPage, vmpck0), 0x020);
        assert_eq!(offset_of!(SnpSecretsPage, vmpck1), 0x040);
        assert_eq!(offset_of!(SnpSecretsPage, vmpck2), 0x060);
        assert_eq!(offset_of!(SnpSecretsPage, vmpck3), 0x080);
    }

    #[test]
    fn iv_derivation() {
        // Low 8 bytes = seqno LE, high 4 bytes = zero.
        let iv = SnpGuestMsg::iv_from_seqno(0x0102_0304_0506_0708);
        assert_eq!(
            iv,
            [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0, 0, 0, 0]
        );
    }

    #[test]
    fn iv_zero_seqno_is_all_zero() {
        assert_eq!(SnpGuestMsg::iv_from_seqno(0), [0u8; AEAD_IV_LEN]);
    }

    #[test]
    fn aad_slice_range() {
        let mut buf = [0u8; SNP_GUEST_MSG_SIZE];
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (i & 0xff) as u8;
        }
        let aad = SnpGuestMsg::aad_slice(&buf);
        assert_eq!(aad.len(), SNP_GUEST_MSG_AAD_LEN);
        assert_eq!(aad.len(), 0x30);
        assert_eq!(aad[0], 0x30); // algo_id byte, offset 0x30
        assert_eq!(aad[aad.len() - 1], 0x5f); // last AAD byte, offset 0x5f
    }

    #[test]
    fn msg_report_req_builder() {
        let mut rd = [0u8; 64];
        rd[0] = 0xaa;
        rd[63] = 0x55;
        let req = MsgReportReq::new(rd, 0);
        assert_eq!(req.report_data[0], 0xaa);
        assert_eq!(req.report_data[63], 0x55);
        assert_eq!({ req.vmpl }, 0);
        assert!(req.reserved.iter().all(|&b| b == 0));
    }

    #[test]
    fn msg_report_req_is_zeroed_except_report_data_and_vmpl() {
        let req = MsgReportReq::new([0; 64], 1);
        let bytes = <MsgReportReq as IntoBytes>::as_bytes(&req);
        // Bytes 0x40..0x44 hold vmpl=1 (LE), rest zero.
        assert_eq!(&bytes[..0x40], &[0; 0x40]);
        assert_eq!(&bytes[0x40..0x44], &[0x01, 0, 0, 0]);
        assert!(bytes[0x44..].iter().all(|&b| b == 0));
    }

    #[test]
    fn report_status_codes() {
        assert_eq!(report_status::SUCCESS, 0);
        assert_eq!(report_status::INVALID_PARAMETER, 0x16);
        assert_eq!(report_status::INVALID_KEY, 0x27);
    }

    #[test]
    fn snp_msg_type_discriminants() {
        assert_eq!(SnpMsgType::ReportReq as u8, 5);
        assert_eq!(SnpMsgType::ReportResp as u8, 6);
    }

    #[test]
    fn constants_match_spec() {
        assert_eq!(SNP_AEAD_AES_256_GCM, 1);
        assert_eq!(SNP_GUEST_MSG_HDR_VERSION, 1);
        assert_eq!(SNP_REPORT_MSG_VERSION, 1);
        assert_eq!(VMPCK_LEN, 32);
        assert_eq!(AEAD_TAG_LEN, 16);
        assert_eq!(AEAD_IV_LEN, 12);
    }

    /// Build an all-zero `SnpSecretsPage` to use as a starting point
    /// for the validation tests.
    fn zero_secrets_page() -> SnpSecretsPage {
        let bytes = [0u8; 4096];
        SnpSecretsPage::read_from_bytes(&bytes).expect("4096 B decodes")
    }

    #[test]
    fn validate_secrets_page_rejects_version_zero() {
        let page = zero_secrets_page();
        assert_eq!(
            validate_secrets_page(&page),
            Err(SecretsValidateError::VersionZero)
        );
    }

    #[test]
    fn validate_secrets_page_rejects_all_zero_vmpcks() {
        let mut page = zero_secrets_page();
        page.version = 1;
        assert_eq!(
            validate_secrets_page(&page),
            Err(SecretsValidateError::AllVmpcksZero)
        );
    }

    #[test]
    fn validate_secrets_page_accepts_vmpck0_only() {
        let mut page = zero_secrets_page();
        page.version = 1;
        page.vmpck0[0] = 0x42;
        assert_eq!(validate_secrets_page(&page), Ok(()));
    }

    #[test]
    fn validate_secrets_page_accepts_vmpck3_only() {
        let mut page = zero_secrets_page();
        page.version = 3;
        page.vmpck3[31] = 0xff;
        assert_eq!(validate_secrets_page(&page), Ok(()));
    }

    #[test]
    fn is_vmpck_nonzero_detects_set_bit() {
        assert!(!is_vmpck_nonzero(&[0u8; 32]));
        let mut v = [0u8; 32];
        v[17] = 0x01;
        assert!(is_vmpck_nonzero(&v));
    }
}
