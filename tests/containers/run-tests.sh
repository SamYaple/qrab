#!/bin/bash
# Run the test suite against containerized QEMU versions.
# Usage:
#   ./run-tests.sh                    # run against all container versions
#   ./run-tests.sh 3.1 7.2            # run against specific versions
#   ./run-tests.sh 3.1 -- --test minimal  # pass extra cargo test args
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
WRAPPER="${SCRIPT_DIR}/qemu-wrapper.sh"

ALL_VERSIONS=("3.1" "5.2" "7.2")

# Parse args: versions before --, cargo args after --
VERSIONS=()
CARGO_ARGS=()
seen_sep=0
for arg in "$@"; do
    if [ "$arg" = "--" ]; then
        seen_sep=1
        continue
    fi
    if [ "$seen_sep" -eq 1 ]; then
        CARGO_ARGS+=("$arg")
    else
        VERSIONS+=("$arg")
    fi
done

if [ ${#VERSIONS[@]} -eq 0 ]; then
    VERSIONS=("${ALL_VERSIONS[@]}")
fi

if [ ${#CARGO_ARGS[@]} -eq 0 ]; then
    CARGO_ARGS=("-p" "qapi-protocol" "--test" "minimal")
fi

# Container QEMU has higher startup latency; limit parallelism
TEST_THREADS="${QEMU_TEST_THREADS:-4}"

cd "$REPO_ROOT"

echo "Testing against QEMU versions: ${VERSIONS[*]}"
echo "Cargo args: ${CARGO_ARGS[*]}"
echo

RESULTS=()
for ver in "${VERSIONS[@]}"; do
    IMAGE="qrab-qemu-${ver}"
    # Verify image exists
    if ! podman --cgroup-manager=cgroupfs image exists "$IMAGE" 2>/dev/null; then
        echo "=== QEMU ${ver}: SKIP (image not built, run build.sh first) ==="
        RESULTS+=("${ver}: SKIP")
        continue
    fi

    echo "=== QEMU ${ver} ==="
    if QEMU_CONTAINER_IMAGE="$IMAGE" QEMU_TEST_BIN_X86_64="$WRAPPER" \
        cargo test "${CARGO_ARGS[@]}" -- --test-threads="$TEST_THREADS" 2>&1 | tee /dev/stderr | grep -q "^test result:.*0 failed"; then
        RESULTS+=("${ver}: PASS")
    else
        RESULTS+=("${ver}: FAIL (see output above)")
    fi
    echo
done

echo "==============================="
echo "Summary:"
for r in "${RESULTS[@]}"; do
    echo "  QEMU $r"
done
