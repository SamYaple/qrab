#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DOCKERFILE="${SCRIPT_DIR}/Dockerfile.debian"

# Define versions: tag|base_image|archive|distro_name
VERSIONS=(
    "qrab-qemu-3.1|debian:buster|1|buster"
    "qrab-qemu-5.2|debian:bullseye|0|"
    "qrab-qemu-7.2|debian:bookworm|0|"
)

build_one() {
    local spec="$1"
    IFS='|' read -r tag base_image archive distro_name <<< "$spec"
    echo "=== Building ${tag} from ${base_image} ==="
    podman --cgroup-manager=cgroupfs build \
        --build-arg "BASE_IMAGE=${base_image}" \
        --build-arg "ARCHIVE=${archive}" \
        --build-arg "DISTRO_NAME=${distro_name}" \
        -t "${tag}" \
        -f "${DOCKERFILE}" \
        "${SCRIPT_DIR}"
    podman --cgroup-manager=cgroupfs run --rm "${tag}" --version | head -1
    echo
}

if [ $# -gt 0 ]; then
    for arg in "$@"; do
        for spec in "${VERSIONS[@]}"; do
            if [[ "$spec" == *"qemu-${arg}|"* ]]; then
                build_one "$spec"
            fi
        done
    done
else
    for spec in "${VERSIONS[@]}"; do
        build_one "$spec"
    done
fi

echo "Done. Available images:"
podman --cgroup-manager=cgroupfs images --filter "reference=qrab-qemu-*" --format "{{.Repository}}:{{.Tag}}  {{.Size}}"
