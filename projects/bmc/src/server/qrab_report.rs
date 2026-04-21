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
//! can exercise it without any QGA plumbing. The absent-vs-failed
//! classification for a `guest-file-open` error is factored into
//! [`classify_open_error_desc`] for the same reason — the QGA wire
//! error carries only `{class, desc}` (no numeric errno), so we must
//! decide from `desc`, which is a locale-sensitive `strerror(errno)`
//! suffix on top of QEMU's stable `"failed to open file '...': "`
//! prefix.

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

/// Cap on how long `fetch_qrab_report` will wait for QGA to accept a
/// new connection. A live agent responds inside a few tens of ms;
/// three seconds covers a slow ARM-under-KVM target without letting
/// a wedged agent wedge the server fn.
const QGA_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Per-op timeout for QGA `guest-file-open` / `guest-file-read` /
/// `guest-file-close`. Larger than the connect budget because a
/// real `guest-file-read` on efivarfs takes a syscall round-trip
/// inside the guest kernel and we'd rather absorb transient kernel
/// stalls than fail the whole server fn.
const QGA_OP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Classification of a QGA `guest-file-open` error's `desc` string.
///
/// The wire-level QGA error object is `{class: "GenericError", desc:
/// "<msg>"}`. `desc` is the `error_setg_errno` output from
/// `qga/commands-posix.c::safe_open_or_create`, which formats as:
///
/// ```text
/// failed to open file '<path>' (mode: '<mode>'): <strerror(errno)>
/// ```
///
/// Only `strerror(errno)` is locale-sensitive; everything before the
/// final `": "` is literal C source. We therefore match against
/// strerror translations for `ENOENT` from the locales most likely to
/// appear on a qrab guest (systemd unit with LC_ALL=C, en_US.UTF-8,
/// plus a handful of other common system locales) and fall back to
/// [`OpenErrorClass::Unknown`] when none match — the caller then
/// probes the guest via `guest-exec` for an authoritative answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenErrorClass {
    /// Desc matches a known `ENOENT` strerror translation.
    Absent,
    /// Desc does not match any known ENOENT pattern; caller should
    /// probe the guest to disambiguate.
    Unknown,
}

