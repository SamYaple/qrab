#!/bin/bash
# qrab attestation helper — runs inside SEV-SNP guest
#
# Reads challenge nonce from virtio-serial (or serial/stdin), performs
# TPM2 quote and SNP attestation report, outputs evidence.
#
# Usage: attest.sh [--serial /dev/ttyS0] [--qr]
#
# Environment:
#   ATTEST_PORT  virtio-serial port (default: /dev/virtio-ports/org.qrab.attestation)
#   SERIAL       fallback serial device (default: /dev/ttyS0)
#   USE_QR       set to 1 to emit QR code of attestation summary

set -euo pipefail

ATTEST_PORT="${ATTEST_PORT:-/dev/virtio-ports/org.qrab.attestation}"
SERIAL="${SERIAL:-/dev/ttyS0}"
USE_QR="${USE_QR:-0}"

# Determine I/O device: prefer virtio-serial, fall back to serial.
if [ -c "$ATTEST_PORT" ] || [ -e "$ATTEST_PORT" ]; then
    IO_DEV="$ATTEST_PORT"
elif [ -c "$SERIAL" ]; then
    IO_DEV="$SERIAL"
else
    echo "ERROR: no attestation channel found" >&2
    echo "Tried: $ATTEST_PORT, $SERIAL" >&2
    exit 1
fi

# Parse CLI args.
while [[ $# -gt 0 ]]; do
    case "$1" in
        --serial)
            IO_DEV="$2"
            shift 2
            ;;
        --qr)
            USE_QR=1
            shift
            ;;
        *)
            echo "Unknown arg: $1" >&2
            exit 1
            ;;
    esac
done

# Read SEV-SNP status from sysfs.
read_sev_status() {
    local sev_dir="/sys/kernel/security/coco/sev-guest"
    if [ -d "$sev_dir" ]; then
        echo "SEV-SNP: active"
        if [ -f "$sev_dir/policy" ]; then
            echo "Policy: $(cat "$sev_dir/policy")"
        fi
    else
        echo "SEV-SNP: not detected"
    fi
}

# Perform TPM2 quote.
do_tpm_quote() {
    local nonce="$1"

    if ! command -v tpm2_quote &>/dev/null; then
        echo "ERROR: tpm2-tools not installed"
        return 1
    fi

    # Create AK (Attestation Key) if not exists.
    if [ ! -f /tmp/ak.ctx ]; then
        tpm2_createek -c /tmp/ek.ctx -G rsa2048
        tpm2_createak -C /tmp/ek.ctx -c /tmp/ak.ctx -G rsa2048 -g sha256 -s rsassa
    fi

    # Quote PCRs 0-7 with nonce.
    tpm2_quote \
        -c /tmp/ak.ctx \
        -l sha256:0,1,2,3,4,5,6,7 \
        -q "$nonce" \
        -m /tmp/quote.msg \
        -s /tmp/quote.sig \
        -o /tmp/quote.pcrs \
        -G sha256

    echo "QUOTE_MSG:$(base64 -w0 /tmp/quote.msg)"
    echo "QUOTE_SIG:$(base64 -w0 /tmp/quote.sig)"
    echo "QUOTE_PCRS:$(base64 -w0 /tmp/quote.pcrs)"
}

# Read SNP attestation report via kernel interface.
read_snp_report() {
    local nonce="$1"
    local report_dev="/dev/sev-guest"

    if [ -c "$report_dev" ]; then
        if command -v snpguest &>/dev/null; then
            snpguest report --random -x "$nonce" /tmp/snp_report.bin
            echo "SNP_REPORT:$(base64 -w0 /tmp/snp_report.bin)"
        else
            echo "SNP_REPORT:unavailable (snpguest not installed)"
        fi
    else
        echo "SNP_REPORT:unavailable (no $report_dev)"
    fi
}

# Main attestation flow.
main() {
    echo "=== QRAB SEV-SNP ATTESTATION ==="
    echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "Channel: $IO_DEV"
    read_sev_status

    echo ""
    echo "Waiting for challenge nonce..."
    echo "Enter hex nonce (64 chars):"

    # Read nonce from I/O device.
    local nonce
    read -r nonce < "$IO_DEV"
    nonce=$(echo "$nonce" | tr -d '[:space:]')

    echo ""
    echo "Challenge received: $nonce"
    echo "Performing attestation..."
    echo ""

    # TPM quote.
    echo "--- TPM2 Quote ---"
    do_tpm_quote "$nonce" 2>&1
    echo ""

    # SNP report.
    echo "--- SNP Attestation Report ---"
    read_snp_report "$nonce" 2>&1
    echo ""

    # Generate QR if requested.
    if [ "$USE_QR" = "1" ] && command -v qrencode &>/dev/null; then
        echo "--- QR Code (attestation summary) ---"
        local payload="{\"nonce\":\"$nonce\",\"timestamp\":\"$(date -u +%s)\"}"
        qrencode -t ANSI256 "$payload"
        echo ""
    fi

    echo "=== ATTESTATION COMPLETE ==="
}

# When using virtio-serial, redirect stdout to the device so the host
# receives the output. For regular serial, output goes to stdout.
if [ "$IO_DEV" = "$ATTEST_PORT" ] && [ -e "$ATTEST_PORT" ]; then
    main "$@" > "$ATTEST_PORT" 2>&1
else
    main "$@"
fi
