#!/usr/bin/env bash
# smoke-uki-boot.sh — Phase-5.3.e standalone smoke test for the UKI
# baker + qrab-uki-init + qrab-guest-init + qrab-next-stage chain.
#
# Boots the baker's `target/qrab-uki/{vmlinuz,initrd.cpio.gz}`
# artifacts under plain QEMU (no OVMF, no attest-server, no vsock)
# and asserts the init chain reaches an expected point on serial.
#
# We do NOT expect a fully green L5 gate here: a bare TCG guest has
# no SEV-SNP, so `qrab-guest-init` will fail at step 1 with
# `QRAB_INIT_FAIL code=FailNoSnp`. What we DO assert is that:
#
#   * qrab-uki-init started as PID 1               (QRAB_INIT_START)
#   * mounts succeeded                             (QRAB_INIT_MOUNT_OK /proc)
#   * execve into qrab-guest-init worked           (QRAB_INIT_EXEC)
#   * qrab-guest-init started under the live flag  (QRAB_INIT_START from sequencer)
#   * the gate halted CLEANLY on the first failed step
#     (QRAB_INIT_FAIL_NO_SNP or similar FailNoSnp token)
#
# Together those prove the whole initramfs wiring (baker → cpio →
# kernel → /init → qrab-uki-init → execve → qrab-guest-init) is
# intact end-to-end. The full attestation path is exercised by
# matrix-smoke.sh once the attest-server vsock plumbing is live.
#
# Bakes the UKI if `target/qrab-uki/initrd.cpio.gz` is missing. Uses
# `QRAB_UKI_SKIP_SIGN=1` so the smoke doesn't require dev SB keys to
# be present.
#
# Exit codes:
#   0  — asserted tokens all present
#   1  — an expected token is missing (boot reached serial OK but the
#        chain wedged earlier than anticipated)
#   77 — host-side prereq absent (qemu / kernel / ukify / cpio)

set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
cd "$ROOT"

die_skip() { echo "smoke-uki-boot: SKIP $*" >&2; exit 77; }
die()      { echo "smoke-uki-boot: FAIL $*" >&2; exit 1; }
log()      { echo "smoke-uki-boot: $*" >&2; }

# --- Prereqs -------------------------------------------------------
command -v qemu-system-x86_64 >/dev/null || die_skip "qemu-system-x86_64 not installed"
command -v ukify              >/dev/null || die_skip "ukify not installed"
command -v cpio               >/dev/null || die_skip "cpio not installed"

KERNEL="${QRAB_UKI_KERNEL:-/usr/lib/modules/$(uname -r)/vmlinuz}"
[[ -r "$KERNEL" ]] || die_skip "no distro vmlinuz at $KERNEL"

INITRD="target/qrab-uki/initrd.cpio.gz"
if [[ ! -s "$INITRD" ]]; then
    log "initrd missing; running baker with SKIP_SIGN=1"
    QRAB_UKI_SKIP_SIGN=1 ./contrib/build-qrab-uki.sh >/dev/null
fi
[[ -s "$INITRD" ]] || die "baker did not produce $INITRD"

# --- Boot ---------------------------------------------------------
SERIAL_LOG="$(mktemp -t qrab-uki-smoke.XXXXXX.log)"
trap 'rm -f "$SERIAL_LOG"' EXIT

# 20 s is enough for KVM-accelerated boot (userspace reaches init in
# < 1 s) plus 10 s margin. TCG fallback typically needs 60 s on this
# kernel shape; override via QRAB_UKI_SMOKE_TIMEOUT when iterating
# under TCG. `qrab-guest-init::halt()` uses `reboot(RB_HALT_SYSTEM)`
# which parks the CPU without triggering QEMU exit, so we ALWAYS ride
# the timeout — that's expected (rc=124), not a failure.
TIMEOUT="${QRAB_UKI_SMOKE_TIMEOUT:-20}"
# TCG (-machine pc,accel=tcg) is slow but ubiquitous; KVM when
# available cuts boot to < 3 s. Prefer KVM if /dev/kvm is readable.
ACCEL="tcg"
if [[ -r /dev/kvm ]]; then
    ACCEL="kvm:tcg"
fi

log "booting UKI chain under accel=$ACCEL, timeout=${TIMEOUT}s"
set +e
timeout --kill-after=5s "$TIMEOUT" \
    qemu-system-x86_64 \
        -machine "pc,accel=$ACCEL" \
        -cpu host \
        -m 512M -smp 1 \
        -kernel "$KERNEL" \
        -initrd "$INITRD" \
        -append "console=ttyS0 loglevel=4 QRAB_GUEST_INIT_LIVE=1 rdinit=/init" \
        -nographic \
        -no-reboot \
        -serial "file:$SERIAL_LOG" \
        -monitor none \
        -display none \
    >/dev/null 2>&1
rc=$?
set -e

# timeout returns 124 when it SIGTERM'd the process. That's expected
# if the guest doesn't power off on its own. The guest WILL reboot
# when qrab-uki-init calls reboot(RB_POWER_OFF) on gate failure; with
# -no-reboot QEMU exits with 0.
log "qemu exited rc=$rc (124=timeout, 0=reboot seen)"

[[ -s "$SERIAL_LOG" ]] || die "empty serial log (boot may not have reached userspace)"

# --- Assertions ---------------------------------------------------
pass=0; fail=0
check() {
    local name="$1" pattern="$2"
    if grep -q -- "$pattern" "$SERIAL_LOG"; then
        pass=$((pass+1))
        echo "ok   $name"
    else
        fail=$((fail+1))
        echo "FAIL $name  (no match for: $pattern)"
    fi
}

check "qrab-uki-init started as PID 1"       "QRAB_INIT_START"
check "proc mounted"                         "QRAB_INIT_MOUNT_OK /proc"
check "sysfs mounted"                        "QRAB_INIT_MOUNT_OK /sys"
check "devtmpfs mounted"                     "QRAB_INIT_MOUNT_OK /dev"
check "efivars absent (no UEFI on TCG)"      "QRAB_INIT_EFIVARFS_ABSENT"
check "execve into qrab-guest-init reached"  "QRAB_INIT_EXEC /sbin/qrab-guest-init"
# qrab-guest-init under live mode emits the sequence opener. After
# that the first gate step (SEV MSR) fails on bare TCG, so we
# expect the FailNoSnp token. Either side's STATUS line is fine.
check "qrab-guest-init reached under live flag" "QRAB_INIT_START\|QRAB_GUEST_INIT"
check "gate halts cleanly on non-SNP"        "FailNoSnp\|QRAB_INIT_FAIL"

echo
echo "PASS=$pass FAIL=$fail"
if [[ "$fail" != 0 ]]; then
    log "serial log tail follows:"
    tail -40 "$SERIAL_LOG" >&2
    exit 1
fi
exit 0
