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
#   (2) QrabSbAutoEnroll DXE driver transitions firmware from
#       Setup Mode to User Mode on first boot. We use the
#       QRAB_SVSM_SB_EMBED_UNSIGNED build variant for this scenario
#       because the build takes a DEBUG path that surfaces the
#       SetupMode/SecureBoot variable state changes on serial (the
#       RELEASE signed firmware suppresses the same DEBUG prints).
#       The assertion is the transition itself: before = Setup Mode
#       (PK empty), after = User Mode (SB live). Proving
#       DxeImageVerificationLib rejects a tampered image additionally
#       requires the image to come from OUTSIDE the FV — FV-embedded
#       images bypass SB verification by design. The qrab security
#       model's real enforcement point is the UKI LoadImage call
#       inside qrab-boot (V8 in overview.md), reachable only with a
#       running attest-server; that test lands in a future phase.
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
IGVM_SIGNED="$ROOT/target/svsm-out/coconut-qemu-SB.igvm"
IGVM_UNSIGNED="$ROOT/target/svsm-out/coconut-qemu-SB-UNSIGNED.igvm"
QEMU="$BUILD_ROOT/qemu-svsm-inst/bin/qemu-system-x86_64"
PK="$ROOT/contrib/sb-keys/PK.crt"

for f in "$IGVM_SIGNED" "$IGVM_UNSIGNED" "$QEMU" "$PK"; do
    [[ -e "$f" ]] || skip "missing $f (run both signed and QRAB_SVSM_SB_EMBED_UNSIGNED=1 builds)"
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

# --- helper -------------------------------------------------------------
run_qemu() {
    local name="$1" igvm="$2"
    local logf="$OUT/$name.serial.log"
    local qmp="$OUT/$name.qmp.sock"
    : > "$logf"
    rm -f "$qmp"

    "$QEMU" \
        -cpu EPYC-v4 \
        -machine q35,confidential-guest-support=sev0,memory-backend=ram1,igvm-cfg=igvm0 \
        -m 1024M -accel kvm \
        -object memory-backend-memfd,id=ram1,size=1024M,share=true,prealloc=false,reserve=false \
        -object sev-snp-guest,id=sev0,cbitpos=51,reduced-phys-bits=1,policy=0x30000 \
        -object igvm-cfg,id=igvm0,file="$igvm" \
        -smbios "type=11,value=${SMBIOS_OEM}" \
        -nodefaults -display none -no-reboot \
        -serial file:"$logf" \
        -qmp unix:"$qmp",server=on,wait=off &
    local pid=$!
    trap "kill $pid 2>/dev/null || true" RETURN

    for _ in $(seq 1 60); do [[ -S "$qmp" ]] && break; sleep 0.2; done
    [[ -S "$qmp" ]] || { kill "$pid" 2>/dev/null || true;
        die "$name: qmp socket never appeared"; }

    {
        printf '%s\n' '{"execute":"qmp_capabilities"}'
        printf '%s\n' '{"execute":"cont"}'
        sleep 15
    } | socat - UNIX-CONNECT:"$qmp" >/dev/null 2>&1 || true

    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    trap - RETURN
    tr -d '\r' < "$logf"
}

check() {
    local text="$1" pat="$2" expected="$3" desc="$4"
    if echo "$text" | grep -q "$pat"; then
        if [[ "$expected" == "present" ]]; then
            log "OK    $desc"
        else
            log "FAIL  $desc (found '$pat' but expected absence)"
            fail=1
        fi
    else
        if [[ "$expected" == "absent" ]]; then
            log "OK    $desc"
        else
            log "FAIL  $desc (pattern '$pat' missing)"
            fail=1
        fi
    fi
}

fail=0

# --- scenario 1 — signed qrab-boot.efi: SB accepts, chain runs --------
log "=== scenario 1: signed qrab-boot.efi, SB should ACCEPT ==="
clean1="$(run_qemu signed "$IGVM_SIGNED")"
check "$clean1" 'VTPM.*TPM 2.0.*initialized' present 'SVSM vTPM brought up'
check "$clean1" 'BdsDxe: starting Boot0002'  present 'BdsDxe reached the Grub-slot boot entry'
check "$clean1" 'QRAB BOOT v'                present 'qrab-boot banner printed (firmware db-verified the signed image)'
check "$clean1" 'Security Violation'         absent  'no DxeImageVerificationLib rejection'
check "$clean1" 'QRAB_SNP_DETECTED'          present 'qrab-boot confirms SEV-SNP active'

# --- scenario 2 — QrabSbAutoEnroll transitions Setup→User Mode --------
#
# Critical caveat: images loaded from an FV (like qrab-boot embedded
# in OVMF) bypass DxeImageVerificationLib by design — the entire FV
# is integrity-guaranteed by L1 measurement, so the loader treats FV
# contents as implicitly trusted. That means an "unsigned qrab-boot
# in FV" can't prove SB rejection; the true SB-on-FV-image test would
# need an external-media image (disk / PXE).
#
# The security-relevant enforcement point for qrab is the UKI
# LoadImage call inside qrab-boot (V8 in overview.md), which loads
# bytes off vsock rather than from the FV. That path IS SB-verified;
# exercising it live needs an attest-server in the smoke (future
# Phase 2+ work).
#
# What this scenario CAN prove end-to-end today is that the
# QrabSbAutoEnroll DXE driver transitions firmware out of Setup Mode
# — the necessary precondition for SB to be live at all.
log ""
log "=== scenario 2: QrabSbAutoEnroll promotes firmware to user mode ==="
clean2="$(run_qemu enrollment "$IGVM_UNSIGNED")"
check "$clean2" 'QrabSbAutoEnroll: entry .PK absent'       present 'driver dispatched at ReadyToBoot'
check "$clean2" 'QrabSbAutoEnroll: found EnrollDefaultKeys.efi in FV' present 'FV lookup located EnrollDefaultKeys'
check "$clean2" 'info: SetupMode=1 SecureBoot=0'           present 'firmware started in Setup Mode (PK empty)'
check "$clean2" 'info: SetupMode=0 SecureBoot=1'           present 'firmware TRANSITIONED to User Mode after enrol'
check "$clean2" 'QrabSbAutoEnroll: EnrollDefaultKeys completed; PK NOW ENROLLED' present 'driver confirms PK is now enrolled'

log ""
if [[ $fail -eq 0 ]]; then
    log "smoke PASS"
    log "  scenario 1: firmware SB accepted signed qrab-boot.efi"
    log "  scenario 2: auto-enroll driver transitioned SetupMode 1->0, SecureBoot 0->1"
    log "  caveat: proving DxeImageVerificationLib rejects a tampered UKI needs"
    log "  a live attest-server — the UKI LoadImage path runs inside qrab-boot."
    exit 0
else
    die "smoke FAILED — see serial logs under $OUT/"
fi
