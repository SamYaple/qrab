#!/usr/bin/env bash
# Phase 4 fault-injection smoke: prove the R20 Phase 3 vTPM gate halts
# under **SEV-SNP** when the firmware exposes Tcg2 but no SVSM-backed
# vTPM stands behind it.
#
# Companion to `contrib/matrix-smoke.sh` scenario 13 (TCG + vTPM-
# required → `QRAB_FAIL_NO_VTPM TCG2_ABSENT`). This smoke covers the
# other half of the failure matrix: the SNP path where Tcg2Dxe IS
# loaded (AmdSev OVMF ships it) but reports `tpm_present == FALSE`
# because no backing TPM — no SVSM, no swtpm on the CoCo path. qrab-
# boot's probe should surface `QRAB_FAIL_NO_VTPM TPM_NOT_PRESENT` on
# serial, proving the Phase 3 gate catches the "SVSM silently failed
# to bring up the vTPM" failure mode (per `docs/security/level-3-
# qrab-boot.md` V7 — vTPM enforcement inside L3).
#
# This is DELIBERATELY an SNP-but-no-SVSM boot: the production path
# is always SNP+SVSM, so "SNP without SVSM" is a fault-injection
# mirror of the SVSM-missing failure class. We deliberately do NOT
# need to corrupt or zero the svsm.bin inside an IGVM blob — the
# vTPM gate's halt condition is the same whether SVSM was never
# started or started-but-broken.
#
# Prereqs:
#   - /dev/sev + /dev/kvm                (host is SEV-SNP capable)
#   - target/ovmf/OVMF.fd                (AmdSev OVMF with TPM2_ENABLE)
#   - qemu-system-x86_64 on PATH         (system QEMU, IGVM not needed)
#
# Build details: qrab-boot.efi compiled WITHOUT the vTPM opt-out so
# the Phase 3 probe compiles in. We keep `QRAB_ALLOW_NON_SNP=1` UNSET
# so the SNP gate also compiles in — both gates should fire in order,
# with the SNP gate passing first and the vTPM gate halting second.
#
# Exit codes:
#   0  — QRAB_FAIL_NO_VTPM observed on serial (gate fired as expected)
#   1  — QRAB_FAIL_NO_VTPM missing (Phase 3 regression)
#  77  — host lacks SNP / AmdSev OVMF missing

set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
BOOT_CRATE="$ROOT/projects/qrab-boot"
EFI="$BOOT_CRATE/target/x86_64-unknown-uefi/release/qrab-boot.efi"
FW="$ROOT/target/ovmf/OVMF.fd"
OUT="${OUT:-$ROOT/target/qrab-boot-snp-vtpm-gate/run-$$}"
SERIAL="$OUT/serial.log"
BOOT_TIMEOUT_SECS="${QRAB_VTPM_GATE_TIMEOUT:-20}"
RPB="${QRAB_SNP_REDUCED_PHYS_BITS:-1}"

log() { echo "smoke-snp-vtpm-gate: $*" >&2; }
die() { log "error: $*"; exit 1; }
skip() { log "skipped: $*"; exit 77; }

# ---- prerequisites -------------------------------------------------------
[[ -f "$FW" ]] || skip "no AmdSev OVMF at $FW — run contrib/build-ovmf.sh"
[[ -e /dev/sev ]] || skip "no /dev/sev — SNP unavailable on this host"
[[ -e /dev/kvm ]] || skip "no /dev/kvm"
command -v qemu-system-x86_64 >/dev/null || die "qemu-system-x86_64 not on PATH"
command -v socat >/dev/null || die "socat not on PATH"
command -v cargo >/dev/null || die "cargo not on PATH"

# Regen dev attest-server keypair if missing (build.rs needs it, even
# though this smoke never contacts an attest-server).
if [[ ! -s "$ROOT/contrib/attest-keys/server.sec1" ]]; then
    log "attest keys missing; regenerating via contrib/gen-attest-keys.sh"
    bash "$ROOT/contrib/gen-attest-keys.sh" >/dev/null 2>&1 \
        || die "gen-attest-keys.sh failed"
fi

# ---- build qrab-boot.efi with the vTPM gate COMPILED IN -----------------
#
# The matrix smokes default to `QRAB_ALLOW_NO_VTPM=1` so plain-TCG
# regression runs aren't blocked by a missing TCG2 stack. Here we
# want the opposite: the default cfg with no opt-out, so
# `measure::require_and_probe_vtpm()` halts the bootloader when the
# Tcg2 protocol reports no backing TPM. Unset the opt-out explicitly
# in case the invoking shell inherited it.
unset QRAB_ALLOW_NO_VTPM
# Keep the SNP gate compiled in too — this smoke REQUIRES the SNP
# gate to pass first (`QRAB_SNP_DETECTED`), then the vTPM gate to
# halt. If SNP gate halts first (TCG accel, no hardware), the test
# is inconclusive rather than a pass.
unset QRAB_ALLOW_NON_SNP
log "building qrab-boot.efi (QRAB_ALLOW_NO_VTPM unset, QRAB_ALLOW_NON_SNP unset)"
(cd "$BOOT_CRATE" && cargo build --release >/dev/null)
[[ -f "$EFI" ]] || die "$EFI not produced"

