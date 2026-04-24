//! vTPM measurement via `EFI_TCG2_PROTOCOL`.
//!
//! Per `reference/qrab-boot-plan.md` phase 5:
//!
//! * PCR 8 is extended with the SHA-256 of the UKI bytes.
//! * PCR 9 is extended with the SHA-256 of
//!   `secret_sha256 || server_nonce`.
//!
//! ## Required-vTPM gate (R20 Phase 3)
//!
//! [`require_and_probe_vtpm`] runs once at `efi_main` entry, right
//! after the SNP gate. It queries `EFI_TCG2_PROTOCOL.GetCapability`,
//! requires `TPMPresentFlag=TRUE`, requires a SHA-256 bank, then
//! issues a raw `TPM2_PCR_Read(PCR 0, SHA-256)` to confirm the
//! firmware (Tcg2Pei) already extended S-CRTM measurements — a
//! sanity check that the vTPM is actually wired, not just "present".
//! Under the `qrab_require_vtpm` cfg (default) the caller halts on
//! any failure with `QRAB_FAIL_NO_VTPM`; the `QRAB_ALLOW_NO_VTPM=1`
//! opt-out compiles the cfg out so the TCG stub smoke (no OVMF,
//! no SVSM vTPM) still runs — the caller emits `QRAB_TCG2_SOFT_ABSENT`
//! and continues.
//!
//! Classification, command encoding, and response parsing live in
//! [`qrab_boot_verify::vtpm`] so they're exercised host-side via
//! `cargo test -p qrab-boot-verify`. Only the actual UEFI protocol
//! plumbing is here.

use alloc::boxed::Box;

use qrab_boot_verify::vtpm::{
    PCR_READ_PCR0_SHA256_CMD, PCR_READ_RESPONSE_MIN_LEN, ParseError, VtpmStatus, VtpmSummary,
    classify_capability, parse_pcr_read_response, summarize,
};
use sha2::{Digest, Sha256};
use uefi::boot;
use uefi::proto::tcg::v2::{HashLogExtendEventFlags, PcrEventInputs, Tcg};
use uefi::proto::tcg::{EventType, PcrIndex};

/// PCR index for the secret+server-nonce binding digest. Plan phase 5.2.
const PCR_SECRET_BINDING: PcrIndex = PcrIndex(9);

/// PCR index for UKI measurement. The bootloader extends with the UKI
/// bytes; TCG2 hashes on all active banks internally.
const PCR_UKI: PcrIndex = PcrIndex(8);

/// Distinguished errors from [`extend_secret_binding`] and [`extend_uki`].
#[derive(Debug)]
pub enum MeasureError {
    /// `boot::open_protocol_exclusive::<Tcg>` returned an error — the
    /// firmware did not produce an `EFI_TCG2_PROTOCOL`. Under the
    /// default `qrab_require_vtpm` build [`require_and_probe_vtpm`]
    /// already halts on this, so reaching the extend callers with
    /// `Tcg2Absent` is only possible under the `QRAB_ALLOW_NO_VTPM=1`
    /// opt-out.
    Tcg2Absent,
    /// `PcrEventInputs::new_in_box` rejected the event data size.
    EventInput,
    /// `Tcg::hash_log_extend_event` returned a non-success EFI status.
    Extend(uefi::Error),
}

impl core::fmt::Display for MeasureError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Tcg2Absent => f.write_str("EFI_TCG2_PROTOCOL not located"),
            Self::EventInput => f.write_str("PcrEventInputs construction failed"),
            Self::Extend(e) => write!(f, "hash_log_extend_event failed: {e:?}"),
        }
    }
}

