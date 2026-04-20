//! Read-only probe of the guest's `QrabReport` EFI variable.
//!
//! The qrab-boot UKI stub (`projects/qrab-boot/src/uki.rs`) publishes an
//! EFI variable named `QrabReport` under GUID
//! `6e6f1cba-b5ba-4e54-a3b3-2a7a8f3d9b00` with attrs `BS|RT` (volatile,
//! per-boot). The BMC exposes this variable via QGA `guest-file-read`
//! on the guest's efivarfs mount so the attestation panel can confirm
//! the bootloader reached the measurement-publish stage.
//!
//! efivarfs layout: first 4 bytes are the EFI attributes (LE u32),
//! followed by the variable body. qrab-boot writes the body as
//! `"QRAB_REPORT_V1"` (14 bytes ASCII magic) + payload. Payload today
//! is the 32-byte `report_data` hash; the plan reserves room for the
//! full 1184-byte signed SNP attestation report.
//!
//! The parse step is factored into [`parse_qrab_report`] so unit tests
//! can exercise it without any QGA plumbing.

use dioxus::prelude::*;

use crate::types::{QrabReportSnapshot, QrabReportStatus};

/// qrab-boot magic prefix for the `QrabReport` EFI variable body.
///
/// 14 bytes ASCII. Kept in sync with `QRAB_REPORT_MAGIC` in qrab-boot's
/// `projects/qrab-boot/src/uki.rs`.
pub const QRAB_REPORT_MAGIC: &[u8] = b"QRAB_REPORT_V1";

/// efivarfs path of the `QrabReport` variable as seen inside the guest.
pub const QRAB_REPORT_EFIVAR_PATH: &str =
    "/sys/firmware/efi/efivars/QrabReport-6e6f1cba-b5ba-4e54-a3b3-2a7a8f3d9b00";

/// Upper bound on how much we ask QGA to read. Comfortably above the
/// plan's worst case: 4-byte attrs + 14-byte magic + 1184-byte full
/// report = 1202.
pub const QRAB_REPORT_READ_MAX: i64 = 2048;

/// Parse an efivarfs blob into a [`QrabReportStatus`].
///
/// `efivar_bytes` is the raw bytes QGA reads from efivarfs — no base64
/// layer and no truncation. Caller already handled the QGA I/O.
///
/// Layout:
///
/// ```text
/// [ 4 bytes LE u32 attrs | 14 bytes magic | N bytes payload ]
/// ```
///
/// If `efivar_bytes.len() < 18` the blob is too short to carry even an
/// empty payload; we return `Failed { reason: "truncated: <len>B" }`.
/// Otherwise we split off the attribute prefix and the magic; a
/// non-matching magic is recorded on the `Ok` variant rather than
/// becoming a hard failure, since future qrab-boot revisions might
/// change the magic and the UI should still surface raw bytes.
#[must_use]
pub fn parse_qrab_report(efivar_bytes: &[u8]) -> QrabReportStatus {
    const ATTR_LEN: usize = 4;
    const MAGIC_LEN: usize = QRAB_REPORT_MAGIC.len();
    const MIN_LEN: usize = ATTR_LEN + MAGIC_LEN;

    if efivar_bytes.len() < MIN_LEN {
        return QrabReportStatus::Failed {
            reason: format!("truncated: {}B", efivar_bytes.len()),
        };
    }

    let (attrs, body) = efivar_bytes.split_at(ATTR_LEN);
    let magic_matched = body.starts_with(QRAB_REPORT_MAGIC);
    let payload: &[u8] = if magic_matched { &body[MAGIC_LEN..] } else { body };
    let prefix_len = payload.len().min(32);

    QrabReportStatus::Ok(QrabReportSnapshot {
        efi_attrs_hex: encode_hex(attrs),
        magic_matched,
        payload_len: payload.len(),
        payload_prefix_hex: encode_hex(&payload[..prefix_len]),
        fetched_at: fetched_at_now(),
    })
}

/// Hand-rolled lowercase hex encoder. Avoids pulling in `hex` as a
/// dependency for three call sites that each hash <= 32 bytes.
fn encode_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(TABLE[(b >> 4) as usize] as char);
        out.push(TABLE[(b & 0x0f) as usize] as char);
    }
    out
}

/// Wall-clock timestamp string. Matches the low-dep style of
/// `vm_status::chrono_now_iso8601` rather than pulling in chrono.
#[cfg(feature = "server")]
fn fetched_at_now() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}Z")
}

#[cfg(not(feature = "server"))]
fn fetched_at_now() -> String {
    // Client-side parse is deterministic for tests — a stable
    // placeholder is fine.
    String::from("0Z")
}

