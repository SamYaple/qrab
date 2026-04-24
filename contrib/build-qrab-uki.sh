#!/usr/bin/env bash
# build-qrab-uki.sh — bake a signed Unified Kernel Image for the L5
# PID-1 smoke path.
#
# Produces: target/qrab-uki/qrab.efi (PE; SB-signed if keys present).
#
# Layout inside the initramfs cpio:
#     /init              → qrab-uki-init  (PID 1; mounts + stages efivars)
#     /sbin/qrab-guest-init
#     /sbin/next-stage   → qrab-next-stage  (smoke stub; swap in prod)
#     /lib64/ld-linux-x86-64.so.2 + libc.so.6 + libgcc_s.so.1
#
# UKI sections:
#     .linux    = distro vmlinuz
#     .initrd   = the cpio above (gzipped)
#     .cmdline  = "console=ttyS0 loglevel=4 QRAB_GUEST_INIT_LIVE=1"
#
# Why no dracut: the UKI path exec's `/sbin/qrab-guest-init` as the
# L5 gate. dracut's /init expects a pivot to a real rootfs; we run
# entirely in initramfs. Rolling our own `/init` is leaner and the
# whole chain is three static-ish binaries + libc.
#
# Overrides:
#   QRAB_UKI_KERNEL   — path to vmlinuz            (default: distro)
#   QRAB_UKI_OUTPUT   — output .efi path           (default: target/qrab-uki/qrab.efi)
#   QRAB_UKI_SB_KEY   — SB private key path        (default: contrib/sb-keys/db.key)
#   QRAB_UKI_SB_CERT  — SB certificate path        (default: contrib/sb-keys/db.crt)
#   QRAB_UKI_CMDLINE  — kernel cmdline             (default: above)
#   QRAB_UKI_SKIP_SIGN=1 — skip sbsign (dev builds)
#
# Regression coverage: contrib/qrab-boot-initrd-module/tests/
# simulate-uki-baker.sh (staging-tree shape) plus
# projects/manager/tests/firmware.rs::qrab_uki_*.

set -euo pipefail

die() { echo "build-qrab-uki: $*" >&2; exit 1; }
log() { echo "build-qrab-uki: $*" >&2; }

QRAB_REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$QRAB_REPO_ROOT"

KERNEL="${QRAB_UKI_KERNEL:-/usr/lib/modules/$(uname -r)/vmlinuz}"
OUTPUT="${QRAB_UKI_OUTPUT:-target/qrab-uki/qrab.efi}"
SB_KEY="${QRAB_UKI_SB_KEY:-contrib/sb-keys/db.key}"
SB_CERT="${QRAB_UKI_SB_CERT:-contrib/sb-keys/db.crt}"
CMDLINE="${QRAB_UKI_CMDLINE:-console=ttyS0 loglevel=4 QRAB_GUEST_INIT_LIVE=1}"
SKIP_SIGN="${QRAB_UKI_SKIP_SIGN:-0}"

[[ -r "$KERNEL" ]] || die "kernel not readable: $KERNEL (set QRAB_UKI_KERNEL)"
command -v ukify  >/dev/null || die "ukify not installed"
command -v cpio   >/dev/null || die "cpio not installed"
if [[ "$SKIP_SIGN" != 1 ]]; then
    [[ -r "$SB_KEY"  ]] || die "SB key not readable: $SB_KEY (or set QRAB_UKI_SKIP_SIGN=1)"
    [[ -r "$SB_CERT" ]] || die "SB cert not readable: $SB_CERT"
    command -v sbsign >/dev/null || die "sbsign not installed"
fi

# --- 1. cargo build -------------------------------------------------
log "building qrab-guest-init + qrab-uki-init + qrab-next-stage (release)"
cargo build -p qrab-guest-init --release --bins \
    --bin qrab-guest-init --bin qrab-uki-init --bin qrab-next-stage

BIN_DIR="target/release"
[[ -x "$BIN_DIR/qrab-guest-init"  ]] || die "missing $BIN_DIR/qrab-guest-init"
[[ -x "$BIN_DIR/qrab-uki-init"        ]] || die "missing $BIN_DIR/qrab-uki-init"
[[ -x "$BIN_DIR/qrab-next-stage"  ]] || die "missing $BIN_DIR/qrab-next-stage"

# --- 2. staging tree ------------------------------------------------
STAGE="$(mktemp -d -t qrab-uki.XXXXXX)"
trap 'rm -rf "$STAGE"' EXIT

