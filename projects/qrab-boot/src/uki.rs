//! UKI `LoadImage` + `StartImage` terminal step (phase 5 StartImage).
//!
//! Called from [`crate::run_vsock_demo`] once:
//!
//! * [`qrab_boot_proto::AttestResponse`] has been decoded, validated,
//!   and its ECDSA-P256 signature verified against the embedded
//!   server pubkey.
//! * The UKI bytes have been fetched over vsock and their SHA-256 has
//!   been compared against the signed `uki_sha256` field — so by the
//!   time we get here, the payload is cryptographically bound to
//!   the signed response and transport-integrity-checked.
//! * PCR 8 has been extended with the UKI bytes (measured boot) and
//!   PCR 9 has been extended with `secret_sha256 || server_nonce`.
//! * The attestation secret has been published to the `QrabSecret`
//!   EFI runtime variable for the initrd to consume post-boot.
//!
//! All that remains is to hand the UKI to the firmware via
//! `BootServices::load_image` + `start_image` and let Secure Boot (when
//! the firmware build has it) validate the PE Authenticode signature
//! against the `db` cert embedded at build time. Per the plan this
//! function is fail-closed — any error logs a distinct
//! `QRAB_FAIL_*` / `QRAB_LOADIMAGE_*` token and halts.
//!
//! ## Stub-UKI skip path
//!
//! `contrib/smoke-vsock-qemu.sh` runs under TCG with the 512-byte
//! [`attest_server::STUB_UKI`] payload (`b"QRAB_STUB_UKI_v0\n"` repeated).
//! That is not a valid PE, so `load_image` would fail with
//! `EFI_INVALID_PARAMETER`. Without a bypass the smoke would flip red
//! on every Phase-5 build. Gate the terminal `load_image` call behind
//! the `qrab_uki_boot` cfg, emitted by `build.rs` only when
//! `QRAB_UKI_BOOT=1` is set at build time. In the default build the
//! function logs `QRAB_LOADIMAGE_STUB_SKIP` and halts — preserving the
//! current smoke behaviour while still exercising the `QrabReport` +
//! measurement path. Real-UKI integration runs (matrix tests, BMC
//! boots) opt into the live path via `QRAB_UKI_BOOT=1 cargo build`.
//!
//! ## `QrabReport` EFI runtime variable
//!
//! Before the last `load_image`/`start_image` call we persist a
//! typed-bytes blob into the `QrabReport` EFI runtime variable under
//! the shared `QRAB_SECRET_GUID` vendor GUID (see
//! [`crate::secret`]). The variable is `BS|RT` and **not** non-
//! volatile: guest userspace (the dracut module) reads it after
//! `ExitBootServices` to surface last-boot attestation state, then
//! wipes it. The blob is intentionally opaque to the bootloader;
//! phase 5d.2 currently stashes a fixed header ("QRAB_REPORT_V1")
//! plus the 32-byte `report_data` the PSP was asked to bind. When
//! the SNP report body is available in this scope we widen the blob
//! to include the full 1184-byte attestation report.

use alloc::vec::Vec;

#[cfg(qrab_uki_boot)]
use qrab_boot_verify::secure_boot::{SbStatus, evaluate_bytes, sb_token};
use uefi::boot::{self, LoadImageSource};
use uefi::runtime::{self, VariableAttributes, VariableVendor};
use uefi::{CStr16, Status, cstr16};

use crate::halt_forever;
use crate::secret::QRAB_SECRET_GUID;

/// `EFI_GLOBAL_VARIABLE::SecureBoot` (UEFI 2.11 §3.3). Read-only UINT8.
const SECURE_BOOT_VAR: &CStr16 = cstr16!("SecureBoot");
/// `EFI_GLOBAL_VARIABLE::SetupMode`.
const SETUP_MODE_VAR: &CStr16 = cstr16!("SetupMode");

