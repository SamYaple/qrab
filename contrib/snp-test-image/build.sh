#!/bin/bash
# Build minimal SNP-capable boot image for SEV-SNP integration tests.
#
# Requirements:
#   - Host kernel with CONFIG_AMD_MEM_ENCRYPT=y (used as guest kernel)
#   - gcc, ukify, sfdisk, mkfs.fat, mtools, qemu-img
#   - /usr/lib/systemd/boot/efi/linuxx64.efi.stub
#
# Output: snp-boot.qcow2 (~21MB)
#   - GPT disk with EFI system partition
#   - UKI (Unified Kernel Image) as BOOTX64.EFI
#   - Kernel: host kernel (EFI stub)
#   - Initramfs: static init binary (9KB, no glibc) that prints
#     "SNP BOOT OK" to serial, checks SEV status via kmsg, then poweroff
#
# The image boots in ~2s under SEV-SNP with EPYC-v4 + AmdSev OVMF.
# CirrosOS cannot be used for SNP testing — its kernel lacks
# CONFIG_AMD_MEM_ENCRYPT and crashes on page state transitions.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="${1:-/tmp/snp-test-image}"
OUTPUT="${2:-${SCRIPT_DIR}/../../target/test-fixtures/snp-boot.qcow2}"

KERNEL="${KERNEL:-/usr/src/linux/arch/x86/boot/bzImage}"
STUB="${STUB:-/usr/lib/systemd/boot/efi/linuxx64.efi.stub}"
CMDLINE="console=tty0 console=ttyS0,115200n8 earlyprintk=serial,ttyS0,115200"

# Validate prerequisites
for cmd in gcc ukify sfdisk mkfs.fat mmd mcopy qemu-img; do
    command -v "$cmd" >/dev/null 2>&1 || { echo "ERROR: $cmd not found"; exit 1; }
done
[ -f "$KERNEL" ] || { echo "ERROR: kernel not found at $KERNEL"; exit 1; }
[ -f "$STUB" ] || { echo "ERROR: EFI stub not found at $STUB"; exit 1; }

mkdir -p "$BUILD_DIR"

echo "=== Building init (no-glibc, raw syscalls) ==="
gcc -static -nostdlib -nostartfiles -O2 -march=x86-64 \
    -mno-avx -mno-avx2 -mno-avx512f -fno-builtin \
    -o "$BUILD_DIR/init" "$SCRIPT_DIR/init.c"
echo "  init: $(ls -lh "$BUILD_DIR/init" | awk '{print $5}')"

echo "=== Building initramfs ==="
rm -rf "$BUILD_DIR/initramfs"
mkdir -p "$BUILD_DIR/initramfs"/{dev,proc,sys,tmp,modules}
cp "$BUILD_DIR/init" "$BUILD_DIR/initramfs/init"

# Copy DRM/VGA modules for framebuffer console (VNC display)
KVER="${KVER:-$(uname -r)}"
MODDIR="/lib/modules/$KVER/kernel/drivers/gpu/drm"
for mod in \
    "$MODDIR/drm_kms_helper.ko" \
    "$MODDIR/drm_shmem_helper.ko" \
    "$MODDIR/clients/drm_client_lib.ko" \
    "$MODDIR/tiny/bochs.ko"; do
    if [ -f "$mod" ]; then
        cp "$mod" "$BUILD_DIR/initramfs/modules/"
        echo "  module: $(basename "$mod")"
    fi
done
(cd "$BUILD_DIR/initramfs" && find . -print0 | cpio --null -o --format=newc) \
    > "$BUILD_DIR/initramfs.cpio" 2>/dev/null
echo "  initramfs: $(ls -lh "$BUILD_DIR/initramfs.cpio" | awk '{print $5}')"

echo "=== Building UKI ==="
ukify build \
    --linux="$KERNEL" \
    --initrd="$BUILD_DIR/initramfs.cpio" \
    --cmdline="$CMDLINE" \
    --stub="$STUB" \
    --output="$BUILD_DIR/snp-test.efi" 2>/dev/null
echo "  UKI: $(ls -lh "$BUILD_DIR/snp-test.efi" | awk '{print $5}')"

echo "=== Building disk image ==="
truncate -s 48M "$BUILD_DIR/snp-boot.img"
echo 'label: gpt
start=2048, type=C12A7328-F81F-11D2-BA4B-00A0C93EC93B, name="EFI"' \
    | sfdisk "$BUILD_DIR/snp-boot.img" >/dev/null 2>&1
dd if="$BUILD_DIR/snp-boot.img" of="$BUILD_DIR/esp.img" \
    bs=512 skip=2048 count=94208 2>/dev/null
mkfs.fat -F 32 -n EFI "$BUILD_DIR/esp.img" >/dev/null 2>&1
mmd -i "$BUILD_DIR/esp.img" ::EFI ::EFI/BOOT 2>/dev/null
mcopy -i "$BUILD_DIR/esp.img" "$BUILD_DIR/snp-test.efi" \
    ::EFI/BOOT/BOOTX64.EFI 2>/dev/null
dd if="$BUILD_DIR/esp.img" of="$BUILD_DIR/snp-boot.img" \
    bs=512 seek=2048 count=94208 conv=notrunc 2>/dev/null

echo "=== Converting to qcow2 ==="
mkdir -p "$(dirname "$OUTPUT")"
qemu-img convert -f raw -O qcow2 "$BUILD_DIR/snp-boot.img" "$OUTPUT"
echo "  output: $OUTPUT ($(ls -lh "$OUTPUT" | awk '{print $5}'))"

echo "=== Done ==="
