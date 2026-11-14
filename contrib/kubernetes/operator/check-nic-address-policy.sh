#!/usr/bin/env bash
# Live check of nic-addresses.yaml's admission policy with server-side dry
# runs: nothing is created. A uniquely named copy of the policy is bound to a
# throwaway identity (impersonated with system:masters so RBAC never decides),
# so the production policy and the operator are untouched. Needs an admin
# context that may impersonate.
#
#   bash contrib/kubernetes/operator/check-nic-address-policy.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
suffix=$(od -An -N4 -tx1 /dev/urandom | tr -d ' \n')
policy=qrab-operator-nic-addresses-check-$suffix
who=system:serviceaccount:qrab-nic-check-$suffix:qrab-operator
as=(--as="$who" --as-group=system:masters --as-group=system:authenticated)
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

python3 - "$here/nic-addresses.yaml" "$policy" "$who" <<'EOF' | kubectl apply -f - >/dev/null
import sys
text, policy, who = open(sys.argv[1]).read(), sys.argv[2], sys.argv[3]
docs = [d for d in text.split('\n---\n') if 'kind: ValidatingAdmissionPolicy' in d]
out = '\n---\n'.join(docs)
out = out.replace('name: qrab-operator-nic-addresses\n', f'name: {policy}\n')
out = out.replace('policyName: qrab-operator-nic-addresses\n', f'policyName: {policy}\n')
assert out.count("'system:serviceaccount:qrab:qrab-operator'") == 1
print(out.replace("'system:serviceaccount:qrab:qrab-operator'", f"'{who}'"))
EOF

pool() { # name cidr [component] [skip-masquerade value]
  cat <<EOF
apiVersion: cilium.io/v2alpha1
kind: CiliumPodIPPool
metadata:
  name: $1
  labels: {app.kubernetes.io/name: qrab, app.kubernetes.io/component: ${3:-vm-nic-address}}
  annotations: {ipam.cilium.io/skip-masquerade: '${4-true}'}
  finalizers: [qrab.yapplesauce.com/nic-address]
spec: {ipv4: {cidrs: ['$2'], maskSize: 32}}
EOF
}
route() { # namespace name prefix [extra neighbor yaml]
  cat <<EOF
apiVersion: frrk8s.metallb.io/v1beta1
kind: FRRConfiguration
metadata:
  name: $2
  namespace: $1
  labels: {app.kubernetes.io/name: qrab, app.kubernetes.io/component: vm-nic-address}
spec:
  nodeSelector: {matchLabels: {kubernetes.io/hostname: kube01}}
  bgp:
    routers:
    - asn: 64512
      prefixes: ['$3']
      neighbors:
      - address: 10.203.0.1
        asn: 64513
        toAdvertise: {allowed: {mode: filtered, prefixes: ['$3']}}
        ${4:-}
EOF
}
create() { kubectl create --dry-run=server "${as[@]}" -o name -f -; }
export -f pool route create
export policy who
run() { bash -c "as=(${as[*]}); $1"; }

# Policies take effect asynchronously; wait until a known-bad write is refused.
for _ in $(seq 1 60); do
  if ! pool qrab-nic-10-244-3-5 10.244.3.5/32 | kubectl create --dry-run=server "${as[@]}" -f - >/dev/null 2>&1; then break; fi
  sleep 1
done

expect_allowed "a labelled /32 pool in the range" run "pool qrab-nic-10-244-128-250 10.244.128.250/32 | create"
expect_denied "a pool outside the NIC range" run "pool qrab-nic-10-244-3-5 10.244.3.5/32 | create"
expect_denied "a pool whose CIDR is not its name" run "pool qrab-nic-10-244-128-250 10.244.128.251/32 | create"
expect_denied "a wider pool" run "pool qrab-nic-10-244-128-0 10.244.128.0/24 | create"
expect_denied "an unlabelled pool" run "pool qrab-nic-10-244-128-250 10.244.128.250/32 other | create"
expect_denied "a pool that masquerades its guest" run "pool qrab-nic-10-244-128-250 10.244.128.250/32 vm-nic-address false | create"
expect_denied "a pool without skip-masquerade" run "pool qrab-nic-10-244-128-250 10.244.128.250/32 vm-nic-address '' | create"
expect_denied "resizing the default pool" kubectl patch ciliumpodippool default --type merge --dry-run=server "${as[@]}" -p '{"spec":{"ipv4":{"cidrs":["10.244.0.0/16"]}}}'
expect_denied "deleting the default pool" kubectl delete ciliumpodippool default --dry-run=server "${as[@]}"
expect_allowed "a NIC route in frr-k8s-system" run "route frr-k8s-system qrab-nic-10-244-128-250 10.244.128.250/32 | create"
expect_denied "a NIC route elsewhere" run "route default qrab-nic-10-244-128-250 10.244.128.250/32 | create"
expect_denied "a route announcing another prefix" run "route frr-k8s-system qrab-nic-10-244-128-250 10.244.0.0/24 | create"
expect_denied "a route that accepts routes" run "route frr-k8s-system qrab-nic-10-244-128-250 10.244.128.250/32 'toReceive: {allowed: {mode: all}}' | create"
expect_denied "changing a node's base route" kubectl -n frr-k8s-system patch frrconfiguration kube01 --type merge --dry-run=server "${as[@]}" -p '{"metadata":{"labels":{"x":"y"}}}'
node=$(kubectl get ciliumnodes -o jsonpath='{.items[0].metadata.name}')
allocated=$(kubectl get ciliumnode "$node" -o jsonpath='{.spec.ipam.pools.allocated}')
expect_denied "dropping a node's default pool" kubectl patch ciliumnode "$node" --type merge --dry-run=server "${as[@]}" -p '{"spec":{"ipam":{"pools":{"allocated":[]}}}}'
expect_denied "relabelling a CiliumNode" kubectl patch ciliumnode "$node" --type merge --dry-run=server "${as[@]}" -p '{"metadata":{"labels":{"x":"y"}}}'
expect_allowed "rewriting a node's pools unchanged" kubectl patch ciliumnode "$node" --type merge --dry-run=server "${as[@]}" -p "{\"spec\":{\"ipam\":{\"pools\":{\"allocated\":$allocated}}}}"
echo "PASS: NIC address policy"
