#!/usr/bin/env bash
# Produce a raw detached RSA-PKCS#1 v1.5 SHA-256 signature for the
# qrab-boot verification flow.
#
# Usage:
#   sign-detached.sh <input> [<output.sig>]
#
# Why detached rather than PE Authenticode?
#   AmdSev OVMF has no Secure Boot infrastructure (see
#   reference/qrab-boot-plan.md phase 2b), so firmware-level PE
#   signature verification is not available on our SEV-SNP targets. The
#   qrab-boot bootloader enforces the signature itself, and a raw
#   detached sig keeps the in-guest verifier trivial:
#
#       openssl dgst -sha256 -sign db.key <payload>
#         → signs sha256(payload) with RSA-PKCS#1 v1.5 padding.
#
# The `contrib/sb-keys/db.key` produced by `sb-enroll-keys.sh` is the
# signer; keep the key rotation contract from that script (no silent
# overwrites of a key a firmware build has baked into its pubkey blob).
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
KEYDIR="$ROOT/contrib/sb-keys"

die() { echo "sign-detached.sh: error: $*" >&2; exit 1; }

IN="${1:-}"
OUT="${2:-}"
[[ -n "$IN" ]] || die "usage: sign-detached.sh <input> [output.sig]"
[[ -f "$IN" ]] || die "input $IN not found"
[[ -f "$KEYDIR/db.key" ]] || die "db.key missing; run contrib/sb-enroll-keys.sh first"

if [[ -z "$OUT" ]]; then
    OUT="${IN}.sig"
fi

openssl dgst -sha256 -sign "$KEYDIR/db.key" -out "$OUT" "$IN"

# Extract SPKI-DER pubkey from the cert; verify the sig here so a broken
# key/cert pairing can't ship downstream.
tmp_pub="$(mktemp)"
trap 'rm -f "$tmp_pub"' EXIT
openssl x509 -in "$KEYDIR/db.crt" -pubkey -noout \
    | openssl pkey -pubin -outform DER -out "$tmp_pub"
openssl dgst -sha256 -verify <(openssl pkey -pubin -inform DER -in "$tmp_pub") \
    -signature "$OUT" "$IN" >/dev/null \
    || die "self-verify failed — sig does not match key/cert pair"

echo "sign-detached.sh: signed $IN → $OUT ($(stat -c'%s bytes' "$OUT"))"
