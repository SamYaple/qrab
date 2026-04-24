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

SCENARIOS="${SCENARIOS:-1 1r 3 4 5 6 8 9 10 11 12a 12b 12c 13}"

# Working dir for scenario-specific fixtures (e.g. the 64 KiB UKI for
# scenario 11). Built once, reused by every matrix run that needs it.
SCRATCH="$ROOT/target/matrix-smoke"
mkdir -p "$SCRATCH"

# Scenario 1r (F10) needs a real PE to hand to
# `BootServices::load_image` + `start_image` — see
# `projects/qrab-real-uki-demo/`. The helper below builds it once per
# matrix invocation; the staged fixture is idempotent so a warm
# checkout is a near no-op. Keep the path in sync with
# `contrib/build-test-uki.sh`.
REAL_UKI_FIXTURE="$ROOT/projects/qrab-boot-tests/fixtures/uki-real.efi"

# Map scenario id -> (smoke env, expected serial token).
scenario_env() {
    case "$1" in
        1)    echo "" ;;  # default build, no fault
        # Scenario 1r (F10): real-UKI variant of scenario 1. Rebuild
        # qrab-boot with `QRAB_UKI_BOOT=1` so `uki::load_and_start`
        # actually calls `BootServices::load_image` + `start_image`,
        # and point attest-server at the 2-KiB fixture PE so the
        # bytes qrab-boot hashes and hands to the firmware are a
        # legal, executable image. On success the child PE writes
        # `QRAB_REAL_UKI_BOOT_OK` to debugcon. See
        # `projects/qrab-real-uki-demo/` and
        # `contrib/build-test-uki.sh`.
        1r)   echo "QRAB_UKI_BOOT=1 QRAB_SMOKE_UKI_PATH=$REAL_UKI_FIXTURE" ;;
        3)    echo "QRAB_UKI_BOOT=1" ;;
        4)    echo "QRAB_ATTEST_FAULT=wrong_hash" ;;
        5)    echo "QRAB_ATTEST_FAULT=bad_sig" ;;
        6)    echo "QRAB_SMOKE_SKIP_ATTEST=1" ;;
        8)    echo "QRAB_SMOKE_NO_VSOCK=1" ;;
        # Scenario 9 proves the R16-H14 default-on SNP gate: smoke's
        # blanket `QRAB_ALLOW_NON_SNP=1` (see smoke-vsock-qemu.sh) is
        # explicitly cancelled here so the check stays compiled in and
        # the TCG guest halts at `QRAB_FAIL_NOSNP`. This is the ONLY
        # scenario (together with 13 below) that must not inherit the
        # harness's TCG opt-out.
        9)    echo "QRAB_ALLOW_NON_SNP=0" ;;
        # Scenario 13 proves the R20 Phase 3 default-on vTPM gate:
        # cancels `QRAB_ALLOW_NO_VTPM=1` so `measure::require_and_probe_vtpm`
        # runs on a TCG boot where Tcg2Dxe reports no TPM (no swtpm,
        # no SVSM in this lane). `QRAB_ALLOW_NON_SNP=1` stays set so
        # the SNP gate passes first — the vTPM halt is what we're
        # measuring, not re-tripping scenario 9.
        13)   echo "QRAB_ALLOW_NO_VTPM=0" ;;
        # Scenario 10 runs TWO back-to-back smokes; the replay
        # cache path is shared between them so attest-server's
        # second invocation serves identical bytes. Handled in the
        # per-scenario special case below — the env here is just
        # what both runs get.
        10)   echo "QRAB_ATTEST_FAULT=replay QRAB_ATTEST_REPLAY_CACHE_PATH=$SCRATCH/replay-cache.bin" ;;
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
        # Scenario 1r terminal token: the child PE writes this to
        # debugcon on entry. Seeing it proves `start_image` really
        # transferred control (the parent never emits this string).
        # The companion `QRAB_LOADIMAGE_OK` token is asserted via the
        # relaxed-required override below so both sides of the hand-
        # off are gated.
        1r)   echo "QRAB_REAL_UKI_BOOT_OK" ;;
        3)    echo "QRAB_FAIL_LOAD" ;;
        4)    echo "QRAB_FAIL_HASH" ;;
        5)    echo "QRAB_ATTEST_BAD_SIG" ;;
        6)    echo "QRAB_VSOCK_CONNECT_TIMEOUT" ;;
        8)    echo "QRAB_VSOCK_MISSING" ;;
        9)    echo "QRAB_FAIL_NOSNP" ;;
        # Scenario 10 each individual run must reach QRAB_ATTEST_OK
        # (wire-level authenticity of the replayed frame holds).
        # The cross-boot report_data comparison is asserted in the
        # special-case block below.
        10)   echo "QRAB_ATTEST_OK" ;;
        11)   echo "QRAB_UKI_OK len=65536" ;;
        12a|12b|12c) echo "QRAB_LOADIMAGE_STUB_SKIP" ;;
        13)   echo "QRAB_FAIL_NO_VTPM" ;;
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
        # Scenario 10 ships the replayed-but-valid AttestResponse;
        # the bootloader reaches every happy token on both boots.
        1|10|11|12a|12b|12c) echo "" ;;
        # Scenario 1r rebuilds with `QRAB_UKI_BOOT=1`, which gates
        # out `QRAB_LOADIMAGE_STUB_SKIP` and substitutes the live
        # `LoadImage` path. The required list is therefore the
        # scenario-1 default with that one token swapped for
        # `QRAB_LOADIMAGE_OK`. The scenario-unique
        # `QRAB_REAL_UKI_BOOT_OK` is grep'd separately via
        # `scenario_expect_token`.
        1r)  echo "QRAB_VSOCK_FOUND QRAB_VSOCK_DRIVER_OK QRAB_VSOCK_CONNECT_OK QRAB_ATTEST_SIG_OK QRAB_REPORT_DATA_HEX QRAB_UKI_OK QRAB_PCR8_EXPECTED_DIGEST QRAB_PCR9_EXPECTED_DIGEST QRAB_SECRET_OK QRAB_ATTEST_OK QRAB_REPORT_OK QRAB_LOADIMAGE_OK" ;;
        # Faults inside phase 4: only early tokens required.
        3|4|5) echo "QRAB_VSOCK_FOUND QRAB_VSOCK_CONNECT_OK" ;;
        # Server unreachable: vsock PCI device found but connect
        # never completes. Require only the probe-level tokens.
        6) echo "QRAB_VSOCK_FOUND QRAB_VSOCK_DRIVER_OK" ;;
        # No NIC: probe itself bails. Require nothing — the
        # expected-token grep on QRAB_VSOCK_MISSING is the gate.
        8) echo " " ;;
        # F6 SNP-locked build on TCG: halts at the CPUID check
        # before any vsock / attest token. Only QRAB_FAIL_NOSNP
        # is required; the expected-token grep below is the gate.
        9) echo "QRAB_FAIL_NOSNP" ;;
        # R20 Phase 3 vTPM gate: halts at `require_and_probe_vtpm`
        # before any vsock / attest token. The SNP gate passes first
        # (`QRAB_ALLOW_NON_SNP=1` default), then the vTPM probe trips
        # on Tcg2 either absent or `tpm_present == false` under the
        # TCG harness. Only QRAB_FAIL_NO_VTPM is required here.
        13) echo "QRAB_FAIL_NO_VTPM" ;;
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

    # Scenario 1r (F10): the real-UKI PE. Delegated to a dedicated
    # script that also enforces the MAX_FRAMED_PAYLOAD size budget —
    # catches a drift where a refactor of `qrab-real-uki-demo`
    # accidentally pulls in enough dependencies to blow the 128 KiB
    # cap. Idempotent; skipped on warm checkouts.
    if [[ " $SCENARIOS " == *" 1r "* ]]; then
        bash "$ROOT/contrib/build-test-uki.sh" \
            || { echo "matrix: build-test-uki.sh failed" >&2; exit 1; }
    fi
}
build_scratch_fixtures

