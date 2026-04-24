#!/usr/bin/env bash
# Expected-fail smoke: qrab-boot + attest-server under SEV-SNP.
#
# Same shape as `contrib/smoke-vsock-qemu.sh` but launches the guest
# with `--preconfig -machine confidential-guest-support=sev0` and the
# AmdSev OVMF image at `target/ovmf/OVMF.fd`.
#
# History:
#
# * Originally filed as the reproducer for the DMA gap in
#   `~/workspace/llm-rag/issues/qrab-boot-snp-dma.md`; the
#   PciRootBridgeIo Map/Unmap HAL shipped 2026-04-18.
# * A second blocker surfaced immediately after — OVMF's
#   `CcExitLib::MmioExit` decodes only MOV-family opcodes, and
#   LLVM happily folded `safe_mmio::ReadPure<u8>::read() == x`
#   into `cmp r/m8, r8`. Tracked in
#   `~/workspace/llm-rag/issues/qrab-boot-snp-high-mmio-gp.md` +
#   worked around via `projects/qrab-boot/src/volatile_transport.rs`
#   (qrab-boot-local `black_box` shim) and by forcing
#   `VIRTIO_F_ACCESS_PLATFORM` acknowledgement in
#   `write_driver_features`.
# * As of 2026-04-18 the full flow is green on EPYC 9354 + SEV-SNP
#   and this script defaults to `QRAB_SNP_SMOKE_EXPECT=pass`.
#
# The upstream OVMF decoder patch is still the right long-term fix;
# the shim lives on until that lands.
# Exit codes:
#   0  — ALL required tokens present under SEV-SNP. This is the
#        default expectation (`QRAB_SNP_SMOKE_EXPECT=pass`) now
#        that the DMA rework + the OVMF #VC decoder shim +
#        ACCESS_PLATFORM force-ack have landed.
#   1  — Any required token missing (regression), or a known-fail
#        probe run succeeded unexpectedly (regression detection).
#  77  — Skipped because the host lacks SNP support or
#        `target/ovmf/OVMF.fd` is missing.
#
# Override with `QRAB_SNP_SMOKE_EXPECT=fail` when probing a new
# regression that is expected to fail (e.g. reverting one of the
# workarounds) so the script exits 0 on the missing-tokens path.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
BOOT_CRATE="$ROOT/projects/qrab-boot"
EFI="$BOOT_CRATE/target/x86_64-unknown-uefi/release/qrab-boot.efi"
FW="$ROOT/target/ovmf/OVMF.fd"
# Per-run output dir. `$OUT` overridable so `contrib/matrix-snp.sh`
# (and any other caller that loops scenarios) can route each
# invocation to its own log directory — otherwise scenarios 13a/13b
# overwrite each other's `serial.log` / `mock.log` and post-mortem of
# the first failure becomes impossible (B12-7). Standalone invocation
# defaults to a per-PID suffix so back-to-back manual runs also stay
# distinct; override via `OUT=/tmp/foo` for a stable path.
OUT="${OUT:-$ROOT/target/qrab-boot-snp-attest/run-$$}"
SERIAL="$OUT/serial.log"
MOCKLOG="$OUT/mock.log"

GUEST_CID="${QRAB_VSOCK_GUEST_CID:-4243}"
PORT="${QRAB_VSOCK_PORT:-1337}"
BOOT_TIMEOUT_SECS="${QRAB_VSOCK_BOOT_TIMEOUT:-30}"
EXPECT="${QRAB_SNP_SMOKE_EXPECT:-pass}"
RPB="${QRAB_SNP_REDUCED_PHYS_BITS:-1}"

log() { echo "smoke-snp-attest: $*" >&2; }
die() { log "error: $*"; exit 1; }
skip() { log "skipped: $*"; exit 77; }

log "reduced-phys-bits=$RPB (override via QRAB_SNP_REDUCED_PHYS_BITS)"

