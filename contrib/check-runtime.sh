#!/usr/bin/env bash
# Fast checks. No QEMU checkout, guest image, hardware or cluster needed.
#
# Usage: contrib/check-runtime.sh [STAGE...]
#   fmt     cargo fmt --check
#   test    cargo test
#   clippy  strict Clippy (-D warnings)
#   deny    cargo deny licenses
# Without a stage it runs all of them, deny only when cargo-deny is installed.
# CI runs one stage per required check (.github/workflows/ci.yml).
set -euo pipefail
cd "$(dirname "$0")/.."
# The workspace's packages. Those with default features are also checked
# without them: that is the build of a checkout that leaves those features
# out.
workspace_packages() {
    cargo metadata --no-deps --format-version 1 | python3 -c '
import json, sys
for package in json.load(sys.stdin)["packages"]:
    if sys.argv[1] == "all" or package["features"].get("default"):
        print(package["name"])' "$1"
}
mapfile -t packages < <(workspace_packages all)
mapfile -t without_defaults < <(workspace_packages defaulted)

stage_fmt() {
    cargo fmt ${packages[@]/#/-p } --check
}

stage_test() {
    cargo test --locked --workspace
    for package in "${without_defaults[@]}"; do
        cargo test --locked -p "$package" --no-default-features
    done
}

stage_clippy() {
    cargo clippy --locked --workspace --all-targets --no-deps -- -D warnings
    for package in "${without_defaults[@]}"; do
        cargo clippy --locked -p "$package" --no-default-features --all-targets --no-deps -- -D warnings
    done
}

stage_deny() {
    cargo deny --locked check licenses
}

if (($# == 0)); then
    set -- fmt test clippy
    if command -v cargo-deny >/dev/null 2>&1; then set -- "$@" deny; fi
fi
for stage; do
    case $stage in
        fmt|test|clippy|deny) "stage_$stage" ;;
        *) printf 'unknown stage %s (fmt, test, clippy or deny)\n' "$stage" >&2; exit 2 ;;
    esac
done
