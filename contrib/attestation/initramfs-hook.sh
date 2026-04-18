#!/bin/bash
# qrab boot-time attestation hook
#
# Drop-in for /etc/initramfs-tools/scripts/local-bottom/ or as a dracut module.
# Performs automatic attestation at early boot in SEV-SNP guests.
#
# Only runs when SEV-SNP is detected. Outputs attestation evidence to
# the virtio-serial attestation port (or serial as fallback).

# Only run in SEV-SNP guests.
if [ ! -d /sys/kernel/security/coco/sev-guest ]; then
    exit 0
fi

ATTEST_PORT="/dev/virtio-ports/org.qrab.attestation"
SERIAL="/dev/ttyS0"

# Pick output device.
if [ -e "$ATTEST_PORT" ]; then
    OUT="$ATTEST_PORT"
elif [ -c "$SERIAL" ]; then
    OUT="$SERIAL"
else
    echo "qrab: no attestation output device" > /dev/kmsg
    exit 0
fi

echo "qrab: SEV-SNP attestation at boot" > /dev/kmsg

# Generate random nonce for self-attestation at boot.
NONCE=$(head -c 32 /dev/urandom | xxd -p | tr -d '\n')

{
    echo "=== BOOT ATTESTATION ==="
    echo "AUTO_NONCE:$NONCE"
    echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo unknown)"

    # If tpm2-tools available, do quote.
    if command -v tpm2_quote &>/dev/null; then
        tpm2_createek -c /tmp/ek.ctx -G rsa2048 2>/dev/null
        tpm2_createak -C /tmp/ek.ctx -c /tmp/ak.ctx -G rsa2048 -g sha256 -s rsassa 2>/dev/null
        tpm2_quote \
            -c /tmp/ak.ctx \
            -l sha256:0,1,2,3,4,5,6,7 \
            -q "$NONCE" \
            -m /tmp/quote.msg \
            -s /tmp/quote.sig 2>/dev/null
        echo "BOOT_QUOTE:$(base64 -w0 /tmp/quote.msg 2>/dev/null)"
        echo "BOOT_SIG:$(base64 -w0 /tmp/quote.sig 2>/dev/null)"
    else
        echo "BOOT_QUOTE:unavailable (tpm2-tools not in initramfs)"
    fi

    # SNP report if snpguest available.
    if command -v snpguest &>/dev/null && [ -c /dev/sev-guest ]; then
        snpguest report --random -x "$NONCE" /tmp/snp_report.bin 2>/dev/null
        echo "BOOT_SNP_REPORT:$(base64 -w0 /tmp/snp_report.bin 2>/dev/null)"
    fi

    echo "=== BOOT ATTESTATION DONE ==="
} > "$OUT"

echo "qrab: boot attestation complete" > /dev/kmsg
