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

mod attest;
mod ghcb;
mod measure;
mod panic;
mod rng;
mod secret;
mod vsock;
mod vsock_connect;
mod vsock_driver;
mod vsock_hal;
mod vsock_pci;

use core::arch::asm;

#[cfg(not(qrab_no_fixtures))]
use qrab_boot_verify::{VerifyError, verify_detached};
use sha2::Digest as _;
use uefi::prelude::*;

/// Banner printed on every boot. Test harness greps for this.
const BANNER: &str = concat!(
    "QRAB BOOT v",
    env!("CARGO_PKG_VERSION"),
    " phase4+secret+pcr9+uki (build ",
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

    let features = ghcb::detect();
    ghcb::log_features(&features);

    run_vsock_demo();

    run_verify_demo();

    log::info!("QRAB_PHASE4_OK");

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

    // Phase 4: every boot picks a fresh 32-byte nonce via RDRAND.
    // A CPU without RDRAND (vanishingly rare on anything new enough
    // to run SEV-SNP, but possible under weird TCG configs) fails
    // closed — the attestation flow is strictly worse with a
    // predictable nonce, and the rest of the boot hangs anyway.
    let client_nonce = match rng::fresh_nonce() {
        Ok(n) => n,
        Err(e) => {
            log::warn!("QRAB_ATTEST_RNG_ERR {e}");
            return;
        }
    };
    log::info!(
        "QRAB_ATTEST_NONCE prefix={:02x}{:02x}{:02x}{:02x}",
        client_nonce[0],
        client_nonce[1],
        client_nonce[2],
        client_nonce[3],
    );

    let request = qrab_boot_proto::new_request(client_nonce);
    let request_bytes = match qrab_boot_proto::encode_framed(&request) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("QRAB_ATTEST_ENCODE_ERR {e}");
            return;
        }
    };

    if let Err(_) = vsock_connect::send_one(
        &mut driver,
        VSOCK_CID_HOST,
        VSOCK_HOST_PORT,
        VSOCK_GUEST_SRC_PORT,
        &request_bytes,
    ) {
        return; // send_one logged the specific failure.
    }

    let mut stream = vsock_connect::FramedStream::new();
    let response_frame = match stream.recv_frame(
        &mut driver,
        VSOCK_CID_HOST,
        VSOCK_HOST_PORT,
        VSOCK_GUEST_SRC_PORT,
    ) {
        Ok(b) => b,
        Err(_) => return, // recv_frame logged the specific failure.
    };

    // The AttestResponse frame's body is straight postcard — skip the
    // outer 4-byte framing (already stripped by FramedStream) and
    // just decode.
    let response: qrab_boot_proto::AttestResponse =
        match qrab_boot_proto::decode(&response_frame) {
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

    // Verify the server's signature before *anything* downstream is
    // allowed to consume the response. An unverified attestation
    // response is strictly more dangerous than no response at all,
    // because the bootloader would otherwise inject an
    // attacker-chosen secret into the guest kernel.
    if !attest::has_embedded_pubkey() {
        log::warn!(
            "QRAB_ATTEST_NO_PUBKEY (build without contrib/attest-keys/server.sec1; skipping verify)"
        );
    } else {
        match attest::verify(&response) {
            Ok(()) => log::info!("QRAB_ATTEST_SIG_OK"),
            Err(attest::VerifyError::NoPubkey) => {
                log::warn!("QRAB_ATTEST_NO_PUBKEY");
                return;
            }
            Err(attest::VerifyError::MalformedPubkey) => {
                log::warn!("QRAB_ATTEST_MALFORMED_PUBKEY");
                return;
            }
            Err(attest::VerifyError::MalformedSig) => {
                log::warn!("QRAB_ATTEST_MALFORMED_SIG");
                return;
            }
            Err(attest::VerifyError::BadSignature) => {
                log::warn!("QRAB_ATTEST_BAD_SIG");
                return;
            }
            Err(attest::VerifyError::SecretHashMismatch) => {
                log::warn!("QRAB_ATTEST_SECRET_HASH_MISMATCH");
                return;
            }
        }
    }

    // Compute + log the SNP `report_data` the bootloader would feed
    // to `MSG_REPORT_REQ`. Issuance of the real request is gated on
    // the DMA fix tracked in issues/qrab-boot-snp-dma.md; the log
    // still lets a post-boot verifier confirm the expected value.
    let report_data = qrab_boot_proto::report_data(&client_nonce, &response);
    ghcb::record_intended_report_data(&report_data);

    // Second framed payload from attest-server is the UKI bytes. The
    // `AttestResponse.uki_sha256` field was already signed by the
    // server, so matching that digest against sha256(received bytes)
    // confirms server + transport + client all agree. A hash mismatch
    // means a man-in-the-middle tampered with the UKI stream (unlikely
    // over vsock, but cheap to assert).
    let uki_bytes = match stream.recv_frame(
        &mut driver,
        VSOCK_CID_HOST,
        VSOCK_HOST_PORT,
        VSOCK_GUEST_SRC_PORT,
    ) {
        Ok(b) => b,
        Err(_) => {
            log::warn!("QRAB_UKI_FETCH_ERR");
            return;
        }
    };
    let actual_uki_hash: [u8; 32] = sha2::Sha256::digest(&uki_bytes).into();
    if actual_uki_hash != response.uki_sha256 {
        log::warn!(
            "QRAB_FAIL_HASH uki_len={} actual_prefix={:02x}{:02x}{:02x}{:02x} expected_prefix={:02x}{:02x}{:02x}{:02x}",
            uki_bytes.len(),
            actual_uki_hash[0], actual_uki_hash[1],
            actual_uki_hash[2], actual_uki_hash[3],
            response.uki_sha256[0], response.uki_sha256[1],
            response.uki_sha256[2], response.uki_sha256[3],
        );
        return;
    }
    log::info!(
        "QRAB_UKI_OK len={} sha256_prefix={:02x}{:02x}{:02x}{:02x}",
        uki_bytes.len(),
        actual_uki_hash[0], actual_uki_hash[1],
        actual_uki_hash[2], actual_uki_hash[3],
    );

    // Extend PCR 8 with the UKI bytes themselves — TCG2 hashes
    // internally. Missing TCG2 is still non-fatal (same rationale as
    // the PCR 9 call below).
    match measure::extend_uki(&uki_bytes) {
        Ok(()) => {}
        Err(measure::MeasureError::Tcg2Absent) => {
            log::info!("QRAB_TCG2_ABSENT");
        }
        Err(e) => {
            log::warn!("QRAB_PCR8_ERR {e}");
            return;
        }
    }

    // Extend PCR 9 with sha256(secret_sha256 || server_nonce) on any
    // firmware that has a TCG2 protocol. A missing TCG2 is non-fatal
    // (AmdSev OVMF ships without Tcg2Dxe by default); the log token
    // tells the smoke harness which leg of the branch ran.
    match measure::extend_secret_binding(&response.secret_sha256, &response.server_nonce) {
        Ok(()) => {}
        Err(measure::MeasureError::Tcg2Absent) => {
            log::info!("QRAB_TCG2_ABSENT");
        }
        Err(e) => {
            log::warn!("QRAB_PCR9_ERR {e}");
            return;
        }
    }

    // Publish the secret as an EFI runtime variable (volatile, BS+RT).
    // The initrd dracut module at contrib/qrab-boot-initrd-module/
    // will read it out and wipe it during early userspace. Any failure
    // here is terminal — without the secret the kernel has no material
    // to mount the rootfs, so a silent skip would only postpone pain.
    match secret::publish(&response.secret) {
        Ok(()) => log::info!("QRAB_SECRET_OK len={}", response.secret.len()),
        Err(e) => {
            log::warn!("QRAB_SECRET_ERR {e}");
            return;
        }
    }

    log::info!(
        "QRAB_ATTEST_OK server_nonce0={:#04x} secret_len={} sig_len={} uki_url_len={}",
        response.server_nonce[0],
        response.secret.len(),
        response.sig.len(),
        response.uki_url.len(),
    );
}

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
