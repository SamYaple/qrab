#!/usr/bin/env bash
# Phase 1 smoke-test: boot target/ovmf/OVMF.fd under SEV-SNP and verify
# the embedded qrab-boot.efi prints the QRAB banner on serial. Requires
# an SNP-capable host (AMD EPYC + /dev/sev + kvm_amd.sev_snp=Y).
set -euo pipefail
ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
FW="$ROOT/target/ovmf/OVMF.fd"
OUT="$ROOT/target/qrab-boot-phase1-smoke"
mkdir -p "$OUT"
: > "$OUT/serial.log"
rm -f "$OUT/qmp.sock"

[[ -f "$FW" ]] || { echo "missing $FW; run contrib/build-ovmf.sh first" >&2; exit 1; }
[[ -e /dev/sev ]] || { echo "no /dev/sev — SNP not available on this host" >&2; exit 1; }

qemu-system-x86_64 --preconfig \
    -machine q35,confidential-guest-support=sev0,firmware="$FW" \
    -m 512M -accel kvm -cpu EPYC-v4 \
    -object sev-snp-guest,id=sev0,reduced-phys-bits=1,cbitpos=51,sev-device=/dev/sev \
    -nodefaults -display none -no-reboot \
    -serial file:"$OUT/serial.log" \
    -qmp unix:"$OUT/qmp.sock",server=on,wait=off &
QEMU_PID=$!
trap 'kill $QEMU_PID 2>/dev/null || true' EXIT

# Wait for QMP socket.
for _ in $(seq 1 40); do
    [[ -S "$OUT/qmp.sock" ]] && break
    sleep 0.1
done
[[ -S "$OUT/qmp.sock" ]] || { echo "qmp socket never appeared" >&2; exit 1; }

# Drive preconfig → running.
{
    printf '%s\n' '{"execute":"qmp_capabilities"}'
    printf '%s\n' '{"execute":"x-exit-preconfig"}'
    printf '%s\n' '{"execute":"cont"}'
    sleep 15
} | socat - UNIX-CONNECT:"$OUT/qmp.sock" >/dev/null 2>&1 || true

kill "$QEMU_PID" 2>/dev/null || true
wait "$QEMU_PID" 2>/dev/null || true

echo "--- serial (filtered) ---"
tr -d '\r' < "$OUT/serial.log" | grep -aE 'QRAB|phase|BdsDxe|UEFI|TianoCore|SEV|failed|error' | head -40 \
    || { echo "(no banner match)"; echo "--- full serial tail ---"; tail -40 "$OUT/serial.log"; exit 1; }
