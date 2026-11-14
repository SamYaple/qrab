#!/usr/bin/env bash
# Live check of dashboard-update-policy.json with server-side dry runs against
# an existing VM: nothing changes. Needs an admin context that may impersonate.
#
#   bash contrib/kubernetes/operator/check-dashboard-update-policy.sh EXISTING_VM
set -euo pipefail
ns=qrab
existing=${1:?usage: $0 EXISTING_VM (a VirtualMachine in namespace qrab)}
policy=qrab-dashboard-vm-updates
# The policy looks only at the username. The admin group lets the dry runs pass
# RBAC whether or not the dashboard's own Role is installed yet.
as_dashboard=(--as=system:serviceaccount:$ns:qrab-dashboard --as-group=system:serviceaccounts
  --as-group=system:serviceaccounts:$ns --as-group=system:authenticated --as-group=kanidm:kubernetes:admin)

patch() { # merge patch; extra args are kubectl flags
  local body=$1; shift
  kubectl -n "$ns" patch --dry-run=server --type merge -o name "$@" virtualmachine "$existing" -p "$body" 2>&1
}
allowed() { # description, patch
  local output
  if output=$(patch "$2" "${as_dashboard[@]}"); then echo "ok: $1"; else echo "FAIL: $1: $output" >&2; exit 1; fi
}
refused() { # description, patch
  local output
  if output=$(patch "$2" "${as_dashboard[@]}"); then echo "FAIL: $1 was admitted" >&2; exit 1; fi
  case $output in
    *$policy*) echo "ok: $1" ;;
    *) echo "FAIL: $1 refused for another reason: $output" >&2; exit 1 ;;
  esac
}

kubectl get validatingadmissionpolicy "$policy" >/dev/null
kubectl get validatingadmissionpolicybinding "$policy" >/dev/null
running=$(kubectl -n "$ns" get virtualmachine "$existing" -o jsonpath='{.spec.running}')
flipped=$([ "$running" = true ] && echo false || echo true)
allowed "the dashboard powers the VM on or off" "{\"spec\":{\"running\":$flipped}}"
refused "the dashboard changes cpus" '{"spec":{"cpus":64}}'
refused "the dashboard grants itself the console" \
  '{"spec":{"console":{"allowedSubjects":["00000000-0000-4000-8000-00000000c0de"]}}}'
refused "the dashboard sets running and another field together" \
  "{\"spec\":{\"running\":$flipped,\"restartNonce\":\"dashboard-check\"}}"
refused "the dashboard adds a label" '{"metadata":{"labels":{"qrab-dashboard-check":"x"}}}'
if output=$(patch '{"spec":{"restartNonce":"admin-check"}}'); then
  echo "ok: other identities are not restricted by this policy"
else
  echo "FAIL: an admin update was refused: $output" >&2; exit 1
fi
echo "PASS: dashboard update policy"
