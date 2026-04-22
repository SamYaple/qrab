#!/usr/bin/env bash
# Phase 1.5-D smoke: boot OVMF-SVSM-SB.fd under SEV-SNP with qrab's PK
# injected via SMBIOS, then exercise two scenarios:
#
#   (1) signed UKI  — firmware Secure Boot accepts, qrab-boot logs
#                     QRAB_SB_OK then QRAB_LOADIMAGE_OK.
#   (2) tampered UKI — firmware rejects via EFI_SECURITY_VIOLATION,
#                     qrab-boot logs QRAB_FAIL_SB.
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
OUT="$ROOT/target/qrab-boot-sb-smoke"
mkdir -p "$OUT"

die()  { echo "smoke-snp-vtpm-sb: $*" >&2; exit 1; }
skip() { echo "smoke-snp-vtpm-sb: SKIP — $*"; exit 0; }
log()  { echo "smoke-snp-vtpm-sb: $*"; }

# --- prereqs -------------------------------------------------------------
IGVM="$ROOT/target/svsm-out/coconut-qemu.igvm"
FW_SB="$ROOT/target/svsm-out/OVMF-SVSM-SB.fd"
QEMU="$ROOT/target/svsm-build/qemu-svsm-inst/bin/qemu-system-x86_64"
PK="$ROOT/contrib/sb-keys/PK.crt"
FIXTURES="$ROOT/projects/qrab-boot-tests/fixtures"
UKI_OK="$FIXTURES/uki-valid.efi"
UKI_BAD="$FIXTURES/uki-tampered.efi"

for f in "$IGVM" "$FW_SB" "$QEMU" "$PK" "$UKI_OK" "$UKI_BAD"; do
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

# --- helper: one QEMU launch with given UKI fixture --------------------
run_scenario() {
    local name="$1" uki="$2" expect_ok="$3" expect_fail="$4"
    local log="$OUT/$name.serial.log"
    : > "$log"
    local qmp="$OUT/$name.qmp.sock"
    rm -f "$qmp"

    # The IGVM file carries OVMF + SVSM and dictates firmware layout;
    # the CLI still feeds qemu --enable-igvm plus SNP + EPYC-v4 + the
    # -smbios OEM string for PK injection. We mount the UKI as a
    # virtio-blk device; qrab-boot fetches it via vsock in production,
    # but the smoke's stub-fixture path uses this disk mount and the
    # QRAB_UKI_BOOT=1 build that exercises LoadImage directly.
    log "run_scenario $name uki=$uki"
    "$QEMU" \
        -cpu EPYC-v4 \
        -machine q35,confidential-guest-support=sev0,memory-backend=ram1,igvm-cfg=igvm0 \
        -m 1024M -accel kvm \
        -object memory-backend-memfd,id=ram1,size=1024M,share=true,prealloc=false,reserve=false \
        -object sev-snp-guest,id=sev0,cbitpos=51,reduced-phys-bits=1,policy=0x130000 \
        -object igvm-cfg,id=igvm0,file="$IGVM" \
        -smbios "type=11,value=${SMBIOS_OEM}" \
        -drive file="$uki",if=none,id=uki0,format=raw,readonly=on \
        -device virtio-blk-pci,drive=uki0 \
        -nodefaults -display none -no-reboot \
        -serial file:"$log" \
        -qmp unix:"$qmp",server=on,wait=off &
    local pid=$!
    trap "kill $pid 2>/dev/null || true" RETURN

    # Wait for qmp socket then advance past preconfig.
    for _ in $(seq 1 60); do [[ -S "$qmp" ]] && break; sleep 0.2; done
    [[ -S "$qmp" ]] || { kill "$pid" 2>/dev/null || true;
        die "$name: qmp socket never appeared"; }

    {
        printf '%s\n' '{"execute":"qmp_capabilities"}'
        printf '%s\n' '{"execute":"cont"}'
        sleep 25
    } | socat - UNIX-CONNECT:"$qmp" >/dev/null 2>&1 || true

    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true

    # Assertions.
    local ok=0
    if grep -q "$expect_ok" "$log"; then
        log "$name: found expected success token '$expect_ok'"
        ok=$((ok + 1))
    else
        log "$name: MISSING success token '$expect_ok' in $log"
    fi
    if grep -q "$expect_fail" "$log"; then
        log "$name: unexpected fail token '$expect_fail' present"
        ok=$((ok - 1))
    fi
    trap - RETURN
    [[ $ok -gt 0 ]]
}

# Scenario 1 — signed UKI: expect QRAB_SB_OK (firmware enforcing);
# no QRAB_FAIL_SB.
if run_scenario signed "$UKI_OK" 'QRAB_SB_OK' 'QRAB_FAIL_SB'; then
    log "SIGNED: PASS"
else
    die "SIGNED: FAIL — see $OUT/signed.serial.log"
fi

# Scenario 2 — tampered UKI: expect QRAB_FAIL_SB (firmware rejected);
# no QRAB_LOADIMAGE_OK (i.e. LoadImage must not succeed).
if run_scenario tampered "$UKI_BAD" 'QRAB_FAIL_SB' 'QRAB_LOADIMAGE_OK'; then
    log "TAMPERED: PASS (firmware rejected image)"
else
    die "TAMPERED: FAIL — firmware accepted a tampered image. See $OUT/tampered.serial.log"
fi

log "all scenarios PASS"
