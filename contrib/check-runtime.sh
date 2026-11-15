#!/usr/bin/env bash
# Fast pod/QAPI checks. No QEMU checkout, guest image, hardware or cluster needed.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt -p qrab-guest-agent -p qrab-iscsi-target -p qrab-operator -p qrab-dashboard -p qrab-kvm-device-plugin -p qrab-netd -p qrab-spiffe -p qrab-proton -p oidc-bearer -p bmc -p qrab-redfish -p manager -p qapi-protocol -p qapi -p qapi-macros --check
cargo test --locked
crd_check_file=$(mktemp)
trap 'rm -f "$crd_check_file"' EXIT
cargo run --locked --quiet -p qrab-operator -- crd > "$crd_check_file"
cmp contrib/kubernetes/operator/crd.json "$crd_check_file"
cargo run --locked --quiet -p qrab-operator -- migration-crd > "$crd_check_file"
cmp contrib/kubernetes/operator/migration-crd.json "$crd_check_file"
cargo run --locked --quiet -p qrab-operator -- dashboard-update-policy > "$crd_check_file"
cmp contrib/kubernetes/operator/dashboard-update-policy.json "$crd_check_file"
cargo clippy --locked -p qrab-guest-agent -p qrab-iscsi-target -p qrab-operator -p qrab-dashboard -p qrab-kvm-device-plugin -p qrab-netd -p qrab-spiffe -p qrab-proton -p oidc-bearer -p bmc -p qrab-redfish -p manager -p qapi-protocol --all-targets --no-deps -- -D warnings
