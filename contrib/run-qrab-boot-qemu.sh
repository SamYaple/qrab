#!/usr/bin/env bash
# Phase 0 smoke-test runner for qrab-boot.
#
# Boots the built `qrab-boot.efi` under QEMU q35 with vanilla OVMF (no SEV
# yet — Phase 1 swaps in AmdSev OVMF). A FAT "ESP" disk image is built with
# the efi at the spec-mandated path `EFI/BOOT/BOOTX64.EFI` so the firmware's
# default boot entry executes it.
#
# Expected output (on stdout):
#     QRAB BOOT v0.1.0 phase0 (build qrab-boot)
#     QRAB_PHASE0_OK
# VM then hangs forever. Kill with Ctrl-A X (monitor).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
BOOT_DIR="$ROOT/projects/qrab-boot"
BUILD_TARGET_DIR="$BOOT_DIR/target/x86_64-unknown-uefi/release"
EFI="$BUILD_TARGET_DIR/qrab-boot.efi"
OUT_DIR="$ROOT/target/qrab-boot-smoke"
ESP_IMG="$OUT_DIR/esp.img"
DEBUGLOG="$OUT_DIR/debugcon.log"
SERIALLOG="$OUT_DIR/serial.log"

# OVMF path — allow override, fall back to common distro locations.
OVMF_CODE="${QRAB_BOOT_OVMF_CODE:-}"
if [[ -z "$OVMF_CODE" ]]; then
    for candidate in \
        /usr/share/OVMF/OVMF_CODE.fd \
        /usr/share/edk2-ovmf/OVMF_CODE.fd \
        /usr/share/edk2/ovmf/OVMF_CODE.fd \
        /usr/share/qemu/OVMF.fd
    do
        if [[ -f "$candidate" ]]; then
            OVMF_CODE="$candidate"
            break
        fi
    done
fi
if [[ -z "$OVMF_CODE" || ! -f "$OVMF_CODE" ]]; then
    echo "error: OVMF_CODE.fd not found. Set QRAB_BOOT_OVMF_CODE." >&2
    exit 1
fi

echo ">>> building qrab-boot.efi"
# R16-H15: phase-0 smoke is banner-only; no attest-server involved, so
# the bootloader's embedded pubkey is irrelevant. Opt into the dev
# build that refuses attest responses at runtime.
(cd "$BOOT_DIR" && QRAB_DEV_NO_PUBKEY=1 cargo build --release)
if [[ ! -f "$EFI" ]]; then
    echo "error: $EFI not produced" >&2
    exit 1
fi

echo ">>> staging ESP image at $ESP_IMG"
mkdir -p "$OUT_DIR"
rm -f "$ESP_IMG"
# 64 MiB is enough for the skeleton; grows in later phases.
truncate -s 64M "$ESP_IMG"
mkfs.vfat -F 32 -n QRABBOOT "$ESP_IMG" >/dev/null
mmd -i "$ESP_IMG" ::/EFI ::/EFI/BOOT
mcopy -i "$ESP_IMG" "$EFI" ::/EFI/BOOT/BOOTX64.EFI

: > "$DEBUGLOG"
: > "$SERIALLOG"

echo ">>> launching QEMU (Ctrl-A X to quit)"
# -debugcon at 0x402 captures uefi crate's log-debugcon output.
# -serial stdio lets us see any early DXE/BDS chatter too.
# No KVM: portable across hosts without /dev/kvm permissions.
exec qemu-system-x86_64 \
    -machine q35,accel=tcg \
    -m 512M \
    -nographic \
    -no-reboot \
    -bios "$OVMF_CODE" \
    -drive if=virtio,format=raw,file="$ESP_IMG" \
    -serial file:"$SERIALLOG" \
    -debugcon file:"$DEBUGLOG" \
    -global isa-debugcon.iobase=0x402 \
    -monitor stdio