/// Distinct failure modes from [`require_and_probe_vtpm`]. Each variant
/// maps 1:1 to a serial-log suffix so operators can grep the halt
/// reason even though the caller's token is always `QRAB_FAIL_NO_VTPM`.
#[derive(Debug)]
pub enum VtpmProbeError {
    /// `boot::get_handle_for_protocol::<Tcg>()` returned an error —
    /// usually "no TCG2 DXE" (wrong OVMF build).
    Tcg2Absent(uefi::Error),
    /// The protocol was located but `open_protocol_exclusive` failed.
    OpenFailed(uefi::Error),
    /// `GetCapability` returned a non-success status.
    GetCapability(uefi::Error),
    /// `BootServiceCapability.present_flag == 0`.
    TpmNotPresent,
    /// SHA-256 bank not in `active_pcr_banks`.
    NoSha256Bank { active_pcr_banks: u32 },
    /// `SubmitCommand` returned a non-success status on the fixed
    /// `TPM2_PCR_Read` call.
    PcrReadFailed(uefi::Error),
    /// TPM response unparseable (truncated, non-zero `responseCode`,
    /// wrong digest count, wrong digest size).
    PcrParse(ParseError),
    /// TPM reported an all-zero PCR 0 digest — Tcg2Pei did not extend
    /// S-CRTM measurements, which indicates the SVSM vTPM was not
    /// reachable during PEI even though Tcg2Dxe later came up.
    PcrAllZero,
}

impl core::fmt::Display for VtpmProbeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Tcg2Absent(e) => write!(f, "TCG2_ABSENT {e:?}"),
            Self::OpenFailed(e) => write!(f, "TCG2_OPEN {e:?}"),
            Self::GetCapability(e) => write!(f, "TCG2_GET_CAPABILITY {e:?}"),
            Self::TpmNotPresent => f.write_str("TPM_NOT_PRESENT"),
            Self::NoSha256Bank { active_pcr_banks } => {
                write!(f, "NO_SHA256_BANK active={active_pcr_banks:#x}")
            }
            Self::PcrReadFailed(e) => write!(f, "PCR_READ {e:?}"),
            Self::PcrParse(e) => write!(f, "PCR_PARSE {e:?}"),
            Self::PcrAllZero => f.write_str("PCR0_ALL_ZERO"),
        }
    }
}

/// Require the firmware to expose a working SHA-256 vTPM and prove it
/// has been extended by reading a non-zero PCR 0 digest. On success
/// logs `QRAB_VTPM_SVSM_OK` with the first 8 hex bytes of PCR 0.
///
/// Callers halt with `QRAB_FAIL_NO_VTPM` on `Err`.
pub fn require_and_probe_vtpm() -> Result<(), VtpmProbeError> {
    let handle = boot::get_handle_for_protocol::<Tcg>().map_err(VtpmProbeError::Tcg2Absent)?;
    let mut tcg = boot::open_protocol_exclusive::<Tcg>(handle).map_err(VtpmProbeError::OpenFailed)?;

    let cap = tcg.get_capability().map_err(VtpmProbeError::GetCapability)?;
    let active_banks = cap.active_pcr_banks.bits();
    log::info!(
        "QRAB_VTPM_CAP protocol={}.{} max_cmd={} max_rsp={} mfr={:#x} active_banks={:#x} \
         present={}",
        { cap.protocol_version.major },
        { cap.protocol_version.minor },
        { cap.max_command_size },
        { cap.max_response_size },
        { cap.manufacturer_id },
        active_banks,
        cap.tpm_present(),
    );
    match classify_capability(cap.tpm_present(), active_banks) {
        VtpmStatus::Enforcing { .. } => {}
        VtpmStatus::NotPresent => return Err(VtpmProbeError::TpmNotPresent),
        VtpmStatus::NoSha256Bank { active_pcr_banks } => {
            return Err(VtpmProbeError::NoSha256Bank { active_pcr_banks });
        }
    }

    let mut response = [0u8; 128];
    debug_assert!(response.len() >= PCR_READ_RESPONSE_MIN_LEN);
    tcg.submit_command(&PCR_READ_PCR0_SHA256_CMD, &mut response)
        .map_err(VtpmProbeError::PcrReadFailed)?;
    let digest = parse_pcr_read_response(&response).map_err(VtpmProbeError::PcrParse)?;

    match summarize(&digest) {
        VtpmSummary::AllZero => Err(VtpmProbeError::PcrAllZero),
        VtpmSummary::NonZero { prefix } => {
            log::info!(
                "QRAB_VTPM_SVSM_OK pcr0_sha256_prefix={:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                prefix[0],
                prefix[1],
                prefix[2],
                prefix[3],
                prefix[4],
                prefix[5],
                prefix[6],
                prefix[7],
            );
            Ok(())
        }
    }
}

