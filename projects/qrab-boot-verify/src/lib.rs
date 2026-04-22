//! Host-testable helpers used by `qrab-boot` to reason about UEFI
//! Secure Boot state.
//!
//! Historical note: this crate used to carry a detached RSA-PKCS#1
//! v1.5 SHA-256 verifier as a Phase-2b stand-in for UEFI Secure Boot
//! — AmdSev OVMF had no `SecurityPkg` wiring and SEV-SNP rejects
//! pflash, so firmware SB was unavailable. With Phase 1.5 (see
//! `issues/svsm-vtpm-integration.md`) `SecurityPkg` + `AuthVariableLib`
//! + `DxeImageVerificationLib` land in the edk2-svsm AmdSev DSC, PK/
//! KEK/db are baked into the measured FV, and `qrab-boot.efi` is PE-
//! Authenticode-signed with `db.key`. Firmware now does the
//! `LoadImage` verification the detached path was emulating, so the
//! detached verifier retired.
//!
//! What remains is the [`secure_boot`] submodule: pure-logic
//! classification of the `SecureBoot` + `SetupMode` UEFI global
//! variables. `qrab-boot::uki` consumes it to halt before calling
//! `LoadImage` when firmware is not enforcing.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod secure_boot;
