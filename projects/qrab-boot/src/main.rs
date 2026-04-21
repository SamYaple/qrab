//! qrab-boot — Rust UEFI bootloader for SEV-SNP guests.
//!
//! On an SNP host the bootloader loads the SNP Secrets page,
//! builds a 64-byte `report_data` over `SHA-256(domain-tag ||
//! client-nonce || server-nonce || uki-hash || secret-hash)`,
//! issues `MSG_REPORT_REQ` through an `SVM_VMGEXIT_GUEST_REQUEST`,
//! publishes the signed report to `attest-server` over virtio-vsock,
//! verifies the ECDSA-P256 signed response, extends PCR 8/9 via
//! the TCG2 EFI protocol, publishes the `QrabReport` EFI runtime
//! variable, and hands the verified UKI to firmware via
//! `LoadImage + StartImage`. On TCG / non-SNP hosts the same flow
//! runs end-to-end modulo the PSP report (the request is built and
//! logged, the vsock round-trip still exercises the server path).
//!
//! Follow-ups F1–F7, F10, F11 and the wire-v2 `AttestPostReport`
//! reshape have all landed; only F8/F9 (upstream-OVMF work tracked
//! in `reference/upstream-ovmf-plan.md`) remain outside the crate.

#![no_main]
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

mod attest;
mod ghcb;
mod ghcb_vmgexit;
mod measure;
mod panic;
mod pci_dma;
mod rng;
mod secret;
mod snp_secrets;
mod uki;
mod volatile_transport;
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
    " phase5-start (build ",
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

    // R16-H14: SNP enforcement is the default. `features.sev_snp` is
    // false under TCG / plain-KVM; on those hosts the bootloader halts
    // with `QRAB_FAIL_NOSNP` rather than fall through to a flow that
    // trusts an unmeasured environment. Building with
    // `QRAB_ALLOW_NON_SNP=1` sets `cfg(qrab_allow_non_snp)` and
    // compiles this check out — the only supported opt-out, intended
    // for the smoke matrix's scenario 9 (non-prod TCG boots).
    #[cfg(not(qrab_allow_non_snp))]
    if !features.sev_snp {
        log::warn!("QRAB_FAIL_NOSNP");
        halt_forever();
    }

    // B12-10 / R12-14: load the SNP Secrets page exactly once.
    // Previously we loaded here for a sanity log and loaded again
    // inside `run_vsock_demo` for the `MSG_REPORT_REQ` call — two
    // stack frames held VMPCK0..3 simultaneously, compounding the
    // key-residual surface that S7-2's Drop impl then has to scrub.
    // Load once up front, fail-closed on SNP if the PSP page can't
    // be read, and hand the owned page down to `run_vsock_demo`.
    let snp_page: Option<qrab_snp_proto::SnpSecretsPage> = if features.sev_snp {
        match snp_secrets::load() {
            Ok(page) => {
                log::info!(
                    "QRAB_SNP_SECRETS_OK version={} vmpck0_nonzero={} \
                     vmpck1_nonzero={} vmpck2_nonzero={} vmpck3_nonzero={}",
                    { page.version },
                    qrab_snp_proto::is_vmpck_nonzero(&page.vmpck0),
                    qrab_snp_proto::is_vmpck_nonzero(&page.vmpck1),
                    qrab_snp_proto::is_vmpck_nonzero(&page.vmpck2),
                    qrab_snp_proto::is_vmpck_nonzero(&page.vmpck3),
                );
                Some(page)
            }
            Err(e) => {
                // Fail-closed on SNP: without VMPCK0 the attestation flow
                // cannot proceed, and continuing would mean signing an
                // `SNP_GUEST_REQUEST` with zeroed key material or skipping
                // attestation entirely. Either is strictly worse than a
                // hang. CPUID-gated so non-SNP TCG boots are untouched.
                log::warn!("QRAB_SNP_SECRETS_ERR {e:?} — halting (fail-closed)");
                halt_forever();
            }
        }
    } else {
        None
    };

    let vsock_result = run_vsock_demo(features, snp_page);
    let verify_ok = run_verify_demo().is_ok();

    match (vsock_result, verify_ok) {
        (Ok(session), true) => {
            log::info!("QRAB_PHASE4_OK");

            // Phase 5 StartImage: publish the `QrabReport` EFI runtime
            // variable so post-boot userspace can surface last-boot
            // attestation state, then hand the UKI off to the
            // firmware via LoadImage + StartImage. Default builds
            // stop at the stub-skip branch inside `load_and_start`;
            // `QRAB_UKI_BOOT=1` at build time opens the real path.
            match uki::publish_report(&session.report_data) {
                Ok(()) => log::info!(
                    "QRAB_REPORT_OK len={}",
                    uki::QRAB_REPORT_MAGIC.len() + session.report_data.len()
                ),
                Err(e) => {
                    // Publishing the report is best-effort: a failure
                    // here does not invalidate the cryptographic
                    // chain the bootloader already completed, so we
                    // log and continue into StartImage. The smoke
                    // harness can still detect regressions via the
                    // absence of `QRAB_REPORT_OK`.
                    log::warn!("QRAB_REPORT_ERR {e}");
                }
            }

            uki::load_and_start(&session.uki_bytes);
        }
        (vsock_result, verify_ok) => {
            log::warn!(
                "QRAB_PHASE4_PARTIAL vsock_ok={} verify_ok={}",
                vsock_result.is_ok(),
                verify_ok,
            );
            // Intentionally do not return control to firmware. Per
            // plan: fail-closed, hang on any terminal state
            // including "phase not complete".
            halt_forever()
        }
    }
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
fn run_verify_demo() -> Result<(), ()> {
    match verify_detached(UKI_VALID, UKI_VALID_SIG, DB_PUBKEY) {
        Ok(()) => log::info!("QRAB_VERIFY_VALID_OK"),
        Err(e) => panic!("valid fixture failed to verify: {e}"),
    }

    match verify_detached(UKI_TAMPERED, UKI_VALID_SIG, DB_PUBKEY) {
        Err(VerifyError::BadSignature) => log::info!("QRAB_VERIFY_TAMPERED_REJECTED"),
        Ok(()) => panic!("tampered fixture verified — verifier is broken"),
        Err(e) => panic!("tampered fixture wrong error: {e}"),
    }

    Ok(())
}

