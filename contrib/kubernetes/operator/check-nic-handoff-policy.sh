#!/usr/bin/env bash
# Live check of the deployed nic-handoff.yaml admission policy with server-side
# dry runs: nothing is created. The policy applies to every requester (only the
# operator may set the annotation), so a renamed copy cannot be tested beside
# it; the check impersonates the operator's own ServiceAccount and another one
# (with system:masters, so RBAC never decides). Apply nic-handoff.yaml first.
# Needs an admin context that may impersonate.
#
#   bash contrib/kubernetes/operator/check-nic-handoff-policy.sh
set -euo pipefail
policy=qrab-nic-handoff
who=system:serviceaccount:qrab:qrab-operator
other=system:serviceaccount:qrab:someone-else
as=(--as="$who" --as-group=system:masters --as-group=system:authenticated)
as_other=(--as="$other" --as-group=system:masters --as-group=system:authenticated)
fail() { echo "FAIL: $*" >&2; exit 1; }
expect_denied() {
  local what=$1 output; shift
  if output=$("$@" 2>&1); then fail "$what was allowed"; fi
  case $output in
    *"$policy"*) echo "ok: $what denied" ;;
    *) fail "$what was refused for another reason: $output" ;;
  esac
}
expect_allowed() {
  local what=$1 output; shift
  output=$("$@" 2>&1) || fail "$what was refused: $output"
  echo "ok: $what allowed"
}


# anchor namespace component source pool [extra annotation yaml]
anchor() {
  cat <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: qrab-nic-handoff-check
  namespace: $1
  labels: {app.kubernetes.io/name: qrab, app.kubernetes.io/component: $2}
  annotations:
    ipam.cilium.io/handoff-from: '$3'
    ipam.cilium.io/ip-pool: '$4'
    ${5:-}
spec:
  automountServiceAccountToken: false
  securityContext: {runAsNonRoot: true, runAsUser: 65532, seccompProfile: {type: RuntimeDefault}}
  containers:
  - name: anchor
    image: $image
    imagePullPolicy: Always
    command: [/pause]
    securityContext: {allowPrivilegeEscalation: false, capabilities: {drop: [ALL]}}
EOF
}
image=$(kubectl -n qrab get deploy qrab-operator -o jsonpath='{.spec.template.spec.containers[0].image}')
export image
export -f anchor
create() { kubectl create --dry-run=server "$@" -o name -f -; }
export -f create
run() { bash -c "$2" -- "${@:3}"; }

good="anchor qrab vm-nic qrab/qrab-nic-src qrab-nic-10-244-128-250"
kubectl get validatingadmissionpolicy "$policy" >/dev/null || fail "apply nic-handoff.yaml first"

expect_allowed "an operator handoff anchor in the NIC range" run - "$good | create \"\$@\"" "${as[@]}"
expect_allowed "a pod without the annotation, from anyone" run - "anchor qrab vm-nic qrab/x qrab-nic-10-244-128-250 | sed '/handoff-from/d' | create \"\$@\"" "${as_other[@]}"
expect_denied "a handoff anchor from another requester" run - "$good | create \"\$@\"" "${as_other[@]}"
expect_denied "a handoff pod outside namespace qrab" run - "anchor default vm-nic qrab/qrab-nic-src qrab-nic-10-244-128-250 | create \"\$@\"" "${as[@]}"
expect_denied "a handoff pod that is not a vm-nic" run - "anchor qrab vm qrab/qrab-nic-src qrab-nic-10-244-128-250 | create \"\$@\"" "${as[@]}"
expect_denied "a handoff from another namespace's pod" run - "anchor qrab vm-nic default/victim qrab-nic-10-244-128-250 | create \"\$@\"" "${as[@]}"
expect_denied "a handoff into a pool outside the NIC range" run - "anchor qrab vm-nic qrab/qrab-nic-src qrab-nic-10-244-3-5 | create \"\$@\"" "${as[@]}"
expect_denied "a handoff into the default pool" run - "anchor qrab vm-nic qrab/qrab-nic-src default | create \"\$@\"" "${as[@]}"
expect_denied "a handoff with an ipv4-pool override" run - "anchor qrab vm-nic qrab/qrab-nic-src qrab-nic-10-244-128-250 'ipam.cilium.io/ipv4-pool: default' | create \"\$@\"" "${as[@]}"

# Updates: annotate an existing pod (any running pod; dry run), then change one.
victim=$(kubectl -n qrab get pods -o jsonpath='{.items[0].metadata.name}')
[ -n "$victim" ] || fail "no pod in namespace qrab to try an update on"
expect_denied "adding the annotation to a running pod" kubectl -n qrab annotate pod "$victim" --dry-run=server "${as[@]}" ipam.cilium.io/handoff-from=qrab/qrab-nic-src
echo "PASS: NIC handoff policy"
