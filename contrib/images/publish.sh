#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
component=${1:?usage: publish.sh qemu|manager TAG}
tag=${2:?image tag required}
[[ $component == qemu || $component == manager ]] || exit 1
[[ $tag =~ ^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$ ]] || exit 1
[[ -z $(git status --porcelain) ]] || { echo 'commit the source before publication' >&2; exit 1; }
registry=${QRAB_REGISTRY:-zot.yapplesauce.com}
image="$registry/qrab/$component:$tag"
revision=$(git rev-parse HEAD)
actual=$(podman image inspect "$image" --format '{{index .Labels "org.opencontainers.image.revision"}}')
[[ $actual == "$revision" ]] || { echo 'image source revision does not match HEAD' >&2; exit 1; }
# Exchange immediately before each individual push; credentials are never
# embedded in images or manifests. Other registries use caller-managed login.
if [[ $registry == zot.yapplesauce.com ]]; then
  towl tokens mint --audience zot --raw | podman login --username oauth2 --password-stdin "$registry"
fi
mkdir -p target/images
podman push --digestfile "target/images/$component.digest" "$image"
digest=$(cat "target/images/$component.digest")
[[ $digest =~ ^sha256:[a-f0-9]{64}$ ]] || exit 1
reference="$registry/qrab/$component@$digest"
podman manifest inspect "$reference" > "target/images/$component.manifest.json"
printf '%s\n' "$reference" > "target/images/$component.ref"
printf '%s\n' "$reference"