log "staging initramfs under $STAGE"
mkdir -p "$STAGE/sbin" "$STAGE/lib64" "$STAGE/lib" \
         "$STAGE/proc" "$STAGE/sys" "$STAGE/dev" "$STAGE/run" \
         "$STAGE/sys/firmware/efi/efivars"

install -m 0755 "$BIN_DIR/qrab-uki-init"        "$STAGE/init"
install -m 0755 "$BIN_DIR/qrab-guest-init"  "$STAGE/sbin/qrab-guest-init"
install -m 0755 "$BIN_DIR/qrab-next-stage"  "$STAGE/sbin/next-stage"

# --- 3. shared library closure -------------------------------------
# `ldd` every bin, copy every hit (and the dynamic linker itself) into
# /lib64. Skip linux-vdso.so.1 which the kernel injects at runtime.
closure_for() {
    local bin="$1"
    ldd "$bin" 2>/dev/null \
        | awk '{
            if ($1 == "linux-vdso.so.1" || $1 == "linux-gate.so.1") next
            if ($3 ~ /^\//) print $3
            else if ($1 ~ /^\//) print $1
        }'
}

declare -A SEEN=()
for bin in "$BIN_DIR/qrab-uki-init" "$BIN_DIR/qrab-guest-init" "$BIN_DIR/qrab-next-stage"; do
    while read -r lib; do
        [[ -z "$lib" ]] && continue
        [[ -n "${SEEN[$lib]:-}" ]] && continue
        SEEN[$lib]=1
        base="$(basename "$lib")"
        install -D -m 0755 "$lib" "$STAGE/lib64/$base"
    done < <(closure_for "$bin")
done
# /lib64/ld-linux-x86-64.so.2 is the kernel-visible interp path; if
# ldd reported it under a different name, synthesize the expected
# symlink.
if [[ ! -e "$STAGE/lib64/ld-linux-x86-64.so.2" ]]; then
    for cand in "$STAGE"/lib64/ld-linux-*.so.* "$STAGE"/lib64/ld-2.*.so; do
        [[ -f "$cand" ]] || continue
        ln -sf "$(basename "$cand")" "$STAGE/lib64/ld-linux-x86-64.so.2"
        break
    done
fi

# --- 4. assemble cpio ----------------------------------------------
# Intermediate initramfs lands next to the final UKI so the
# `contrib/smoke-uki-boot.sh` harness can boot `-kernel` + `-initrd`
# directly without having to objcopy the .initrd section back out of
# the PE. Stable path = `target/qrab-uki/initrd.cpio.gz`.
mkdir -p "$(dirname "$OUTPUT")"
INITRD="$(dirname "$OUTPUT")/initrd.cpio.gz"
log "packing cpio → $INITRD"
( cd "$STAGE" && find . -print0 \
    | cpio --null --create --format=newc --quiet ) \
    | gzip -9 > "$INITRD"
INITRD_SIZE="$(stat -c '%s' "$INITRD")"
log "initramfs gz size: $INITRD_SIZE bytes"

# --- 5. ukify build -------------------------------------------------
UKIFY_ARGS=(
    build
    --linux="$KERNEL"
    --initrd="$INITRD"
    --cmdline="$CMDLINE"
    --output="$OUTPUT"
)
if [[ "$SKIP_SIGN" != 1 ]]; then
    UKIFY_ARGS+=(
        --signtool=sbsign
        --secureboot-private-key="$SB_KEY"
        --secureboot-certificate="$SB_CERT"
    )
fi

log "ukify build → $OUTPUT"
ukify "${UKIFY_ARGS[@]}"

# --- 6. report ------------------------------------------------------
UKI_SIZE="$(stat -c '%s' "$OUTPUT")"
log "UKI built: $OUTPUT ($UKI_SIZE bytes)"

if [[ "$SKIP_SIGN" != 1 ]] && command -v sbverify >/dev/null; then
    if sbverify --cert="$SB_CERT" "$OUTPUT" >/dev/null 2>&1; then
        log "sbverify: OK"
    else
        log "sbverify: FAIL (cert/signature mismatch)"
        exit 2
    fi
fi

# Emit a machine-readable tail the test harness can grep.
echo "QRAB_UKI_OK path=$OUTPUT size=$UKI_SIZE initrd=$INITRD_SIZE"
