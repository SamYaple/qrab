#!/usr/bin/env bash
# Phase 7 minimum-viable scenario matrix for the qrab-boot attest loop.
#
# Runs the following scenarios from reference/qrab-boot-plan.md, each
# end-to-end under QEMU q35 + TCG with vhost-vsock and
# projects/attest-server/:
#
#   1  Happy SNP       : valid UKI + valid server sig + hash match
#                        → QRAB_LOADIMAGE_STUB_SKIP (default build) or
#                          QRAB_LOADIMAGE_OK (QRAB_UKI_BOOT=1 + real UKI)
#   3  Tampered UKI    : QRAB_UKI_BOOT=1 rebuild, stub (non-PE) body
#                        → QRAB_FAIL_LOAD (proxy for QRAB_FAIL_SB while
#                          firmware SB on AmdSev is deferred; see plan
#                          risk note)
#   4  Wrong hash      : attest-server QRAB_ATTEST_FAULT=wrong_hash
#                        → QRAB_FAIL_HASH
#   5  Bad server sig  : attest-server QRAB_ATTEST_FAULT=bad_sig
#                        → QRAB_ATTEST_BAD_SIG
#
# Exits 0 on full pass, 1 on any missing / unexpected token, 77 when
# the host lacks the vsock / KVM plumbing the underlying smoke script
# skips on (matches GNU convention).
#
# Override `SCENARIOS="1 4 5"` (for example) to run a subset. This is
# also how the CI smoke currently opts out of scenario 3 in
# environments that cannot afford the second qrab-boot build (it
# doubles compile time for the matrix run).

set -euo pipefail

ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
SMOKE="$ROOT/contrib/smoke-vsock-qemu.sh"

SCENARIOS="${SCENARIOS:-1 3 4 5 6 8 11 12a 12b 12c}"

# Working dir for scenario-specific fixtures (e.g. the 64 KiB UKI for
# scenario 11). Built once, reused by every matrix run that needs it.
SCRATCH="$ROOT/target/matrix-smoke"
mkdir -p "$SCRATCH"

# Map scenario id -> (smoke env, expected serial token).
scenario_env() {
    case "$1" in
        1)    echo "" ;;  # default build, no fault
        3)    echo "QRAB_UKI_BOOT=1" ;;
        4)    echo "QRAB_ATTEST_FAULT=wrong_hash" ;;
        5)    echo "QRAB_ATTEST_FAULT=bad_sig" ;;
        6)    echo "QRAB_SMOKE_SKIP_ATTEST=1" ;;
        8)    echo "QRAB_SMOKE_NO_VSOCK=1" ;;
        # 40 s boot budget keeps margin for slower TCG hosts while
        # the in-guest vsock path drains the 64 KiB UKI. Regular
        # 20 s is enough on a modern EPYC / Ryzen but not on all
        # CI shapes.
        11)   echo "QRAB_SMOKE_UKI_PATH=$SCRATCH/large-uki.bin QRAB_VSOCK_BOOT_TIMEOUT=40" ;;
        12a)  echo "QRAB_SMOKE_MEM=256M" ;;
        12b)  echo "QRAB_SMOKE_MEM=1G" ;;
        12c)  echo "QRAB_SMOKE_MEM=2G" ;;
        *) echo "unknown scenario: $1" >&2; return 1 ;;
    esac
}

# Scenario-specific expected-token set. The smoke script enforces its
# own base set (QRAB_VSOCK_FOUND etc.); this adds the scenario-unique
# terminal token and, where applicable, asserts an absence.
scenario_expect_token() {
    case "$1" in
        1)    echo "QRAB_LOADIMAGE_STUB_SKIP" ;;
        3)    echo "QRAB_FAIL_LOAD" ;;
        4)    echo "QRAB_FAIL_HASH" ;;
        5)    echo "QRAB_ATTEST_BAD_SIG" ;;
        6)    echo "QRAB_VSOCK_CONNECT_TIMEOUT" ;;
        8)    echo "QRAB_VSOCK_MISSING" ;;
        11)   echo "QRAB_UKI_OK len=65536" ;;
        12a|12b|12c) echo "QRAB_LOADIMAGE_STUB_SKIP" ;;
    esac
}