#[cfg(qrab_no_fixtures)]
fn run_verify_demo() -> Result<(), ()> {
    log::info!("QRAB_VERIFY_FIXTURES_ABSENT: run contrib/build-test-ukis.sh and rebuild to exercise qrab-boot-verify at boot");
    // Fixture-less builds are a build-time choice, not a runtime failure
    // — return Ok so `QRAB_PHASE4_OK` can still gate on the vsock leg.
    Ok(())
}

/// Default vsock port the host attestation mock listens on. Matches
/// `contrib/mock-vsock-server.py` + `contrib/smoke-vsock-qemu.sh`.
const VSOCK_HOST_PORT: u32 = 1337;

/// Source-side port used when opening the attestation stream. Picked
/// from the ephemeral-ish high range; the host side does not care.
const VSOCK_GUEST_SRC_PORT: u32 = 40042;

/// Host CID for vhost-vsock, per the virtio-vsock spec.
const VSOCK_CID_HOST: u64 = 2;

/// Bound state carried out of [`run_vsock_demo`] for the phase-5
/// StartImage tail in `efi_main`. The two fields are whatever the
/// terminal `publish_report` + `load_and_start` calls need; keeping
/// them in a struct keeps the signature stable when future phases
/// add more (e.g. the full 1184-byte SNP attestation report body).
pub struct VsockSession {
    /// 64-byte SNP `report_data` binding client nonce, server nonce,
    /// UKI hash, and secret hash — see
    /// [`qrab_boot_proto::report_data`]. The SHA-256 digest occupies
    /// the first 32 bytes; the remaining 32 are zero-padding.
    pub report_data: [u8; 64],
    /// UKI bytes fetched from the attestation server. Already
    /// SHA-256-matched against the signed `uki_sha256` field.
    pub uki_bytes: alloc::vec::Vec<u8>,
}

