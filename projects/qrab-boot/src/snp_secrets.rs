//! SEV-SNP Secrets-page loader.
//!
//! The PSP places a 4 KiB Secrets page (AMD SEV-SNP ABI §5.4.3) at
//! a fixed PA during launch, carrying the four Virtual Machine
//! Platform Communication Keys (VMPCK0..3) the guest needs to
//! encrypt `SNP_GUEST_REQUEST` traffic. OVMF reserves the page in
//! its `MEMFD` region; `OvmfPkg/Include/Fdf/MemFd.fdf.inc` sets the
//! offset to `0x0D000` and `OvmfPkg/Include/Fdf/OvmfPkgDefines.fdf.inc`
//! pins `MEMFD_BASE_ADDRESS = 0x0080_0000`, so the absolute PA is
//! `0x0080_0000 + 0x0D000 = 0x0080_D000` for every AmdSev OVMF
//! build this tree produces.
//!
//! ## Workaround scope
//!
//! Hard-coding the PA is a **workaround** while we wait for upstream
//! OVMF to publish the Secrets-page location in an EFI configuration
//! table (analogous to `gConfidentialComputingSecretGuid` but for
//! the SNP VMPCK area rather than the SEV launch-secret blob).
//! Tracking ticket:
//! `llm-rag/reference/snp-secrets-upstream-plan.md`.
//!
//! Consequences of the workaround:
//!
//!   * Any rebase of `vendor/edk2` that shifts `MEMFD_BASE_ADDRESS`
//!     or the `PcdOvmfSnpSecretsBase` offset silently breaks
//!     [`load`]. Runtime validation (version != 0, VMPCK0 non-zero)
//!     catches the common case; pinning both constants via
//!     compile-time `debug_assert!`s inside `contrib/build-ovmf.sh`
//!     would be more robust.
//!   * Swapping to `OvmfPkgX64.fdf` or TDX firmware breaks the
//!     constant. The loader refuses to proceed on non-AmdSev
//!     builds; callers gate on `ghcb::SevFeatures::sev_snp` first.
//!
//! ## Security
//!
//! VMPCK0 is a 32-byte AES-256 key. This module:
//!
//!   * Copies the page into a [`SnpSecretsPage`] value so the
//!     guest can zeroise its own copy after `ghcb::request_report`
//!     finishes. The pages themselves stay in MEMFD; the PSP
//!     zeroises their contents once the VMPCKs are no longer
//!     needed.
//!   * Never logs VMPCK bytes, only non-zero-ness + the `version`
//!     integer so the smoke harness can assert the Secrets page is
//!     present without leaking key material.

use alloc::vec::Vec;
use core::ptr;

pub use qrab_snp_proto::SecretsValidateError;
use qrab_snp_proto::{
    EnvelopeError, MSG_REPORT_RESP_SIZE, MsgReportReq, MsgReportResp, SNP_GUEST_MSG_SIZE,
    SnpAttestationReport, SnpGuestMsg, SnpMsgType, SnpSecretsPage, VMPCK_LEN,
    decrypt_guest_msg, encrypt_guest_msg, ghcb::GhcbError, report_status,
    validate_secrets_page,
};
use zerocopy::{FromBytes, IntoBytes};

use uefi_raw::protocol::pci::root_bridge::PciRootBridgeIoProtocolOperation;

use crate::{ghcb_vmgexit, pci_dma};

/// 4 KiB page size used throughout `pci_dma` / OVMF `IoMmu` ops.
const PAGE_SIZE: usize = 4096;

/// Starting cert-chain data-page allocation for the extended
/// guest-request path. Real AMD chains on a Genoa / Bergamo host
/// are ~3 KiB (ARK 1036 B + ASK 916 B + VCEK 1355 B in DER), so one
/// page suffices in practice. Starting at 4 leaves headroom and
/// matches the Linux kernel's `SNP_EXT_GUEST_REQ_INIT_PAGES`
/// starting value. A `SNP_GUEST_VMM_ERR_INVALID_LEN` retry still
/// catches up to hypervisors that stash additional PSP metadata.
const CERT_PAGES_INITIAL: u64 = 4;