# AmdSev OVMF embeds `qrab-boot.efi` into its firmware volume at
# build time — BdsDxe's Boot0002 loads THAT copy, not the one we
# staged on the ESP below. If `$FW` is older than the freshly-
# built qrab-boot.efi, the smoke would boot a stale binary and
# never exercise the Phase-3 probe we just compiled in. Rebuild
# OVMF before every run so the FV payload matches the source
# tree. `contrib/build-ovmf.sh` is idempotent — when the staged
# edk2 tree is clean and the qrab-boot.efi hasn't changed, the
# script short-circuits in a few seconds.
if [[ "$FW" -ot "$EFI" || ! -f "$FW" ]]; then
    log "rebuilding OVMF (qrab-boot.efi newer than $FW)"
    bash "$ROOT/contrib/build-ovmf.sh" >/dev/null \
        || die "contrib/build-ovmf.sh failed — inspect target/edk2-build logs"
    [[ "$FW" -nt "$EFI" ]] || die "build-ovmf.sh ran but $FW still older than $EFI"
fi

# ---- stage + launch ------------------------------------------------------
mkdir -p "$OUT"
: > "$SERIAL"

# Bootable stub ESP. qrab-boot.efi is staged at the EFI BOOTX64 path so
# BdsDxe's default boot option picks it. No UKI, no vsock, no attest-
# server — this smoke is strictly about the early probe halt.
ESP="$OUT/esp.raw"
truncate -s 32M "$ESP"
mkfs.vfat -F 32 -n QRABBOOT "$ESP" >/dev/null
mmd -i "$ESP" ::/EFI ::/EFI/BOOT
mcopy -i "$ESP" "$EFI" ::/EFI/BOOT/BOOTX64.EFI

log "launching SNP guest (${BOOT_TIMEOUT_SECS}s budget)"
QMP="$OUT/qmp.sock"
rm -f "$QMP"

set +e
timeout "$BOOT_TIMEOUT_SECS" qemu-system-x86_64 --preconfig \
    -machine q35,confidential-guest-support=sev0,firmware="$FW" \
    -m 512M -accel kvm -cpu EPYC-v4 \
    -object sev-snp-guest,id=sev0,reduced-phys-bits="$RPB",cbitpos=51,sev-device=/dev/sev \
    -drive if=ide,format=raw,file="$ESP" \
    -nodefaults -display none -no-reboot \
    -serial file:"$SERIAL" \
    -qmp unix:"$QMP",server=on,wait=off &
QEMU_PID=$!

# Kick preconfig as soon as QMP is up; otherwise the guest sits
# paused at pre-init forever.
for _ in $(seq 1 40); do [[ -S "$QMP" ]] && break; sleep 0.1; done
if [[ -S "$QMP" ]]; then
    {
        printf '%s\n' '{"execute":"qmp_capabilities"}'
        printf '%s\n' '{"execute":"x-exit-preconfig"}'
        printf '%s\n' '{"execute":"cont"}'
        sleep "$BOOT_TIMEOUT_SECS"
    } | socat - UNIX-CONNECT:"$QMP" >/dev/null 2>&1 || true
else
    log "warn: QMP socket never appeared; QEMU may have died at launch"
fi
kill "$QEMU_PID" 2>/dev/null || true
wait "$QEMU_PID" 2>/dev/null || true
set -e

# ---- assertions ----------------------------------------------------------
# The SNP gate should pass first (hardware supports it), then the vTPM
# probe should halt. Surfacing both proves the ordering is right.
fail=0
clean="$(tr -d '\r' < "$SERIAL")"
grep_present() {
    local pat="$1" desc="$2"
    if echo "$clean" | grep -qE "$pat"; then
        log "OK    $desc"
    else
        log "FAIL  $desc — pattern '$pat' absent"
        fail=1
    fi
}
grep_absent() {
    local pat="$1" desc="$2"
    if echo "$clean" | grep -qE "$pat"; then
        log "FAIL  $desc — pattern '$pat' surfaced"
        fail=1
    else
        log "OK    $desc"
    fi
}

grep_present 'QRAB BOOT v'              'qrab-boot banner (firmware loaded + started qrab-boot.efi)'
grep_present 'QRAB_SNP_DETECTED'        'SNP gate PASSED (hardware + cfg(not qrab_allow_non_snp))'
grep_present 'QRAB_FAIL_NO_VTPM'        'vTPM gate HALTED (cfg(qrab_require_vtpm) + no SVSM)'
grep_absent  'QRAB_VTPM_SVSM_OK'        'no false-positive vTPM probe success'
grep_absent  'QRAB_VSOCK_FOUND'         'boot halted BEFORE reaching vsock stage'

if [[ $fail -eq 0 ]]; then
    log "PASS — SNP+no-SVSM fault surfaced QRAB_FAIL_NO_VTPM as expected"
    exit 0
else
    log "FAIL — see $SERIAL"
    exit 1
fi