# Scenarios that expect the bootloader to halt BEFORE the happy-path
# tokens the base smoke normally requires. The matrix runs against a
# relaxed smoke profile via QRAB_SMOKE_REQUIRED_OVERRIDE="" so an
# earlier-halt scenario doesn't falsely fail on missing downstream
# tokens. Scenarios 1, 11, 12* traverse the full chain.
scenario_relaxed_required() {
    case "$1" in
        # Full chain must run — let smoke's default list gate.
        1|11|12a|12b|12c) echo "" ;;
        # Faults inside phase 4: only early tokens required.
        3|4|5) echo "QRAB_VSOCK_FOUND QRAB_VSOCK_CONNECT_OK" ;;
        # Server unreachable: vsock PCI device found but connect
        # never completes. Require only the probe-level tokens.
        6) echo "QRAB_VSOCK_FOUND QRAB_VSOCK_DRIVER_OK" ;;
        # No NIC: probe itself bails. Require nothing — the
        # expected-token grep on QRAB_VSOCK_MISSING is the gate.
        8) echo " " ;;
    esac
}

# Build scenario-specific fixtures once per matrix invocation.
build_scratch_fixtures() {
    local target="$SCRATCH/large-uki.bin"
    # Scenario 11: 64 KiB UKI. qrab-boot-proto's MAX_FRAMED_PAYLOAD
    # is 128 KiB; 64 KiB fits comfortably and still exercises a
    # bigger-than-default payload path. Content is opaque to the
    # smoke (stub-skip path doesn't parse it).
    if [[ ! -s "$target" || "$(stat -c '%s' "$target")" != "65536" ]]; then
        dd if=/dev/zero of="$target" bs=1024 count=64 status=none
        # Sprinkle a magic tag at the start so a corrupted fixture
        # is obvious in the qemu serial log.
        printf 'QRAB_LARGE_UKI_FIXTURE_v1\n' | dd of="$target" bs=1 seek=0 conv=notrunc status=none
    fi
}
build_scratch_fixtures

total=0
failed=()
skipped=0

for s in $SCENARIOS; do
    total=$((total+1))
    env_line="$(scenario_env "$s")"
    want="$(scenario_expect_token "$s")"
    required="$(scenario_relaxed_required "$s")"

    echo "=== scenario $s === env: ${env_line:-(none)} expect: $want"

    # Scenario 3 requires a different build of qrab-boot.efi; plumb
    # the env var through so build.rs picks it up. The smoke script
    # invokes `cargo build --release` from the qrab-boot crate,
    # which respects cargo:rerun-if-env-changed=QRAB_UKI_BOOT.
    out="$(env ${env_line:-} QRAB_SMOKE_REQUIRED_OVERRIDE="$required" \
              "$SMOKE" 2>&1)" || rc=$?
    rc=${rc:-0}

    if [[ $rc -eq 77 ]]; then
        echo "--- skipped by smoke (rc=77)"
        skipped=$((skipped+1))
        continue
    fi

    serial_log="$ROOT/target/qrab-boot-vsock-smoke/serial.log"
    if [[ -n "$want" ]] && ! grep -a -q "$want" "$serial_log" 2>/dev/null; then
        echo "--- FAIL: missing expected token '$want'"
        echo "$out" | tail -15
        failed+=("scenario-$s")
        continue
    fi

    # Expected-FAIL enforcement: each fault scenario must NOT log
    # the terminal happy-path token *for the stage it fails in*.
    # Scenario 3 faults in Phase 5 StartImage, so it reaches
    # `QRAB_PHASE4_OK` legitimately — the regression guard is the
    # absence of `QRAB_LOADIMAGE_OK`. Scenarios 4/5/6/8 fault before
    # Phase 4 completes and must not reach PHASE4_OK.
    must_not=""
    case "$s" in
        3)       must_not="QRAB_LOADIMAGE_OK" ;;
        4|5|6|8) must_not="QRAB_PHASE4_OK"    ;;
        # Happy-chain scenarios (1, 11, 12*) — no regression guard
        # other than the expected-token grep above.
    esac
    if [[ -n "$must_not" ]] && grep -a -q "$must_not" "$serial_log"; then
        echo "--- FAIL: fault scenario $s reached '$must_not' (happy path)"
        failed+=("scenario-$s-falsepass")
        continue
    fi

    echo "--- pass"
done

echo
echo "matrix: $((total-${#failed[@]}-skipped))/$total pass ${#failed[@]} fail $skipped skipped"
if [[ ${#failed[@]} -gt 0 ]]; then
    printf '  failed: %s\n' "${failed[@]}"
    exit 1
fi