/// Upper bound on cert-chain data pages we will ever allocate.
/// Stops a hostile hypervisor from asking us to allocate arbitrarily
/// large runs of RMP-shared memory. 16 pages = 64 KiB — large enough
/// for an ARK + ASK + VCEK + VLEK chain with AMD's spec-max cert
/// sizes plus a cert-table header, and still trivial to zero.
const CERT_PAGES_MAX: u64 = 16;

/// Hard-coded physical address of the SNP Secrets page for the
/// `OvmfPkg/AmdSev/AmdSevX64.fdf` build this tree produces.
/// Computed from `MEMFD_BASE_ADDRESS (0x0080_0000) +
/// PcdOvmfSnpSecretsBase (0x0D000)`.
pub const SNP_SECRETS_PA: u64 = 0x0080_0000 + 0x0D000;

/// Read the SNP Secrets page from its hard-coded PA and return an
/// owned copy. Caller is responsible for zeroising the returned
/// value once VMPCK0 is no longer needed.
///
/// ## Safety
///
/// Guaranteed-safe preconditions the caller must maintain:
///
///   * The guest is running under SEV-SNP (`ghcb::detect()` reports
///     `sev_snp = true`).
///   * The firmware image embedded in this boot is a qrab-boot
///     AmdSev OVMF build whose MEMFD layout matches the constants
///     documented above.
///
/// Under UEFI boot services the 4 KiB page at [`SNP_SECRETS_PA`] is
/// identity-mapped and readable; a bad PA in a non-SNP or non-AmdSev
/// boot manifests as either a `SecretsValidateError::VersionZero` /
/// `SecretsValidateError::AllVmpcksZero` (if the page happens to be
/// plain RAM) or a `#PF` (if the page is not mapped). Both failure
/// modes are strictly less dangerous than silently returning wrong
/// key material, which is why the validation checks are part of the
/// public contract.
pub fn load() -> Result<SnpSecretsPage, SecretsValidateError> {
    // SAFETY: SNP_SECRETS_PA is the PA of a 4 KiB page that the PSP
    // populated during guest launch, identity-mapped by the
    // firmware and accessible during boot services. `read_volatile`
    // is required because the page is populated by out-of-band
    // hardware and we do not want LLVM to cache the initial read.
    let page: SnpSecretsPage = unsafe { ptr::read_volatile(SNP_SECRETS_PA as *const SnpSecretsPage) };
    validate_secrets_page(&page)?;
    Ok(page)
}

/// Distinguished errors from [`request_report`].
#[derive(Debug)]
pub enum ReportReqError {
    /// `pci_dma::allocate_buffer` failed — the PSP needs a pair of
    /// 4 KiB shared pages and we could not get them (either the
    /// bridge pointer was not set up or firmware refused the
    /// allocation).
    AllocFailed,
    /// AES-256-GCM envelope build / verify failed. See
    /// [`qrab_snp_proto::EnvelopeError`] for the variant.
    Envelope(EnvelopeError),
    /// GHCB VMGEXIT returned an error (result bit clear or PSP
    /// status non-zero). See [`qrab_snp_proto::ghcb::GhcbError`].
    Ghcb(GhcbError),
    /// The PSP wrote a non-zero `MsgReportResp::status` (inside the
    /// decrypted response). `0x16` = `INVALID_PARAMETER`,
    /// `0x27` = `INVALID_KEY`, etc.
    PspStatus(u32),
    /// Decrypted response payload could not be reinterpreted as a
    /// [`MsgReportResp`] — should not happen in practice because
    /// `MsgReportResp` is a `FromBytes` POD with fixed 1280 B size.
    Decode,
    /// Hypervisor asked for more cert-chain data pages than
    /// [`CERT_PAGES_MAX`]. Treated as hostile — we refuse to allocate
    /// arbitrary runs of RMP-shared memory.
    CertTooLarge {
        /// Page count the hypervisor requested via RBX.
        required_pages: u64,
    },
    /// `cert_table::parse_cert_table` rejected the blob the
    /// hypervisor returned.
    CertParse(qrab_snp_proto::cert_table::CertTableError),
}

