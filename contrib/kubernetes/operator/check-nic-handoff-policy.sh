#!/usr/bin/env bash
# Live check of nic-handoff.yaml's admission policy with server-side dry runs:
# nothing is created. A uniquely named copy of the policy is bound in place of
# the operator's identity to a throwaway one (impersonated with system:masters
# so RBAC never decides); the production policy and the operator are untouched.
# Needs an admin context that may impersonate.
#
#   bash contrib/kubernetes/operator/check-nic-handoff-policy.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
suffix=$(od -An -N4 -tx1 /dev/urandom | tr -d ' \n')
policy=qrab-nic-handoff-check-$suffix
who=system:serviceaccount:qrab-nic-check-$suffix:qrab-operator
other=system:serviceaccount:qrab-nic-check-$suffix:someone-else
as=(--as="$who" --as-group=system:masters --as-group=system:authenticated)
as_other=(--as="$other" --as-group=system:masters --as-group=system:authenticated)
cleanup() {
  kubectl delete validatingadmissionpolicybinding "$policy" --ignore-not-found >/dev/null
  kubectl delete validatingadmissionpolicy "$policy" --ignore-not-found >/dev/null
}
trap cleanup EXIT
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

python3 - "$here/nic-handoff.yaml" "$policy" "$who" <<'EOF' | kubectl apply -f - >/dev/null
import sys
text, policy, who = open(sys.argv[1]).read(), sys.argv[2], sys.argv[3]
out = text.replace('name: qrab-nic-handoff\n', f'name: {policy}\n')
out = out.replace('policyName: qrab-nic-handoff\n', f'policyName: {policy}\n')
assert out.count("'system:serviceaccount:qrab:qrab-operator'") == 1
print(out.replace("'system:serviceaccount:qrab:qrab-operator'", f"'{who}'"))
EOF

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
# Policies take effect asynchronously; wait until a known-bad write is refused.
for _ in $(seq 1 60); do
  if ! anchor qrab vm-nic qrab/x qrab-nic-10-244-3-5 | kubectl create --dry-run=server "${as[@]}" -f - >/dev/null 2>&1; then break; fi
  sleep 1
done

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
