#!/usr/bin/env bash
# Print console.yaml bound to an EXISTING VM's immutable UID; it applies nothing
# and never creates a VM. Review the output, then `kubectl -n NAMESPACE apply -f -`.
#
#   bash contrib/kubernetes/examples/qrab-demo/render-console.sh NAMESPACE VM
set -euo pipefail
namespace=${1:?usage: render-console.sh NAMESPACE VM}
vm=${2:?usage: render-console.sh NAMESPACE VM}
console_directory=$(cd "$(dirname "$0")" && pwd)
vm_uid=$(kubectl -n "$namespace" get virtualmachine "$vm" -o jsonpath='{.metadata.uid}')
[[ $vm_uid =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]] || {
    echo "VirtualMachine $namespace/$vm has no valid immutable UID." >&2
    exit 1
}
sed "s/QRAB_VM_UID/$vm_uid/g" "$console_directory/console.yaml"
