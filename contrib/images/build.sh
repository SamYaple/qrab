#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
component=${1:?usage: build.sh qemu|manager|operator|dashboard|tpm-issuer TAG}
tag=${2:?image tag required}
[[ $tag =~ ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]] || { echo 'invalid image tag' >&2; exit 1; }
revision=$(git rev-parse HEAD)
registry=${QRAB_REGISTRY:-zot.yapplesauce.com}
image="$registry/qrab/$component:$tag"
case "$component" in
  qemu)
    contrib/images/prepare-source.py
    podman build --label "org.opencontainers.image.revision=$revision" -f contrib/images/Containerfile.qemu --build-arg "QRAB_REVISION=$revision" -t "$image" target/images/context
    ;;
  manager)
    qemu_image=${QEMU_IMAGE:-$(cat target/images/qemu.ref 2>/dev/null || true)}
    [[ $qemu_image =~ @sha256:[a-f0-9]{64}$ ]] || { echo 'QEMU_IMAGE must be an immutable published digest reference' >&2; exit 1; }
    podman build --label "org.opencontainers.image.revision=$revision" -f contrib/images/Containerfile.manager --build-arg "QRAB_REVISION=$revision" --build-arg "QEMU_IMAGE=$qemu_image" -t "$image" .
    ;;
  operator)
    qemu_image=${QEMU_IMAGE:-$(cat target/images/qemu.ref 2>/dev/null || true)}
    [[ $qemu_image =~ @sha256:[a-f0-9]{64}$ ]] || { echo 'QEMU_IMAGE must be an immutable published digest reference (it supplies qemu-img)' >&2; exit 1; }
    podman build --label "org.opencontainers.image.revision=$revision" -f contrib/images/Containerfile.operator --build-arg "QRAB_REVISION=$revision" --build-arg "QEMU_IMAGE=$qemu_image" -t "$image" .
    ;;
  dashboard)
    podman build --label "org.opencontainers.image.revision=$revision" -f contrib/images/Containerfile.dashboard --build-arg "QRAB_REVISION=$revision" -t "$image" .
    ;;
  tpm-issuer)
    podman build --label "org.opencontainers.image.revision=$revision" -f contrib/images/Containerfile.tpm-issuer --build-arg "QRAB_REVISION=$revision" -t "$image" .
    ;;
  *) echo 'component must be qemu, manager, operator, dashboard or tpm-issuer' >&2; exit 1;;
esac
printf '%s\n' "$image"
