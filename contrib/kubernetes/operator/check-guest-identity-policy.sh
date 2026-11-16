#!/usr/bin/env bash
# Live check of guest-identity.yaml's admission policy with server-side dry
# runs: nothing is created. A uniquely named copy of the policy is bound to a
# throwaway identity (impersonated with system:masters so RBAC never decides),
# so the production policy and the operator are untouched. Needs an admin
# context that may impersonate, and spire-controller-manager's
# ClusterStaticEntry CRD.
#
#   bash contrib/kubernetes/operator/check-guest-identity-policy.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
kubectl get crd clusterstaticentries.spire.spiffe.io >/dev/null
suffix=$(od -An -N4 -tx1 /dev/urandom | tr -d ' \n')
policy=qrab-operator-guest-identity-check-$suffix
who=system:serviceaccount:qrab-guest-check-$suffix:qrab-operator
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

python3 - "$here/guest-identity.yaml" "$policy" "$who" <<'EOF' | kubectl apply -f - >/dev/null
import sys
text, policy, who = open(sys.argv[1]).read(), sys.argv[2], sys.argv[3]
docs = [d for d in text.split('\n---\n') if 'kind: ValidatingAdmissionPolicy' in d]
out = '\n---\n'.join(docs)
out = out.replace('name: qrab-operator-guest-identity\n', f'name: {policy}\n')
out = out.replace('policyName: qrab-operator-guest-identity\n', f'policyName: {policy}\n')
assert out.count("'system:serviceaccount:qrab:qrab-operator'") == 1
print(out.replace("'system:serviceaccount:qrab:qrab-operator'", f"'{who}'"))
EOF

vm=0b7c4e6a-1d2e-4f30-8a4b-5c6d7e8f9a0b
other=11111111-2222-4333-8444-555555555555
server=spiffe://yapplesauce.com/spire/server
node_id=spiffe://yapplesauce.com/qrab/guest-node/$vm
subject="tpm_devid:subject:cn:qrab-vm-$vm"
issuer="tpm_devid:issuer:cn:qrab-devid-ca-1"
entry() { # name spiffeID parentID selectors [extra spec fields] [owner-uid] [className]
  cat <<EOF
apiVersion: spire.spiffe.io/v1alpha1
kind: ClusterStaticEntry
metadata:
  name: $1
  labels: {app.kubernetes.io/name: qrab, app.kubernetes.io/component: guest-identity, qrab.yapplesauce.com/owner-uid: '${6-$vm}', qrab.yapplesauce.com/vm-namespace: qrab}
spec: {spiffeID: '$2', parentID: '$3', selectors: $4, className: '${7-kubernetes-tpm}'${5:+, $5}}
EOF
}
alias_entry() { # selectors [parentID] [extra] [owner-uid] [className]
  entry "qrab-guest-node-$vm" "$node_id" "${2:-$server}" "$1" "${3:-}" "${4-$vm}" "${5-kubernetes-tpm}"
}
workload_entry() { # selectors [parentID] [extra] [name]
  local name=${4:-web}
  entry "qrab-guest-$vm-$name" "spiffe://yapplesauce.com/qrab/guest/$vm/$name" "${2:-$node_id}" "$1" "${3:-x509SVIDTTL: 10m}"
}
create() { kubectl create --dry-run=server "${as[@]}" -o name -f -; }
export -f entry alias_entry workload_entry create
export vm other server node_id subject issuer
run() { bash -c "as=(${as[*]}); $1"; }

# Policies take effect asynchronously on each API server; wait until a
# known-bad write is refused five times in a row.
refused=0
for _ in $(seq 1 90); do
  if entry other-entry "$node_id" "$server" "['$subject', '$issuer']" | kubectl create --dry-run=server "${as[@]}" -f - >/dev/null 2>&1; then
    refused=0
  else
    refused=$((refused + 1)); [ "$refused" -ge 5 ] && break
  fi
  sleep 1
done

