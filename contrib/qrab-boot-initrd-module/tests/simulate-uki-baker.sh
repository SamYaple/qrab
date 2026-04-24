#!/usr/bin/env bash
# Host-side functional test for contrib/build-qrab-uki.sh.
#
# Runs the baker against the current workspace and asserts:
#
#   * exit 0 with the machine-readable `QRAB_UKI_OK path=… size=… initrd=…`
#     tail line
#   * output PE exists and `ukify inspect` reports the expected sections:
#       .linux   — size > 1 MiB
#       .initrd  — size > 100 KiB (gzipped cpio)
#       .cmdline — contains QRAB_GUEST_INIT_LIVE=1
#       .uname   — matches the running kernel
#   * `sbverify` against contrib/sb-keys/db.crt returns clean
#   * unpacked initramfs contains:
#       /init                     (qrab-init)
#       /sbin/qrab-guest-init
#       /sbin/next-stage          (qrab-next-stage)
#       /lib64/ld-linux-x86-64.so.2
#       /lib64/libc.so.6
#
# Skipped on hosts without ukify / sbsign / sbverify / cpio (CI-runner
# sandboxes may be missing these) — the test prints `SKIP` and exits 0
# so the tree's `make check` stays green.
#
# Exit 0 on all green, 1 on assertion failure, 0 with SKIP token on
# tool-absence.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../../.." && pwd)"
BAKER="$REPO/contrib/build-qrab-uki.sh"
[[ -x "$BAKER" ]] || { echo "FAIL baker missing: $BAKER"; exit 1; }

for tool in ukify cpio gzip; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "SKIP simulate-uki-baker: $tool not installed"
        exit 0
    fi
done

SB_KEY="$REPO/contrib/sb-keys/db.key"
SB_CERT="$REPO/contrib/sb-keys/db.crt"
SIGN_READY=1
if [[ ! -r "$SB_KEY" || ! -r "$SB_CERT" ]]; then
    SIGN_READY=0
fi
for tool in sbsign sbverify; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        SIGN_READY=0
    fi
done

KERNEL="/usr/lib/modules/$(uname -r)/vmlinuz"
if [[ ! -r "$KERNEL" ]]; then
    echo "SKIP simulate-uki-baker: no distro vmlinuz at $KERNEL"
    exit 0
fi

scratch="$(mktemp -d)"
cleanup() { rm -rf "$scratch"; }
trap cleanup EXIT

out="$scratch/qrab.efi"

pass=0; fail=0
assert() {
    local name="$1"; shift
    if "$@"; then
        pass=$((pass + 1)); echo "ok   $name"
    else
        fail=$((fail + 1)); echo "FAIL $name"
    fi
}

# --- 1. Run the baker in a fresh env --------------------------------
cd "$REPO"
export QRAB_UKI_OUTPUT="$out"
export QRAB_UKI_KERNEL="$KERNEL"
if [[ "$SIGN_READY" == 0 ]]; then
    export QRAB_UKI_SKIP_SIGN=1
else
    export QRAB_UKI_SB_KEY="$SB_KEY"
    export QRAB_UKI_SB_CERT="$SB_CERT"
fi

log="$scratch/baker.log"
if ! "$BAKER" > "$log" 2>&1; then
    echo "FAIL baker exited non-zero; log follows:"
    cat "$log"
    exit 1
fi

assert "baker emitted QRAB_UKI_OK tail" \
    grep -q '^QRAB_UKI_OK path=.* size=[0-9]\+ initrd=[0-9]\+$' "$log"
assert "output PE exists"       test -f "$out"
assert "output PE non-empty"    test -s "$out"

# --- 2. ukify inspect sections --------------------------------------
inspect="$scratch/inspect.txt"
ukify inspect "$out" > "$inspect" 2>&1

# A shell-based extractor: find `.NAME:` followed by indented `size: N bytes`
section_size() {
    local sect="$1"
    awk -v sect=".$sect:" '
        $0 == sect { want=1; next }
        want && /^  size:/ { print $2; exit }
        want && /^\.[a-z]/ { exit }
    ' "$inspect"
}
section_text() {
    local sect="$1"
    awk -v sect=".$sect:" '
        $0 == sect { want=1; next }
        want && /^  text:/ { intext=1; next }
        want && intext && /^    / { sub(/^    /,""); print; next }
        want && /^\.[a-z]/ { exit }
    ' "$inspect"
}

linux_size="$(section_size linux)"
initrd_size="$(section_size initrd)"
cmdline="$(section_text cmdline)"
uname_s="$(section_text uname)"

assert ".linux > 1 MiB"         test "${linux_size:-0}" -gt 1048576
assert ".initrd > 100 KiB"      test "${initrd_size:-0}" -gt 102400
assert ".cmdline carries live flag" \
    bash -c "[[ '$cmdline' == *QRAB_GUEST_INIT_LIVE=1* ]]"
assert ".uname matches running kernel" \
    bash -c "[[ '$uname_s' == *$(uname -r)* ]]"

# --- 3. Signature --------------------------------------------------
if [[ "$SIGN_READY" == 1 ]]; then
    assert "sbverify with db.crt" \
        sbverify --cert="$SB_CERT" "$out"
else
    echo "ok   signature (skipped — keys/sbsign absent)"
fi

# --- 4. Unpack initramfs + check payload contents -------------------
# Extract the raw .initrd blob from the PE, then gunzip + cpio -i.
initrd_bin="$scratch/initrd.bin"
# ukify inspect does not expose raw section dumps; use objcopy/binutils
# instead. Fall back to llvm-objcopy if GNU objcopy is missing.
if command -v objcopy >/dev/null 2>&1; then
    OBJCOPY=objcopy
elif command -v llvm-objcopy >/dev/null 2>&1; then
    OBJCOPY=llvm-objcopy
else
    echo "SKIP initrd content check: objcopy absent"
    echo "PASS=$pass FAIL=$fail"
    [[ "$fail" == 0 ]]
    exit $?
fi

if ! "$OBJCOPY" -O binary --only-section=.initrd "$out" "$initrd_bin" 2>/dev/null; then
    # Some objcopy builds refuse to dump a single section by name
    # on PE-COFF. Use `ukify inspect FILE --section=.initrd` which
    # also writes raw bytes. Best-effort.
    ukify inspect "$out" --section=.initrd > "$initrd_bin" 2>/dev/null || true
fi

if [[ -s "$initrd_bin" ]]; then
    unpack="$scratch/unpack"
    mkdir -p "$unpack"
    ( cd "$unpack" && gzip -dc "$initrd_bin" | cpio --quiet -i 2>/dev/null ) || true

    assert "initramfs has /init"                    test -x "$unpack/init"
    assert "initramfs has /sbin/qrab-guest-init"    test -x "$unpack/sbin/qrab-guest-init"
    assert "initramfs has /sbin/next-stage"         test -x "$unpack/sbin/next-stage"
    assert "initramfs has libc.so.6" \
        bash -c "ls $unpack/lib64/libc.so.* 2>/dev/null | grep -q libc.so"
    assert "initramfs has ld-linux" \
        bash -c "test -L $unpack/lib64/ld-linux-x86-64.so.2 -o -f $unpack/lib64/ld-linux-x86-64.so.2"
else
    echo "SKIP initrd content check (extract produced empty file)"
fi

# --- summary --------------------------------------------------------
echo
echo "PASS=$pass FAIL=$fail"
[[ "$fail" == 0 ]]
