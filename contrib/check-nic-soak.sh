#!/usr/bin/env bash
# NIC stability soak (docs/networking.md, "Stability"). Run on the VM host as
# an admin kubectl user.
#   1. MOVES (default 20) live migrations through contrib/check-nic-migration.sh,
#      which checks every move (loss within QEMU's pause, one TCP stream, same
#      boot/TPM/UEFI, egress from the guest's /32); then a cluster-wide audit
#      that the VM left no pool, route, FRRConfiguration, CiliumEndpoint,
#      anchor or forward route behind.
#   2. A fresh VM pinged every 0.2 s and streaming over one TCP connection
#      while qrab-netd, then the Cilium agent, restart on its node; the longest
#      ping gap and the stream must stay within RESTART_GAP_S, without a reset.
#
#   MOVES=20 bash contrib/check-nic-soak.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
ns=qrab
moves=${MOVES:-20}
host_ip=${HOST_IP:-10.203.0.1}
host_port=${HOST_PORT:-18191}
restart_gap=${RESTART_GAP_S:-3}
vm=nicsoak
work=$(mktemp -d)
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
fail() { log "FAIL: $*"; exit 1; }
pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  kubectl -n "$ns" delete virtualmachine "$vm-rst" --ignore-not-found --wait=true --timeout=180s >/dev/null 2>&1 || true
  kubectl -n "$ns" delete pvc "$vm-rst-state" --ignore-not-found >/dev/null 2>&1 || true
  kubectl -n "$ns" delete ciliumnetworkpolicy "sg-$vm-rst" --ignore-not-found >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT
field() { kubectl -n "$ns" get "$1" "$2" -o jsonpath="$3"; }
wait_for() {
  local limit=$1 what=$2; shift 2
  for _ in $(seq 1 "$limit"); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done
  fail "$what within ${limit}s"
}
node_ip_routes() { # node: proto-252 forward routes in its host namespace
  local netd
  netd=$(kubectl -n qrab-node get pods -o wide | awk -v n="$1" '/qrab-netd/ && $7 == n {print $1}')
  kubectl -n qrab-node exec "$netd" -- nsenter -t 1 -n ip -4 route show proto 252
}
summary=()

# 1. Repeated migrations, then the leftover audit.
log "phase 1: $moves migrations"
MOVES=$moves bash "$here/check-nic-migration.sh" "$vm" | tee "$work/migration.log"
grep -q 'PASS: tap NIC live migration' "$work/migration.log" || fail "the migrations failed"
address=$(sed -n 's/.*guest \(10\.[0-9.]*\), boot.*/\1/p' "$work/migration.log" | head -1)
pool="qrab-nic-${address//./-}"
log "auditing what $vm ($address) left behind"
kubectl get ciliumpodippool "$pool" >/dev/null 2>&1 && fail "pool $pool remains"
kubectl -n frr-k8s-system get frrconfiguration "$pool" >/dev/null 2>&1 && fail "route $pool remains"
[ -z "$(kubectl -n "$ns" get pods -l "app.kubernetes.io/instance=$vm" -o name)" ] || fail "Pods of $vm remain"
kubectl -n "$ns" get ciliumendpoints -o name | grep -q "/$vm" && fail "CiliumEndpoints of $vm remain"
for n in kube01 kube02 kube03; do
  kubectl get ciliumnode "$n" -o jsonpath='{.spec.ipam.pools.requested[*].pool}' | tr ' ' '\n' | grep -qx "$pool" \
    && fail "CiliumNode $n still requests $pool"
  node_ip_routes "$n" | grep -q "^$address " && fail "$n still forwards $address"
done
summary+=("$moves migrations passed; no pool, route, Pod, endpoint, CiliumNode entry or forward route left")
summary+=("$(grep -c '^  move ' "$work/migration.log" || true) per-move lines:")
while read -r line; do summary+=("  $line"); done < <(grep '^  move ' "$work/migration.log")

# 2. Agent restarts under traffic.
log "phase 2: qrab-netd and Cilium agent restarts under traffic"
python3 -u -c '
import socketserver,sys,time,itertools
ids=itertools.count(1)
class H(socketserver.StreamRequestHandler):
    def handle(self):
        n=next(ids)
        for line in self.rfile:
            print(f"{time.time():.3f}", n, line.decode(errors="replace").strip(), flush=True)
        print(f"{time.time():.3f}", n, "closed", flush=True)
