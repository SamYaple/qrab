#!/usr/bin/env bash
# qrab-boot initrd hook: read QrabSecret from efivarfs, stash in
# /run/qrab/secret, wipe the variable.
#
# Runs at the dracut `pre-pivot` stage (see `module-setup.sh`), so
# efivarfs is mounted and `/run` exists but userspace has not yet
# taken over.
#
# The vendor GUID + variable name MUST match
# `projects/qrab-boot/src/secret.rs`. Rotating either end requires
# rebuilding both qrab-boot.efi and this initrd module together.
#
# efivarfs format note: each variable in
# `/sys/firmware/efi/efivars/` is a single file whose first four
# bytes are a little-endian `EFI_VARIABLE_*` attributes mask, with
# the rest being the raw variable value. We skip the prefix with
# `dd bs=1 skip=4`.
#
# Failure modes:
#   * no efivarfs    → log + exit 0 (dev hosts, non-UEFI boots).
#   * no variable    → log QRAB_INITRD_ABSENT + exit 0.
#   * zero-length    → log QRAB_INITRD_EMPTY + exit 1 (we expected a
#                      payload; failing closed beats silent skip).
#   * write failure  → log QRAB_INITRD_WRITE_ERR + exit 1.

set -eu

# Keep these tokens in lockstep with projects/qrab-boot/src/secret.rs.
QRAB_GUID="6e6f1cba-b5ba-4e54-a3b3-2a7a8f3d9b00"
QRAB_NAME="QrabSecret"

# The two paths below can be overridden at invocation time so the
# host-side `tests/simulate.sh` harness can point the hook at a
# scratch tmpdir without touching real firmware or `/run`. Production
# initrd invocations leave them unset and use the system defaults.
EFIVARFS="${QRAB_INITRD_EFIVARFS:-/sys/firmware/efi/efivars}"
VAR_FILE="$EFIVARFS/${QRAB_NAME}-${QRAB_GUID}"

RUN_DIR="${QRAB_INITRD_RUN_DIR:-/run/qrab}"
SECRET_PATH="$RUN_DIR/secret"

log() {
    # dracut pipes these into the journal.
    echo "qrab-boot: $*" >&2
}

if [ ! -d "$EFIVARFS" ]; then
    log "QRAB_INITRD_NO_EFIVARFS ($EFIVARFS absent)"
    exit 0
fi

if [ ! -f "$VAR_FILE" ]; then
    log "QRAB_INITRD_ABSENT ($VAR_FILE)"
    exit 0
fi

# Skip the 4-byte efivarfs attribute prefix; everything after is the
# secret payload.
mkdir -p "$RUN_DIR"
chmod 0700 "$RUN_DIR"
tmp="$(mktemp --tmpdir="$RUN_DIR" secret.XXXXXX)"
chmod 0600 "$tmp"

if ! dd if="$VAR_FILE" bs=1 skip=4 of="$tmp" status=none 2>/dev/null; then
    rm -f "$tmp"
    log "QRAB_INITRD_READ_ERR"
    exit 1
fi

if [ ! -s "$tmp" ]; then
    rm -f "$tmp"
    log "QRAB_INITRD_EMPTY"
    exit 1
fi

# Atomic rename so a consumer that appears mid-copy sees either the
# old file or the new one, never a half-written payload.
if ! mv "$tmp" "$SECRET_PATH"; then
    rm -f "$tmp"
    log "QRAB_INITRD_WRITE_ERR"
    exit 1
fi
chmod 0600 "$SECRET_PATH"

len="$(wc -c <"$SECRET_PATH" 2>/dev/null || echo 0)"
log "QRAB_INITRD_OK len=$len"

# Wipe the variable. Per efivarfs(5), setting the immutable flag
# off and writing zero bytes removes the variable entirely; we take
# the simpler path of `rm` which efivarfs translates to a DeleteVar
# call into the firmware.
if ! rm -f "$VAR_FILE" 2>/dev/null; then
    log "QRAB_INITRD_WIPE_ERR (variable still readable post-boot)"
    exit 1
fi
log "QRAB_INITRD_WIPED"
