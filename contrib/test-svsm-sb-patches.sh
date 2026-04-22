#!/usr/bin/env bash
# Unit test: apply-sb-wiring.py edits the AmdSev DSC + FDF correctly.
# Copies the vendored AmdSev files into a tmp dir, runs the patcher, and
# asserts the expected SB wiring is present. Re-runs to confirm idempotence.
# Fast: no edk2 build, no network, no QEMU.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
VENDOR="$ROOT/vendor/edk2-svsm"
PATCHER="$ROOT/contrib/patches/edk2-svsm/apply-sb-wiring.py"

[[ -f "$VENDOR/OvmfPkg/AmdSev/AmdSevX64.dsc" ]] \
    || { echo "missing $VENDOR/OvmfPkg/AmdSev/AmdSevX64.dsc" >&2; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$TMP/OvmfPkg/AmdSev" "$TMP/OvmfPkg/EnrollDefaultKeys"
cp "$VENDOR/OvmfPkg/AmdSev/AmdSevX64.dsc" "$TMP/OvmfPkg/AmdSev/"
cp "$VENDOR/OvmfPkg/AmdSev/AmdSevX64.fdf" "$TMP/OvmfPkg/AmdSev/"
cp "$VENDOR/OvmfPkg/EnrollDefaultKeys/AuthData.c" "$TMP/OvmfPkg/EnrollDefaultKeys/"

echo "[1/4] first apply"
python3 "$PATCHER" "$TMP"

dsc="$TMP/OvmfPkg/AmdSev/AmdSevX64.dsc"
fdf="$TMP/OvmfPkg/AmdSev/AmdSevX64.fdf"

echo "[2/4] assert post-patch symbols present"
fail=0
check() {
    local file="$1" pat="$2"
    if grep -qF -- "$pat" "$file"; then
        echo "  OK   $file contains: $pat"
    else
        echo "  FAIL $file missing:  $pat" >&2
        fail=1
    fi
}
check "$dsc" 'SecurityPkg/Library/AuthVariableLib/AuthVariableLib.inf'
check "$dsc" 'SecureBootVariableLib/SecureBootVariableLib.inf'
check "$dsc" 'PcdSecureBootSupported|TRUE'
check "$dsc" 'PcdRequireSelfSignedPk|TRUE'
check "$dsc" 'DxeImageVerificationLib/DxeImageVerificationLib.inf'
check "$dsc" 'SecureBootConfigDxe/SecureBootConfigDxe.inf'
check "$dsc" 'OvmfPkg/EnrollDefaultKeys/EnrollDefaultKeys.inf'
check "$fdf" 'SecureBootConfigDxe/SecureBootConfigDxe.inf'
check "$fdf" 'OvmfPkg/EnrollDefaultKeys/EnrollDefaultKeys.inf'
# Null fallback preserved for SECURE_BOOT_ENABLE == FALSE path:
check "$dsc" 'AuthVariableLibNull/AuthVariableLibNull.inf'
# All edits gated on SECURE_BOOT_ENABLE:
check "$dsc" '!if $(SECURE_BOOT_ENABLE) == TRUE'

echo "[3/4] idempotence: re-run must be a no-op"
before_dsc="$(sha256sum "$dsc" | awk '{print $1}')"
before_fdf="$(sha256sum "$fdf" | awk '{print $1}')"
python3 "$PATCHER" "$TMP"
after_dsc="$(sha256sum "$dsc" | awk '{print $1}')"
after_fdf="$(sha256sum "$fdf" | awk '{print $1}')"
if [[ "$before_dsc" != "$after_dsc" ]]; then
    echo "  FAIL dsc changed on re-apply" >&2; fail=1
else
    echo "  OK   dsc unchanged on re-apply"
fi
if [[ "$before_fdf" != "$after_fdf" ]]; then
    echo "  FAIL fdf changed on re-apply" >&2; fail=1
else
    echo "  OK   fdf unchanged on re-apply"
fi

echo "[4/6] key baking: regenerate AuthData.c from contrib/sb-keys/"
KEYS_DIR="$ROOT/contrib/sb-keys"
if [[ ! -f "$KEYS_DIR/KEK.crt" || ! -f "$KEYS_DIR/db.crt" ]]; then
    echo "  SKIP keys absent (run contrib/sb-enroll-keys.sh first)"
else
    python3 "$PATCHER" --bake-keys "$KEYS_DIR" "$TMP"
    auth="$TMP/OvmfPkg/EnrollDefaultKeys/AuthData.c"
    check "$auth" 'qrab-generated'
    check "$auth" 'mMicrosoftKek'
    check "$auth" 'mMicrosoftPca'
    check "$auth" 'mMicrosoftUefiCa'
    check "$auth" 'mSha256OfDevNull'
    # Subject CNs from contrib/sb-enroll-keys.sh:
    check "$auth" 'CN=qrab-boot Key Exchange Key'
    check "$auth" 'CN=qrab-boot db signer'
    # Microsoft cert header text gone:
    if grep -qi 'Microsoft Corporation KEK CA 2011' "$auth"; then
        echo "  FAIL MS KEK header still present in $auth" >&2; fail=1
    else
        echo "  OK   MS KEK header stripped"
    fi
    # Second --bake-keys run must be a no-op except for timestamp comment.
    before="$(grep -v 'Generated:' "$auth" | sha256sum | awk '{print $1}')"
    python3 "$PATCHER" --bake-keys "$KEYS_DIR" "$TMP" >/dev/null
    after="$(grep -v 'Generated:' "$auth" | sha256sum | awk '{print $1}')"
    if [[ "$before" != "$after" ]]; then
        echo "  FAIL AuthData.c body changed on re-bake (non-timestamp diff)" >&2; fail=1
    else
        echo "  OK   AuthData.c body stable on re-bake"
    fi
fi

echo "[5/6] bake-keys with bogus dir fails closed"
if python3 "$PATCHER" --bake-keys /nonexistent "$TMP" 2>/dev/null; then
    echo "  FAIL patcher accepted nonexistent keys dir" >&2; fail=1
else
    echo "  OK   patcher rejects nonexistent keys dir"
fi

echo "[6/6] missing-anchor detection"
bogus="$(mktemp -d)"
mkdir -p "$bogus/OvmfPkg/AmdSev"
cat > "$bogus/OvmfPkg/AmdSev/AmdSevX64.dsc" <<EOF
# intentionally empty — no anchors
EOF
cat > "$bogus/OvmfPkg/AmdSev/AmdSevX64.fdf" <<EOF
# intentionally empty — no anchors
EOF
if python3 "$PATCHER" "$bogus" 2>/dev/null; then
    echo "  FAIL patcher accepted file without anchors" >&2; fail=1
else
    echo "  OK   patcher rejects file without anchors"
fi
rm -rf "$bogus"

if [[ $fail -eq 0 ]]; then
    echo
    echo "test-svsm-sb-patches: all checks passed"
    exit 0
else
    echo
    echo "test-svsm-sb-patches: FAILED"
    exit 1
fi