/// Fetch `SecureBoot` + `SetupMode` and evaluate them via
/// `qrab_boot_verify::secure_boot::evaluate_bytes`. All firmware
/// errors collapse to `SbStatus::VarMissing(...)` so the caller can
/// treat "fetch failed" and "variable empty" identically.
#[cfg(qrab_uki_boot)]
fn check_firmware_sb() -> SbStatus {
    let mut sb_buf = [0u8; 4];
    let mut sm_buf = [0u8; 4];
    let (sb_slice, sm_slice): (&[u8], &[u8]) = (
        match runtime::get_variable(
            SECURE_BOOT_VAR,
            &VariableVendor::GLOBAL_VARIABLE,
            &mut sb_buf,
        ) {
            Ok((bytes, _)) => bytes,
            Err(_) => &[],
        },
        match runtime::get_variable(
            SETUP_MODE_VAR,
            &VariableVendor::GLOBAL_VARIABLE,
            &mut sm_buf,
        ) {
            Ok((bytes, _)) => bytes,
            Err(_) => &[],
        },
    );
    evaluate_bytes(sb_slice, sm_slice)
}

/// Enforce `SecureBoot == 1 && SetupMode == 0` before handing the UKI
/// to firmware. Halts on any other status after emitting the matching
/// `QRAB_FAIL_SB_*` token. On the happy path logs `QRAB_SB_OK` and
/// returns so the caller may proceed to LoadImage.
///
/// Only called on the real-UKI path (`qrab_uki_boot` cfg). Stub-skip
/// builds never reach this and continue to emit `QRAB_LOADIMAGE_STUB_SKIP`
/// as before.
#[cfg(qrab_uki_boot)]
fn require_sb_enforced() {
    let status = check_firmware_sb();
    log::info!("{}", sb_token(status));
    if status != SbStatus::Enforcing {
        halt_forever();
    }
}

/// Vendor GUID under which the last-boot attestation blob is published.
///
/// The variable lives alongside `QrabSecret` under the same GUID —
/// initrd consumers can enumerate both with a single directory scan in
/// `/sys/firmware/efi/efivars/Qrab*-<GUID>`.
pub const QRAB_REPORT_GUID: uefi::Guid = QRAB_SECRET_GUID;

/// UEFI variable name for the attestation-report snapshot.
pub const QRAB_REPORT_NAME: &CStr16 = cstr16!("QrabReport");

/// 8-byte magic prefix. Same discipline as `QRABSECRET_V1` in the
/// initrd module: the first bytes of the blob commit to a schema so
/// userspace can tell "no qrab-boot ran" from "qrab-boot ran with a
/// schema older/newer than I understand".
pub const QRAB_REPORT_MAGIC: &[u8; 14] = b"QRAB_REPORT_V1";

/// Distinguished errors from [`publish_report`]. Each variant maps to
/// a `QRAB_REPORT_*` serial token the smoke harness can grep.
#[derive(Debug)]
pub enum ReportError {
    /// `runtime::set_variable` returned a non-success status.
    SetVariable(uefi::Error),
}

impl core::fmt::Display for ReportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SetVariable(e) => write!(f, "SetVariable failed: {e:?}"),
        }
    }
}

/// Publish `report_data` as the volatile, boot+runtime-accessible
/// `QrabReport` EFI variable. `report_data` is opaque to this
/// function: callers may pass just the 32-byte SNP `report_data`
/// (plain-TCG boots where the PSP exchange is skipped) or the full
/// 1184-byte signed attestation report (SNP boots — matches
/// `qrab_snp_proto::SnpAttestationReport`'s size-of). The variable
/// body is always `MAGIC || body_bytes`. qrab-guest-init's
/// `report::parse` handles the MAGIC strip + length check.
pub fn publish_report(body: &[u8]) -> Result<(), ReportError> {
    let mut blob = Vec::with_capacity(QRAB_REPORT_MAGIC.len() + body.len());
    blob.extend_from_slice(QRAB_REPORT_MAGIC);
    blob.extend_from_slice(body);
    let attrs = VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS;
    runtime::set_variable(
        QRAB_REPORT_NAME,
        &VariableVendor(QRAB_REPORT_GUID),
        attrs,
        &blob,
    )
    .map_err(ReportError::SetVariable)
}

