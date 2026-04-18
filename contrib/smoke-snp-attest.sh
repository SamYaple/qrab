#!/usr/bin/env bash
# Expected-fail smoke: qrab-boot + attest-server under SEV-SNP.
#
# Same shape as `contrib/smoke-vsock-qemu.sh` but launches the guest
# with `--preconfig -machine confidential-guest-support=sev0` and the
# AmdSev OVMF image at `target/ovmf/OVMF.fd`. Originally filed as the
# reproducer for the DMA correctness gap in
# `~/workspace/llm-rag/issues/qrab-boot-snp-dma.md`; that rework
# shipped on 2026-04-18 (PciRootBridgeIo Map/Unmap HAL) but the
# smoke is still expected-fail because a separate blocker surfaced:
# `~/workspace/llm-rag/issues/qrab-boot-snp-high-mmio-gp.md`. The
# current failure signature is a `#GP - General Protection` on the
# first virtio common-config MMIO read (64-bit BAR at ~0xC000000000),
# not the `QRAB_VSOCK_DRIVER_SOCKET_ERR` that the first ticket
# guessed.
#
# Until the high-MMIO ticket closes this script is **expected to fail**.
# Exit codes:
#   0  — ALL required tokens present *and* we are running on a
#        bootloader that has closed the SNP DMA ticket. Unexpected
#        until ticket is resolved; the script exits 1 in that case
#        so the ticket-close regression lights up loudly.
#   1  — Either the tokens failed to appear (the current expected
#        outcome, but treated as a test failure here because the
#        real goal is closing the ticket), or the flow fully
#        succeeded while the ticket remains open (regression
#        detection).
#  77  — Skipped because the host lacks SNP support or
#        `target/ovmf/OVMF.fd` is missing.
#
# Set `QRAB_SNP_SMOKE_EXPECT=pass` after closing the SNP DMA ticket to
# flip the polarity; by default the script runs with
# `QRAB_SNP_SMOKE_EXPECT=fail` and exits 0 on the expected failure.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
BOOT_CRATE="$ROOT/projects/qrab-boot"
EFI="$BOOT_CRATE/target/x86_64-unknown-uefi/release/qrab-boot.efi"
FW="$ROOT/target/ovmf/OVMF.fd"
OUT="$ROOT/target/qrab-boot-snp-attest"
SERIAL="$OUT/serial.log"
MOCKLOG="$OUT/mock.log"

GUEST_CID="${QRAB_VSOCK_GUEST_CID:-4243}"
PORT="${QRAB_VSOCK_PORT:-1337}"
BOOT_TIMEOUT_SECS="${QRAB_VSOCK_BOOT_TIMEOUT:-30}"
EXPECT="${QRAB_SNP_SMOKE_EXPECT:-fail}"

log() { echo "smoke-snp-attest: $*" >&2; }
die() { log "error: $*"; exit 1; }
skip() { log "skipped: $*"; exit 77; }

# ---- prerequisites -------------------------------------------------------
[[ -f "$FW" ]] || skip "no AmdSev OVMF at $FW — run contrib/build-ovmf.sh"
[[ -e /dev/sev ]] || skip "no /dev/sev — SNP unavailable on this host"
[[ -e /dev/vhost-vsock ]] || skip "/dev/vhost-vsock missing (load vhost_vsock?)"
[[ -r /dev/vhost-vsock && -w /dev/vhost-vsock ]] \
    || skip "/dev/vhost-vsock not accessible to this user"
command -v qemu-system-x86_64 >/dev/null || die "qemu-system-x86_64 not on PATH"
command -v socat >/dev/null || die "socat not on PATH"
command -v cargo >/dev/null || die "cargo not on PATH"

# ---- build ---------------------------------------------------------------
log "building qrab-boot.efi + attest-server"
(cd "$BOOT_CRATE" && cargo build --release >/dev/null)
(cd "$ROOT" && cargo build -p attest-server --release >/dev/null 2>&1) \
    || die "attest-server build failed"
[[ -f "$EFI" ]] || die "$EFI not produced"
ATTEST_BIN="$ROOT/target/release/attest-server"
[[ -x "$ATTEST_BIN" ]] || die "attest-server binary not produced"

mkdir -p "$OUT"
: > "$SERIAL"
: > "$MOCKLOG"

# ---- attest-server -------------------------------------------------------
log "launching attest-server (port $PORT)"
"$ATTEST_BIN" "$PORT" >"$MOCKLOG" 2>&1 &
MOCK_PID=$!
cleanup() {
    kill -TERM "$MOCK_PID" 2>/dev/null || true
    wait "$MOCK_PID" 2>/dev/null || true
}
trap cleanup EXIT
sleep 0.3

# ---- qemu ----------------------------------------------------------------
log "launching SNP guest (guest_cid=$GUEST_CID, ~${BOOT_TIMEOUT_SECS}s)"
QMP="$OUT/qmp.sock"
rm -f "$QMP"

set +e
timeout "$BOOT_TIMEOUT_SECS" qemu-system-x86_64 --preconfig \
    -machine q35,confidential-guest-support=sev0,firmware="$FW" \
    -m 512M -accel kvm -cpu EPYC-v4 \
    -object sev-snp-guest,id=sev0,reduced-phys-bits=1,cbitpos=51,sev-device=/dev/sev \
    -device vhost-vsock-pci,guest-cid="$GUEST_CID" \
    -nodefaults -display none -no-reboot \
    -serial file:"$SERIAL" \
    -qmp unix:"$QMP",server=on,wait=off &
QEMU_PID=$!

# Wait for QMP, then kick through preconfig.
for _ in $(seq 1 40); do [[ -S "$QMP" ]] && break; sleep 0.1; done
if [[ -S "$QMP" ]]; then
    {
        printf '%s\n' '{"execute":"qmp_capabilities"}'
        printf '%s\n' '{"execute":"x-exit-preconfig"}'
        printf '%s\n' '{"execute":"cont"}'
        sleep "$BOOT_TIMEOUT_SECS"
    } | socat - UNIX-CONNECT:"$QMP" >/dev/null 2>&1 || true
else
    log "warn: QMP socket never appeared; flow likely dead on arrival"
fi
kill "$QEMU_PID" 2>/dev/null || true
wait "$QEMU_PID" 2>/dev/null || true
set -e

echo "--- serial log (filtered) ---"
tr -d '\r' < "$SERIAL" | grep -aE 'QRAB|phase' | head -30 || true
echo "--- mock log ---"
tail -5 "$MOCKLOG" || true

required=(QRAB_VSOCK_FOUND QRAB_VSOCK_DRIVER_OK QRAB_VSOCK_CONNECT_OK QRAB_ATTEST_OK)
missing=()
for tok in "${required[@]}"; do
    grep -a -q "$tok" "$SERIAL" || missing+=("$tok")
done

case "$EXPECT" in
    fail)
        if [[ ${#missing[@]} -eq 0 ]]; then
            log "unexpected PASS — issues/qrab-boot-snp-dma.md should be closed; flip QRAB_SNP_SMOKE_EXPECT=pass"
            exit 1
        fi
        log "expected FAIL observed (missing: ${missing[*]}); ticket issues/qrab-boot-snp-dma.md still open"
        exit 0
        ;;
    pass)
        if [[ ${#missing[@]} -ne 0 ]]; then
            log "regression — missing tokens: ${missing[*]}"
            exit 1
        fi
        log "all required tokens present under SNP"
        exit 0
        ;;
    *)
        die "QRAB_SNP_SMOKE_EXPECT=$EXPECT invalid; set to 'fail' or 'pass'"
        ;;
esac
