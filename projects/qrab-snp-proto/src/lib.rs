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
//! The `envelope` module handles AES-256-GCM encrypt / decrypt with
//! the spec IV/AAD rules; qrab-boot drives the full VMGEXIT from
//! `ghcb.rs`'s opcode + save-area helpers and its own inline-asm
//! `rep; vmmcall`. The `cert_table` module (feature `alloc`) parses
//! the VCEK / ASK / ARK blob returned by
//! `SVM_VMGEXIT_EXT_GUEST_REQUEST` or fetched from AMD KDS. The
//! `verify` module (feature `verify`, host-only) performs the
//! ECDSA-P384 signature check over the 672-byte signed prefix of
//! an `SnpAttestationReport`.
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

#[cfg(feature = "alloc")]
extern crate alloc;

mod envelope;
pub mod ghcb;

/// ECDSA-P384 signature verification for [`SnpAttestationReport`].
/// Host-side only; gated on the `verify` feature so the no_std
/// qrab-boot build never pulls in `p384` + `sha2`.
#[cfg(feature = "verify")]
pub mod verify;

/// `snp_cert_table` parser for the VCEK / ASK / ARK blob that the
/// hypervisor returns via `SVM_VMGEXIT_EXT_GUEST_REQUEST` (A02) or
/// that AMD KDS ships in `/vcek/…/ask-ark.bin` responses. Needs a
/// `Vec` allocator; gated on the `alloc` feature so the no_std
/// qrab-boot build can opt in while keeping `std` off.
#[cfg(feature = "alloc")]
pub mod cert_table;
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
/// fields up through the OS-area `msg_seqno_0..3` counters and
/// absorb the rest into `reserved_tail`. Offsets match the AMD
/// SEV-SNP ABI Spec §5.4.3 Table 7 and edk2's
/// `OvmfPkg/Include/Library/PlatformInitLib.h`.
///
/// ### OS area semantics (A14)
///
/// `msg_seqno_0..3` live in the 0x60-byte "OS area" starting at
/// offset 0xa0. These are **guest-maintained** counters: the PSP
/// itself tracks its own per-VMPCK sequence numbers internally and
/// never reads our copy. The guest bumps its `msg_seqno_N` by two
/// on every round-trip (one for the request envelope, one for the
/// response) and must pass `msg_seqno_N + 1` as the request seqno
/// so the PSP's expected-next-seqno matches.
///
/// On a fresh `SNP_LAUNCH_UPDATE` the OS area is zeroed, so any
/// first-boot guest can use `msg_seqno_N = 0` and pass `1` as the
/// request seqno. Persistent boot chains (kexec, qrab-boot →
/// kernel handoff) should update the field before transferring
/// control so the downstream stage resumes at the correct seqno.
///
/// `reserved_tail` absorbs everything past the OS area so the
/// struct still totals 4096 B.
#[repr(C, packed)]
#[derive(Clone, FromBytes, IntoBytes, Immutable, KnownLayout)]
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
    /// Per-VMPCK message sequence counters. See the doc-comment on
    /// [`SnpSecretsPage`]. Stored LE as native `u32`s.
    pub msg_seqno_0: u32,
    pub msg_seqno_1: u32,
    pub msg_seqno_2: u32,
    pub msg_seqno_3: u32,
    /// Reserved OS-area bytes (0xb0..0xc0) — kept opaque.
    pub os_reserved_1: [u8; 16],
    /// Guest-written AP jump-table GPA. We don't use this in
    /// qrab-boot (we're uniprocessor until early kernel) but name
    /// the field so the spec-shaped struct stays inspectable.
    pub ap_jump_table_pa: u64,
    pub reserved_tail: [u8; 4096 - 0xc8],
}