/// Full round-trip: encrypt a `MSG_REPORT_REQ` with `report_data`
/// under `vmpck0` with `msg_seqno`, issue `SVM_VMGEXIT_GUEST_REQUEST`,
/// decrypt the response under `msg_seqno + 1`, and return the signed
/// 1184-byte [`SnpAttestationReport`].
///
/// Caller must:
///
///   * Have initialised `pci_dma` via `vsock_driver::init` (so
///     `allocate_buffer` can route through `PciRootBridgeIo`).
///   * Have verified the guest is SEV-SNP (`ghcb::detect`).
///   * Have loaded the Secrets page ([`load`]) and passed the
///     `vmpck0` + initial `msg_seqno` from it. The caller is
///     responsible for tracking / incrementing `msg_seqno` across
///     subsequent invocations — every `request_report` consumes
///     two sequence numbers (one for the request envelope, one for
///     the response envelope), so the next call must use
///     `msg_seqno + 2`.
pub fn request_report(
    vmpck0: &[u8; VMPCK_LEN],
    msg_seqno: u64,
    report_data: [u8; 64],
) -> Result<SnpAttestationReport, ReportReqError> {
    // 1. Allocate two 4 KiB PSP-shared pages (IoMmuAllocateBuffer
    //    clears the C-bit on SNP; identity-map on TCG).
    let req_host = pci_dma::allocate_buffer(1).map_err(|_| ReportReqError::AllocFailed)?;
    let resp_host = pci_dma::allocate_buffer(1).map_err(|err| {
        // Free the first allocation before we bubble up.
        let _ = pci_dma::free_buffer(req_host, 1);
        let _ = err;
        ReportReqError::AllocFailed
    })?;

    // 2. Encrypt the MSG_REPORT_REQ into `req_host` *before* Map'ing
    //    the pages. OVMF's CcIoMmu Map(CommonBuffer) allocates a
    //    fresh shared bounce buffer and snapshots `host -> bounce`
    //    at Map time; anything written after Map lives only in the
    //    private host page. So we populate `req_host` here, then
    //    Map — and the encryption + Rust-visible bytes end up in
    //    the bounce the PSP reads. See
    //    OvmfPkg/IoMmuDxe/CcIoMmu.c `IoMmuMap` CommonBuffer path.
    let req_msg: &mut SnpGuestMsg = unsafe { &mut *(req_host as *mut SnpGuestMsg) };
    let resp_msg_zeros = &[0u8; SNP_GUEST_MSG_SIZE];
    *req_msg = SnpGuestMsg::read_from_bytes(resp_msg_zeros)
        .expect("4 KiB decodes into SnpGuestMsg");
    let req = MsgReportReq::new(report_data, 0);
    let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
    if let Err(e) = encrypt_guest_msg(
        req_msg,
        vmpck0,
        msg_seqno,
        SnpMsgType::ReportReq,
        0,
        plaintext,
    ) {
        let _ = pci_dma::free_buffer(req_host, 1);
        let _ = pci_dma::free_buffer(resp_host, 1);
        return Err(ReportReqError::Envelope(e));
    }
    // Zero the response host buffer so its Map-time snapshot is
    // all-zero in the bounce — otherwise OVMF copies stale encrypted
    // bytes as pseudo-plaintext which can pass AEAD quickly-ish and
    // hide a missing PSP reply.
    let resp_msg: &mut SnpGuestMsg = unsafe { &mut *(resp_host as *mut SnpGuestMsg) };
    *resp_msg = SnpGuestMsg::read_from_bytes(resp_msg_zeros)
        .expect("4 KiB decodes into SnpGuestMsg");

    // 3. Map both pages with BUS_MASTER_COMMON_BUFFER64. On SEV-SNP
    //    OVMF's IoMmu clears the C-bit, PSC's a shared bounce
    //    buffer, and copies `req_host -> bounce` at Map time. The
    //    returned `device_addr` is the bounce GPA — that is what
    //    the PSP will read (req) / write (resp).
    let op = PciRootBridgeIoProtocolOperation::BUS_MASTER_COMMON_BUFFER64;
    let req_map = match pci_dma::map(op, req_host, SNP_GUEST_MSG_SIZE) {
        Ok(m) => m,
        Err(_) => {
            let _ = pci_dma::free_buffer(req_host, 1);
            let _ = pci_dma::free_buffer(resp_host, 1);
            return Err(ReportReqError::AllocFailed);
        }
    };
    let resp_map = match pci_dma::map(op, resp_host, SNP_GUEST_MSG_SIZE) {
        Ok(m) => m,
        Err(_) => {
            let _ = pci_dma::unmap(req_map.cookie);
            let _ = pci_dma::free_buffer(req_host, 1);
            let _ = pci_dma::free_buffer(resp_host, 1);
            return Err(ReportReqError::AllocFailed);
        }
    };

    // 4. Issue VMGEXIT against the bounce GPAs.
    log::info!(
        "QRAB_SNP_RR_PRE req_host={:p} req_bus={:#x} resp_host={:p} resp_bus={:#x}",
        req_host, req_map.device_addr, resp_host, resp_map.device_addr,
    );
    let vmgexit_result = ghcb_vmgexit::guest_request(req_map.device_addr, resp_map.device_addr)
        .map_err(ReportReqError::Ghcb);

    // 5. Unmap the response mapping first. For CommonBuffer ops
    //    OVMF's CcIoMmu Unmap copies `bounce -> resp_host`, which
    //    is exactly the direction we need to see the PSP's reply
    //    through the CPU's SEV AES on the following reads.
    let _ = pci_dma::unmap(resp_map.cookie);
    let _ = pci_dma::unmap(req_map.cookie);

    // 6. Always dump first 8 bytes of the response host page so we
    //    can distinguish "PSP wrote + OVMF copied back" from "PSP
    //    never touched bounce" when diagnosing failures.
    {
        // SAFETY: resp_host points at a 4 KiB page we own.
        let first: [u8; 8] = unsafe {
            core::ptr::read_volatile(resp_host as *const [u8; 8])
        };
        log::info!(
            "QRAB_SNP_RR_RESP first8={:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            first[0], first[1], first[2], first[3],
            first[4], first[5], first[6], first[7],
        );
    }

    let result = match vmgexit_result {
        Ok(()) => decode_response(resp_host, vmpck0, msg_seqno),
        Err(e) => Err(e),
    };

    // 7. Release pages.
    let _ = pci_dma::free_buffer(req_host, 1);
    let _ = pci_dma::free_buffer(resp_host, 1);
    result
}