/// Classify the `desc` field of a QGA `guest-file-open` error.
///
/// Substring-matches against known `strerror(ENOENT)` translations
/// from `glibc`'s common locales. A miss returns
/// [`OpenErrorClass::Unknown`] so the caller can fall back to a
/// locale-independent probe (see `probe_path_exists`).
///
/// Cases covered:
/// - English / C / POSIX: `"No such file or directory"`
/// - German: `"Datei oder Verzeichnis nicht gefunden"`
/// - French: `"Aucun fichier ou dossier de ce nom"`
/// - Spanish: `"No existe el archivo o el directorio"`
/// - Italian: `"File o directory non esistente"`
/// - Portuguese: `"Arquivo ou diretório inexistente"`
/// - Russian: `"Нет такого файла или каталога"`
/// - Japanese: `"そのようなファイルやディレクトリはありません"`
/// - Simplified Chinese: `"没有那个文件或目录"`
/// - Korean: `"그런 파일이나 디렉터리가 없습니다"`
///
/// Unicode substrings are matched as-is: `String::contains(&str)`
/// operates on byte sequences, which is correct for UTF-8 input.
#[must_use]
pub fn classify_open_error_desc(desc: &str) -> OpenErrorClass {
    // English / C / POSIX — the overwhelmingly common case on a
    // systemd-managed guest (LANG=C or en_US.UTF-8).
    const ENOENT_PATTERNS: &[&str] = &[
        "No such file or directory",
        // German
        "Datei oder Verzeichnis nicht gefunden",
        // French
        "Aucun fichier ou dossier de ce nom",
        // Spanish
        "No existe el archivo o el directorio",
        // Italian
        "File o directory non esistente",
        // Portuguese
        "Arquivo ou diretório inexistente",
        // Russian
        "Нет такого файла или каталога",
        // Japanese
        "そのようなファイルやディレクトリはありません",
        // Simplified Chinese
        "没有那个文件或目录",
        // Korean
        "그런 파일이나 디렉터리가 없습니다",
    ];
    for pat in ENOENT_PATTERNS {
        if desc.contains(pat) {
            return OpenErrorClass::Absent;
        }
    }
    OpenErrorClass::Unknown
}

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
/// Wire path: BMC opens the QGA Unix socket plumbed by `VmBuilder::qga()`
/// (see `projects/manager/src/vm.rs`), issues `guest-file-open` on the
/// efivarfs path, reads up to [`QRAB_REPORT_READ_MAX`] bytes via
/// `guest-file-read`, base64-decodes the `buf-b64` field, then hands the
/// raw efivarfs blob to [`parse_qrab_report`]. The file handle is closed
/// best-effort — a failure there does not mask the parse result, since
/// the guest agent will reap on disconnect anyway.
#[server]
pub async fn fetch_qrab_report() -> Result<QrabReportStatus, ServerFnError> {
    use base64::Engine;
    use crate::state::{bmc, QgaSockProbe};

    let state = bmc();

    // B13-3: single atomic probe. Previously we did an "is Running?"
    // lock-release followed by a "get qga_sock?" lock-release, which
    // raced with shutdown and mis-reported a stale VM as
    // "qga_sock not on VmProcess". The probe holds the lock across
    // both checks and is released before any `.await` below so UI
    // polls (status, CPU info) are not blocked by a wedged agent.
    let qga_sock = match state.qga_sock_probe().await {
        QgaSockProbe::NotRunning => return Ok(QrabReportStatus::Absent),
        QgaSockProbe::NoQgaSocket => {
            return Ok(QrabReportStatus::Failed {
                reason: "qga_sock not on VmProcess".into(),
            });
        }
        QgaSockProbe::Ok(p) => p,
    };

    // B13-4: every QGA call needs a timeout. A wedged guest agent
    // (kernel stuck in a syscall, qrab-init panic, cloud-init
    // crashloop) would otherwise leave the server fn awaiting
    // indefinitely and wedge every subsequent BMC request that
    // routes through the same Tokio task / handler.
    let mut qga = match tokio::time::timeout(
        QGA_CONNECT_TIMEOUT,
        qapi_protocol::qga::Qga::new(&qga_sock),
    )
    .await
    {
        Ok(Ok(q)) => q,
        Ok(Err(e)) => {
            return Ok(QrabReportStatus::Failed {
                reason: format!("QGA connect failed: {e}"),
            });
        }
        Err(_) => {
            return Ok(QrabReportStatus::Failed {
                reason: format!(
                    "QGA connect timed out after {}s",
                    QGA_CONNECT_TIMEOUT.as_secs()
                ),
            });
        }
    };

    // `guest-file-open` in read mode.
    let handle: i64 = match tokio::time::timeout(
        QGA_OP_TIMEOUT,
        qga.execute(&qapi::qga::commands::GuestFileOpen {
            path: QRAB_REPORT_EFIVAR_PATH.into(),
            mode: Some("r".into()),
        }),
    )
    .await
    {
        Ok(Ok(h)) => h,
        Ok(Err(e)) => {
            // B14-4: the QGA wire error is only `{class, desc}` — no
            // numeric errno — so we can't distinguish ENOENT from
            // EACCES/EIO directly. Try a locale-aware substring match
            // first (covers the LANG=C / en_US / common-locale case
            // cheaply); on miss, probe the guest with
            // `guest-exec test -e` for an authoritative, locale-free
            // answer. Anything inconclusive surfaces as `Failed` so
            // operators don't silently confuse absent-file with a
            // real agent/kernel failure.
            let msg = e.to_string();
            match classify_open_error_desc(&msg) {
                OpenErrorClass::Absent => return Ok(QrabReportStatus::Absent),
                OpenErrorClass::Unknown => match probe_path_exists(
                    &mut qga,
                    QRAB_REPORT_EFIVAR_PATH,
                )
                .await
                {
                    PathProbe::Absent => return Ok(QrabReportStatus::Absent),
                    PathProbe::Present | PathProbe::Inconclusive => {
                        return Ok(QrabReportStatus::Failed {
                            reason: format!("guest-file-open: {msg}"),
                        });
                    }
                },
            }
        }
        Err(_) => {
            return Ok(QrabReportStatus::Failed {
                reason: format!(
                    "guest-file-open timed out after {}s",
                    QGA_OP_TIMEOUT.as_secs()
                ),
            });
        }
    };

    // `guest-file-read` — typed return in qapi::qga aliases to the
    // request struct (`{count, handle}`), not the response body. Use
    // `execute_raw` with a local struct that matches the actual wire
    // shape `{count, buf-b64, eof}`.
    #[derive(serde::Deserialize)]
    struct ReadReturn {
        #[serde(rename = "buf-b64")]
        buf_b64: String,
    }
    let req = serde_json::json!({
        "handle": handle,
        "count": QRAB_REPORT_READ_MAX,
    });
    let read_result: Result<Result<ReadReturn, _>, _> = tokio::time::timeout(
        QGA_OP_TIMEOUT,
        qga.execute_raw("guest-file-read", req),
    )
    .await;

    // Close the handle best-effort before returning — the agent reaps
    // on disconnect, but explicit close keeps per-call fd usage tight
    // and avoids surprising the operator with agent-side warnings.
    let _ = tokio::time::timeout(
        QGA_OP_TIMEOUT,
        qga.execute(&qapi::qga::commands::GuestFileClose { handle }),
    )
    .await;

    let read = match read_result {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            return Ok(QrabReportStatus::Failed {
                reason: format!("guest-file-read: {e}"),
            });
        }
        Err(_) => {
            return Ok(QrabReportStatus::Failed {
                reason: format!(
                    "guest-file-read timed out after {}s",
                    QGA_OP_TIMEOUT.as_secs()
                ),
            });
        }
    };

    let bytes = match base64::engine::general_purpose::STANDARD.decode(&read.buf_b64) {
        Ok(b) => b,
        Err(e) => {
            return Ok(QrabReportStatus::Failed {
                reason: format!("base64 decode: {e}"),
            });
        }
    };

    Ok(parse_qrab_report(&bytes))
}

