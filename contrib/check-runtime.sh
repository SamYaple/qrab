#!/usr/bin/env bash
# Fast pod/QAPI checks. No QEMU checkout, guest image, hardware or cluster needed.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt -p qrab-operator -p bmc -p manager -p qapi-protocol -p qapi -p qapi-macros --check
cargo test --locked
crd_check_file=$(mktemp)
trap 'rm -f "$crd_check_file"' EXIT
cargo run --locked --quiet -p qrab-operator -- crd > "$crd_check_file"
cmp contrib/kubernetes/operator/crd.json "$crd_check_file"
cargo clippy --locked -p qrab-operator -p bmc -p manager -p qapi-protocol --all-targets --no-deps -- -D warnings