/// S7-2 / R12-12 / R12-13 / B14-8: zeroize every post-use-sensitive
/// field on drop so a `SnpSecretsPage` moved on the stack, cloned,
/// or stored on the heap does not leave residual material for a
/// core-dump / swap / heap-reuse attacker to recover. VMPCK0..3 are
/// the primary secrets. `msg_seqno_0..3` carry replay state — a
/// post-drop read could desync a downstream stage or reveal how many
/// guest-PSP round-trips happened. `gosv` and `ap_jump_table_pa`
/// leak guest-OS identity / memory-map layout respectively. Explicit
/// `write_volatile` through `addr_of_mut!` because the struct is
/// `#[repr(C, packed)]` — taking `&mut self.fieldN` in safe code is
/// UB on unaligned fields, and a plain assignment is likely to be
/// DCE'd by the optimiser in release builds.
impl Drop for SnpSecretsPage {
    fn drop(&mut self) {
        use core::ptr::{addr_of_mut, write_volatile};
        // SAFETY: `addr_of_mut!` yields a raw pointer to each field
        // without taking a reference, which is sound even on a
        // packed struct. Every write uses a `[u8; N]` of the field's
        // exact size — byte arrays have alignment 1, so
        // `write_volatile` is valid even when the underlying field is
        // unaligned in the packed layout. Fields are disjoint so no
        // overlap.
        unsafe {
            write_volatile(addr_of_mut!(self.vmpck0), [0u8; VMPCK_LEN]);
            write_volatile(addr_of_mut!(self.vmpck1), [0u8; VMPCK_LEN]);
            write_volatile(addr_of_mut!(self.vmpck2), [0u8; VMPCK_LEN]);
            write_volatile(addr_of_mut!(self.vmpck3), [0u8; VMPCK_LEN]);
            write_volatile(addr_of_mut!(self.msg_seqno_0).cast::<[u8; 4]>(), [0u8; 4]);
            write_volatile(addr_of_mut!(self.msg_seqno_1).cast::<[u8; 4]>(), [0u8; 4]);
            write_volatile(addr_of_mut!(self.msg_seqno_2).cast::<[u8; 4]>(), [0u8; 4]);
            write_volatile(addr_of_mut!(self.msg_seqno_3).cast::<[u8; 4]>(), [0u8; 4]);
            write_volatile(addr_of_mut!(self.gosv), [0u8; 16]);
            write_volatile(addr_of_mut!(self.ap_jump_table_pa).cast::<[u8; 8]>(), [0u8; 8]);
        }
    }
}

/// R12-13: `Debug` on `SnpSecretsPage` must NOT render the VMPCK
/// bytes. A future `log::info!("{:?}", page)` or `dbg!(page)` would
/// otherwise dump four 32-byte secrets into stderr / a log file.
/// Custom impl names each sensitive field but prints `REDACTED` in
/// place of its contents; non-secret fields render normally.
impl core::fmt::Debug for SnpSecretsPage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let version = { self.version };
        let imien = { self.imien };
        let fms = { self.fms };
        let msg_seqno_0 = { self.msg_seqno_0 };
        let msg_seqno_1 = { self.msg_seqno_1 };
        let msg_seqno_2 = { self.msg_seqno_2 };
        let msg_seqno_3 = { self.msg_seqno_3 };
        let ap_jump_table_pa = { self.ap_jump_table_pa };
        f.debug_struct("SnpSecretsPage")
            .field("version", &version)
            .field("imien", &imien)
            .field("fms", &fms)
            .field("vmpck0", &"REDACTED [u8; 32]")
            .field("vmpck1", &"REDACTED [u8; 32]")
            .field("vmpck2", &"REDACTED [u8; 32]")
            .field("vmpck3", &"REDACTED [u8; 32]")
            .field("msg_seqno_0", &msg_seqno_0)
            .field("msg_seqno_1", &msg_seqno_1)
            .field("msg_seqno_2", &msg_seqno_2)
            .field("msg_seqno_3", &msg_seqno_3)
            .field("ap_jump_table_pa", &ap_jump_table_pa)
            .finish_non_exhaustive()
    }
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

/// VMPCK slot + the `msg_seqno_N` counter the guest should resume
/// from, returned by [`vmpck_slots_descending`]. Callers iterate this
/// in order and try each slot until the PSP accepts one — the right
/// slot depends on the guest's current VMPL, which a no_std UEFI
/// guest cannot query directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmpckSlot {
    /// Slot index (0..=3). Matches the header `msg_vmpck` byte and
    /// the `MsgReportReq.vmpl` field.
    pub index: u8,
    /// Current `msg_seqno_N` from the Secrets page (pre-bump). The
    /// caller passes `seqno + 1` as the request seqno and expects
    /// `seqno + 2` as the response seqno — the spec's two-per-round
    /// discipline.
    pub seqno: u32,
}

