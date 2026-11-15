#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

# This check owns its bounded builds; start with the complete API surface.
unset QEMU_MIN_VERSION QEMU_MAX_VERSION

cargo test --locked --release -p qapi-codegen -- --test-threads=2
cargo test --locked -p qapi-macros -p qapi -p qapi-protocol

qapi_check_dir=$(mktemp -d)
trap 'rm -rf "$qapi_check_dir"' EXIT
cargo run --locked --release -p qapi-codegen -- vendor/qemu --output-dir "$qapi_check_dir/qmp"
cargo run --locked --release -p qapi-codegen -- vendor/qemu --qga --output-dir "$qapi_check_dir/qga"
diff -ru projects/qapi/src/generated "$qapi_check_dir/qmp"
diff -ru projects/qapi/src/qga "$qapi_check_dir/qga"

# Compile each maintained series using its latest pinned patch.
for qapi_series in 9.2 10.0 10.1 10.2 11.0 11.1; do
    qapi_max=$(awk -v prefix="v$qapi_series." \
        'index($1, prefix) == 1 { version = substr($1, 2) } END { if (!version) exit 1; print version }' \
        projects/qapi-codegen/qemu-versions.lock)
    QEMU_MIN_VERSION="$qapi_series.0" QEMU_MAX_VERSION="$qapi_max" \
        cargo check --locked -p qapi
done
# Check the combined maintained range as well as each individual series.
QEMU_MIN_VERSION=9.2.0 QEMU_MAX_VERSION="$qapi_max" cargo check --locked -p qapi

# Opt-in exploratory coverage; older releases are outside the maintained matrix.
if [[ ${QRAB_QAPI_HISTORICAL_BOUNDS:-0} == 1 ]]; then
    while IFS= read -r qapi_version; do
        printf 'Checking historical QAPI bounds for QEMU %s\n' "$qapi_version"
        QEMU_MIN_VERSION="$qapi_version" QEMU_MAX_VERSION="$qapi_version" \
            cargo check --locked -p qapi
    done < <(awk '
        !/^#/ && NF {
            split(substr($1, 2), parts, ".")
            version = parts[1] "." parts[2] "." (parts[3] ? parts[3] : 0)
            if (!seen[version]++) print version
        }
    ' projects/qapi-codegen/qemu-versions.lock)
fi
# Leave the ordinary development build with the complete API surface.
env -u QEMU_MIN_VERSION -u QEMU_MAX_VERSION cargo check --locked -p qapi
