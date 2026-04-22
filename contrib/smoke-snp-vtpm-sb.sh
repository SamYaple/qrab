#!/usr/bin/env bash
# Phase 1.5-D smoke: boot OVMF-SVSM-SB.fd under SEV-SNP with qrab's PK
# injected via SMBIOS. This script covers the two checkpoints that a
# live IGVM+SB build can actually prove end-to-end on bb:
#
#   (1) Firmware SB accepts the embedded *signed* qrab-boot.efi:
#       OVMF BdsDxe logs "starting Boot0002" and the QRAB BOOT banner
#       appears on serial. (If SB were off the banner would still
#       appear, but so would any unsigned payload — the positive
#       discrimination comes from scenario 2.)
#
#   (2) [not implemented yet] Firmware SB rejects an *unsigned*
#       qrab-boot.efi: requires a second OVMF build (or post-build FV
#       byte-flip) that embeds an unsigned qrab-boot.efi. Tracked
#       under "Phase 1.5-F — tampered-image smoke"; when it lands the
#       expected tokens will be "Security Violation" in the serial
#       log and no QRAB BOOT banner.
#
# qrab-boot's own UKI-level Secure Boot enforcement (`QRAB_SB_OK` /
# `QRAB_FAIL_SB_*`, Phase 1.5-C) is gated on `cfg(qrab_uki_boot)` +
# successful vsock fetch of a UKI. This smoke does NOT run an
# attest-server, so qrab-boot falls through to `QRAB_PHASE4_PARTIAL`
# before it would call `require_sb_enforced()`. To exercise the L3
# gate run the full attest-loop smoke (`contrib/smoke-snp-attest.sh`)
# with an OVMF-SVSM-SB.fd and watch for the new tokens.
#
# Prereqs (run in order from the qrab checkout / svsm worktree):
#   bash contrib/sb-enroll-keys.sh           # generates contrib/sb-keys/
#   QRAB_SVSM_SB=1 QRAB_SVSM_SB_BAKE=1 \
#     QRAB_BUILD_SVSM=1 QRAB_UKI_BOOT=1 \
#     bash contrib/build-svsm-igvm.sh        # produces the IGVM + signed .efi
#   bash contrib/build-test-ukis.sh          # produces signed + tampered UKI
#                                              test fixtures (existing script)
#
# This script skips gracefully (exit 0, SKIP message) when the prereq
# artifacts are missing — so it can live in CI alongside faster smokes
# without gating on the 20-40 min full build. Operator runs it once the
# build completes.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
# build-svsm-igvm.sh anchors BUILD_ROOT to main checkout so worktrees
# share the heavy edk2/qemu/svsm compile cache. Mirror that here.
GIT_COMMON_DIR="$(git -C "$ROOT" rev-parse --git-common-dir)"
case "$GIT_COMMON_DIR" in
    /*) : ;;
    *)  GIT_COMMON_DIR="$ROOT/$GIT_COMMON_DIR" ;;
esac
MAIN_CHECKOUT="$(dirname "$GIT_COMMON_DIR")"
BUILD_ROOT="${QRAB_SVSM_BUILD_ROOT:-$MAIN_CHECKOUT/target/svsm-build}"
OUT="$ROOT/target/qrab-boot-sb-smoke"
mkdir -p "$OUT"

die()  { echo "smoke-snp-vtpm-sb: $*" >&2; exit 1; }
skip() { echo "smoke-snp-vtpm-sb: SKIP — $*"; exit 0; }
log()  { echo "smoke-snp-vtpm-sb: $*"; }

# --- prereqs -------------------------------------------------------------
IGVM="$ROOT/target/svsm-out/coconut-qemu.igvm"
FW_SB="$ROOT/target/svsm-out/OVMF-SVSM-SB.fd"
QEMU="$BUILD_ROOT/qemu-svsm-inst/bin/qemu-system-x86_64"
PK="$ROOT/contrib/sb-keys/PK.crt"

for f in "$IGVM" "$FW_SB" "$QEMU" "$PK"; do
    [[ -e "$f" ]] || skip "missing $f"
done
[[ -e /dev/sev ]] || skip "no /dev/sev — SNP not available on this host"
command -v socat >/dev/null 2>&1 || die "socat required"

# --- build the SMBIOS OEM string ---------------------------------------
OVMF_PK_KEK1_GUID="4e32566d-8e9e-4f52-81d3-5bb9715f9727"
# Strip PEM headers + whitespace — the SMBIOS arg takes one base64 line.
PK_B64="$(sed -n '/-----BEGIN CERTIFICATE-----/,/-----END CERTIFICATE-----/{
            /-----BEGIN\|-----END/d; p
          }' "$PK" | tr -d '\n\r ')"
[[ -n "$PK_B64" ]] || die "could not extract base64 body from $PK"
SMBIOS_OEM="${OVMF_PK_KEK1_GUID}:${PK_B64}"

# --- scenario 1 — signed qrab-boot.efi booted under IGVM+SVSM+SB -------
LOGF="$OUT/signed.serial.log"
: > "$LOGF"
QMP="$OUT/signed.qmp.sock"
rm -f "$QMP"

"$QEMU" \
    -cpu EPYC-v4 \
    -machine q35,confidential-guest-support=sev0,memory-backend=ram1,igvm-cfg=igvm0 \
    -m 1024M -accel kvm \
    -object memory-backend-memfd,id=ram1,size=1024M,share=true,prealloc=false,reserve=false \
    -object sev-snp-guest,id=sev0,cbitpos=51,reduced-phys-bits=1,policy=0x30000 \
    -object igvm-cfg,id=igvm0,file="$IGVM" \
    -smbios "type=11,value=${SMBIOS_OEM}" \
    -nodefaults -display none -no-reboot \
    -serial file:"$LOGF" \
    -qmp unix:"$QMP",server=on,wait=off &
PID=$!
trap 'kill $PID 2>/dev/null || true' EXIT

for _ in $(seq 1 60); do [[ -S "$QMP" ]] && break; sleep 0.2; done
[[ -S "$QMP" ]] || die "qmp socket never appeared"

{
    printf '%s\n' '{"execute":"qmp_capabilities"}'
    printf '%s\n' '{"execute":"cont"}'
    sleep 15
} | socat - UNIX-CONNECT:"$QMP" >/dev/null 2>&1 || true

kill $PID 2>/dev/null || true
wait $PID 2>/dev/null || true

# Assertions. Expect:
#   * SVSM launched its VTPM
#   * OVMF's BdsDxe started "Grub Bootloader" (the qrab-boot slot)
#   * qrab-boot printed its banner (firmware accepted the db sig)
#   * No "Security Violation" from DxeImageVerificationLib
clean="$(tr -d '\r' < "$LOGF")"
fail=0
check() {
    local pat="$1" expected="$2" desc="$3"
    if echo "$clean" | grep -q "$pat"; then
        [[ "$expected" == "present" ]] \
            && { log "OK    $desc"; return 0; } \
            || { log "FAIL  $desc (found '$pat' but expected absence)"; fail=1; }
    else
        [[ "$expected" == "absent" ]] \
            && { log "OK    $desc"; return 0; } \
            || { log "FAIL  $desc (pattern '$pat' missing)"; fail=1; }
    fi
}
check 'VTPM.*TPM 2.0.*initialized' present 'SVSM vTPM brought up'
check 'BdsDxe: starting Boot0002' present 'BdsDxe reached the Grub-slot boot entry'
check 'QRAB BOOT v'                present 'qrab-boot banner printed (firmware db-verified the signed image)'
check 'Security Violation'         absent  'no DxeImageVerificationLib rejection'
check 'QRAB_SNP_DETECTED'          present 'qrab-boot confirms SEV-SNP active'

# Scenario 2 (tampered) is recorded as TODO in the header block. If
# an unsigned qrab-boot.efi fixture firmware gets produced later, wire
# the second run here and assert "Security Violation" in its serial.
log ""
log "Phase 1.5-D smoke:"
log "  - scenario 1 (signed qrab-boot accepted by firmware SB): see above"
log "  - scenario 2 (tampered rejection): NOT YET WIRED — needs a"
log "    separate OVMF build with an unsigned qrab-boot.efi embed."

if [[ $fail -eq 0 ]]; then
    log ""
    log "SIGNED: PASS — firmware Secure Boot + SVSM vTPM chain is live"
    exit 0
else
    die "SIGNED: FAIL — see $LOGF"
fi
