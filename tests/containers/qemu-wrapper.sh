#!/bin/bash
# Wrapper script that runs a containerized QEMU as a drop-in replacement.
# Usage: QEMU_CONTAINER_IMAGE=qrab-qemu-3.1 ./qemu-wrapper.sh [qemu args...]
#
# Set QEMU_TEST_BIN_X86_64 to this script's path to use with the test harness.
set -euo pipefail

IMAGE="${QEMU_CONTAINER_IMAGE:?Set QEMU_CONTAINER_IMAGE to the container tag (e.g. qrab-qemu-3.1)}"

declare -A SEEN_VOLS
VOLUMES=()

add_vol() {
    local mount="$1"
    if [ -z "${SEEN_VOLS[$mount]+x}" ]; then
        SEEN_VOLS[$mount]=1
        VOLUMES+=("-v" "$mount")
    fi
}

mount_path() {
    local path="$1"
    if [ -e "$path" ]; then
        local dir
        dir="$(dirname "$(realpath "$path")")"
        add_vol "${dir}:${dir}"
    fi
}

add_vol "/tmp:/tmp"

# Scan args for file paths and add volume mounts
for arg in "$@"; do
    # Direct file path argument
    mount_path "$arg"
    # Extract paths from key=value compound arguments
    # Match filename=, file=, but not node-name=, if= etc.
    while IFS= read -r path; do
        mount_path "$path"
    done < <(echo "$arg" | grep -oP '(?<=filename=|,file=)[^,]+' 2>/dev/null || true)
done

exec podman --cgroup-manager=cgroupfs run --rm \
    --network=none \
    --security-opt label=disable \
    "${VOLUMES[@]}" \
    "$IMAGE" \
    "$@"