/// Extend PCR 9 with `sha256(secret_sha256 || server_nonce)`. The
/// `data_to_hash` passed to `Tcg::hash_log_extend_event` is the raw
/// 64-byte concatenation — the TCG2 protocol performs the SHA-256
/// internally on every active bank before extending.
///
/// Also records a SHA-256 of the same bytes into the serial log via
/// `QRAB_PCR9_EXPECTED_DIGEST` so a post-boot verifier can compare
/// `tpm2_pcrread sha256:9` against the bootloader's claim.
pub fn extend_secret_binding(
    secret_sha256: &[u8; 32],
    server_nonce: &[u8; 32],
) -> Result<(), MeasureError> {
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(secret_sha256);
    data[32..].copy_from_slice(server_nonce);

    let expected = Sha256::digest(data);
    log::info!(
        "QRAB_PCR9_EXPECTED_DIGEST {:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        expected[0], expected[1], expected[2], expected[3],
        expected[4], expected[5], expected[6], expected[7],
    );

    let handle = boot::get_handle_for_protocol::<Tcg>().map_err(|_| MeasureError::Tcg2Absent)?;
    let mut tcg = boot::open_protocol_exclusive::<Tcg>(handle)
        .map_err(|_| MeasureError::Tcg2Absent)?;

    let event: Box<PcrEventInputs> =
        PcrEventInputs::new_in_box(PCR_SECRET_BINDING, EventType::IPL, b"QrabSecretBinding")
            .map_err(|_| MeasureError::EventInput)?;

    tcg.hash_log_extend_event(HashLogExtendEventFlags::empty(), &data, &event)
        .map_err(MeasureError::Extend)?;

    log::info!("QRAB_PCR9_OK");
    Ok(())
}

/// Extend PCR 8 with the raw UKI bytes. TCG2 applies SHA-256 (and any
/// other active bank) internally; we log the SHA-256 alongside so a
/// post-boot verifier can reproduce `tpm2_pcrread sha256:8` from the
/// signed `uki_sha256`.
///
/// Logs `QRAB_PCR8_EXPECTED_DIGEST <first 8 hex chars>` before the
/// extend and `QRAB_PCR8_OK` on success.
pub fn extend_uki(uki_bytes: &[u8]) -> Result<(), MeasureError> {
    let expected = Sha256::digest(uki_bytes);
    log::info!(
        "QRAB_PCR8_EXPECTED_DIGEST {:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        expected[0], expected[1], expected[2], expected[3],
        expected[4], expected[5], expected[6], expected[7],
    );

    let handle = boot::get_handle_for_protocol::<Tcg>().map_err(|_| MeasureError::Tcg2Absent)?;
    let mut tcg = boot::open_protocol_exclusive::<Tcg>(handle)
        .map_err(|_| MeasureError::Tcg2Absent)?;

    let event: Box<PcrEventInputs> =
        PcrEventInputs::new_in_box(PCR_UKI, EventType::IPL, b"QrabUki")
            .map_err(|_| MeasureError::EventInput)?;

    tcg.hash_log_extend_event(HashLogExtendEventFlags::empty(), uki_bytes, &event)
        .map_err(MeasureError::Extend)?;

    log::info!("QRAB_PCR8_OK");
    Ok(())
}