/// Decrypt the response-envelope page after Unmap has copied the
/// PSP bounce bytes back into it, verify the AEAD tag, and pull out
/// the 1184-byte attestation report on success.
fn decode_response(
    resp_host: *mut u8,
    vmpck0: &[u8; VMPCK_LEN],
    msg_seqno: u64,
) -> Result<SnpAttestationReport, ReportReqError> {
    // SAFETY: resp_host was allocated as a 4 KiB page owned by us
    // and just Unmap'd back to RMP-PRIVATE; no other code holds
    // aliases. CPU reads decrypt via SEV.
    let resp_msg: &mut SnpGuestMsg = unsafe { &mut *(resp_host as *mut SnpGuestMsg) };

    // `plain_buf` is sized to fit the full `MsgReportResp` (status +
    // report_size + reserved + the 1184-byte report + padding). The
    // PSP often emits `msg_size = 1216` (no trailing padding) — our
    // pre-VMGEXIT zeroing of `resp_host` is what keeps the padding
    // bytes zero and lets `MsgReportResp::read_from_bytes` succeed
    // on the full 1280-byte slice.
    let mut plain_buf = [0u8; MSG_REPORT_RESP_SIZE];
    let n = decrypt_guest_msg(
        resp_msg,
        vmpck0,
        msg_seqno.wrapping_add(1),
        SnpMsgType::ReportResp,
        0,
        &mut plain_buf,
    )
    .map_err(ReportReqError::Envelope)?;

    // Accept any PSP payload that is at least large enough to cover
    // `status + report_size + reserved + report` (1216 B). The
    // trailing 64 B of padding is optional in some firmware
    // revisions; the rest of the resp buffer stays zero from the
    // pre-VMGEXIT scrub, so parsing the fixed-size struct is safe.
    const MSG_REPORT_RESP_MIN: usize = 4 + 4 + 24 + 1184;
    if n < MSG_REPORT_RESP_MIN {
        log::warn!(
            "QRAB_SNP_RR_SHORT_PAYLOAD n={n} min={MSG_REPORT_RESP_MIN}"
        );
        return Err(ReportReqError::Decode);
    }

    let resp = MsgReportResp::read_from_bytes(&plain_buf)
        .map_err(|_| ReportReqError::Decode)?;
    let status = { resp.status };
    if status != report_status::SUCCESS {
        return Err(ReportReqError::PspStatus(status));
    }
    Ok(resp.report)
}

