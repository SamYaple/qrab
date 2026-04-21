#!/usr/bin/env bash
# SNP-only scenario matrix for qrab-boot (scenarios 13a/13b reduced-phys-bits).
# Wraps contrib/smoke-snp-attest.sh. Requires SNP host (/dev/sev-guest, AmdSev OVMF).
#
# Exit codes:
#   0  — all scenarios pass
#   1  — any scenario failed
#   77 — skipped (no SNP host plumbing)
set -euo pipefail
ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
SMOKE="$ROOT/contrib/smoke-snp-attest.sh"

SCENARIOS="${SCENARIOS:-13a 13b}"

[[ -e /dev/sev ]]            || { echo "matrix-snp: skipped: no /dev/sev"; exit 77; }
[[ -e /dev/vhost-vsock ]]    || { echo "matrix-snp: skipped: no /dev/vhost-vsock"; exit 77; }
[[ -f "$ROOT/target/ovmf/OVMF.fd" ]] || { echo "matrix-snp: skipped: no AmdSev OVMF — run contrib/build-ovmf.sh"; exit 77; }

scenario_env() {
    case "$1" in
        13a) echo "QRAB_SNP_REDUCED_PHYS_BITS=1" ;;
        13b) echo "QRAB_SNP_REDUCED_PHYS_BITS=2" ;;
        *)   echo "" ;;
    esac
}

# Root for per-scenario log directories. Each scenario gets its own
# subdir so that scenarios 13a and 13b (and any future scenarios)
# don't overwrite each other's serial.log / mock.log / qmp.sock
# between runs — post-mortem of a scenario 13a failure was previously
# impossible once 13b ran (B12-7).
LOG_ROOT="${QRAB_MATRIX_SNP_LOG_ROOT:-$ROOT/target/matrix-snp-logs}"

pass=0; fail=0; skipped=0
for s in $SCENARIOS; do
    echo
    echo "=== scenario $s ==="
    env_line="$(scenario_env "$s")"
    scenario_out="$LOG_ROOT/$s"
    mkdir -p "$scenario_out"
    echo "logs: $scenario_out"
    if env $env_line OUT="$scenario_out" bash "$SMOKE"; then
        echo "scenario $s: ok"
        pass=$((pass+1))
    else
        rc=$?
        if [[ $rc == 77 ]]; then
            echo "scenario $s: skipped (rc=77)"
            skipped=$((skipped+1))
        else
            echo "scenario $s: FAIL (rc=$rc)"
            fail=$((fail+1))
        fi
    fi
done

echo
echo "matrix-snp: $pass ok, $fail failed, $skipped skipped"
[[ $fail == 0 ]] || exit 1