/// Outcome of a `guest-exec`-based path-existence probe.
///
/// Used by [`fetch_qrab_report`] to disambiguate a
/// `guest-file-open` failure when [`classify_open_error_desc`] returns
/// [`OpenErrorClass::Unknown`] (e.g. a non-en guest locale produced a
/// `strerror(errno)` string we don't recognise).
#[cfg(feature = "server")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathProbe {
    /// `test -e` returned exit code 0: the path exists.
    Present,
    /// `test -e` returned non-zero: the path does not exist.
    Absent,
    /// Probe could not be carried out (QGA error, missing `test`
    /// binary, timeout, etc.). Caller should default to `Failed`.
    Inconclusive,
}

/// Total wall-time budget for the `guest-exec` + `guest-exec-status`
/// probe. Kept tight: the probe only runs on the error path, but a
/// wedged guest agent must not tie up the BMC task longer than a few
/// operator-visible seconds.
#[cfg(feature = "server")]
const QGA_PROBE_BUDGET: std::time::Duration = std::time::Duration::from_secs(3);

/// How often to re-check `guest-exec-status` for a reaped pid. `test -e`
/// is a fast execve + single `stat(2)` so in practice the first poll
/// almost always sees `exited: true`.
#[cfg(feature = "server")]
const QGA_PROBE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

