#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

cargo test --locked --release -p qapi-codegen -- --test-threads=2
cargo test --locked -p qapi-macros -p qapi -p qapi-protocol

qapi_check_dir=$(mktemp -d)
trap 'rm -rf "$qapi_check_dir"' EXIT
cargo run --locked --release -p qapi-codegen -- vendor/qemu --output-dir "$qapi_check_dir/qmp"
cargo run --locked --release -p qapi-codegen -- vendor/qemu --qga --output-dir "$qapi_check_dir/qga"
diff -ru projects/qapi/src/generated "$qapi_check_dir/qmp"
diff -ru projects/qapi/src/qga "$qapi_check_dir/qga"