total=0
failed=()
skipped=0

# Scenario 10 helper: run the smoke once with the given run label,
# copy serial.log aside so the second run does not clobber it, and
# extract the QRAB_REPORT_DATA_HEX token. Echoes the hex value on
# stdout; on failure echoes nothing and returns non-zero. Relies on
# `QRAB_ATTEST_FAULT=replay QRAB_ATTEST_REPLAY_CACHE_PATH=…` being
# in $env_line so attest-server persists bytes between runs.
run_scenario10_boot() {
    local label="$1"
    local env_line="$2"
    local required="$3"
    local serial_log="$ROOT/target/qrab-boot-vsock-smoke/serial.log"
    local snap="$ROOT/target/qrab-boot-vsock-smoke/serial-10-${label}.log"

    local boot_out rc=0
    boot_out="$(env ${env_line:-} QRAB_SMOKE_REQUIRED_OVERRIDE="$required" \
                    "$SMOKE" 2>&1)" || rc=$?
    cp -f "$serial_log" "$snap" 2>/dev/null || true
    if [[ $rc -ne 0 ]]; then
        echo "scenario 10/$label smoke failed rc=$rc" >&2
        echo "$boot_out" | tail -15 >&2
        return "$rc"
    fi
    # QRAB_REPORT_DATA_HEX <128 hex chars> — space-separated per
    # projects/qrab-boot/src/ghcb.rs:140. Grab the first hex value
    # each boot emits.
    local hex
    hex="$(grep -a -oE 'QRAB_REPORT_DATA_HEX [0-9a-f]{128}' "$snap" \
            | head -1 | awk '{print $2}')"
    if [[ -z "$hex" ]]; then
        echo "scenario 10/$label missing QRAB_REPORT_DATA_HEX in $snap" >&2
        return 1
    fi
    printf '%s\n' "$hex"
}