/// Probe whether `path` exists inside the guest, locale-free.
///
/// Spawns `/usr/bin/test -e <path>` via `guest-exec`, then polls
/// `guest-exec-status` until the pid has been reaped (or the budget
/// expires). Exit code 0 → present; non-zero → absent; any wire/
/// timeout error → [`PathProbe::Inconclusive`].
///
/// This is the locale-independent counterpart to
/// [`classify_open_error_desc`]: the caller tries the cheap substring
/// match first and only falls back to this probe when the match is
/// inconclusive, so a standard LANG=C guest pays no extra RTT.
#[cfg(feature = "server")]
async fn probe_path_exists(qga: &mut qapi_protocol::qga::Qga, path: &str) -> PathProbe {
    let deadline = tokio::time::Instant::now() + QGA_PROBE_BUDGET;

    // Kick off `test -e <path>`. We use the GNU coreutils path first —
    // any Linux distro with a userland has either `/usr/bin/test` or
    // `/bin/test`, and modern systemd-managed distros usermerge them.
    // Capture is `none` so QGA doesn't allocate buffers for a silent
    // exit-code-only check.
    let exec_req = serde_json::json!({
        "path": "/usr/bin/test",
        "arg": ["-e", path],
        "capture-output": { "mode": "none" },
    });
    #[derive(serde::Deserialize)]
    struct ExecReturn {
        pid: i64,
    }
    let Some(remaining) = deadline.checked_duration_since(tokio::time::Instant::now()) else {
        return PathProbe::Inconclusive;
    };
    let pid: i64 = match tokio::time::timeout(
        remaining,
        qga.execute_raw::<ExecReturn>("guest-exec", exec_req),
    )
    .await
    {
        Ok(Ok(r)) => r.pid,
        // Any error (binary not found, QGA wire fault, timeout) → we
        // can't make an authoritative statement. Surface Inconclusive
        // so the caller reports the original open error rather than
        // mis-labelling as Absent.
        _ => return PathProbe::Inconclusive,
    };

    // Poll `guest-exec-status` until `exited: true` or the budget
    // runs out. `test -e` is a syscall away from a stat(2) so the
    // first poll almost always sees the reaped status.
    #[derive(serde::Deserialize)]
    struct StatusReturn {
        exited: bool,
        #[serde(rename = "exitcode")]
        exit_code: Option<i64>,
    }
    loop {
        let Some(remaining) = deadline.checked_duration_since(tokio::time::Instant::now()) else {
            return PathProbe::Inconclusive;
        };
        let status_req = serde_json::json!({ "pid": pid });
        let status = match tokio::time::timeout(
            remaining,
            qga.execute_raw::<StatusReturn>("guest-exec-status", status_req),
        )
        .await
        {
            Ok(Ok(s)) => s,
            _ => return PathProbe::Inconclusive,
        };

        if status.exited {
            return match status.exit_code {
                Some(0) => PathProbe::Present,
                Some(_) => PathProbe::Absent,
                // `exited: true` with no `exitcode` means `test` died
                // on a signal — treat as Inconclusive rather than
                // guessing.
                None => PathProbe::Inconclusive,
            };
        }

        // Don't spin: yield for a short interval before the next poll
        // (or until the budget expires, whichever comes first).
        let Some(remaining) = deadline.checked_duration_since(tokio::time::Instant::now()) else {
            return PathProbe::Inconclusive;
        };
        let sleep_for = std::cmp::min(QGA_PROBE_POLL_INTERVAL, remaining);
        tokio::time::sleep(sleep_for).await;
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

    // --- B14-4: locale-aware ENOENT classification ---

    /// QGA error string we want to classify — the underlying
    /// `error_setg_errno` format is `"failed to open file '%s'
    /// (mode: '%s'): <strerror(errno)>"`, and `Qmp::execute` wraps
    /// the whole thing as `"QMP error <class>: <desc>"` before we see
    /// it. The classifier operates on the full rendered string to
    /// match what `fetch_qrab_report` hands in.
    fn qga_msg(strerror: &str) -> String {
        format!(
            "QMP error GenericError: failed to open file \
             '/sys/firmware/efi/efivars/QrabReport-...' (mode: 'r'): {strerror}"
        )
    }

    /// English / C / POSIX is the overwhelmingly common guest locale
    /// (systemd units run with LC_ALL=C or LANG=en_US.UTF-8 by
    /// default). Regression guard for the pre-B14-4 substring.
    #[test]
    fn classify_english_enoent_is_absent() {
        let msg = qga_msg("No such file or directory");
        assert_eq!(classify_open_error_desc(&msg), OpenErrorClass::Absent);
    }

    /// Plain desc (without QMP error framing) must also classify — the
    /// helper is format-agnostic and just substring-matches.
    #[test]
    fn classify_bare_english_enoent_is_absent() {
        let desc = "failed to open file 'x' (mode: 'r'): No such file or directory";
        assert_eq!(classify_open_error_desc(desc), OpenErrorClass::Absent);
    }

    /// Non-English locales must classify too — this is the whole
    /// point of B14-4.
    #[test]
    fn classify_non_en_enoent_is_absent() {
        let cases: &[&str] = &[
            // German (de_DE)
            "Datei oder Verzeichnis nicht gefunden",
            // French (fr_FR)
            "Aucun fichier ou dossier de ce nom",
            // Spanish (es_ES)
            "No existe el archivo o el directorio",
            // Italian (it_IT)
            "File o directory non esistente",
            // Portuguese (pt_BR)
            "Arquivo ou diretório inexistente",
            // Russian (ru_RU)
            "Нет такого файла или каталога",
            // Japanese (ja_JP)
            "そのようなファイルやディレクトリはありません",
            // Simplified Chinese (zh_CN)
            "没有那个文件或目录",
            // Korean (ko_KR)
            "그런 파일이나 디렉터리가 없습니다",
        ];
        for strerror in cases {
            let msg = qga_msg(strerror);
            assert_eq!(
                classify_open_error_desc(&msg),
                OpenErrorClass::Absent,
                "strerror variant did not classify as Absent: {strerror}",
            );
        }
    }

    /// A non-ENOENT errno must NOT classify as Absent — we want the
    /// caller to fall through to the probe (or, if that also fails,
    /// to a plain `Failed`). Otherwise EACCES / EIO / ENOTDIR would
    /// silently look like Absent and confuse operators.
    #[test]
    fn classify_non_enoent_is_unknown() {
        let cases: &[&str] = &[
            "Permission denied",
            "Input/output error",
            "Is a directory",
            "Too many open files",
            "Not a directory",
        ];
        for strerror in cases {
            let msg = qga_msg(strerror);
            assert_eq!(
                classify_open_error_desc(&msg),
                OpenErrorClass::Unknown,
                "non-ENOENT strerror mis-classified: {strerror}",
            );
        }
    }

    /// Empty / noise input must not panic and must classify as
    /// Unknown so the caller probes authoritatively.
    #[test]
    fn classify_empty_or_noise_is_unknown() {
        assert_eq!(classify_open_error_desc(""), OpenErrorClass::Unknown);
        assert_eq!(
            classify_open_error_desc("some unrelated garbage"),
            OpenErrorClass::Unknown,
        );
    }
}
