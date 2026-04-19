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

use core::ptr;

pub use qrab_snp_proto::SecretsValidateError;
use qrab_snp_proto::{SnpSecretsPage, validate_secrets_page};

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
