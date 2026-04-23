#!/usr/bin/env bash
# qrab-boot initrd hook: copy the PUBLIC efivars QrabReport +
# QrabSession to /run/qrab/{report,session} so qrab-guest-init
# (running as PID 1 post-pivot) can read them from userspace.
#
# Unlike the secret hook, these blobs are NOT sensitive:
#   - QrabReport  is the PSP-signed SnpAttestationReport (public once
#                 released on the vsock wire).
#   - QrabSession is `client_nonce || server_nonce` (both broadcast
#                 in the AttestRequest / AttestResponse frames).
# They matter for L5 integrity + freshness, not confidentiality.
# Published by projects/qrab-boot/src/uki.rs on Phase-4 success.
#
# The hook also does NOT wipe these variables (unlike the secret
# hook). The Phase-4 invariants don't require their deletion —
# they're reproducible from the on-disk /run copies, and keeping
# them in efivarfs lets post-boot debugging read the same data
# without replaying the session. The secret hook owns the
# secret-wipe responsibility.
#
# Runs at the dracut `pre-pivot` stage AFTER qrab-read-secret.sh
# (priority 71 vs 70) so if the secret hook fails-closed, we don't
# bother copying the public vars for a boot that will halt anyway.
#
# Failure modes:
#   * no efivarfs   → log QRAB_INITRD_PUB_NO_EFIVARFS + exit 0
#                     (dev hosts, non-UEFI boots).
#   * no variable   → log QRAB_INITRD_PUB_ABSENT_<name> + exit 0
#                     (the public vars are "best effort" — a partial
#                     drop still lets qrab-guest-init emit a distinct
#                     failure token downstream).
#   * write failure → log QRAB_INITRD_PUB_WRITE_ERR_<name> + exit 1.

set -eu

# Keep the GUID in lockstep with projects/qrab-boot/src/secret.rs::QRAB_SECRET_GUID
# (shared across QrabSecret / QrabReport / QrabSession).
QRAB_GUID="6e6f1cba-b5ba-4e54-a3b3-2a7a8f3d9b00"

# Overridable for the host-side simulate.sh harness.
EFIVARFS="${QRAB_INITRD_EFIVARFS:-/sys/firmware/efi/efivars}"
RUN_DIR="${QRAB_INITRD_RUN_DIR:-/run/qrab}"

log() {
    echo "qrab-boot: $*" >&2
}

if [ ! -d "$EFIVARFS" ]; then
    log "QRAB_INITRD_PUB_NO_EFIVARFS ($EFIVARFS absent)"
    exit 0
fi

mkdir -p "$RUN_DIR"
chmod 0700 "$RUN_DIR"

# Copy one efivar into /run/qrab/<dst>, stripping the 4-byte attrs
# prefix. Caller supplies the efivar's name + the /run filename.
# Mode 0644 (public readability) — these blobs are not secret.
copy_var() {
    local var_name="$1"
    local dst_name="$2"
    local var_file="$EFIVARFS/${var_name}-${QRAB_GUID}"
    local dst_path="$RUN_DIR/$dst_name"

    if [ ! -f "$var_file" ]; then
        log "QRAB_INITRD_PUB_ABSENT_${var_name} ($var_file)"
        return 0
    fi

    local tmp
    tmp="$(mktemp --tmpdir="$RUN_DIR" "${dst_name}.XXXXXX")"
    chmod 0644 "$tmp"

    if ! dd if="$var_file" bs=1 skip=4 of="$tmp" status=none 2>/dev/null; then
        rm -f "$tmp"
        log "QRAB_INITRD_PUB_READ_ERR_${var_name}"
        return 1
    fi

    if ! mv "$tmp" "$dst_path"; then
        rm -f "$tmp"
        log "QRAB_INITRD_PUB_WRITE_ERR_${var_name}"
        return 1
    fi
    chmod 0644 "$dst_path"

    local len
    len="$(wc -c <"$dst_path" 2>/dev/null || echo 0)"
    log "QRAB_INITRD_PUB_OK_${var_name} len=$len"
}

# QrabReport: MAGIC || SnpAttestationReport (or MAGIC || report_data
# on plain-TCG). Consumed by qrab-guest-init's report::parse.
copy_var QrabReport report

# QrabSession: client_nonce[32] || server_nonce[32] (raw 64 bytes).
# Consumed by qrab-guest-init's policy::parse_session.
copy_var QrabSession session

log "QRAB_INITRD_PUB_DONE"
