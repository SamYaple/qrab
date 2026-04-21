#!/usr/bin/env bash
# End-to-end smoke test for the qrab-boot phase-3b vsock path.
#
# Boots `qrab-boot.efi` under QEMU q35 + TCG with a virtio-vsock PCI
# device bound to vhost-vsock on the host, and verifies that the guest
# serial log mentions the full chain:
#
#   QRAB_VSOCK_FOUND  →  QRAB_VSOCK_DRIVER_OK  →  QRAB_VSOCK_CONNECT_OK
#
# The corresponding host-side `contrib/mock-vsock-server.py` is
# launched in the background; it logs to `target/qrab-boot-vsock-smoke/mock.log`.
#
# Prerequisites on the host:
#   - `CONFIG_VHOST_VSOCK=m`/y + `vhost_vsock` loaded (check with
#     `modprobe vhost_vsock`).
#   - `/dev/vhost-vsock` readable / writable by this user (commonly
#     `kvm` group on distros; Gentoo's `vhost-vsock` kmod or manual
#     `chmod g+rw /dev/vhost-vsock` + udev rule).
#   - A free guest CID in the [3, 4_294_967_295] range (we default to
#     4242, override via `QRAB_VSOCK_GUEST_CID`).
#
# On a host without those, the script prints a diagnostic and exits 77
# (GNU "skipped") so CI harnesses can special-case it.
set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
BOOT_CRATE="$ROOT/projects/qrab-boot"
EFI="$BOOT_CRATE/target/x86_64-unknown-uefi/release/qrab-boot.efi"
OUT="$ROOT/target/qrab-boot-vsock-smoke"
ESP="$OUT/esp.img"
SERIAL="$OUT/serial.log"
MOCKLOG="$OUT/mock.log"

GUEST_CID="${QRAB_VSOCK_GUEST_CID:-4242}"
PORT="${QRAB_VSOCK_PORT:-1337}"
BOOT_TIMEOUT_SECS="${QRAB_VSOCK_BOOT_TIMEOUT:-20}"

OVMF="${QRAB_BOOT_OVMF_CODE:-}"
if [[ -z "$OVMF" ]]; then
    for c in /usr/share/edk2-ovmf/OVMF_CODE.fd /usr/share/OVMF/OVMF_CODE.fd \
             /usr/share/edk2/ovmf/OVMF_CODE.fd; do
        [[ -f "$c" ]] && OVMF="$c" && break
    done
fi

log() { echo "smoke-vsock: $*" >&2; }
die() { log "error: $*"; exit 1; }
skip() { log "skipped: $*"; exit 77; }

# ---- prerequisites -------------------------------------------------------
[[ -n "$OVMF" && -f "$OVMF" ]] || skip "no OVMF_CODE.fd (set QRAB_BOOT_OVMF_CODE)"
[[ -e /dev/vhost-vsock ]] || skip "/dev/vhost-vsock missing (load vhost_vsock?)"
[[ -r /dev/vhost-vsock && -w /dev/vhost-vsock ]] \
    || skip "/dev/vhost-vsock not accessible (check group membership)"
command -v qemu-system-x86_64 >/dev/null || die "qemu-system-x86_64 not on PATH"
command -v python3 >/dev/null || die "python3 not on PATH"
command -v mkfs.vfat >/dev/null || die "mkfs.vfat not on PATH"
command -v mcopy >/dev/null || die "mcopy not on PATH"
command -v mmd >/dev/null || die "mmd not on PATH"

# ---- build ---------------------------------------------------------------
# R16-H14 follow-up: bootloader fail-closes on non-SNP by default. This
# smoke always drives `accel=tcg` (no SEV hardware required), so the
# only sane default here is to compile the check out via
# `QRAB_ALLOW_NON_SNP=1`. Scenario 9 (SNP-gate proof) intentionally
# inverts this by exporting `QRAB_ALLOW_NON_SNP=0` before invoking the
# smoke — we honour that opt-out so the TCG guest halts at
# `QRAB_FAIL_NOSNP`. Any caller that wants SNP enforcement can set the
# var to anything != 1 (empty string included).
: "${QRAB_ALLOW_NON_SNP:=1}"
export QRAB_ALLOW_NON_SNP
log "building qrab-boot.efi (QRAB_ALLOW_NON_SNP=$QRAB_ALLOW_NON_SNP)"
(cd "$BOOT_CRATE" && cargo build --release >/dev/null)
[[ -f "$EFI" ]] || die "$EFI not produced"

# ---- stage ESP + logs ----------------------------------------------------
mkdir -p "$OUT"
rm -f "$ESP" "$SERIAL" "$MOCKLOG"
truncate -s 64M "$ESP"
mkfs.vfat -F 32 -n QRABBOOT "$ESP" >/dev/null
mmd -i "$ESP" ::/EFI ::/EFI/BOOT
mcopy -i "$ESP" "$EFI" ::/EFI/BOOT/BOOTX64.EFI
: > "$SERIAL"
: > "$MOCKLOG"

