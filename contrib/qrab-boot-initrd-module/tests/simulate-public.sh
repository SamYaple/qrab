#!/usr/bin/env bash
# Host-side simulation of the qrab-boot public-var initrd hook.
#
# Builds a fake efivarfs layout + runs qrab-read-public.sh against
# it via the QRAB_INITRD_EFIVARFS / QRAB_INITRD_RUN_DIR overrides.
# Asserts:
#
#   1. Each of QrabReport / QrabSession gets copied to the matching
#      /run-override/qrab/{report,session} path with the 4-byte
#      attribute prefix stripped.
#   2. The efivars themselves are NOT removed — unlike the secret
#      hook, the public hook deliberately preserves the source.
#   3. The hook logs QRAB_INITRD_PUB_OK_<name> for each copied var
#      + QRAB_INITRD_PUB_DONE at the end.
#   4. Absent-var and no-efivarfs branches exit 0 cleanly.
#
# Exit 0 on all green, 1 on any assertion failure.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOOK="$HERE/../qrab-read-public.sh"
[[ -x "$HOOK" ]] || chmod +x "$HOOK"

QRAB_GUID="6e6f1cba-b5ba-4e54-a3b3-2a7a8f3d9b00"
REPORT_BASENAME="QrabReport-${QRAB_GUID}"
SESSION_BASENAME="QrabSession-${QRAB_GUID}"

# 14-byte magic + 1184-byte zero-filled body = the production shape
# of QrabReport on SEV-SNP boots. The hook doesn't care about the
# body's internal layout; it only strips the 4-byte attr prefix and
# mv's the rest into place.
REPORT_BODY_LEN=$((14 + 1184))
SESSION_BODY_LEN=64

write_fake_var() {
    local out="$1" body_len="$2"
    # 4-byte EFI attribute mask, then `body_len` zeros.
    { printf '\x07\x00\x00\x00'; head -c "$body_len" /dev/zero; } > "$out"
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

# --- case 1: both vars present, happy path -------------------------------
c1="$scratch/happy"
mkdir -p "$c1/efivars" "$c1/run"
write_fake_var "$c1/efivars/$REPORT_BASENAME"  "$REPORT_BODY_LEN"
write_fake_var "$c1/efivars/$SESSION_BASENAME" "$SESSION_BODY_LEN"
log1="$c1/hook.log"
set +e
run_hook "$c1" "$log1"
rc1=$?
set -e

assert "happy: hook exit 0"                          "[[ $rc1 -eq 0 ]]"
assert "happy: /run/qrab/report exists"              "[[ -f '$c1/run/qrab/report' ]]"
assert "happy: report length == body"                "[[ \$(wc -c <'$c1/run/qrab/report') -eq $REPORT_BODY_LEN ]]"
assert "happy: /run/qrab/session exists"             "[[ -f '$c1/run/qrab/session' ]]"
assert "happy: session length == 64"                 "[[ \$(wc -c <'$c1/run/qrab/session') -eq $SESSION_BODY_LEN ]]"
assert "happy: efivar QrabReport preserved"          "[[ -f '$c1/efivars/$REPORT_BASENAME' ]]"
assert "happy: efivar QrabSession preserved"         "[[ -f '$c1/efivars/$SESSION_BASENAME' ]]"
assert "happy: log has QRAB_INITRD_PUB_OK_QrabReport" "grep -q 'QRAB_INITRD_PUB_OK_QrabReport' '$log1'"
assert "happy: log has QRAB_INITRD_PUB_OK_QrabSession" "grep -q 'QRAB_INITRD_PUB_OK_QrabSession' '$log1'"
assert "happy: log has QRAB_INITRD_PUB_DONE"         "grep -q 'QRAB_INITRD_PUB_DONE' '$log1'"

# --- case 2: one var present, one absent --------------------------------
# QrabReport present, QrabSession missing — hook still exits 0 and
# copies what it can. The absent branch emits a distinct log token so
# the matrix harness can distinguish "partial drop" from "total miss".
c2="$scratch/partial"
mkdir -p "$c2/efivars" "$c2/run"
write_fake_var "$c2/efivars/$REPORT_BASENAME" "$REPORT_BODY_LEN"
log2="$c2/hook.log"
set +e
run_hook "$c2" "$log2"
rc2=$?
set -e

assert "partial: hook exit 0"                         "[[ $rc2 -eq 0 ]]"
assert "partial: report copied"                       "[[ -f '$c2/run/qrab/report' ]]"
assert "partial: no session file"                     "[[ ! -e '$c2/run/qrab/session' ]]"
assert "partial: log has QRAB_INITRD_PUB_ABSENT_QrabSession" "grep -q 'QRAB_INITRD_PUB_ABSENT_QrabSession' '$log2'"

# --- case 3: no efivarfs -------------------------------------------------
c3="$scratch/no_efivarfs"
mkdir -p "$c3/run"
# Deliberately NOT creating $c3/efivars.
log3="$c3/hook.log"
set +e
run_hook "$c3" "$log3"
rc3=$?
set -e

assert "no_efivarfs: hook exit 0"                     "[[ $rc3 -eq 0 ]]"
assert "no_efivarfs: no /run files written"           "[[ ! -e '$c3/run/qrab/report' ]]"
assert "no_efivarfs: log has QRAB_INITRD_PUB_NO_EFIVARFS" "grep -q 'QRAB_INITRD_PUB_NO_EFIVARFS' '$log3'"

echo
echo "simulate-public.sh: ${pass} ok, ${fail} failed"
[[ $fail -eq 0 ]]
