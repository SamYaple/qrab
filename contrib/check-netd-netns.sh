#!/usr/bin/env bash
# Run qrab-netd's root-only kernel tests (projects/qrab-netd/tests/netns.rs)
# where they can really run: a throwaway privileged Pod from the deployed
# qrab-netd image, which has ip, nft and wg (the nodes have no nft or wg, and
# the VM host's kernel is not the nodes'). Run on the VM host as an admin
# kubectl user. NODE picks the node (default kube03). Fails if any test
# skipped, so a missing tool cannot pass for a green run.
set -euo pipefail
cd "$(dirname "$0")/.."
node=${NODE:-kube03}
pod=qrab-netd-netns-test-$(od -An -N3 -tx1 /dev/urandom | tr -d ' \n')
image=$(kubectl -n qrab-node get ds qrab-netd -o jsonpath='{.spec.template.spec.containers[0].image}')
cleanup() { kubectl -n qrab-node delete pod "$pod" --ignore-not-found --wait=false >/dev/null; }
trap cleanup EXIT
bin=$(cargo test -q --locked -p qrab-netd --test netns --no-run --message-format=json 2>/dev/null |
  python3 -I -c 'import json,sys
for line in sys.stdin:
    try: m=json.loads(line)
    except ValueError: continue
    if m.get("reason")=="compiler-artifact" and m.get("executable") and m["target"]["name"]=="netns": print(m["executable"])' | tail -1)
[ -x "$bin" ] || { echo "FAIL: the netns test binary did not build" >&2; exit 1; }
kubectl apply -f - >/dev/null <<EOF
apiVersion: v1
kind: Pod
metadata: {name: $pod, namespace: qrab-node, labels: {app.kubernetes.io/name: qrab-netd-netns-test}}
spec:
  # Pull rights only: the plugin account holds no API rights.
  serviceAccountName: qrab-kvm-device-plugin
  automountServiceAccountToken: false
  nodeName: $node
  restartPolicy: Never
  tolerations: [{key: node-role.kubernetes.io/control-plane, operator: Exists, effect: NoSchedule}]
  containers:
  - name: test
    image: $image
    imagePullPolicy: Always
    command: [sleep, '900']
    securityContext: {privileged: true, runAsUser: 0}
    volumeMounts: [{name: work, mountPath: /work}]
  volumes: [{name: work, emptyDir: {}}]
EOF
kubectl -n qrab-node wait --for=condition=Ready "pod/$pod" --timeout=180s >/dev/null
kubectl -n qrab-node exec -i "$pod" -- sh -c 'cat > /work/netns && chmod +x /work/netns' < "$bin"
out=$(kubectl -n qrab-node exec "$pod" -- /work/netns --test-threads=1 --nocapture 2>&1) || { echo "$out"; echo "FAIL: netns tests" >&2; exit 1; }
echo "$out" | grep -E '^test |test result'
if echo "$out" | grep -q 'skipped:'; then
  echo "$out" | grep 'skipped:' >&2
  echo "FAIL: a netns test skipped" >&2
  exit 1
fi
echo "PASS: qrab-netd netns tests on $node in $image"