# ---- prerequisites -------------------------------------------------------
[[ -f "$FW" ]] || skip "no AmdSev OVMF at $FW — run contrib/build-ovmf.sh"
[[ -e /dev/sev ]] || skip "no /dev/sev — SNP unavailable on this host"
[[ -e /dev/vhost-vsock ]] || skip "/dev/vhost-vsock missing (load vhost_vsock?)"
[[ -r /dev/vhost-vsock && -w /dev/vhost-vsock ]] \
    || skip "/dev/vhost-vsock not accessible to this user"
command -v qemu-system-x86_64 >/dev/null || die "qemu-system-x86_64 not on PATH"
command -v socat >/dev/null || die "socat not on PATH"
command -v cargo >/dev/null || die "cargo not on PATH"

# ---- build ---------------------------------------------------------------
# Regen dev attest-server keypair if gitignored fixture is missing.
# Idempotent (script early-exits when files already exist). Keeps
# fresh worktrees / CI from hitting QRAB_ATTEST_NO_PUBKEY on scenarios
# that depend on actual signature verification.
if [[ ! -s "$ROOT/contrib/attest-keys/server.sec1" ]]; then
    log "attest keys missing; regenerating via contrib/gen-attest-keys.sh"
    bash "$ROOT/contrib/gen-attest-keys.sh" >/dev/null 2>&1 \
        || die "gen-attest-keys.sh failed"
fi
log "building qrab-boot.efi + attest-server"
(cd "$BOOT_CRATE" && cargo build --release >/dev/null)
(cd "$ROOT" && cargo build -p attest-server --release >/dev/null 2>&1) \
    || die "attest-server build failed"
[[ -f "$EFI" ]] || die "$EFI not produced"
ATTEST_BIN="$ROOT/target/release/attest-server"
[[ -x "$ATTEST_BIN" ]] || die "attest-server binary not produced"

mkdir -p "$OUT"
: > "$SERIAL"
: > "$MOCKLOG"

# ---- attest-server -------------------------------------------------------
log "launching attest-server (port $PORT)"
# Point attest-server at the same signing key whose public half was
# embedded into qrab-boot.efi at build time, so the bootloader's
# `p256` verify succeeds. Fall back to an ephemeral key if the dev
# keypair is absent (the bootloader will log QRAB_ATTEST_NO_PUBKEY
# in that case and the smoke still at least exercises the wire).
KEY_PATH="$ROOT/contrib/attest-keys/server.pem"
if [[ -s "$KEY_PATH" ]]; then
    export QRAB_ATTEST_KEY_PATH="$KEY_PATH"
fi
"$ATTEST_BIN" "$PORT" >"$MOCKLOG" 2>&1 &
MOCK_PID=$!
cleanup() {
    kill -TERM "$MOCK_PID" 2>/dev/null || true
    wait "$MOCK_PID" 2>/dev/null || true
}
trap cleanup EXIT
sleep 0.3

# ---- qemu ----------------------------------------------------------------
log "launching SNP guest (guest_cid=$GUEST_CID, ~${BOOT_TIMEOUT_SECS}s)"
QMP="$OUT/qmp.sock"
rm -f "$QMP"

# F2: optional host cert-chain population. When
# QRAB_SNP_USE_CERT_CHAIN=1, extend the sev-snp-guest object with
# `certs-path=` so the PSP hands the guest a non-empty cert blob
# (ARK || ASK || VCEK DER entries). Default off keeps existing
# smoke behaviour unchanged.
SNP_OBJ="sev-snp-guest,id=sev0,reduced-phys-bits=$RPB,cbitpos=51,sev-device=/dev/sev"
if [[ "${QRAB_SNP_USE_CERT_CHAIN:-0}" == "1" ]]; then
    CERT_DIR="${QRAB_SNP_CERT_CHAIN_DIR:-$ROOT/target/snp-cert-chain}"
    [[ -d "$CERT_DIR" ]] || die "QRAB_SNP_USE_CERT_CHAIN=1 but $CERT_DIR missing — run contrib/snp-fetch-host-cert-chain.sh"
    SNP_OBJ="$SNP_OBJ,certs-path=$CERT_DIR"
    log "cert chain: $CERT_DIR"
fi