# ---- attest-server -------------------------------------------------------
# `QRAB_SMOKE_SKIP_ATTEST=1` lets the matrix harness launch QEMU
# without any attest-server listening, which is the "server
# unreachable" scenario (Plan §scenario 6). qrab-boot's vsock
# connect will still find the PCI device but time out waiting
# for a reply.
MOCK_PID=""
cleanup() {
    if [[ -n "$MOCK_PID" ]]; then
        kill -TERM "$MOCK_PID" 2>/dev/null || true
        wait "$MOCK_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

if [[ "${QRAB_SMOKE_SKIP_ATTEST:-0}" == "1" ]]; then
    log "QRAB_SMOKE_SKIP_ATTEST=1 — not launching attest-server (server-unreachable scenario)"
else
    # Build the Rust attestation server up front so the bind-time log line
    # below is deterministic (and so a compile error fails fast instead of
    # mid-handshake).
    # Auto-regen the dev attest-server keypair when missing. Gitignored
    # fixture, so a fresh worktree / CI checkout would otherwise hit
    # the QRAB_ATTEST_NO_PUBKEY path — fine for scenario wiring but
    # breaks scenario 5 (bad_sig) which expects the verify to actually
    # fire. Re-running gen-attest-keys.sh is a no-op when keys exist.
    if [[ ! -s "$ROOT/contrib/attest-keys/server.sec1" ]]; then
        log "attest keys missing; regenerating via contrib/gen-attest-keys.sh"
        bash "$ROOT/contrib/gen-attest-keys.sh" >/dev/null 2>&1 \
            || die "gen-attest-keys.sh failed"
    fi
    log "building attest-server"
    (cd "$ROOT" && cargo build -p attest-server --release >/dev/null 2>&1) \
        || die "attest-server build failed"
    ATTEST_BIN="$ROOT/target/release/attest-server"
    [[ -x "$ATTEST_BIN" ]] || die "attest-server binary not produced"

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
    # `QRAB_SMOKE_UKI_PATH=<path>` overrides the built-in 512-byte
    # stub. Used by the matrix's scenario 11 (large UKI) via the
    # attest-server's own `QRAB_ATTEST_UKI_PATH` env var.
    if [[ -n "${QRAB_SMOKE_UKI_PATH:-}" && -f "$QRAB_SMOKE_UKI_PATH" ]]; then
        export QRAB_ATTEST_UKI_PATH="$QRAB_SMOKE_UKI_PATH"
    fi
    "$ATTEST_BIN" "$PORT" >"$MOCKLOG" 2>&1 &
    MOCK_PID=$!
    # Give the listener a moment to bind.
    sleep 0.3
fi

# ---- qemu ----------------------------------------------------------------
# Scenario-driven cmdline tweaks:
#   * QRAB_SMOKE_NO_VSOCK=1  — don't attach vhost-vsock-pci at all
#     (qrab-boot's `vsock::probe` emits QRAB_VSOCK_MISSING). Plan
#     §scenario 8.
#   * QRAB_SMOKE_MEM=<qemu-mem-string> — override -m, e.g. 256M or 2G
#     for the memsize matrix (Plan §scenario 12).
MEM_FLAG="-m ${QRAB_SMOKE_MEM:-512M}"
VSOCK_FLAG="-device vhost-vsock-pci,guest-cid=$GUEST_CID"
if [[ "${QRAB_SMOKE_NO_VSOCK:-0}" == "1" ]]; then
    log "QRAB_SMOKE_NO_VSOCK=1 — omitting vhost-vsock-pci device (no-NIC scenario)"
    VSOCK_FLAG=""
fi

log "launching qemu (guest_cid=$GUEST_CID, mem=${QRAB_SMOKE_MEM:-512M}, ~${BOOT_TIMEOUT_SECS}s)"
set +e
# shellcheck disable=SC2086  # deliberate word-split of $VSOCK_FLAG / $MEM_FLAG.
timeout "$BOOT_TIMEOUT_SECS" qemu-system-x86_64 \
    -machine q35,accel=tcg $MEM_FLAG -display none -no-reboot \
    -cpu max \
    -bios "$OVMF" \
    -drive if=virtio,format=raw,file="$ESP" \
    $VSOCK_FLAG \
    -serial file:"$SERIAL" >/dev/null 2>&1
QEMU_RC=$?
set -e
# `timeout` returns 124 on timeout; qrab-boot hangs at `hlt` so we
# expect exactly that for the happy path.
case "$QEMU_RC" in
    0|124) ;;
    *) log "qemu exited with rc=$QEMU_RC (continuing — serial log is authoritative)";;
esac

# ---- verify --------------------------------------------------------------
echo "--- serial log (filtered) ---"
tr -d '\r' < "$SERIAL" | grep -aE 'QRAB|phase' | head -20 || true
echo "--- mock log ---"
tail -5 "$MOCKLOG" || true

# Success requires all of these tokens on the guest serial by default.
# The matrix-smoke.sh harness overrides the list via
# QRAB_SMOKE_REQUIRED_OVERRIDE for fault scenarios that intentionally
# halt earlier (e.g. QRAB_FAIL_HASH fires before the secret/pcr/report
# tail). Setting the override to an empty string disables this check
# entirely and lets the matrix's own expected-token grep be
# authoritative.
if [[ -n "${QRAB_SMOKE_REQUIRED_OVERRIDE+x}" ]]; then
    # shellcheck disable=SC2206  # deliberate word-split of override.
    required=(${QRAB_SMOKE_REQUIRED_OVERRIDE})
else
    required=(QRAB_VSOCK_FOUND QRAB_VSOCK_DRIVER_OK QRAB_VSOCK_CONNECT_OK QRAB_ATTEST_SIG_OK QRAB_REPORT_DATA_HEX QRAB_UKI_OK QRAB_PCR8_EXPECTED_DIGEST QRAB_PCR9_EXPECTED_DIGEST QRAB_SECRET_OK QRAB_ATTEST_OK QRAB_REPORT_OK QRAB_LOADIMAGE_STUB_SKIP)
fi
missing=()
for tok in "${required[@]}"; do
    grep -a -q "$tok" "$SERIAL" || missing+=("$tok")
done
if [[ ${#missing[@]} -ne 0 ]]; then
    log "missing tokens: ${missing[*]}"
    exit 1
fi
log "all required tokens present"