/// Extended-variant of [`request_report`] (A02): issues
/// `SVM_VMGEXIT_EXT_GUEST_REQUEST` so the hypervisor also fills a
/// caller-supplied cert-chain data buffer. Returns the signed
/// attestation report **plus** the raw cert-table blob the
/// hypervisor wrote back. The caller is expected to run the blob
/// through [`qrab_snp_proto::cert_table::parse_cert_table`] to split
/// it into ARK / ASK / VCEK (or VLEK) DER certs.
///
/// Retry discipline: on the first `SNP_GUEST_VMM_ERR_INVALID_LEN`
/// from the hypervisor we realloc the data pages to the page count
/// the hypervisor wrote back into RBX, then re-issue the VMGEXIT
/// *once*. A second INVALID_LEN (or any other error) bubbles up.
/// Anything larger than [`CERT_PAGES_MAX`] is treated as hostile
/// and surfaced as [`ReportReqError::CertTooLarge`].
pub fn request_report_with_certs(
    vmpck0: &[u8; VMPCK_LEN],
    msg_seqno: u64,
    report_data: [u8; 64],
) -> Result<(SnpAttestationReport, Vec<u8>), ReportReqError> {
    // The request / response envelope handling is identical to
    // `request_report`. Inline the full prologue so this fn stays
    // self-contained — the duplication is ~50 lines and means
    // `request_report`'s well-tested code path is untouched by the
    // A02 addition.
    let req_host = pci_dma::allocate_buffer(1).map_err(|_| ReportReqError::AllocFailed)?;
    let resp_host = pci_dma::allocate_buffer(1).map_err(|err| {
        let _ = pci_dma::free_buffer(req_host, 1);
        let _ = err;
        ReportReqError::AllocFailed
    })?;

    let req_msg: &mut SnpGuestMsg = unsafe { &mut *(req_host as *mut SnpGuestMsg) };
    let zeros = &[0u8; SNP_GUEST_MSG_SIZE];
    *req_msg = SnpGuestMsg::read_from_bytes(zeros)
        .expect("4 KiB decodes into SnpGuestMsg");
    let req = MsgReportReq::new(report_data, 0);
    let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
    if let Err(e) = encrypt_guest_msg(
        req_msg,
        vmpck0,
        msg_seqno,
        SnpMsgType::ReportReq,
        0,
        plaintext,
    ) {
        let _ = pci_dma::free_buffer(req_host, 1);
        let _ = pci_dma::free_buffer(resp_host, 1);
        return Err(ReportReqError::Envelope(e));
    }
    let resp_msg: &mut SnpGuestMsg = unsafe { &mut *(resp_host as *mut SnpGuestMsg) };
    *resp_msg = SnpGuestMsg::read_from_bytes(zeros)
        .expect("4 KiB decodes into SnpGuestMsg");

    let op = PciRootBridgeIoProtocolOperation::BUS_MASTER_COMMON_BUFFER64;
    let req_map = match pci_dma::map(op, req_host, SNP_GUEST_MSG_SIZE) {
        Ok(m) => m,
        Err(_) => {
            let _ = pci_dma::free_buffer(req_host, 1);
            let _ = pci_dma::free_buffer(resp_host, 1);
            return Err(ReportReqError::AllocFailed);
        }
    };
    let resp_map = match pci_dma::map(op, resp_host, SNP_GUEST_MSG_SIZE) {
        Ok(m) => m,
        Err(_) => {
            let _ = pci_dma::unmap(req_map.cookie);
            let _ = pci_dma::free_buffer(req_host, 1);
            let _ = pci_dma::free_buffer(resp_host, 1);
            return Err(ReportReqError::AllocFailed);
        }
    };

    // Cert-chain data pages. Start with `CERT_PAGES_INITIAL`; if the
    // hypervisor says we need more, retry once with the requested
    // size (capped at `CERT_PAGES_MAX`).
    let mut data_pages = CERT_PAGES_INITIAL;
    let mut attempt = 0u32;
    let result: Result<(SnpAttestationReport, Vec<u8>), ReportReqError> = loop {
        attempt += 1;
        let data_host = match pci_dma::allocate_buffer(data_pages as usize) {
            Ok(p) => p,
            Err(_) => break Err(ReportReqError::AllocFailed),
        };
        // Zero so stale host bytes don't leak into the bounce or
        // confuse the parser if the hypervisor writes less than a
        // full buffer.
        // SAFETY: data_host covers `data_pages * PAGE_SIZE` bytes we
        // just allocated; no aliases.
        unsafe {
            core::ptr::write_bytes(
                data_host,
                0,
                (data_pages as usize) * PAGE_SIZE,
            );
        }
        let data_map = match pci_dma::map(op, data_host, (data_pages as usize) * PAGE_SIZE) {
            Ok(m) => m,
            Err(_) => {
                let _ = pci_dma::free_buffer(data_host, data_pages as usize);
                break Err(ReportReqError::AllocFailed);
            }
        };

        log::info!(
            "QRAB_SNP_RR_EXT_PRE attempt={attempt} data_pages={data_pages} data_bus={:#x}",
            data_map.device_addr,
        );
        let ext_rr = ghcb_vmgexit::guest_request_extended(
            req_map.device_addr,
            resp_map.device_addr,
            data_map.device_addr,
            data_pages,
        );

        // Unmap ordering matches the basic request_report path: the
        // IoMmu's CommonBuffer Unmap copies bounce -> host for write
        // ops, which is what we need so the CPU reads see the
        // cert-chain bytes + PSP response.
        let _ = pci_dma::unmap(data_map.cookie);

        match ext_rr {
            Ok(()) => {
                // Copy cert bytes before freeing the page.
                // SAFETY: data_host points at `data_pages * PAGE_SIZE`
                // bytes we own, just populated by the hypervisor via
                // the Unmap bounce copy.
                let cert_bytes = unsafe {
                    core::slice::from_raw_parts(
                        data_host,
                        (data_pages as usize) * PAGE_SIZE,
                    )
                    .to_vec()
                };
                let _ = pci_dma::free_buffer(data_host, data_pages as usize);

                // Response envelope handling identical to the basic path.
                let _ = pci_dma::unmap(resp_map.cookie);
                let _ = pci_dma::unmap(req_map.cookie);
                match decode_response(resp_host, vmpck0, msg_seqno) {
                    Ok(report) => {
                        let _ = pci_dma::free_buffer(req_host, 1);
                        let _ = pci_dma::free_buffer(resp_host, 1);
                        return Ok((report, cert_bytes));
                    }
                    Err(e) => {
                        let _ = pci_dma::free_buffer(req_host, 1);
                        let _ = pci_dma::free_buffer(resp_host, 1);
                        return Err(e);
                    }
                }
            }
            Err(GhcbError::InvalidLen { required_pages }) => {
                let _ = pci_dma::free_buffer(data_host, data_pages as usize);
                if required_pages > CERT_PAGES_MAX {
                    break Err(ReportReqError::CertTooLarge { required_pages });
                }
                if required_pages <= data_pages || attempt >= 2 {
                    // Either hypervisor lied (asked for same-or-less
                    // than what we already passed) or we already
                    // retried once — bail instead of looping.
                    break Err(ReportReqError::Ghcb(GhcbError::InvalidLen { required_pages }));
                }
                data_pages = required_pages;
                continue;
            }
            Err(e) => {
                let _ = pci_dma::free_buffer(data_host, data_pages as usize);
                break Err(ReportReqError::Ghcb(e));
            }
        }
    };

    // Any exit from the loop *that did not already return* still
    // needs to release req/resp mappings. Successful return paths
    // handled those above.
    let _ = pci_dma::unmap(resp_map.cookie);
    let _ = pci_dma::unmap(req_map.cookie);
    let _ = pci_dma::free_buffer(req_host, 1);
    let _ = pci_dma::free_buffer(resp_host, 1);
    result
}
