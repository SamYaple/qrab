#!/usr/bin/env bash
# Host-side simulation of the qrab-boot initrd hook.
#
# Builds a fake efivarfs layout under a tmpdir, writes a variable
# file whose first 4 bytes are the EFI_VARIABLE_* attribute mask
# and whose tail is a known payload, runs the hook against it via
# the QRAB_INITRD_EFIVARFS / QRAB_INITRD_RUN_DIR overrides, and
# asserts:
#
#   1. /run-override/qrab/secret contains the payload bytes.
#   2. The efivarfs variable file was removed after the hook ran.
#   3. The hook logged both QRAB_INITRD_OK and QRAB_INITRD_WIPED.
#
# Also exercises the "no variable" and "empty payload" branches so
# the error paths stay honest under edits.
#
# Exit 0 on all green, 1 on any assertion failure.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK="$HERE/../qrab-read-secret.sh"
[[ -x "$HOOK" ]] || chmod +x "$HOOK"

QRAB_GUID="6e6f1cba-b5ba-4e54-a3b3-2a7a8f3d9b00"
VAR_BASENAME="QrabSecret-${QRAB_GUID}"

PAYLOAD="QRAB_STUB_SECRET_v0"
# Writing the 4-byte EFI attribute prefix directly in printf — a bash
# variable built with `$'\x07\x00\x00\x00'` would be truncated at the
# first NUL because bash strings are C-style NUL-terminated. Callers
# below use `write_fake_var` which concatenates the escape + payload
# in one printf call.
write_fake_var() {
    local out="$1" payload="$2"
    printf '\x07\x00\x00\x00%s' "$payload" > "$out"
}
write_empty_var() {
    local out="$1"
    printf '\x07\x00\x00\x00' > "$out"
}

scratch="$(mktemp -d)"
cleanup() { rm -rf "$scratch"; }
trap cleanup EXIT

pass=0
fail=0
assert() {
    local name="$1" cond="$2"
    if eval "$cond"; then
        pass=$((pass + 1))
        echo "ok   $name"
    else
        fail=$((fail + 1))
        echo "FAIL $name  ($cond)"
    fi
}

run_hook() {
    local case_dir="$1" out_log="$2"
    QRAB_INITRD_EFIVARFS="$case_dir/efivars" \
        QRAB_INITRD_RUN_DIR="$case_dir/run/qrab" \
        bash "$HOOK" 2>"$out_log"
}

# --- case 1: happy path --------------------------------------------------
c1="$scratch/happy"
mkdir -p "$c1/efivars" "$c1/run"
write_fake_var "$c1/efivars/$VAR_BASENAME" "$PAYLOAD"
log1="$c1/hook.log"
set +e
run_hook "$c1" "$log1"
rc1=$?
set -e

assert "happy: hook exit 0"                 "[[ $rc1 -eq 0 ]]"
assert "happy: secret file exists"          "[[ -f '$c1/run/qrab/secret' ]]"
assert "happy: secret bytes match payload"  "diff <(printf %s \"$PAYLOAD\") '$c1/run/qrab/secret' >/dev/null"
assert "happy: efivar removed"              "[[ ! -e '$c1/efivars/$VAR_BASENAME' ]]"
assert "happy: log has QRAB_INITRD_OK"      "grep -q 'QRAB_INITRD_OK len=${#PAYLOAD}' '$log1'"
assert "happy: log has QRAB_INITRD_WIPED"   "grep -q 'QRAB_INITRD_WIPED' '$log1'"

# --- case 2: no variable -------------------------------------------------
c2="$scratch/absent"
mkdir -p "$c2/efivars" "$c2/run"
log2="$c2/hook.log"
set +e
run_hook "$c2" "$log2"
rc2=$?
set -e

assert "absent: hook exit 0"                "[[ $rc2 -eq 0 ]]"
assert "absent: no secret file written"     "[[ ! -e '$c2/run/qrab/secret' ]]"
assert "absent: log has QRAB_INITRD_ABSENT" "grep -q 'QRAB_INITRD_ABSENT' '$log2'"

# --- case 3: empty variable (attr prefix only) ---------------------------
c3="$scratch/empty"
mkdir -p "$c3/efivars" "$c3/run"
write_empty_var "$c3/efivars/$VAR_BASENAME"
log3="$c3/hook.log"
set +e
run_hook "$c3" "$log3"
rc3=$?
set -e

assert "empty: hook exit 1 (fail-closed)"   "[[ $rc3 -eq 1 ]]"
assert "empty: log has QRAB_INITRD_EMPTY"   "grep -q 'QRAB_INITRD_EMPTY' '$log3'"

echo
echo "simulate.sh: ${pass} ok, ${fail} failed"
[[ $fail -eq 0 ]]
