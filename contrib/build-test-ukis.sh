#!/usr/bin/env bash
# Build the Phase 2a test-fixture EFI images.
#
# Phase 2a treats a "UKI" as any PE32+ EFI binary that would normally be
# subject to a Secure Boot check — we use the already-built qrab-boot.efi
# as the stand-in. A real Linux UKI replaces this later (Phase 5+), but
# the signing/tampering semantics are identical.
#
# Outputs in projects/qrab-boot-tests/fixtures/:
#   uki-valid.efi     — signed with db key; sbverify passes.
#   uki-tampered.efi  — signed, then one byte flipped in the PE text
#                        section's middle; sbverify must reject.
#   uki-unsigned.efi  — the raw unsigned binary for comparison.
#
# Idempotent; rebuilds always.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
BOOT_CRATE="$ROOT/projects/qrab-boot"
BOOT_EFI="$BOOT_CRATE/target/x86_64-unknown-uefi/release/qrab-boot.efi"
FIXTURES="$ROOT/projects/qrab-boot-tests/fixtures"
KEYDIR="$ROOT/contrib/sb-keys"

die() { echo "build-test-ukis.sh: error: $*" >&2; exit 1; }

[[ -f "$KEYDIR/db.crt" ]] || die "SB keys missing; run contrib/sb-enroll-keys.sh"

mkdir -p "$FIXTURES"

# (Re)build qrab-boot.efi so fixtures always match the current source.
(cd "$BOOT_CRATE" && cargo build --release >/dev/null)
[[ -f "$BOOT_EFI" ]] || die "qrab-boot.efi missing at $BOOT_EFI"

cp "$BOOT_EFI" "$FIXTURES/uki-unsigned.efi"
bash "$ROOT/contrib/sign-efi.sh" "$FIXTURES/uki-unsigned.efi" "$FIXTURES/uki-valid.efi"

# Tampered variant: start from the valid signed image, copy it, then flip
# a single byte well past the PE header and outside the Authenticode
# signature blob. Offset 0x1000 lands inside .text on every build of this
# binary (section alignment is 0x1000 for UEFI PEs); if qrab-boot.efi is
# ever < 0x1100 bytes this will need tweaking.
cp "$FIXTURES/uki-valid.efi" "$FIXTURES/uki-tampered.efi"
size="$(stat -c '%s' "$FIXTURES/uki-tampered.efi")"
tamper_offset=$((0x1000))
(( tamper_offset + 1 < size )) \
    || die "valid.efi smaller than tamper offset; refusing to tamper"
printf '\x5a' | dd of="$FIXTURES/uki-tampered.efi" bs=1 seek="$tamper_offset" count=1 conv=notrunc status=none

# Sanity: valid passes, tampered fails, and the failures are from the
# Authenticode hash check (not a missing cert).
sbverify --cert "$KEYDIR/db.crt" "$FIXTURES/uki-valid.efi"    >/dev/null \
    || die "uki-valid.efi failed sbverify against db.crt"
if sbverify --cert "$KEYDIR/db.crt" "$FIXTURES/uki-tampered.efi" >/dev/null 2>&1; then
    die "uki-tampered.efi still verifies — tamper offset did not land in signed region"
fi

echo "build-test-ukis.sh: fixtures at $FIXTURES:"
ls -la "$FIXTURES"