/// UEFI variable name for the session nonces snapshot.
///
/// L5 qrab-guest-init reads this back (via the dracut pre-pivot
/// hook that copies it to `/run/qrab/session`) and feeds the 64
/// bytes into `policy::reconstruct_report_data`. The body is
/// exactly `client_nonce[32] || server_nonce[32]` — no magic
/// prefix because the consumer is length-addressed, not
/// schema-sniffed.
pub const QRAB_SESSION_NAME: &CStr16 = cstr16!("QrabSession");

/// Publish the 64-byte `client_nonce || server_nonce` blob under
/// the shared `QRAB_SECRET_GUID` vendor GUID. Same attribute set
/// as `QrabReport` / `QrabSecret`: `BOOTSERVICE_ACCESS |
/// RUNTIME_ACCESS`, explicitly volatile so the variable vanishes
/// on next boot. The dracut module at
/// `contrib/qrab-boot-initrd-module/` copies it to
/// `/run/qrab/session` pre-pivot alongside the secret.
///
/// Secrecy argument: the nonces are public once the pre-commit is
/// on the wire — the server broadcasts `server_nonce` inside the
/// signed response and the guest's `client_nonce` is visible in
/// the request frame. Only integrity + freshness matter, both of
/// which the firmware variable path provides across
/// ExitBootServices.
pub fn publish_session(nonces: &[u8; 64]) -> Result<(), ReportError> {
    let attrs = VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS;
    runtime::set_variable(
        QRAB_SESSION_NAME,
        &VariableVendor(QRAB_REPORT_GUID),
        attrs,
        nonces,
    )
    .map_err(ReportError::SetVariable)
}

/// Hand `uki_bytes` to the firmware via `BootServices::load_image`
/// and transfer control via `start_image`. Never returns.
///
/// The default build halts on the stub-UKI skip path before
/// `load_image` is ever called; see module docs. Setting
/// `QRAB_UKI_BOOT=1` at build time (picked up by `build.rs`) emits the
/// `qrab_uki_boot` cfg and this function will drive the real
/// firmware path.
pub fn load_and_start(uki_bytes: &[u8]) -> ! {
    if !cfg!(qrab_uki_boot) {
        log::info!(
            "QRAB_LOADIMAGE_STUB_SKIP len={} (rebuild with QRAB_UKI_BOOT=1 to exercise LoadImage)",
            uki_bytes.len()
        );
        halt_forever();
    }

    // Refuse to hand the UKI to firmware unless Secure Boot is
    // enforcing. The check is compiled in only when qrab_uki_boot
    // is set because that is the only configuration where LoadImage
    // actually runs. Under any other firmware state (setup mode,
    // SB disabled, missing vars) we fail closed.
    #[cfg(qrab_uki_boot)]
    require_sb_enforced();

    // `load_image` wants a `&[u8]` slice; we have one. `file_path=None`
    // tells the firmware the image has no backing device path, which
    // is what we want — the UKI arrived over vsock, not from a disk.
    let image = match boot::load_image(
        boot::image_handle(),
        LoadImageSource::FromBuffer {
            buffer: uki_bytes,
            file_path: None,
        },
    ) {
        Ok(h) => {
            log::info!("QRAB_LOADIMAGE_OK");
            h
        }
        Err(e) if e.status() == Status::SECURITY_VIOLATION => {
            // Firmware Secure Boot rejected the PE Authenticode sig.
            // This is the canonical "tampered UKI caught by SB" token
            // the plan's scenario 3 expects. Fail-closed: no retry,
            // no reset, no shell fallback.
            log::warn!("QRAB_FAIL_SB");
            halt_forever();
        }
        Err(e) => {
            // Any other load failure (malformed PE, unsupported
            // format, out-of-memory). Distinct token so the matrix
            // tests can assert the specific branch.
            log::warn!("QRAB_FAIL_LOAD status={:#x}", e.status().0);
            halt_forever();
        }
    };

    // `start_image` only returns on a non-zero guest exit. A UKI that
    // successfully transfers to the kernel never comes back, so any
    // `Ok`/`Err` here is a failure from our POV.
    match boot::start_image(image) {
        Ok(()) => {
            log::warn!("QRAB_FAIL_STARTIMAGE ok_returned");
            halt_forever();
        }
        Err(e) => {
            log::warn!("QRAB_FAIL_STARTIMAGE status={:#x}", e.status().0);
            halt_forever();
        }
    }
}
