//! qrab-boot — Rust UEFI bootloader for SEV-SNP guests.
//!
//! Phase 2b: the bootloader links `qrab-boot-verify` and, when test
//! fixtures are available at build time, drives it against a signed
//! and a tampered payload so that a QEMU boot serves as an end-to-end
//! integration test for the verifier on the UEFI target.
//!
//! Still no networking or attestation. See
//! `reference/qrab-boot-plan.md` in llm-rag for the remaining phases.

#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

mod panic;
mod vsock;
mod vsock_connect;
mod vsock_driver;
mod vsock_hal;
mod vsock_pci;

use core::arch::asm;

#[cfg(not(qrab_no_fixtures))]
use qrab_boot_verify::{VerifyError, verify_detached};
use uefi::prelude::*;

/// Banner printed on every boot. Test harness greps for this.
const BANNER: &str = concat!(
    "QRAB BOOT v",
    env!("CARGO_PKG_VERSION"),
    " phase3b (build ",
    env!("CARGO_PKG_NAME"),
    ")",
);

// Build-time fixture embedding: `build.rs` copies the bytes it finds on
// disk into `OUT_DIR`, or emits zero-length placeholders plus a
// `qrab_no_fixtures` cfg when the fixtures are missing. Either way the
// `include_bytes!` paths resolve and the binary links.
const DB_PUBKEY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/db.pubkey.der"));
const UKI_VALID: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/uki-valid.efi"));
const UKI_VALID_SIG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/uki-valid.efi.sig"));
const UKI_TAMPERED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/uki-tampered.efi"));

#[entry]
fn efi_main() -> Status {
    // SAFETY: called exactly once at entry, no prior UEFI state touched.
    uefi::helpers::init().expect("uefi::helpers::init failed");

    log::info!("{BANNER}");

    run_vsock_demo();

    run_verify_demo();

    log::info!("QRAB_PHASE3B_OK");

    // Intentionally do not return control to firmware. Per plan: fail-closed,
    // hang on any terminal state including "phase not complete".
    halt_forever()
}

/// Boot-time driver for [`qrab_boot_verify::verify_detached`].
///
/// Two cases:
///
/// * The valid UKI and its signature must verify — logging `QRAB_VERIFY_VALID_OK`.
/// * The tampered UKI reused with the valid signature must fail — logging
///   `QRAB_VERIFY_TAMPERED_REJECTED`.
///
/// Any deviation from those two outcomes panics via the fail-closed
/// handler, because a bootloader that cannot tell a good payload from a
/// tampered one is strictly worse than one that never boots.
#[cfg(not(qrab_no_fixtures))]
fn run_verify_demo() {
    match verify_detached(UKI_VALID, UKI_VALID_SIG, DB_PUBKEY) {
        Ok(()) => log::info!("QRAB_VERIFY_VALID_OK"),
        Err(e) => panic!("valid fixture failed to verify: {e}"),
    }

    match verify_detached(UKI_TAMPERED, UKI_VALID_SIG, DB_PUBKEY) {
        Err(VerifyError::BadSignature) => log::info!("QRAB_VERIFY_TAMPERED_REJECTED"),
        Ok(()) => panic!("tampered fixture verified — verifier is broken"),
        Err(e) => panic!("tampered fixture wrong error: {e}"),
    }
}

#[cfg(qrab_no_fixtures)]
fn run_verify_demo() {
    log::info!("QRAB_VERIFY_FIXTURES_ABSENT: run contrib/build-test-ukis.sh and rebuild to exercise qrab-boot-verify at boot");
}

/// Default vsock port the host attestation mock listens on. Matches
/// `contrib/mock-vsock-server.py` + `contrib/smoke-vsock-qemu.sh`.
const VSOCK_HOST_PORT: u32 = 1337;

/// Source-side port used when opening the attestation stream. Picked
/// from the ephemeral-ish high range; the host side does not care.
const VSOCK_GUEST_SRC_PORT: u32 = 40042;

/// Host CID for vhost-vsock, per the virtio-vsock spec.
const VSOCK_CID_HOST: u64 = 2;

/// Boot-time driver for the 3a→3b.3→3b.4 chain:
///
///   probe → init driver → connect to host attestation endpoint.
///
/// No data transfer on this commit. Phase 3b.6+ will layer the postcard
/// attest wire protocol on top. A missing device / timeout / handshake
/// failure is logged but does *not* yet panic the boot so the verify
/// demo still has a chance to run in test environments where vsock is
/// unavailable (e.g. the plain-TCG smoke path).
fn run_vsock_demo() {
    let status = vsock::probe();
    let mut driver = match vsock_driver::init(status) {
        Ok(d) => d,
        Err(_) => return, // init logged its own QRAB_* token.
    };
    if vsock_connect::connect_and_wait(
        &mut driver,
        VSOCK_CID_HOST,
        VSOCK_HOST_PORT,
        VSOCK_GUEST_SRC_PORT,
    )
    .is_err()
    {
        return; // connect_and_wait logged the specific failure.
    }

    // Phase 3b.9: speak the postcard + length-prefix dialect from
    // `qrab-boot-proto` against the `attest-server` binary. Real
    // randomness lands in a later commit; a build-time fixed nonce
    // is still enough to tell the response apart from a loopback
    // echo (attest-server inverts the bits when constructing
    // `server_nonce`).
    let request = qrab_boot_proto::new_request(CLIENT_NONCE);
    let request_bytes = match qrab_boot_proto::encode_framed(&request) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("QRAB_ATTEST_ENCODE_ERR {e}");
            return;
        }
    };

    let mut reply = [0u8; 2048];
    let n = match vsock_connect::send_and_recv(
        &mut driver,
        VSOCK_CID_HOST,
        VSOCK_HOST_PORT,
        VSOCK_GUEST_SRC_PORT,
        &request_bytes,
        &mut reply,
    ) {
        Ok(n) => n,
        Err(_) => return, // send_and_recv logged the specific failure.
    };

    let response: qrab_boot_proto::AttestResponse =
        match qrab_boot_proto::decode_framed(&reply[..n]) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("QRAB_ATTEST_DECODE_ERR {e}");
                return;
            }
        };
    if let Err(e) = qrab_boot_proto::validate_response(&response) {
        log::warn!("QRAB_ATTEST_VALIDATE_ERR {e}");
        return;
    }

    log::info!(
        "QRAB_ATTEST_OK server_nonce0={:#04x} secret_len={} sig_len={} uki_url_len={}",
        response.server_nonce[0],
        response.secret.len(),
        response.sig.len(),
        response.uki_url.len(),
    );
}

/// Hard-coded attestation-client nonce used until a real PRNG lands.
/// The server inverts the bits of this value to produce
/// `server_nonce`, so the bootloader's log should print
/// `server_nonce0=0xaa` when it is talking to `attest-server` (and
/// the original `0x55` if it sees an echo or a misconfigured peer).
const CLIENT_NONCE: [u8; 32] = [0x55; 32];

/// Disable interrupts and halt the CPU indefinitely.
///
/// Returns `!` masquerading as `Status` — callers rely on the never-return.
fn halt_forever() -> ! {
    // SAFETY: `cli` and `hlt` are safe on x86_64 when the CPU is in the
    // privilege level we run at (ring 0 in a UEFI application context).
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
    }
    loop {
        // SAFETY: `hlt` is a legal halt instruction at our privilege level.
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
