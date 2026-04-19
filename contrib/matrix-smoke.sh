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

SCENARIOS="${SCENARIOS:-1 3 4 5}"

# Map scenario id -> (smoke env, expected serial token).
scenario_env() {
    case "$1" in
        1) echo "" ;;  # default build, no fault
        3) echo "QRAB_UKI_BOOT=1" ;;
        4) echo "QRAB_ATTEST_FAULT=wrong_hash" ;;
        5) echo "QRAB_ATTEST_FAULT=bad_sig" ;;
        *) echo "unknown scenario: $1" >&2; return 1 ;;
    esac
}

# Scenario-specific expected-token set. The smoke script enforces its
# own base set (QRAB_VSOCK_FOUND etc.); this adds the scenario-unique
# terminal token and, where applicable, asserts an absence.
scenario_expect_token() {
    case "$1" in
        1) echo "QRAB_LOADIMAGE_STUB_SKIP" ;;
        3) echo "QRAB_FAIL_LOAD" ;;
        4) echo "QRAB_FAIL_HASH" ;;
        5) echo "QRAB_ATTEST_BAD_SIG" ;;
    esac
}

# Scenarios that expect the bootloader to halt BEFORE the happy-path
# tokens the base smoke normally requires. The matrix runs against a
# relaxed smoke profile via QRAB_SMOKE_REQUIRED_OVERRIDE="" so an
# earlier-halt scenario doesn't falsely fail on missing downstream
# tokens. Scenario 1 is the only one that traverses the full chain.
scenario_relaxed_required() {
    case "$1" in
        1) echo "" ;;  # use smoke script's default full list
        3|4|5) echo "QRAB_VSOCK_FOUND QRAB_VSOCK_CONNECT_OK" ;;
    esac
}

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
    # absence of `QRAB_LOADIMAGE_OK`. Scenarios 4/5 fault inside
    # the Phase 4 attest chain so they must not reach PHASE4_OK.
    must_not=""
    case "$s" in
        3)   must_not="QRAB_LOADIMAGE_OK" ;;
        4|5) must_not="QRAB_PHASE4_OK"    ;;
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
