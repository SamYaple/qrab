#!/usr/bin/env bash
# Experiment B0: build context for Containerfile.qemu-spice without touching
# vendor/qemu or target/. Mirrors contrib/images/prepare-source.py (same pinned
# revision, same subproject archives, same qrab patch list) from a scratch clone:
#   git init q && git -C q fetch --depth 1 https://gitea.yapplesauce.com/mirrors/qemu.git \
#     005c61c0ef6f6ae615283440b4bb7be51b2c6ddd && git -C q checkout FETCH_HEAD
#   contrib/experiments/console-protocol/prepare-context.sh q ctx
#   podman build -f contrib/experiments/console-protocol/Containerfile.qemu-spice \
#     -t localhost/qrab-experiment/qemu-spice:11.1.3 ctx
set -euo pipefail
root=$(cd "$(dirname "$0")/../../.." && pwd)
checkout=$(cd "$1" && pwd)
context=$2
[[ $(git -C "$checkout" rev-parse HEAD) == $(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["qemu"]["revision"])' "$root/contrib/images/qemu-source.json") ]]
rm -rf "$context/qemu-source"
mkdir -p "$context/qemu-source"
git -C "$checkout" archive HEAD | tar -x -C "$context/qemu-source"
for name in berkeley-softfloat-3 berkeley-testfloat-3 keycodemapdb; do
  mkdir "$context/qemu-source/subprojects/$name"
  tar -xzf "$root/vendor/qemu-build/$name.tar.gz" -C "$context/qemu-source/subprojects/$name"
  if [[ -d $context/qemu-source/subprojects/packagefiles/$name ]]; then
    cp -r "$context/qemu-source/subprojects/packagefiles/$name/." "$context/qemu-source/subprojects/$name/"
  fi
done
for patch in "$root"/contrib/images/qemu-patches/*.patch; do
  patch -d "$context/qemu-source" -p1 --forward --batch --silent -i "$patch"
done
echo "$context"
