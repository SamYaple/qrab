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
# Finalizers: the dashboard may add or remove only its own.
own=qrab.yapplesauce.com/access-groups
finalizers=$(kubectl -n "$ns" get virtualmachine "$existing" -o json | python3 -I -c '
import json, sys
own = sys.argv[1]
f = json.load(sys.stdin)["metadata"].get("finalizers", [])
others = [x for x in f if x != own]
print(json.dumps(others))
print(json.dumps(others + [own]))
print(json.dumps(others + ["example.com/dashboard-check"]))
print(json.dumps([x for x in others if x != "qrab.yapplesauce.com/iscsi-state"] if "qrab.yapplesauce.com/iscsi-state" in others else others[1:]))
' "$own")
{ read -r without_own; read -r with_own; read -r with_foreign; read -r one_less; }<<<"$finalizers"
allowed "the dashboard adds its own finalizer" "{\"metadata\":{\"finalizers\":$with_own}}"
allowed "the dashboard removes its own finalizer" "{\"metadata\":{\"finalizers\":$without_own}}"
refused "the dashboard adds another finalizer" "{\"metadata\":{\"finalizers\":$with_foreign}}"
if [[ $one_less != "$without_own" ]]; then
  refused "the dashboard removes another component's finalizer" "{\"metadata\":{\"finalizers\":$one_less}}"
fi
if output=$(patch '{"spec":{"restartNonce":"admin-check"}}'); then
  echo "ok: other identities are not restricted by this policy"
else
  echo "FAIL: an admin update was refused: $output" >&2; exit 1
fi
echo "PASS: dashboard update policy"
