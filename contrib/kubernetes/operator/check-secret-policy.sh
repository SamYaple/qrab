#!/usr/bin/env bash
# Live check of the operator's write-only Secret RBAC and admission policy in a
# disposable namespace. Needs cluster-admin (impersonation and cluster-scoped
# policy objects); creates only uniquely named test objects and removes them.
# It never touches the qrab namespace or the production policy objects.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
suffix=$(od -An -N4 -tx1 /dev/urandom | tr -d ' \n')
ns=qrab-secret-policy-$suffix
policy=qrab-operator-disk-secrets-check-$suffix
sa=system:serviceaccount:$ns:qrab-operator
cleanup() {
  kubectl delete validatingadmissionpolicybinding "$policy" --ignore-not-found >/dev/null
  kubectl delete validatingadmissionpolicy "$policy" --ignore-not-found >/dev/null
  kubectl delete namespace "$ns" --ignore-not-found --wait=false >/dev/null
}
trap cleanup EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
expect_denied() {
  local what=$1; shift
  if "$@" >/dev/null 2>&1; then fail "$what was allowed"; fi
  echo "ok: $what denied"
}
expect_allowed() {
  local what=$1; shift
  "$@" >/dev/null || fail "$what was refused"
  echo "ok: $what allowed"
}

kubectl create namespace "$ns" >/dev/null
sed -e "s/name: qrab-operator-disk-secrets$/name: $policy/" \
    -e "s/policyName: qrab-operator-disk-secrets$/policyName: $policy/" \
    -e "s/kubernetes.io\/metadata.name: qrab$/kubernetes.io\/metadata.name: $ns/" \
    "$here/secret-policy.yaml" | kubectl apply -f - >/dev/null
kubectl -n "$ns" create serviceaccount qrab-operator >/dev/null
kubectl -n "$ns" create role qrab-operator-secrets --verb=create,patch,delete --resource=secrets >/dev/null
kubectl -n "$ns" create rolebinding qrab-operator-secrets --role=qrab-operator-secrets --serviceaccount="$ns:qrab-operator" >/dev/null
kubectl -n "$ns" apply -f - >/dev/null <<EOF
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachine
metadata: {name: vm1}
spec:
  running: false
  stateVolumeClaim: state
  console: {publicUrl: 'https://vm1.example', allowedSubjects: ['6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b']}
EOF
uid=$(kubectl -n "$ns" get virtualmachine vm1 -o jsonpath='{.metadata.uid}')
kubectl -n "$ns" create secret generic hand --from-literal=password=0123456789abcdef0123456789abcdef >/dev/null
# Hand-made, unlabelled, but named like an operator credential.
kubectl -n "$ns" create secret generic vm1-disk-data --from-literal=password=0123456789abcdef0123456789abcdef >/dev/null

disk_secret() { # name disk owner-uid
  cat <<EOF
apiVersion: v1
kind: Secret
type: Opaque
metadata:
  name: $1
  labels: {app.kubernetes.io/name: qrab, app.kubernetes.io/component: disk-credential, app.kubernetes.io/instance: vm1, qrab.yapplesauce.com/owner-uid: '$3'}
  annotations: {qrab.yapplesauce.com/disk: '$2', qrab.yapplesauce.com/credential-generation: '1'}
  ownerReferences: [{apiVersion: qrab.yapplesauce.com/v1alpha1, kind: VirtualMachine, name: vm1, uid: '$3', controller: true, blockOwnerDeletion: false}]
data: {password: MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=}
EOF
}
apply_as_operator() { kubectl -n "$ns" apply --server-side --force-conflicts --field-manager=qrab-operator --as="$sa" -f -; }

# Policies take effect asynchronously on each API server; wait until a
# known-bad write is refused five times in a row.
refused=0
for _ in $(seq 1 90); do
  if disk_secret vm1-disk-x root "$uid" | apply_as_operator >/dev/null 2>&1; then
    refused=0
  else
    refused=$((refused + 1)); [ "$refused" -ge 5 ] && break
  fi
  kubectl -n "$ns" delete secret vm1-disk-x --ignore-not-found >/dev/null
  sleep 1
done

expect_denied "reading a hand-declared Secret" kubectl -n "$ns" get secret hand --as="$sa"
expect_denied "listing Secrets" kubectl -n "$ns" get secrets --as="$sa"
expect_denied "a misnamed disk Secret" sh -c "$(declare -f disk_secret apply_as_operator); ns=$ns sa=$sa; disk_secret vm1-disk-x root $uid | apply_as_operator"
expect_allowed "creating this VM's disk Secret" sh -c "$(declare -f disk_secret apply_as_operator); ns=$ns sa=$sa; disk_secret vm1-disk-root root $uid | apply_as_operator"
expect_allowed "rotating this VM's disk Secret" sh -c "$(declare -f disk_secret apply_as_operator); ns=$ns sa=$sa; disk_secret vm1-disk-root root $uid | sed 's/generation: .1./generation: \"2\"/' | apply_as_operator"
expect_denied "re-owning a disk Secret to another UID" sh -c "$(declare -f disk_secret apply_as_operator); ns=$ns sa=$sa; disk_secret vm1-disk-root root 00000000-0000-4000-8000-000000000000 | apply_as_operator"
expect_denied "overwriting a look-alike hand-made Secret" sh -c "$(declare -f disk_secret apply_as_operator); ns=$ns sa=$sa; disk_secret vm1-disk-data data $uid | apply_as_operator"
expect_denied "deleting a look-alike hand-made Secret" kubectl -n "$ns" delete secret vm1-disk-data --as="$sa"
expect_denied "deleting a hand-declared Secret" kubectl -n "$ns" delete secret hand --as="$sa"
expect_allowed "deleting this VM's disk Secret" kubectl -n "$ns" delete secret vm1-disk-root --as="$sa"
kubectl -n "$ns" get secret hand vm1-disk-data >/dev/null || fail "a hand-made Secret was changed or removed"
[ "$(kubectl -n "$ns" get secret vm1-disk-data -o jsonpath='{.metadata.labels}')" = "" ] || fail "a hand-made Secret was relabelled"
echo "secret policy check passed"