/// Return the VMPCK slots that are populated on the Secrets page,
/// ordered highest-index-first (3, 2, 1, 0) and skipping zero slots.
///
/// ### Why highest-first?
///
/// coconut-svsm runs at VMPL0 and zeroes VMPCK0 before launching the
/// OS; the OS's VMPCK lives at a higher (less-privileged) slot. A
/// plain kvm-hypervisor launch without SVSM populates only VMPCK0 +
/// its own `msg_seqno_0`. Walking highest-first covers both cases:
///
///   * SVSM/VMPL3 OS → slot 3 first, matches the guest's VMPL.
///   * SVSM/VMPL2 OS → slot 3 rejects with `INVALID_KEY` (caller
///     retries), slot 2 accepted.
///   * bare SNP at VMPL0 → slots 3..1 all zero (skipped), slot 0
///     accepted.
///
/// The function is no_std/no_alloc and returns a fixed-size array
/// padded with `None` so UEFI callers can iterate without an
/// allocator. Callers can drive the retry loop by feeding each
/// `Some(VmpckSlot)` in turn into
/// `request_report_with_certs` until the PSP accepts one.
pub fn vmpck_slots_descending(page: &SnpSecretsPage) -> [Option<VmpckSlot>; 4] {
    // Read through raw accessors so the `#[repr(C, packed)]` layout
    // is safe under Rust 2024 without taking unaligned references.
    let entries: [(u8, &[u8; VMPCK_LEN], u32); 4] = [
        (3, &page.vmpck3, { page.msg_seqno_3 }),
        (2, &page.vmpck2, { page.msg_seqno_2 }),
        (1, &page.vmpck1, { page.msg_seqno_1 }),
        (0, &page.vmpck0, { page.msg_seqno_0 }),
    ];
    let mut out: [Option<VmpckSlot>; 4] = [None; 4];
    let mut w = 0;
    for (index, key, seqno) in entries {
        if is_vmpck_nonzero(key) {
            out[w] = Some(VmpckSlot { index, seqno });
            w += 1;
        }
    }
    out
}

