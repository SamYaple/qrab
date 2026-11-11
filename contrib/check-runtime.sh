#!/usr/bin/env bash
# Fast pod/QAPI checks. No QEMU checkout, guest image, hardware or cluster needed.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt -p bmc -p manager -p qapi-protocol -p qapi -p qapi-macros --check
cargo test --locked
cargo clippy --locked -p bmc -p manager -p qapi-protocol --all-targets --no-deps -- -D warnings