/// Boot-time driver for the 3a→3b.3→3b.4 chain:
///
///   probe → init driver → connect to host attestation endpoint.
///
/// On success returns a [`VsockSession`] with the fields the phase-5
/// StartImage tail needs. On any failure logs a distinct `QRAB_*`
/// token and returns `Err(())`; the caller decides whether that
/// failure is terminal (phase-4 happy path is required for phase-5).
fn run_vsock_demo(
    features: ghcb::SevFeatures,
    snp_page: Option<qrab_snp_proto::SnpSecretsPage>,
) -> Result<VsockSession, ()> {
    let status = vsock::probe();
    let mut driver = match vsock_driver::init(status) {
        Ok(d) => d,
        Err(_) => return Err(()), // init logged its own QRAB_* token.
    };
    if vsock_connect::connect_and_wait(
        &mut driver,
        VSOCK_CID_HOST,
        VSOCK_HOST_PORT,
        VSOCK_GUEST_SRC_PORT,
    )
    .is_err()
    {
        return Err(()); // connect_and_wait logged the specific failure.
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
            return Err(());
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
            return Err(());
        }
    };

    if vsock_connect::send_one(
        &mut driver,
        VSOCK_CID_HOST,
        VSOCK_HOST_PORT,
        VSOCK_GUEST_SRC_PORT,
        &request_bytes,
    )
    .is_err()
    {
        return Err(()); // send_one logged the specific failure.
    }

    let mut stream = vsock_connect::FramedStream::new();
    let response_frame = match stream.recv_frame(
        &mut driver,
        VSOCK_CID_HOST,
        VSOCK_HOST_PORT,
        VSOCK_GUEST_SRC_PORT,
    ) {
        Ok(b) => b,
        Err(_) => return Err(()), // recv_frame logged the specific failure.
    };

    // The AttestResponse frame's body is straight postcard — skip the
    // outer 4-byte framing (already stripped by FramedStream) and
    // just decode.
    let response: qrab_boot_proto::AttestResponse =
        match qrab_boot_proto::decode(&response_frame) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("QRAB_ATTEST_DECODE_ERR {e}");
                return Err(());
            }
        };
    if let Err(e) = qrab_boot_proto::validate_response(&response) {
        log::warn!("QRAB_ATTEST_VALIDATE_ERR {e}");
        return Err(());
    }

    // Verify the server's signature before *anything* downstream is
    // allowed to consume the response. An unverified attestation
    // response is strictly more dangerous than no response at all,
    // because the bootloader would otherwise inject an
    // attacker-chosen secret into the guest kernel.
    if !attest::has_embedded_pubkey() {
        // R16-H15: reachable only via explicit `QRAB_DEV_NO_PUBKEY=1`
        // at build time. Shout about the unverified response so this
        // mode is unmistakable in serial logs.
        log::warn!(
            "QRAB_ATTEST_NO_PUBKEY (QRAB_DEV_NO_PUBKEY=1 build; signature verification SKIPPED — do not ship)"
        );
    } else {
        match attest::verify(&response, &client_nonce) {
            Ok(()) => log::info!("QRAB_ATTEST_SIG_OK"),
            Err(attest::VerifyError::NoPubkey) => {
                log::warn!("QRAB_ATTEST_NO_PUBKEY");
                return Err(());
            }
            Err(attest::VerifyError::MalformedPubkey) => {
                log::warn!("QRAB_ATTEST_MALFORMED_PUBKEY");
                return Err(());
            }
            Err(attest::VerifyError::MalformedSig) => {
                log::warn!("QRAB_ATTEST_MALFORMED_SIG");
                return Err(());
            }
            Err(attest::VerifyError::BadSignature) => {
                log::warn!("QRAB_ATTEST_BAD_SIG");
                return Err(());
            }
            Err(attest::VerifyError::SecretHashMismatch) => {
                log::warn!("QRAB_ATTEST_SECRET_HASH_MISMATCH");
                return Err(());
            }
        }
    }

    // Compute the SNP `report_data` and, under SEV-SNP, fire off the
    // full `SNP_GUEST_REQUEST MSG_REPORT_REQ` round-trip to the PSP
    // via VMGEXIT. On plain-TCG we still log the intended value so a
    // post-boot verifier can confirm the computation is right.
    let report_data = qrab_boot_proto::report_data(&client_nonce, &response);
    ghcb::record_intended_report_data(&report_data);

    // F2: optional post-report frame the guest sends back to
    // attest-server once the SNP report is obtained + UKI is
    // received. Built inside the SNP branch, sent after the UKI
    // recv so it does not interleave with the server's pending
    // UKI write.
    let mut post_report_frame: Option<alloc::vec::Vec<u8>> = None;

    if features.sev_snp
        && let Some(page) = snp_page.as_ref()
    {
        {
                // A14: initial seqno = Secrets.msg_seqno_0 + 1. On a
                // fresh SNP_LAUNCH that field is zero so we send 1 —
                // same as the hard-coded value we used before — but
                // a kexec or chain-loaded guest that updated the
                // field before handing off to us gets the correct
                // next-seqno without re-coding.
                let initial_seqno = (page.msg_seqno_0 as u64).wrapping_add(1);
                // A02: use the extended guest-request so the
                // hypervisor also hands us the VCEK / ASK / ARK
                // chain, and we can log + downstream-verify the
                // full trust path in one round-trip.
                match snp_secrets::request_report_with_certs(
                    &page.vmpck0,
                    initial_seqno,
                    report_data,
                ) {
                    Ok((report, cert_bytes)) => {
                        // Copy out of the #[repr(C, packed)] struct so the
                        // 2024-edition `unaligned_references` rule is happy.
                        let m = { report.measurement };
                        let version = { report.version };
                        let vmpl = { report.vmpl };
                        let policy = { report.policy };
                        log::info!(
                            "QRAB_SNP_REPORT_OK measurement_prefix={:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x} \
                             version={} vmpl={} policy={:#x} seqno={}",
                            m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7],
                            version, vmpl, policy, initial_seqno,
                        );

                        // Parse the cert-table blob. A non-fatal
                        // parse failure still gets the PSP report
                        // logged above — the attest chain itself
                        // is cryptographically bound, and the
                        // cert chain is a convenience for host-side
                        // verification (A03). Log fields extracted
                        // from the scan so we don't have to hold a
                        // borrow of `cert_bytes` past the
                        // `new_post_report` consume below (B12-11:
                        // avoid an up-to-64 KiB clone).
                        let cert_blob_len = cert_bytes.len();
                        match qrab_snp_proto::cert_table::parse_cert_table(&cert_bytes) {
                            Ok(entries) => {
                                let mut vcek_der_len = 0usize;
                                let mut vlek_der_len = 0usize;
                                for e in &entries {
                                    use qrab_snp_proto::cert_table::CertKind;
                                    match e.kind {
                                        CertKind::Vcek => vcek_der_len = e.der.len(),
                                        CertKind::Vlek => vlek_der_len = e.der.len(),
                                        _ => {}
                                    }
                                }
                                log::info!(
                                    "QRAB_SNP_CERT_CHAIN entries={} vcek_der_len={} vlek_der_len={} blob_len={}",
                                    entries.len(),
                                    vcek_der_len,
                                    vlek_der_len,
                                    cert_blob_len,
                                );
                            }
                            Err(e) => log::warn!("QRAB_SNP_CERT_PARSE_ERR {e}"),
                        }

                        // F2: assemble AttestPostReport frame for
                        // the host attest-server. `report` is
                        // `#[repr(C, packed)]` with `FromBytes +
                        // IntoBytes` derives; turn it into a Vec
                        // via the zerocopy IntoBytes impl. The
                        // cert_bytes blob is consumed verbatim
                        // (B12-11: no clone — peak cert_blob is
                        // ~64 KiB on extended guest-request).
                        use zerocopy::IntoBytes as _;
                        let report_vec: alloc::vec::Vec<u8> =
                            report.as_bytes().to_vec();
                        let post = qrab_boot_proto::new_post_report(
                            report_vec,
                            cert_bytes,
                        );
                        match qrab_boot_proto::encode_framed(&post) {
                            Ok(bytes) => post_report_frame = Some(bytes),
                            Err(e) => log::warn!("QRAB_POST_REPORT_ENCODE_ERR {e}"),
                        }
                    }
                    Err(e) => log::warn!("QRAB_SNP_REPORT_ERR {e:?}"),
                }
        }
    }

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
            return Err(());
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
        return Err(());
    }
    log::info!(
        "QRAB_UKI_OK len={} sha256_prefix={:02x}{:02x}{:02x}{:02x}",
        uki_bytes.len(),
        actual_uki_hash[0], actual_uki_hash[1],
        actual_uki_hash[2], actual_uki_hash[3],
    );

    // F2 / R16-H22: send the AttestPostReport frame and wait for
    // the server's AttestRelease (carrying the actual secret body).
    // The boot decision is now gated on receiving + verifying that
    // release — before v3 the secret rode inside the earlier
    // AttestResponse, which meant a malicious guest could skip
    // sending the post-report and still leave with a valid secret.
    // Under v3 the server holds the secret back until chain-walk OK,
    // so no post-report frame = no secret = boot halts.
    let release_secret: alloc::vec::Vec<u8> = if let Some(frame) = post_report_frame.as_ref() {
        match vsock_connect::send_one(
            &mut driver,
            VSOCK_CID_HOST,
            VSOCK_HOST_PORT,
            VSOCK_GUEST_SRC_PORT,
            frame,
        ) {
            Ok(()) => log::info!(
                "QRAB_POST_REPORT_OK frame_len={}",
                frame.len(),
            ),
            Err(e) => {
                log::warn!("QRAB_POST_REPORT_SEND_ERR {e:?}");
                return Err(());
            }
        }

        let release_frame = match stream.recv_frame(
            &mut driver,
            VSOCK_CID_HOST,
            VSOCK_HOST_PORT,
            VSOCK_GUEST_SRC_PORT,
        ) {
            Ok(b) => b,
            Err(_) => {
                // Server closed without sending AttestRelease —
                // either chain walk failed or the protocol was
                // cut short. Fail closed: no secret published.
                log::warn!("QRAB_ATTEST_RELEASE_TIMEOUT");
                return Err(());
            }
        };
        let release: qrab_boot_proto::AttestRelease =
            match qrab_boot_proto::decode(&release_frame) {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("QRAB_ATTEST_RELEASE_DECODE_ERR {e}");
                    return Err(());
                }
            };
        if let Err(e) = qrab_boot_proto::validate_release(&release) {
            log::warn!("QRAB_ATTEST_RELEASE_VALIDATE_ERR {e}");
            return Err(());
        }
        match attest::verify_release(
            &release,
            &client_nonce,
            &response.server_nonce,
            &response.secret_sha256,
        ) {
            Ok(()) => log::info!("QRAB_ATTEST_RELEASE_OK len={}", release.secret.len()),
            Err(e) => {
                log::warn!("QRAB_ATTEST_RELEASE_SIG_ERR {e}");
                return Err(());
            }
        }
        release.secret
    } else {
        log::warn!("QRAB_POST_REPORT_SKIP no_snp_or_no_report — cannot request AttestRelease");
        return Err(());
    };

    // Measurement + publish now run AFTER release verify so nothing
    // touches the secret until the server has chain-walked this
    // guest's post-report.
    match measure::extend_uki(&uki_bytes) {
        Ok(()) => {}
        Err(measure::MeasureError::Tcg2Absent) => {
            log::info!("QRAB_TCG2_ABSENT");
        }
        Err(e) => {
            log::warn!("QRAB_PCR8_ERR {e}");
            panic!("PCR 8 extend failed with TCG2 present: {e}");
        }
    }

    match measure::extend_secret_binding(&response.secret_sha256, &response.server_nonce) {
        Ok(()) => {}
        Err(measure::MeasureError::Tcg2Absent) => {
            log::info!("QRAB_TCG2_ABSENT");
        }
        Err(e) => {
            log::warn!("QRAB_PCR9_ERR {e}");
            panic!("PCR 9 extend failed with TCG2 present: {e}");
        }
    }

    // Publish the verified release secret as an EFI runtime variable
    // (volatile, BS+RT). The initrd dracut module at
    // contrib/qrab-boot-initrd-module/ will read it out and wipe it
    // during early userspace.
    match secret::publish(&release_secret) {
        Ok(()) => log::info!("QRAB_SECRET_OK len={}", release_secret.len()),
        Err(e) => {
            log::warn!("QRAB_SECRET_ERR {e}");
            return Err(());
        }
    }

    log::info!(
        "QRAB_ATTEST_OK server_nonce0={:#04x} secret_len={} sig_len={} uki_url_len={}",
        response.server_nonce[0],
        release_secret.len(),
        response.sig.len(),
        response.uki_url.len(),
    );

    Ok(VsockSession {
        report_data,
        uki_bytes,
    })
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
