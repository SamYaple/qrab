#!/usr/bin/env bash
# End-to-end smoke test for the qrab-boot phase-3b vsock path.
#
# Boots `qrab-boot.efi` under QEMU q35 + TCG with a virtio-vsock PCI
# device bound to vhost-vsock on the host, and verifies that the guest
# serial log mentions the full chain:
#
#   QRAB_VSOCK_FOUND  →  QRAB_VSOCK_DRIVER_OK  →  QRAB_VSOCK_CONNECT_OK
#
# The corresponding host-side `contrib/mock-vsock-server.py` is
# launched in the background; it logs to `target/qrab-boot-vsock-smoke/mock.log`.
#
# Prerequisites on the host:
#   - `CONFIG_VHOST_VSOCK=m`/y + `vhost_vsock` loaded (check with
#     `modprobe vhost_vsock`).
#   - `/dev/vhost-vsock` readable / writable by this user (commonly
#     `kvm` group on distros; Gentoo's `vhost-vsock` kmod or manual
#     `chmod g+rw /dev/vhost-vsock` + udev rule).
#   - A free guest CID in the [3, 4_294_967_295] range (we default to
#     4242, override via `QRAB_VSOCK_GUEST_CID`).
#
# On a host without those, the script prints a diagnostic and exits 77
# (GNU "skipped") so CI harnesses can special-case it.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
BOOT_CRATE="$ROOT/projects/qrab-boot"
EFI="$BOOT_CRATE/target/x86_64-unknown-uefi/release/qrab-boot.efi"
OUT="$ROOT/target/qrab-boot-vsock-smoke"
ESP="$OUT/esp.img"
SERIAL="$OUT/serial.log"
MOCKLOG="$OUT/mock.log"

GUEST_CID="${QRAB_VSOCK_GUEST_CID:-4242}"
PORT="${QRAB_VSOCK_PORT:-1337}"
BOOT_TIMEOUT_SECS="${QRAB_VSOCK_BOOT_TIMEOUT:-20}"

OVMF="${QRAB_BOOT_OVMF_CODE:-}"
if [[ -z "$OVMF" ]]; then
    for c in /usr/share/edk2-ovmf/OVMF_CODE.fd /usr/share/OVMF/OVMF_CODE.fd \
             /usr/share/edk2/ovmf/OVMF_CODE.fd; do
        [[ -f "$c" ]] && OVMF="$c" && break
    done
fi

log() { echo "smoke-vsock: $*" >&2; }
die() { log "error: $*"; exit 1; }
skip() { log "skipped: $*"; exit 77; }

# ---- prerequisites -------------------------------------------------------
[[ -n "$OVMF" && -f "$OVMF" ]] || skip "no OVMF_CODE.fd (set QRAB_BOOT_OVMF_CODE)"
[[ -e /dev/vhost-vsock ]] || skip "/dev/vhost-vsock missing (load vhost_vsock?)"
[[ -r /dev/vhost-vsock && -w /dev/vhost-vsock ]] \
    || skip "/dev/vhost-vsock not accessible (check group membership)"
command -v qemu-system-x86_64 >/dev/null || die "qemu-system-x86_64 not on PATH"
command -v python3 >/dev/null || die "python3 not on PATH"
command -v mkfs.vfat >/dev/null || die "mkfs.vfat not on PATH"
command -v mcopy >/dev/null || die "mcopy not on PATH"
command -v mmd >/dev/null || die "mmd not on PATH"

# ---- build ---------------------------------------------------------------
log "building qrab-boot.efi"
(cd "$BOOT_CRATE" && cargo build --release >/dev/null)
[[ -f "$EFI" ]] || die "$EFI not produced"

# ---- stage ESP + logs ----------------------------------------------------
mkdir -p "$OUT"
rm -f "$ESP" "$SERIAL" "$MOCKLOG"
truncate -s 64M "$ESP"
mkfs.vfat -F 32 -n QRABBOOT "$ESP" >/dev/null
mmd -i "$ESP" ::/EFI ::/EFI/BOOT
mcopy -i "$ESP" "$EFI" ::/EFI/BOOT/BOOTX64.EFI
: > "$SERIAL"
: > "$MOCKLOG"

# ---- attest-server -------------------------------------------------------
# Build the Rust attestation server up front so the bind-time log line
# below is deterministic (and so a compile error fails fast instead of
# mid-handshake).
log "building attest-server"
(cd "$ROOT" && cargo build -p attest-server --release >/dev/null 2>&1) \
    || die "attest-server build failed"
ATTEST_BIN="$ROOT/target/release/attest-server"
[[ -x "$ATTEST_BIN" ]] || die "attest-server binary not produced"

log "launching attest-server (port $PORT)"
"$ATTEST_BIN" "$PORT" >"$MOCKLOG" 2>&1 &
MOCK_PID=$!
cleanup() {
    kill -TERM "$MOCK_PID" 2>/dev/null || true
    wait "$MOCK_PID" 2>/dev/null || true
}
trap cleanup EXIT
# Give the listener a moment to bind.
sleep 0.3

# ---- qemu ----------------------------------------------------------------
log "launching qemu (guest_cid=$GUEST_CID, ~${BOOT_TIMEOUT_SECS}s)"
set +e
timeout "$BOOT_TIMEOUT_SECS" qemu-system-x86_64 \
    -machine q35,accel=tcg -m 512M -display none -no-reboot \
    -bios "$OVMF" \
    -drive if=virtio,format=raw,file="$ESP" \
    -device vhost-vsock-pci,guest-cid="$GUEST_CID" \
    -serial file:"$SERIAL" >/dev/null 2>&1
QEMU_RC=$?
set -e
# `timeout` returns 124 on timeout; qrab-boot hangs at `hlt` so we
# expect exactly that for the happy path.
case "$QEMU_RC" in
    0|124) ;;
    *) log "qemu exited with rc=$QEMU_RC (continuing — serial log is authoritative)";;
esac

# ---- verify --------------------------------------------------------------
echo "--- serial log (filtered) ---"
tr -d '\r' < "$SERIAL" | grep -aE 'QRAB|phase' | head -20 || true
echo "--- mock log ---"
tail -5 "$MOCKLOG" || true

# Success requires all three of these tokens on the guest serial.
required=(QRAB_VSOCK_FOUND QRAB_VSOCK_DRIVER_OK QRAB_VSOCK_CONNECT_OK QRAB_ATTEST_OK)
missing=()
for tok in "${required[@]}"; do
    grep -a -q "$tok" "$SERIAL" || missing+=("$tok")
done
if [[ ${#missing[@]} -ne 0 ]]; then
    log "missing tokens: ${missing[*]}"
    exit 1
fi
log "all required tokens present"