for s in $SCENARIOS; do
    total=$((total+1))
    env_line="$(scenario_env "$s")"
    want="$(scenario_expect_token "$s")"
    required="$(scenario_relaxed_required "$s")"

    echo "=== scenario $s === env: ${env_line:-(none)} expect: $want"

    # Scenario 10 (replay / stale response audit): wire-level
    # authenticity of a replayed AttestResponse holds (sig verifies,
    # report_data binds both nonces) — freshness is a policy layer
    # concern. We run the smoke twice back-to-back with the same
    # QRAB_ATTEST_REPLAY_CACHE_PATH; attest-server primes the cache
    # on boot 1 and serves identical bytes on boot 2. Because the
    # full AttestResponse (including server_nonce) is byte-identical
    # and all other derivation inputs match, the guest's
    # QRAB_REPORT_DATA_HEX must match across boots. Equality here =
    # "replay succeeded" = the thing the audit layer would flag.
    if [[ "$s" == "10" ]]; then
        # Fresh cache file per matrix invocation: discard any
        # leftover bytes from a prior run so boot 1 always primes.
        rm -f "$SCRATCH/replay-cache.bin"

        echo "--- scenario 10 boot 1/2 (primes replay cache)"
        hex1="$(run_scenario10_boot boot1 "$env_line" "$required")" || {
            failed+=("scenario-10-boot1")
            continue
        }
        echo "--- scenario 10 boot 2/2 (serves cached bytes)"
        hex2="$(run_scenario10_boot boot2 "$env_line" "$required")" || {
            failed+=("scenario-10-boot2")
            continue
        }
        if [[ "$hex1" != "$hex2" ]]; then
            echo "--- FAIL: scenario 10 report_data differs between boots"
            echo "    boot1=$hex1"
            echo "    boot2=$hex2"
            failed+=("scenario-10-report-data-drift")
            continue
        fi
        echo "--- scenario 10 PASS report_data=$hex1 replay_detected=true"
        continue
    fi

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
        # Scenario 1r (F10) regression guard: a tampered-fixture or
        # broken rebuild that drove `load_image` into its failure
        # branches would leave `QRAB_FAIL_LOAD` / `QRAB_FAIL_SB` on
        # the serial log despite the happy-path required-token list
        # still passing (since scenario-1r's relaxed list only gates
        # the stages up to `QRAB_LOADIMAGE_OK`). Assert their
        # absence to make that class of regression loud.
        1r)      must_not="QRAB_FAIL_LOAD\|QRAB_FAIL_SB" ;;
        4|5|6|8) must_not="QRAB_PHASE4_OK"    ;;
        # Scenario 9: SNP gate fires before any downstream token.
        # Enforce absence of every later stage's happy marker so a
        # regression that lets the guest proceed past the gate
        # cannot silently slip past.
        9)       must_not="QRAB_LOADIMAGE_OK\|QRAB_PHASE4_OK\|QRAB_ATTEST_OK" ;;
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
