#!/usr/bin/env bash
# Live check of rbac.yaml's qrab-netd-anchor-status policy with server-side
# dry runs. A uniquely named copy is bound to a throwaway identity
# (impersonated with system:masters, with a node-name extra like a Pod-bound
# token's), and a throwaway tap-mode anchor Pod is created in qrab and deleted
# afterwards; the production policy and agent are untouched. Needs an admin
# context that may impersonate.
#
#   bash contrib/kubernetes/netd/check-status-policy.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
suffix=$(od -An -N4 -tx1 /dev/urandom | tr -d ' \n')
policy=qrab-netd-anchor-status-check-$suffix
who=system:serviceaccount:qrab-netd-check-$suffix:qrab-netd
pod=qrab-netd-check-$suffix
cleanup() {
  kubectl -n qrab delete pod "$pod" --ignore-not-found --wait=false >/dev/null
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

python3 - "$here/rbac.yaml" "$policy" "$who" <<'EOF' | kubectl apply -f - >/dev/null
import sys
text, policy, who = open(sys.argv[1]).read(), sys.argv[2], sys.argv[3]
docs = [d for d in text.split('\n---\n') if 'kind: ValidatingAdmissionPolicy' in d]
out = '\n---\n'.join(docs).replace('name: qrab-netd-anchor-status\n', f'name: {policy}\n')
out = out.replace('policyName: qrab-netd-anchor-status\n', f'policyName: {policy}\n')
old = "'system:serviceaccount:qrab-node:qrab-kvm-device-plugin'"
assert out.count(old) == 1
print(out.replace(old, f"'{who}'"))
EOF

node=$(kubectl get nodes -o jsonpath='{.items[0].metadata.name}')
other=$(kubectl get nodes -o jsonpath='{.items[1].metadata.name}')
kubectl apply -f - >/dev/null <<EOF
apiVersion: v1
kind: Pod
metadata:
  name: $pod
  namespace: qrab
  labels: {app.kubernetes.io/name: qrab, app.kubernetes.io/component: vm-nic, qrab.yapplesauce.com/owner-uid: check-$suffix, qrab.yapplesauce.com/nic: lan}
  annotations: {qrab.yapplesauce.com/nic-datapath: tap}
spec:
  nodeName: $node
  automountServiceAccountToken: false
  terminationGracePeriodSeconds: 0
  securityContext: {runAsNonRoot: true, runAsUser: 65532, seccompProfile: {type: RuntimeDefault}}
  containers:
  - name: anchor
    image: docker.io/library/busybox:1.37
    command: [sleep, '3600']
    securityContext: {allowPrivilegeEscalation: false, capabilities: {drop: [ALL]}}
EOF

status() { # node-extra condition-type
  kubectl -n qrab patch pod "$pod" --subresource=status --type=strategic --dry-run=server \
    --as="$who" --as-group=system:masters --as-group=system:authenticated \
    ${1:+--as-user-extra=authentication.kubernetes.io/node-name=$1} \
    -p "{\"status\":{\"conditions\":[{\"type\":\"$2\",\"status\":\"True\",\"reason\":\"Check\"}]}}"
}
# Policies take effect asynchronously; wait until a known-bad write is refused.
for _ in $(seq 1 60); do
  if ! status "$other" qrab.yapplesauce.com/nic-plumbed >/dev/null 2>&1; then break; fi
  sleep 1
done

expect_allowed "its condition on an anchor of its own node" status "$node" qrab.yapplesauce.com/nic-plumbed
expect_denied "an anchor on another node" status "$other" qrab.yapplesauce.com/nic-plumbed
expect_denied "a token without a node" status "" qrab.yapplesauce.com/nic-plumbed
expect_denied "another condition" status "$node" Ready
kubectl -n qrab annotate pod "$pod" qrab.yapplesauce.com/nic-datapath=other --overwrite >/dev/null
expect_denied "a NIC Pod not annotated for tap" status "$node" qrab.yapplesauce.com/nic-plumbed
echo "PASS: qrab-netd status policy"