expect_allowed "a guest node alias" run "alias_entry \"['\$subject', '\$issuer']\" | create"
expect_allowed "a guest workload" run "workload_entry \"['unix:uid:1000', 'systemd:id:web.service']\" | create"
expect_denied "an entry not named qrab-guest-*" run "entry other-entry \"\$node_id\" \"\$server\" \"['\$subject', '\$issuer']\" | create"
expect_denied "an entry outside the guest IDs" run "entry qrab-guest-node-\$vm spiffe://yapplesauce.com/kubernetes/qrab/qrab-operator \"\$server\" \"['\$subject', '\$issuer']\" | create"
expect_denied "an upper-case VM UID" run "entry qrab-guest-node-\$vm spiffe://yapplesauce.com/qrab/guest-node/\${vm^^} \"\$server\" \"['\$subject', '\$issuer']\" | create"
expect_denied "an alias named after another VM" run "entry qrab-guest-node-\$other \"\$node_id\" \"\$server\" \"['\$subject', '\$issuer']\" | create"
expect_denied "an entry without its VM's owner label" run "alias_entry \"['\$subject', '\$issuer']\" '' '' \$other | create"
expect_denied "an alias not parented by the server" run "alias_entry \"['\$subject', '\$issuer']\" spiffe://yapplesauce.com/spire/agent/x | create"
expect_denied "an alias for another VM's DevID" run "alias_entry \"['tpm_devid:subject:cn:qrab-vm-\$other', '\$issuer']\" | create"
expect_denied "an alias with an extra selector" run "alias_entry \"['\$subject', '\$issuer', 'tpm_devid:subject:cn:x']\" | create"
expect_denied "an alias without an issuer" run "alias_entry \"['\$subject', 'tpm_devid:subject:cn:x']\" | create"
expect_denied "an alias with an empty issuer CN" run "alias_entry \"['\$subject', 'tpm_devid:issuer:cn:']\" | create"
expect_denied "a workload parented by the server" run "workload_entry \"['unix:uid:1000']\" \"\$server\" | create"
expect_denied "a workload under another VM's alias" run "workload_entry \"['unix:uid:1000']\" spiffe://yapplesauce.com/qrab/guest-node/\$other | create"
expect_denied "a workload with a k8s selector" run "workload_entry \"['unix:uid:1000', 'k8s:ns:qrab']\" | create"
expect_denied "a workload with nine selectors" run "workload_entry \"['unix:uid:1', 'unix:uid:2', 'unix:uid:3', 'unix:uid:4', 'unix:uid:5', 'unix:uid:6', 'unix:uid:7', 'unix:uid:8', 'unix:uid:9']\" | create"
expect_denied "a workload named unlike its spiffeID" run "entry qrab-guest-\$vm-api spiffe://yapplesauce.com/qrab/guest/\$vm/web \"\$node_id\" \"['unix:uid:1000']\" | create"
expect_denied "an admin entry" run "workload_entry \"['unix:uid:1000']\" '' 'admin: true' | create"
expect_denied "a downstream entry" run "workload_entry \"['unix:uid:1000']\" '' 'downstream: true' | create"
expect_denied "a stored SVID" run "workload_entry \"['unix:uid:1000']\" '' 'storeSVID: true' | create"
expect_denied "DNS names" run "workload_entry \"['unix:uid:1000']\" '' 'dnsNames: [example.com]' | create"
expect_denied "federation" run "workload_entry \"['unix:uid:1000']\" '' 'federatesWith: [example.org]' | create"
expect_denied "another class" run "alias_entry \"['\$subject', '\$issuer']\" '' '' \$vm kubernetes | create"

# Deletes are admitted by the existing object's name, so they need real
# entries; check them against whatever the cluster holds.
names=$(kubectl get clusterstaticentries -o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{end}')
foreign=$(grep -v '^qrab-guest-' <<<"$names" | head -n1 || true)
owned=$(grep '^qrab-guest-' <<<"$names" | head -n1 || true)
if [ -n "$foreign" ]; then
  expect_denied "deleting $foreign" kubectl delete clusterstaticentry "$foreign" --dry-run=server "${as[@]}"
else
  echo "skip: no ClusterStaticEntry outside qrab-guest-* to try deleting"
fi
if [ -n "$owned" ]; then
  expect_allowed "deleting $owned" kubectl delete clusterstaticentry "$owned" --dry-run=server "${as[@]}"
else
  echo "skip: no qrab-guest-* ClusterStaticEntry to try deleting"
fi
echo "PASS: guest identity policy"