/// On-demand fetch of the guest's `QrabReport` EFI variable.
///
/// Returns:
/// - `Absent` when the VM is not Running (caller should not see this
///   during normal flow — the UI disables the button when the phase
///   badge is Off), or when the guest has no such variable yet.
/// - `Ok(snapshot)` on a successful parse.
/// - `Failed { reason }` on any other error.
///
/// # Current limitation
///
/// The BMC's `manager::Vm` handle does not currently expose a QGA
/// socket (see `projects/manager/src/vm.rs` — only `qmp_sock`,
/// `serial_log`, `vnc_sock`, `attest_sock` are surfaced). Until
/// `VmBuilder` grows a `.qga()` helper + virtio-serial chardev
/// wiring, this server fn short-circuits to
/// `Failed { reason: "qga socket not plumbed in VmBuilder; see \
/// projects/manager/src/vm.rs" }` whenever the VM is Running.
/// The parse path ([`parse_qrab_report`]) is fully covered by unit
/// tests so the UI wiring can land first and the QGA plumbing can
/// follow in a focused follow-up PR.
#[server]
pub async fn fetch_qrab_report() -> Result<QrabReportStatus, ServerFnError> {
    use crate::state::{bmc, VmState};

    let state = bmc();
    let vm_guard = state.vm.lock().await;

    match &*vm_guard {
        VmState::Running(_) => {
            // Drop the guard before returning — nothing below touches it.
            drop(vm_guard);
            Ok(QrabReportStatus::Failed {
                reason:
                    "qga socket not plumbed in VmBuilder; see projects/manager/src/vm.rs"
                        .into(),
            })
        }
        _ => Ok(QrabReportStatus::Absent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blob shorter than 4 attrs + 14 magic is unusable — the parser
    /// must report it as `Failed` rather than panicking on the slice.
    #[test]
    fn parse_empty_is_truncated() {
        match parse_qrab_report(&[]) {
            QrabReportStatus::Failed { reason } => {
                assert!(reason.starts_with("truncated"), "reason: {reason}");
                assert!(reason.contains("0B"), "reason: {reason}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    /// 17 bytes is one byte short of the 18-byte minimum (4 attrs +
    /// 14 magic). Still truncated.
    #[test]
    fn parse_just_below_minimum_is_truncated() {
        let bytes = [0u8; 17];
        match parse_qrab_report(&bytes) {
            QrabReportStatus::Failed { reason } => {
                assert!(reason.contains("17B"), "reason: {reason}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    /// Happy path: attrs + magic + 32-byte payload. The `report_data`
    /// shape qrab-boot emits today.
    #[test]
    fn parse_magic_with_32_byte_payload() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0x07, 0x00, 0x00, 0x00]); // BS|RT|NV-ish
        bytes.extend_from_slice(QRAB_REPORT_MAGIC);
        bytes.extend_from_slice(&[0xAB; 32]);

        match parse_qrab_report(&bytes) {
            QrabReportStatus::Ok(snap) => {
                assert!(snap.magic_matched);
                assert_eq!(snap.payload_len, 32);
                assert_eq!(snap.efi_attrs_hex, "07000000");
                assert_eq!(snap.payload_prefix_hex, "ab".repeat(32));
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    /// Magic mismatch: the variable exists and is long enough, but the
    /// body does not start with `QRAB_REPORT_V1`. We surface the raw
    /// bytes rather than failing hard so the UI can still show the
    /// operator something meaningful (e.g. a foreign writer clobbered
    /// the var, or qrab-boot bumped the magic).
    #[test]
    fn parse_magic_mismatch_exposes_raw_body() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
        bytes.extend_from_slice(b"NOT_QRAB");
        bytes.extend_from_slice(&[0x00; 32]);

        match parse_qrab_report(&bytes) {
            QrabReportStatus::Ok(snap) => {
                assert!(!snap.magic_matched);
                // body_len == 8 + 32 = 40, all of which is the "payload"
                // when magic didn't match.
                assert_eq!(snap.payload_len, 40);
                // First 32 bytes of the 40-byte body: "NOT_QRAB" + 24
                // zero bytes.
                let expected = {
                    let mut v = Vec::new();
                    v.extend_from_slice(b"NOT_QRAB");
                    v.extend_from_slice(&[0; 24]);
                    encode_hex(&v)
                };
                assert_eq!(snap.payload_prefix_hex, expected);
                assert_eq!(snap.payload_prefix_hex.len(), 64);
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    /// Future shape: attrs + magic + 1184-byte full attestation report.
    /// `payload_len` must report the full 1184 while `payload_prefix_hex`
    /// stays clipped to 32 bytes (= 64 hex chars).
    #[test]
    fn parse_full_1184_byte_report_clips_prefix() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0xff, 0xff, 0xff, 0xff]);
        bytes.extend_from_slice(QRAB_REPORT_MAGIC);
        bytes.extend_from_slice(&vec![0x5a; 1184]);

        match parse_qrab_report(&bytes) {
            QrabReportStatus::Ok(snap) => {
                assert!(snap.magic_matched);
                assert_eq!(snap.payload_len, 1184);
                assert_eq!(snap.payload_prefix_hex, "5a".repeat(32));
                assert_eq!(snap.efi_attrs_hex, "ffffffff");
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    /// `encode_hex` must be lowercase and preserve leading zeros.
    #[test]
    fn encode_hex_lowercase_zeros() {
        assert_eq!(encode_hex(&[0x00]), "00");
        assert_eq!(encode_hex(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
        assert_eq!(encode_hex(&[0x01, 0x0a]), "010a");
    }
}
