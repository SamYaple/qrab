#!/usr/bin/env bash
# Live check of creator-policy.yaml with server-side dry runs: nothing is
# created and the operator never sees these objects. Needs an admin context
# that may impersonate (the default certificate context does).
#
#   bash contrib/kubernetes/operator/check-creator-policy.sh [EXISTING_VM]
set -euo pipefail
ns=qrab
existing=${1:-qrab-demo}
key=qrab.yapplesauce.com/creator-subject
person=00000000-0000-4000-8000-00000000c0de
as_kanidm=(--as="kanidm:$person" --as-group=kanidm:kubernetes:admin --as-group=system:authenticated)

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

for kind in mutatingadmissionpolicy mutatingadmissionpolicybinding validatingadmissionpolicy validatingadmissionpolicybinding; do
  kubectl get "$kind" qrab-vm-creator >/dev/null
done
expect "a certificate identity records no creator" "$(created "")" ""
expect "a forged creator from a non-Kanidm identity is removed" "$(created "$person")" ""
expect "a Kanidm user is recorded as the creator" "$(created "" "${as_kanidm[@]}")" "$person"
expect "a forged creator from a Kanidm user is replaced" \
  "$(created "11111111-1111-4111-8111-111111111111" "${as_kanidm[@]}")" "$person"
if kubectl -n "$ns" annotate --overwrite --dry-run=server virtualmachine "$existing" "$key=$person" >/dev/null 2>&1; then
  echo "FAIL: an update set the creator annotation" >&2; exit 1
fi
echo "ok: updates cannot set or change the creator"
echo "PASS: creator policy"