set +e
timeout "$BOOT_TIMEOUT_SECS" qemu-system-x86_64 --preconfig \
    -machine q35,confidential-guest-support=sev0,firmware="$FW" \
    -m 512M -accel kvm -cpu EPYC-v4 \
    -object "$SNP_OBJ" \
    -device vhost-vsock-pci,guest-cid="$GUEST_CID" \
    -nodefaults -display none -no-reboot \
    -serial file:"$SERIAL" \
    -qmp unix:"$QMP",server=on,wait=off &
QEMU_PID=$!

# Wait for QMP, then kick through preconfig.
for _ in $(seq 1 40); do [[ -S "$QMP" ]] && break; sleep 0.1; done
if [[ -S "$QMP" ]]; then
    {
        printf '%s\n' '{"execute":"qmp_capabilities"}'
        printf '%s\n' '{"execute":"x-exit-preconfig"}'
        printf '%s\n' '{"execute":"cont"}'
        sleep "$BOOT_TIMEOUT_SECS"
    } | socat - UNIX-CONNECT:"$QMP" >/dev/null 2>&1 || true
else
    log "warn: QMP socket never appeared; flow likely dead on arrival"
fi
kill "$QEMU_PID" 2>/dev/null || true
wait "$QEMU_PID" 2>/dev/null || true
set -e

echo "--- serial log (filtered) ---"
tr -d '\r' < "$SERIAL" | grep -aE 'QRAB|phase' | head -30 || true
echo "--- mock log ---"
tail -5 "$MOCKLOG" || true

required=(QRAB_VSOCK_FOUND QRAB_VSOCK_DRIVER_OK QRAB_VSOCK_CONNECT_OK QRAB_ATTEST_SIG_OK QRAB_REPORT_DATA_HEX QRAB_SNP_REPORT_OK QRAB_SNP_CERT_CHAIN QRAB_UKI_OK QRAB_VTPM_SVSM_OK QRAB_PCR8_EXPECTED_DIGEST QRAB_PCR9_EXPECTED_DIGEST QRAB_SECRET_OK QRAB_ATTEST_OK)

# B12-8: with cert chain enabled, the server also drives the full
# ARK->ASK->VCEK+report chain walk and logs one of two green
# tokens. Require at least one, and treat the _FAIL path as a hard
# regression. Without this gate a chain-walker regression kept the
# smoke green because earlier tokens fire before F2 runs.
post_verify_regressed=0
if [[ "${QRAB_SNP_USE_CERT_CHAIN:-0}" == "1" ]]; then
    if grep -a -q "QRAB_SNP_POST_VERIFY_OK\b" "$SERIAL" \
        || grep -a -q "QRAB_SNP_POST_VERIFY_OK_NO_MEASUREMENT" "$SERIAL"; then
        :  # chain walk completed (green or amber)
    else
        required+=(QRAB_SNP_POST_VERIFY_OK)  # surfaces as missing
    fi
    if grep -a -q "QRAB_SNP_POST_VERIFY_FAIL" "$SERIAL" \
        || grep -a -q "QRAB_SNP_POST_VERIFY_IO_ERR" "$SERIAL"; then
        post_verify_regressed=1
    fi
fi

missing=()
for tok in "${required[@]}"; do
    grep -a -q "$tok" "$SERIAL" || missing+=("$tok")
done

case "$EXPECT" in
    fail)
        if [[ ${#missing[@]} -eq 0 ]]; then
            log "unexpected PASS — the regression you were probing did not reproduce; unset QRAB_SNP_SMOKE_EXPECT or set it to 'pass'"
            exit 1
        fi
        log "expected FAIL observed (missing: ${missing[*]})"
        exit 0
        ;;
    pass)
        if [[ ${#missing[@]} -ne 0 ]]; then
            log "regression — missing tokens: ${missing[*]}"
            exit 1
        fi
        if [[ "$post_verify_regressed" -eq 1 ]]; then
            log "regression — QRAB_SNP_POST_VERIFY_FAIL / _IO_ERR present in serial"
            exit 1
        fi
        log "all required tokens present under SNP"
        exit 0
        ;;
    *)
        die "QRAB_SNP_SMOKE_EXPECT=$EXPECT invalid; set to 'fail' or 'pass'"
        ;;
esac