/// Borrow the VMPCK bytes for `slot` on `page`. Returned as a
/// reference so callers never copy the key onto a fresh stack frame
/// (defense in depth against residual-key / swap leak).
pub fn vmpck_bytes(page: &SnpSecretsPage, slot: u8) -> Option<&[u8; VMPCK_LEN]> {
    match slot {
        0 => Some(&page.vmpck0),
        1 => Some(&page.vmpck1),
        2 => Some(&page.vmpck2),
        3 => Some(&page.vmpck3),
        _ => None,
    }
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
        // OS area — per AMD SEV-SNP ABI 1.55 §5.4.3 Table 7.
        assert_eq!(offset_of!(SnpSecretsPage, msg_seqno_0), 0x0a0);
        assert_eq!(offset_of!(SnpSecretsPage, msg_seqno_1), 0x0a4);
        assert_eq!(offset_of!(SnpSecretsPage, msg_seqno_2), 0x0a8);
        assert_eq!(offset_of!(SnpSecretsPage, msg_seqno_3), 0x0ac);
        assert_eq!(offset_of!(SnpSecretsPage, ap_jump_table_pa), 0x0c0);
        assert_eq!(offset_of!(SnpSecretsPage, reserved_tail), 0x0c8);
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

    // ------- vmpck_slots_descending / vmpck_bytes (SVSM slot-select fix) -------

    #[test]
    fn vmpck_slots_descending_empty_when_all_zero() {
        // Baseline sanity: a page with no populated VMPCKs returns
        // an array of `None`s. Caller should fail closed — there is
        // no legitimate guest-originated SNP_GUEST_REQUEST from a
        // page with no keys.
        let page = zero_secrets_page();
        let slots = vmpck_slots_descending(&page);
        assert!(slots.iter().all(Option::is_none));
    }

    #[test]
    fn vmpck_slots_descending_bare_snp_uses_slot_0() {
        // Non-SVSM SEV-SNP guest: PSP populates VMPCK0 + `msg_seqno_0`,
        // other slots stay zero. Walk must return only slot 0.
        let mut page = zero_secrets_page();
        page.version = 1;
        page.vmpck0[0] = 0xAA;
        page.msg_seqno_0 = 7;
        let slots = vmpck_slots_descending(&page);
        assert_eq!(
            slots[0],
            Some(VmpckSlot { index: 0, seqno: 7 }),
        );
        assert!(slots[1..].iter().all(Option::is_none));
    }

    #[test]
    fn vmpck_slots_descending_svsm_vmpl3_os_prefers_slot_3() {
        // coconut-svsm at VMPL0 populates VMPCKs for the VMPLs it
        // exposes to the OS. A VMPL3 OS sees VMPCK3 populated (and
        // possibly VMPCK2). Walk must return slot 3 FIRST so the
        // first VMGEXIT uses the key that matches the guest's VMPL.
        let mut page = zero_secrets_page();
        page.version = 2;
        page.vmpck2[0] = 0x11;
        page.msg_seqno_2 = 42;
        page.vmpck3[0] = 0x22;
        page.msg_seqno_3 = 99;
        let slots = vmpck_slots_descending(&page);
        assert_eq!(
            slots[0],
            Some(VmpckSlot { index: 3, seqno: 99 }),
            "VMPL3 slot must come first for SVSM-launched OS",
        );
        assert_eq!(
            slots[1],
            Some(VmpckSlot { index: 2, seqno: 42 }),
            "VMPL2 slot must fall through second as retry target",
        );
        assert!(slots[2..].iter().all(Option::is_none));
    }

    #[test]
    fn vmpck_slots_descending_skips_zero_slots_even_at_top() {
        // Sparse population (only VMPCK2 non-zero). Walk must still
        // surface slot 2, skipping the zero slot 3 above it.
        let mut page = zero_secrets_page();
        page.version = 2;
        page.vmpck2[7] = 0x33;
        page.msg_seqno_2 = 5;
        let slots = vmpck_slots_descending(&page);
        assert_eq!(
            slots[0],
            Some(VmpckSlot { index: 2, seqno: 5 }),
        );
        assert!(slots[1..].iter().all(Option::is_none));
    }

    #[test]
    fn vmpck_slots_descending_order_is_strictly_3_2_1_0() {
        // Populate all four slots with distinct seqnos so the test
        // can spot a slot swap. Ordering must be 3,2,1,0 top-to-bottom.
        let mut page = zero_secrets_page();
        page.version = 4;
        page.vmpck0[0] = 1;
        page.vmpck1[0] = 2;
        page.vmpck2[0] = 3;
        page.vmpck3[0] = 4;
        page.msg_seqno_0 = 10;
        page.msg_seqno_1 = 20;
        page.msg_seqno_2 = 30;
        page.msg_seqno_3 = 40;
        let slots = vmpck_slots_descending(&page);
        let indices: std::vec::Vec<u8> = slots
            .iter()
            .flatten()
            .map(|s| s.index)
            .collect();
        assert_eq!(indices, std::vec![3u8, 2, 1, 0]);
        let seqnos: std::vec::Vec<u32> = slots
            .iter()
            .flatten()
            .map(|s| s.seqno)
            .collect();
        assert_eq!(
            seqnos,
            std::vec![40u32, 30, 20, 10],
            "each slot carries its own msg_seqno_N",
        );
    }

    #[test]
    fn vmpck_bytes_returns_correct_slot() {
        // Pin that vmpck_bytes maps index->slot correctly, with a
        // distinct marker in each slot so a swap shows up.
        let mut page = zero_secrets_page();
        page.version = 4;
        page.vmpck0 = [0xA0u8; VMPCK_LEN];
        page.vmpck1 = [0xB1u8; VMPCK_LEN];
        page.vmpck2 = [0xC2u8; VMPCK_LEN];
        page.vmpck3 = [0xD3u8; VMPCK_LEN];
        assert_eq!(vmpck_bytes(&page, 0).map(|v| v[0]), Some(0xA0));
        assert_eq!(vmpck_bytes(&page, 1).map(|v| v[0]), Some(0xB1));
        assert_eq!(vmpck_bytes(&page, 2).map(|v| v[0]), Some(0xC2));
        assert_eq!(vmpck_bytes(&page, 3).map(|v| v[0]), Some(0xD3));
        assert!(vmpck_bytes(&page, 4).is_none());
        assert!(vmpck_bytes(&page, 255).is_none());
    }

    /// S7-2 close-out: `SnpSecretsPage` now has an explicit `Drop`
    /// impl that zeroizes the four VMPCKs. `needs_drop` must flip
    /// to `true`. If a future refactor accidentally drops the
    /// `impl Drop for SnpSecretsPage` or loses the
    /// `#[derive]` conflict by re-adding `Copy`, this test fails.
    #[test]
    fn secrets_page_has_drop_glue() {
        assert!(
            core::mem::needs_drop::<SnpSecretsPage>(),
            "SnpSecretsPage lost its Drop impl — S7-2 VMPCK \
             zeroization regressed. Re-add `impl Drop for \
             SnpSecretsPage` in qrab-snp-proto/src/lib.rs.",
        );
    }

    /// Positive zeroization check: build a page with non-zero values
    /// in every sensitive field inside a `core::mem::ManuallyDrop` so
    /// we can peek at the same backing storage after manually
    /// invoking `drop_in_place`. Proves the volatile writes actually
    /// land — not just that `needs_drop` is true. Covers VMPCK0..3
    /// (S7-2), `msg_seqno_0..3` / `gosv` / `ap_jump_table_pa` (B14-8
    /// — replay-state / layout-info leak).
    #[test]
    fn drop_zeroizes_secrets_and_seqno_slots() {
        use core::mem::ManuallyDrop;
        use core::ptr;

        let mut page = zero_secrets_page();
        page.vmpck0 = [0xA1u8; VMPCK_LEN];
        page.vmpck1 = [0xB2u8; VMPCK_LEN];
        page.vmpck2 = [0xC3u8; VMPCK_LEN];
        page.vmpck3 = [0xD4u8; VMPCK_LEN];
        page.msg_seqno_0 = 0x1111_1111;
        page.msg_seqno_1 = 0x2222_2222;
        page.msg_seqno_2 = 0x3333_3333;
        page.msg_seqno_3 = 0x4444_4444;
        page.gosv = [0x5au8; 16];
        page.ap_jump_table_pa = 0xDEAD_BEEF_CAFE_F00D;

        // Move into a `ManuallyDrop` so we can call `drop_in_place`
        // explicitly and then inspect the backing bytes.
        let mut wrapped = ManuallyDrop::new(page);
        let raw: *mut SnpSecretsPage = &mut *wrapped as *mut SnpSecretsPage;
        unsafe { ptr::drop_in_place(raw) };

        // SAFETY: `drop_in_place` above has run the destructor. The
        // struct is now logically dropped; we are only inspecting
        // bytes to confirm zeroization, not using the fields through
        // any trait that requires a valid value.
        let bytes_view: &[u8] = unsafe {
            core::slice::from_raw_parts(
                raw as *const u8,
                core::mem::size_of::<SnpSecretsPage>(),
            )
        };
        // VMPCK slots: offsets 0x20, 0x40, 0x60, 0x80 (each 32 B).
        for base in [0x20, 0x40, 0x60, 0x80] {
            assert!(
                bytes_view[base..base + VMPCK_LEN].iter().all(|&b| b == 0),
                "VMPCK slot at offset {:#x} not zeroized: {:x?}",
                base,
                &bytes_view[base..base + VMPCK_LEN],
            );
        }
        // gosv: offset 0x10, 16 B.
        assert!(
            bytes_view[0x10..0x20].iter().all(|&b| b == 0),
            "gosv not zeroized: {:x?}",
            &bytes_view[0x10..0x20],
        );
        // msg_seqno_0..3: offsets 0xa0..0xb0 (four u32s).
        assert!(
            bytes_view[0xa0..0xb0].iter().all(|&b| b == 0),
            "msg_seqno_0..3 not zeroized: {:x?}",
            &bytes_view[0xa0..0xb0],
        );
        // ap_jump_table_pa: offset 0xc0, 8 B.
        assert!(
            bytes_view[0xc0..0xc8].iter().all(|&b| b == 0),
            "ap_jump_table_pa not zeroized: {:x?}",
            &bytes_view[0xc0..0xc8],
        );
    }

    /// R12-12 close-out: `SnpSecretsPage` must NOT derive `Copy`.
    /// Having `Copy` would mean every move / assignment silently
    /// duplicates VMPCK0..3 on the stack, compounding the residual
    /// key leak that S7-2's Drop impl is meant to clean up. This
    /// test would compile-fail (cannot coerce to `fn(T) -> T` via
    /// `Copy` requirement) if `Copy` were reintroduced.
    #[test]
    fn secrets_page_is_not_copy() {
        // `Clone` is fine and required by zerocopy consumers.
        fn assert_clone<T: Clone>() {}
        assert_clone::<SnpSecretsPage>();

        // `Copy` proof-by-compilation: the below line would pass
        // only if SnpSecretsPage: Copy. We assert the NEGATIVE
        // indirectly by checking that `needs_drop` is true — `Copy`
        // requires `!needs_drop`.
        assert!(core::mem::needs_drop::<SnpSecretsPage>());
    }

    /// R12-13 close-out: `Debug` for `SnpSecretsPage` must not
    /// render the VMPCK bytes. Render one page and grep for the
    /// `REDACTED` marker emitted by the custom impl. If someone
    /// reverts to a derived `Debug`, each VMPCK would serialise as
    /// `[0, 1, 2, ...]` and the marker disappears.
    #[test]
    fn debug_redacts_vmpck_bytes() {
        let mut page = zero_secrets_page();
        page.vmpck0 = [0x11u8; VMPCK_LEN];
        page.vmpck1 = [0x22u8; VMPCK_LEN];
        page.vmpck2 = [0x33u8; VMPCK_LEN];
        page.vmpck3 = [0x44u8; VMPCK_LEN];

        let rendered = std::format!("{:?}", page);
        assert!(
            rendered.contains("REDACTED"),
            "Debug output missing REDACTED marker: {rendered}"
        );
        assert!(
            !rendered.contains("0x11"),
            "Debug leaks vmpck0 byte 0x11: {rendered}"
        );
        assert!(
            !rendered.contains("0x22"),
            "Debug leaks vmpck1 byte 0x22: {rendered}"
        );
    }

    /// The `vmpck` parameter accepted by
    /// [`encrypt_guest_msg`] / [`decrypt_guest_msg`] is still a
    /// `&[u8; 32]` reference. We chose not to migrate the envelope
    /// API to a zeroizing newtype because the caller owns the
    /// `SnpSecretsPage` (whose Drop impl now zeroizes VMPCKs); the
    /// envelope only borrows the key for a brief AES-GCM op and
    /// does not retain it. Pin the signature so a future change
    /// that would require additional zeroization shows up here.
    #[test]
    fn envelope_api_accepts_vmpck_reference() {
        let _: fn(
            &mut SnpGuestMsg,
            &[u8; VMPCK_LEN],
            u64,
            SnpMsgType,
            u8,
            &[u8],
        ) -> Result<(), EnvelopeError> = encrypt_guest_msg;
        let _: fn(
            &SnpGuestMsg,
            &[u8; VMPCK_LEN],
            u64,
            SnpMsgType,
            u8,
            &mut [u8],
        ) -> Result<usize, EnvelopeError> = decrypt_guest_msg;
    }

    // ------- R13 round 16: MsgReportReq/Resp functional tests -------

    #[test]
    fn msg_report_req_preserves_report_data_byte_for_byte() {
        // Every byte of report_data must survive intact through
        // `MsgReportReq::new`. Pattern-matched fill ensures we
        // catch any accidental zero-fill or truncation.
        let mut rd = [0u8; 64];
        for i in 0..64 {
            rd[i] = (i as u8).wrapping_mul(7) ^ 0x5A;
        }
        let req = MsgReportReq::new(rd, 0);
        assert_eq!(req.report_data, rd);
        assert!(req.reserved.iter().all(|&b| b == 0));
    }

    #[test]
    fn msg_report_req_vmpl_fan_out() {
        // All 4 VMPL values (0-3) must round-trip through
        // `MsgReportReq::new` unchanged. VMPL3 is the guest's
        // least-privilege level; VMPL0 is reserved for SVSM.
        for vmpl in 0u32..=3 {
            let req = MsgReportReq::new([0; 64], vmpl);
            assert_eq!({ req.vmpl }, vmpl, "vmpl {vmpl}");
        }
    }

    #[test]
    fn msg_report_req_envelope_round_trip_each_vmpl() {
        // Functional: encrypt → decrypt a `MsgReportReq` for each
        // VMPL, proving the whole plaintext survives envelope
        // encryption with the value intact.
        use zerocopy::IntoBytes;
        const KEY: [u8; VMPCK_LEN] = [0x7F; VMPCK_LEN];
        for vmpl in 0u32..=3 {
            let req = MsgReportReq::new([vmpl as u8; 64], vmpl);
            let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
            let mut env =
                SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
            encrypt_guest_msg(
                &mut env, &KEY, vmpl as u64, SnpMsgType::ReportReq, 0, plaintext,
            ).unwrap();
            let mut decoded = [0u8; MSG_REPORT_REQ_SIZE];
            let n = decrypt_guest_msg(
                &env, &KEY, vmpl as u64, SnpMsgType::ReportReq, 0, &mut decoded,
            ).unwrap();
            assert_eq!(n, MSG_REPORT_REQ_SIZE);
            assert_eq!(&decoded[..], plaintext, "vmpl {vmpl} round-trip differed");
            let back = MsgReportReq::read_from_bytes(&decoded).unwrap();
            assert_eq!(back.report_data, [vmpl as u8; 64]);
            assert_eq!({ back.vmpl }, vmpl);
        }
    }

    #[test]
    fn msg_report_resp_round_trip_preserves_embedded_report() {
        // Functional: build a `MsgReportResp` with a distinctive
        // marker in `status`, encrypt, decrypt, and confirm the
        // embedded SnpAttestationReport survives byte-for-byte.
        use zerocopy::IntoBytes;
        const KEY: [u8; VMPCK_LEN] = [0x3C; VMPCK_LEN];
        let mut resp = MsgReportResp::read_from_bytes(
            &[0u8; MSG_REPORT_RESP_SIZE],
        ).unwrap();
        resp.status = 0x0000_0042;
        resp.report_size = SNP_ATTESTATION_REPORT_SIZE as u32;
        // Mark the embedded report's measurement so decrypt round
        // round-trip can assert on it.
        resp.report.measurement = [0xABu8; 48];

        let plaintext = <MsgReportResp as IntoBytes>::as_bytes(&resp);
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(
            &mut env, &KEY, 99, SnpMsgType::ReportResp, 0, plaintext,
        ).unwrap();
        let mut decoded = [0u8; MSG_REPORT_RESP_SIZE];
        let n = decrypt_guest_msg(
            &env, &KEY, 99, SnpMsgType::ReportResp, 0, &mut decoded,
        ).unwrap();
        assert_eq!(n, MSG_REPORT_RESP_SIZE);
        let back = MsgReportResp::read_from_bytes(&decoded).unwrap();
        assert_eq!({ back.status }, 0x0000_0042);
        assert_eq!({ back.report_size }, SNP_ATTESTATION_REPORT_SIZE as u32);
        assert_eq!({ back.report.measurement }, [0xABu8; 48]);
    }

    #[test]
    fn msg_report_req_fan_out_distinct_report_data_distinct_ciphertext() {
        // Feed 4 distinct report_data values through envelope with
        // the same key + seqno + slot. Ciphertexts must all differ
        // (AES-GCM covers the plaintext, so changing one byte in
        // the input changes the entire output).
        use zerocopy::IntoBytes;
        const KEY: [u8; VMPCK_LEN] = [0x88; VMPCK_LEN];
        let mut ciphertexts = std::collections::BTreeSet::new();
        for marker in 0u8..4 {
            let req = MsgReportReq::new([marker; 64], 0);
            let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
            // Different seqno per request so IVs differ too — this
            // is the monotonic-counter discipline the caller is
            // required to follow.
            let mut env =
                SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
            encrypt_guest_msg(
                &mut env, &KEY, 100 + marker as u64, SnpMsgType::ReportReq, 0, plaintext,
            ).unwrap();
            let ct: std::vec::Vec<u8> = env.payload[..plaintext.len()].to_vec();
            assert!(ciphertexts.insert(ct), "collision at marker {marker}");
        }
    }

    #[test]
    fn msg_report_req_size_and_layout_invariant() {
        // Pinning the on-wire layout so a future field reorder
        // breaks the round-trip test rather than silently producing
        // a cert the PSP will reject.
        assert_eq!(core::mem::size_of::<MsgReportReq>(), 96);
        assert_eq!(core::mem::offset_of!(MsgReportReq, report_data), 0);
        assert_eq!(core::mem::offset_of!(MsgReportReq, vmpl), 64);
        assert_eq!(core::mem::offset_of!(MsgReportReq, reserved), 68);
    }

    #[test]
    fn msg_report_resp_size_and_layout_invariant() {
        assert_eq!(core::mem::size_of::<MsgReportResp>(), MSG_REPORT_RESP_SIZE);
        assert_eq!(core::mem::offset_of!(MsgReportResp, status), 0);
        assert_eq!(core::mem::offset_of!(MsgReportResp, report_size), 4);
        assert_eq!(core::mem::offset_of!(MsgReportResp, reserved), 8);
        assert_eq!(core::mem::offset_of!(MsgReportResp, report), 32);
    }
}
