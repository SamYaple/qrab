#!/usr/bin/env bash
# Live check of the ProtonVPN broker's RBAC and admission policy in a
# disposable namespace, as the broker's ServiceAccount (impersonated,
# server-side dry runs where nothing needs to persist). Needs cluster-admin;
# creates only uniquely named test objects and removes them. Never touches
# the qrab namespace or the production policy objects.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
suffix=$(od -An -N4 -tx1 /dev/urandom | tr -d ' \n')
ns=qrab-proton-policy-$suffix
policy=qrab-proton-broker-writes-check-$suffix
as=(--as "system:serviceaccount:$ns:qrab-proton-broker")
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
secret() { # name owner-name owner-uid keys...
  local name=$1 owner=$2 uid=$3; shift 3
  local data="" key
  for key in "$@"; do data+="\"$key\": \"eA==\","; done
  cat <<EOF
{"apiVersion": "v1", "kind": "Secret", "type": "Opaque",
 "metadata": {"name": "$name", "namespace": "$ns",
   "labels": {"app.kubernetes.io/component": "proton-vpn", "qrab.yapplesauce.com/vpn-profile": "$owner"},
   "ownerReferences": [{"apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "VpnProfile",
                        "name": "$owner", "uid": "$uid", "controller": true}]},
 "data": {${data%,}}}
EOF
}
apply_as() { kubectl "${as[@]}" -n "$ns" apply --server-side --field-manager qrab-proton-broker --force-conflicts --dry-run=server -f - ; }

kubectl create namespace "$ns" >/dev/null
sed -e "s/name: qrab-proton-broker-writes$/name: $policy/" \
    -e "s/policyName: qrab-proton-broker-writes$/policyName: $policy/" \
    "$here/policy.yaml" | kubectl apply -f - >/dev/null
kubectl -n "$ns" create serviceaccount qrab-proton-broker >/dev/null
kubectl -n "$ns" apply -f "$here/rbac.yaml" >/dev/null
# Admission policies take a moment to load.
sleep 3
uid=11111111-2222-4333-8444-555555555555

expect_allowed "a profile Secret owned by its profile" \
  apply_as < <(secret vpn-ch ch "$uid" wg.conf)
expect_denied "a profile Secret with another key" \
  apply_as < <(secret vpn-ch ch "$uid" wg.conf key)
expect_denied "a profile Secret named for another profile" \
  apply_as < <(secret vpn-other ch "$uid" wg.conf)
expect_denied "a Secret outside vpn-*" \
  apply_as < <(secret qrab-demo-iscsi ch "$uid" wg.conf)
expect_denied "creating proton-session" \
  kubectl "${as[@]}" -n "$ns" create secret generic proton-session --from-literal=uid=u --from-literal=refresh_token=r --dry-run=server
expect_allowed "creating proton-keys" \
  kubectl "${as[@]}" -n "$ns" create secret generic proton-keys --from-literal=uid-1=seed --dry-run=server
expect_allowed "ConfigMap proton-catalog" \
  kubectl "${as[@]}" -n "$ns" create configmap proton-catalog --from-literal=x=y --dry-run=server
expect_denied "another ConfigMap" \
  kubectl "${as[@]}" -n "$ns" create configmap other --from-literal=x=y --dry-run=server

# A hand-written vpn-* Secret cannot be taken over.
kubectl -n "$ns" create secret generic vpn-ch --from-literal=wg.conf=handmade >/dev/null
expect_denied "taking over a hand-written vpn-* Secret" \
  apply_as < <(secret vpn-ch ch "$uid" wg.conf)
kubectl -n "$ns" delete secret vpn-ch >/dev/null

# proton-session: the broker may update only uid and refresh_token.
kubectl -n "$ns" create secret generic proton-session --from-literal=uid=u --from-literal=refresh_token=r >/dev/null
expect_allowed "updating proton-session's tokens" \
  kubectl "${as[@]}" -n "$ns" patch secret proton-session --type merge -p '{"data":{"refresh_token":"cjI="}}' --dry-run=server
expect_denied "adding a key to proton-session" \
  kubectl "${as[@]}" -n "$ns" patch secret proton-session --type merge -p '{"data":{"extra":"eA=="}}' --dry-run=server
expect_denied "deleting a Secret" \
  kubectl "${as[@]}" -n "$ns" delete secret proton-session --dry-run=server
# No reads beyond its two Secrets.
expect_denied "reading another Secret" \
  kubectl "${as[@]}" -n "$ns" get secret default-token-none
expect_denied "listing Secrets" \
  kubectl "${as[@]}" -n "$ns" get secrets
expect_allowed "reading proton-session" \
  kubectl "${as[@]}" -n "$ns" get secret proton-session
expect_allowed "listing Pods (NIC anchors)" \
  kubectl "${as[@]}" -n "$ns" get pods
expect_denied "reading Pod logs" \
  kubectl "${as[@]}" -n "$ns" logs nothing
echo "PASS: the broker's Secret and ConfigMap writes are confined"