socketserver.ThreadingTCPServer.allow_reuse_address=True
socketserver.ThreadingTCPServer((sys.argv[1],int(sys.argv[2])),H).serve_forever()' "$host_ip" "$host_port" > "$work/stream.log" 2>&1 &
pids+=($!)
kubectl apply -f - >/dev/null <<EOF
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata: {name: sg-$vm-rst, namespace: $ns}
spec:
  description: 'Security group $vm-rst (check-nic-soak.sh)'
  endpointSelector: {matchLabels: {sg.qrab.yapplesauce.com/$vm-rst: ""}}
  ingress:
  - fromEntities: [world, cluster]
    icmps: [{fields: [{type: 8, family: IPv4}]}]
  egress:
  - toEntities: [world]
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata: {name: $vm-rst-state, namespace: $ns}
spec: {accessModes: [ReadWriteOncePod], storageClassName: zfs-block, volumeMode: Filesystem, resources: {requests: {storage: 1Gi}}}
---
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachine
metadata: {name: $vm-rst, namespace: $ns}
spec:
  stateVolumeClaim: $vm-rst-state
  accelerator: kvm
  cpus: 1
  memoryMiB: 512
  storageNode: kube03
  components: {bmc: {enabled: false}}
  disks: [{id: root, sizeBytes: 1073741824, bootIndex: 0, baseImage: {name: cirros, generation: 1}}]
  nics: [{name: lan, securityGroups: [$vm-rst]}]
  cloudInit:
    hostname: $vm-rst
    userData: |
      #!/bin/sh
      i=0; while true; do i=\$((i + 1)); echo \$i; sleep 1; done | nc $host_ip $host_port &
EOF
wait_for 420 "$vm-rst Running" sh -c "[ \"\$(kubectl -n $ns get virtualmachine $vm-rst -o jsonpath='{.status.phase}')\" = Running ]"
wait_for 180 "the guest's TCP stream" grep -q '^[0-9.]* 1 [0-9]' "$work/stream.log"
address=$(field virtualmachine "$vm-rst" '{.status.nics[0].address}')
node=$(field virtualmachine "$vm-rst" '{.status.nodeName}')
ping -i 0.2 -W 1 -D -O "$address" > "$work/ping.log" 2>&1 &
pids+=($!)
sleep 5
gap_since() { # start-time: longest ping and stream gaps after it
  local p s
  p=$(awk -v s="$1" -F'[][]' '/bytes from/ {t=$2+0; if (t>=s) {if (q && t-q>g) g=t-q; q=t}} END {printf "%.2f", g+0}' "$work/ping.log")
  s=$(awk -v s="$1" '$2 == 1 && $3 ~ /^[0-9]+$/ {t=$1+0; if (t>=s) {if (q && t-q>g) g=t-q; q=t}} END {printf "%.2f", g+0}' "$work/stream.log")
  echo "$p $s"
}
for component in qrab-netd cilium; do
  case $component in
    qrab-netd) pod=$(kubectl -n qrab-node get pods -o wide | awk -v n="$node" '/qrab-netd/ && $7 == n {print $1}'); pns=qrab-node; label=qrab-netd ;;
    cilium) pod=$(kubectl -n kube-system get pods -l k8s-app=cilium -o wide | awk -v n="$node" '$7 == n {print $1}'); pns=kube-system; label=k8s-app=cilium ;;
  esac
  start=$(date +%s.%N)
  log "restarting $pns/$pod on $node"
  kubectl -n "$pns" delete pod "$pod" --wait=true >/dev/null
  if [ "$component" = qrab-netd ]; then
    wait_for 180 "a new qrab-netd on $node" sh -c "kubectl -n qrab-node get pods -o wide | awk -v n=$node '/qrab-netd/ && \$7 == n && \$2 == \"1/1\"' | grep -q ."
  else
    wait_for 300 "a new Cilium agent on $node" sh -c "kubectl -n kube-system get pods -l $label -o wide | awk -v n=$node '\$7 == n && \$2 == \"1/1\"' | grep -q ."
  fi
  sleep 20
  read -r ping_gap stream_gap < <(gap_since "$start")
  grep -q ' 1 closed$' "$work/stream.log" && fail "the guest's TCP stream was reset by the $component restart"
  summary+=("$component restart on $node: longest ping gap ${ping_gap}s, stream gap ${stream_gap}s, stream intact")
  awk -v a="$ping_gap" -v b="$stream_gap" -v m="$restart_gap" 'BEGIN {exit !(a <= m && b <= m + 1)}' \
    || fail "the $component restart interrupted the guest: ping gap ${ping_gap}s, stream gap ${stream_gap}s"
done

log "summary:"; printf '  %s\n' "${summary[@]}"
log "PASS: NIC soak"
