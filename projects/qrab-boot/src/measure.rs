//! vTPM measurement via `EFI_TCG2_PROTOCOL`.
//!
//! Per `reference/qrab-boot-plan.md` phase 5:
//!
//! * PCR 8 is extended with the SHA-256 of the UKI bytes.
//! * PCR 9 is extended with the SHA-256 of
//!   `secret_sha256 || server_nonce`.
//!
//! Phase 4-partial ships only the PCR 9 leg because the UKI bytes are
//! not yet fetched (the wire protocol carries `uki_sha256` but not
//! the UKI body — that will change alongside the vsock stream helper
//! work in a later phase). PCR 8 is still declared here as a TODO so
//! the call site is ready the moment UKI bytes arrive.
//!
//! ## Missing vTPM
//!
//! AmdSev OVMF builds do not include `Tcg2Dxe` by default. A missing
//! TCG2 protocol is not a fatal error — we log `QRAB_TCG2_ABSENT`
//! and move on so local smoke flows without `-tpmdev` / `swtpm` still
//! exercise the rest of the bootloader. Production SNP VMs launched
//! via `manager` get a vTPM plumbed in (see
//! `reference/qrab-boot-plan.md` phase 5.3) and the extend calls
//! actually run.

use alloc::boxed::Box;

use sha2::{Digest, Sha256};
use uefi::boot;
use uefi::proto::tcg::{EventType, PcrIndex};
use uefi::proto::tcg::v2::{HashLogExtendEventFlags, PcrEventInputs, Tcg};

/// PCR index for the secret+server-nonce binding digest. Plan phase 5.2.
const PCR_SECRET_BINDING: PcrIndex = PcrIndex(9);

/// PCR index for UKI measurement. The bootloader extends with the UKI
/// bytes; TCG2 hashes on all active banks internally.
const PCR_UKI: PcrIndex = PcrIndex(8);

/// Distinguished errors from [`extend_secret_binding`].
#[derive(Debug)]
pub enum MeasureError {
    /// `boot::open_protocol_exclusive::<Tcg>` returned an error — the
    /// firmware did not produce an `EFI_TCG2_PROTOCOL`.
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
/// extend and `QRAB_PCR8_OK` on success. Returns the same error
/// variants as [`extend_secret_binding`], with `Tcg2Absent` being
/// the non-fatal case the caller logs as `QRAB_TCG2_ABSENT`.
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
