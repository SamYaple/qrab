#!/usr/bin/env bash
# Live check of creator-policy.yaml with server-side dry runs: nothing is
# created and the operator never sees these objects. Needs an admin context
# that may impersonate (the default certificate context does).
#
#   bash contrib/kubernetes/operator/check-creator-policy.sh EXISTING_VM
set -euo pipefail
ns=qrab
existing=${1:?usage: $0 EXISTING_VM (a VirtualMachine in namespace qrab)}
key=qrab.yapplesauce.com/creator-subject
person=00000000-0000-4000-8000-00000000c0de
as_kanidm=(--as="kanidm:$person" --as-group=kanidm:kubernetes:admin --as-group=system:authenticated)
# The policy looks only at the username. The admin group lets the dry runs pass
# RBAC whether or not the dashboard's own Role is installed yet.
as_dashboard=(--as=system:serviceaccount:$ns:qrab-dashboard --as-group=system:serviceaccounts
  --as-group=system:serviceaccounts:$ns --as-group=system:authenticated --as-group=kanidm:kubernetes:admin)

manifest() { # annotation value, or empty for none
  local annotations=""
  [ -n "$1" ] && annotations="  annotations: {\"$key\": \"$1\"}"
  cat <<EOF
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachine
metadata:
  name: qrab-creator-check
  namespace: $ns
$annotations
spec:
  stateVolumeClaim: qrab-creator-check-state
  memoryMiB: 512
  cpus: 1
  running: false
  bootDisk: /opt/qrab/demo/boot.img
  components:
    bmc: {enabled: false}
EOF
}
created() { # annotation value as admission leaves it; extra args are kubectl flags
  local value=$1; shift
  manifest "$value" | kubectl create --dry-run=server -o json "$@" -f - |
    python3 -c 'import json,sys; print((json.load(sys.stdin)["metadata"].get("annotations") or {}).get(sys.argv[1], ""))' "$key"
}
expect() { # description, actual, expected
  if [ "$2" = "$3" ]; then echo "ok: $1"; else echo "FAIL: $1 (got '$2', want '$3')" >&2; exit 1; fi
}
refused() { # description, annotation value; extra args are kubectl flags
  local what=$1 value=$2 output; shift 2
  if output=$(manifest "$value" | kubectl create --dry-run=server -o name "$@" -f - 2>&1); then
    echo "FAIL: $what was admitted" >&2; exit 1
  fi
  # Refused by this policy, not by RBAC or anything else.
  case $output in
    *qrab-vm-creator*) echo "ok: $what" ;;
    *) echo "FAIL: $what: $output" >&2; exit 1 ;;
  esac
}

for kind in mutatingadmissionpolicy mutatingadmissionpolicybinding validatingadmissionpolicy validatingadmissionpolicybinding; do
  kubectl get "$kind" qrab-vm-creator >/dev/null
done
expect "a certificate identity records no creator" "$(created "")" ""
expect "a forged creator from a non-Kanidm identity is removed" "$(created "$person")" ""
expect "a Kanidm user is recorded as the creator" "$(created "" "${as_kanidm[@]}")" "$person"
expect "a forged creator from a Kanidm user is replaced" \
  "$(created "11111111-1111-4111-8111-111111111111" "${as_kanidm[@]}")" "$person"
expect "the dashboard records its user's subject" "$(created "$person" "${as_dashboard[@]}")" "$person"
refused "a dashboard create without a creator" "" "${as_dashboard[@]}"
refused "a dashboard create with a non-UUID creator" "kanidm:$person" "${as_dashboard[@]}"
refused "a dashboard create with an uppercase UUID" "${person^^}" "${as_dashboard[@]}"
if output=$(kubectl -n "$ns" annotate --overwrite --dry-run=server "${as_dashboard[@]}" \
    virtualmachine "$existing" "$key=11111111-1111-4111-8111-111111111111" 2>&1); then
  echo "FAIL: a dashboard update set the creator annotation" >&2; exit 1
fi
case $output in
  *immutable*) echo "ok: the dashboard cannot change the creator either" ;;
  *) echo "FAIL: dashboard update refused for another reason: $output" >&2; exit 1 ;;
esac
if kubectl -n "$ns" annotate --overwrite --dry-run=server virtualmachine "$existing" "$key=$person" >/dev/null 2>&1; then
  echo "FAIL: an update set the creator annotation" >&2; exit 1
fi
echo "ok: updates cannot set or change the creator"
echo "PASS: creator policy"
