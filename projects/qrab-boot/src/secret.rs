//! EFI runtime-variable injection of the attestation secret.
//!
//! After [`crate::attest::verify`] signs off on an
//! [`qrab_boot_proto::AttestResponse`] the bootloader hands the opaque
//! `secret` bytes to the guest kernel via a UEFI runtime variable
//! named `QrabSecret`. The initrd dracut module at
//! `contrib/qrab-boot-initrd-module/` reads the variable out of
//! `/sys/firmware/efi/efivars/QrabSecret-<GUID>`, copies the payload
//! into `/run/qrab/secret`, and wipes the variable.
//!
//! ## Variable attributes
//!
//! Per the plan (phase 6), the variable is:
//!
//! * `BOOTSERVICE_ACCESS | RUNTIME_ACCESS` — readable from the kernel
//!   after `ExitBootServices`.
//! * **not** `NON_VOLATILE` — the secret must not survive a reboot.
//!
//! ## Vendor GUID
//!
//! `QRAB_SECRET_GUID` is a fixed, randomly-generated UUID. The initrd
//! module hardcodes the same value; rotating it requires rebuilding
//! both this binary and the initrd module together.

use uefi::guid;
use uefi::runtime::{self, VariableAttributes, VariableVendor};
use uefi::{CStr16, cstr16};

/// Vendor GUID for every `Qrab*` EFI variable. Pinned at build time;
/// the initrd module keeps a matching copy.
pub const QRAB_SECRET_GUID: uefi::Guid = guid!("6e6f1cba-b5ba-4e54-a3b3-2a7a8f3d9b00");

/// UEFI variable name for the attestation secret.
pub const QRAB_SECRET_NAME: &CStr16 = cstr16!("QrabSecret");

/// Distinguished errors from [`publish`]. Each variant maps to a
/// `QRAB_SECRET_*` serial token so the smoke harness can grep.
#[derive(Debug)]
pub enum SecretError {
    /// `runtime::set_variable` returned a non-success EFI status.
    SetVariable(uefi::Error),
    /// Empty secret — would silently succeed otherwise, which is
    /// indistinguishable from "didn't try" to the initrd consumer.
    Empty,
}

impl core::fmt::Display for SecretError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SetVariable(e) => write!(f, "SetVariable failed: {e:?}"),
            Self::Empty => f.write_str("secret is empty"),
        }
    }
}

/// Publish `secret` as the volatile, boot+runtime-accessible EFI
/// variable `QrabSecret`. Fails fail-closed on any SetVariable error
/// or on an empty secret.
pub fn publish(secret: &[u8]) -> Result<(), SecretError> {
    if secret.is_empty() {
        return Err(SecretError::Empty);
    }
    let attrs = VariableAttributes::BOOTSERVICE_ACCESS | VariableAttributes::RUNTIME_ACCESS;
    runtime::set_variable(QRAB_SECRET_NAME, &VariableVendor(QRAB_SECRET_GUID), attrs, secret)
        .map_err(SecretError::SetVariable)?;
    Ok(())
}
