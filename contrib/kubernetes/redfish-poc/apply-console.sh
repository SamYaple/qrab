#!/usr/bin/env bash
# Explicit public console infrastructure for an existing POC; never a VM create.
set -euo pipefail
console_directory=$(cd "$(dirname "$0")" && pwd)
console_vm_uid=$(kubectl -n qrab get virtualmachine qrab-redfish-poc -o jsonpath='{.metadata.uid}')
[[ $console_vm_uid =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]] || {
    echo 'The existing POC has no valid immutable VM UID.' >&2
    exit 1
}
console_rendered=$(mktemp)
trap 'rm -f "$console_rendered"' EXIT
sed "s/QRAB_VM_UID/$console_vm_uid/g" "$console_directory/console.yaml" > "$console_rendered"
kubectl -n qrab apply -f "$console_rendered"
