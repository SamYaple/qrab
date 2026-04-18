#!/usr/bin/env bash
# Sign a PE32+ EFI image with the qrab-boot db key.
#
# Usage:
#   sign-efi.sh <input.efi> [output.efi]
#
# If <output.efi> is omitted, the signed image is written next to the
# input with suffix `.signed`. The wrapped `sbsign` invocation is strict:
# it errors if the key/cert pair is missing or if sbverify cannot verify
# the result against the same cert.
#
# Prerequisites:
#   contrib/sb-enroll-keys.sh has been run once (populates contrib/sb-keys/).
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
KEYDIR="$ROOT/contrib/sb-keys"

die() { echo "sign-efi.sh: error: $*" >&2; exit 1; }

IN="${1:-}"
OUT="${2:-}"
[[ -n "$IN" ]] || die "usage: sign-efi.sh <input.efi> [output.efi]"
[[ -f "$IN"  ]] || die "input image $IN not found"
[[ -f "$KEYDIR/db.key" && -f "$KEYDIR/db.crt" ]] \
    || die "db.{key,crt} missing in $KEYDIR; run contrib/sb-enroll-keys.sh first"

if [[ -z "$OUT" ]]; then
    OUT="${IN}.signed"
fi

sbsign \
    --key    "$KEYDIR/db.key" \
    --cert   "$KEYDIR/db.crt" \
    --output "$OUT" \
    "$IN" >/dev/null

# Verify against the same cert. This catches a wrong key/cert pairing
# before anyone ships a firmware that would reject its own signer.
sbverify --cert "$KEYDIR/db.crt" "$OUT" >/dev/null \
    || die "sbverify disagreed with sbsign — refusing to emit $OUT"

echo "sign-efi.sh: signed $IN → $OUT ($(stat -c'%s bytes' "$OUT"))"
