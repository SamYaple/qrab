#!/usr/bin/env bash
# The parts of a tap NIC's live-migration handoff, measured live
# (docs/networking.md, "Live migration"). Run on the VM host as an admin
# kubectl user. One CirrOS VM moves MOVES times (default 3) while:
#   - a probe Pod on every node pings the guest every 0.1 s;
#   - each node's Cilium agent reports the guest address's ipcache entry
#     every ~0.1 s.
# For every move it prints, from the agents' and managers' timestamped logs:
# the forward route, target activate and source standby completion times
# relative to the manager's switch end; when each node's ipcache named the new
# node; each node's longest ping gap; and Hubble's drops for the guest's
# address. It fails only on a failed move, a probe gap over MAX_GAP_S, or a
# drop other than the expected standby drops.
#
#   bash contrib/check-nic-handoff-detail.sh [vm-name]
set -euo pipefail
vm=${1:-nichandoff}
ns=qrab
moves=${MOVES:-3}
max_gap=${MAX_GAP_S:-2}
work=$(mktemp -d)
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
fail() { log "FAIL: $*"; exit 1; }
pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  for n in kube01 kube02 kube03; do kubectl -n default delete pod "$vm-probe-$n" --ignore-not-found --wait=false >/dev/null 2>&1 || true; done
  kubectl -n "$ns" delete virtualmachinemigration -l "qrab-check=$vm" --ignore-not-found >/dev/null 2>&1 || true
  kubectl -n "$ns" delete virtualmachine "$vm" --ignore-not-found --wait=true --timeout=180s >/dev/null 2>&1 || true
  kubectl -n "$ns" delete pvc "$vm-state" --ignore-not-found >/dev/null 2>&1 || true
  kubectl -n "$ns" delete pvc -l "qrab-check=$vm" --ignore-not-found >/dev/null 2>&1 || true
  kubectl -n "$ns" delete ciliumnetworkpolicy "sg-$vm" --ignore-not-found >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT
field() { kubectl -n "$ns" get "$1" "$2" -o jsonpath="$3"; }
wait_for() {
  local limit=$1 what=$2; shift 2
  for _ in $(seq 1 "$limit"); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done
  fail "$what within ${limit}s"
}
node_ip() { kubectl get node "$1" -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}'; }
agent() { kubectl -n kube-system get pods -l k8s-app=cilium --field-selector "spec.nodeName=$1" -o name | head -1; }
netd_pod() { kubectl -n qrab-node get pods -o wide | awk -v n="$1" '/qrab-netd/ && $7 == n {print $1}'; }
ts() { date -u -d "$1" +%s.%N; }

log "creating VM $vm"
kubectl apply -f - >/dev/null <<EOF
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata: {name: sg-$vm, namespace: $ns}
spec:
  description: 'Security group $vm (check-nic-handoff-detail.sh)'
  endpointSelector: {matchLabels: {sg.qrab.yapplesauce.com/$vm: ""}}
  ingress:
  - fromEntities: [world, cluster]
    icmps: [{fields: [{type: 8, family: IPv4}]}]
  egress:
  - toEntities: [world]
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata: {name: $vm-state, namespace: $ns}
spec: {accessModes: [ReadWriteOncePod], storageClassName: zfs-block, volumeMode: Filesystem, resources: {requests: {storage: 1Gi}}}
---
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachine
metadata: {name: $vm, namespace: $ns}
spec:
  stateVolumeClaim: $vm-state
  accelerator: kvm
  cpus: 1
  memoryMiB: 512
  storageNode: kube03
  components: {bmc: {enabled: false}}
  disks: [{id: root, sizeBytes: 1073741824, bootIndex: 0, baseImage: {name: cirros, generation: 1}}]
  nics: [{name: eth0, securityGroups: [$vm]}]
EOF
wait_for 420 "VM Running" sh -c "[ \"\$(kubectl -n $ns get virtualmachine $vm -o jsonpath='{.status.phase}')\" = Running ]"
address=$(field virtualmachine "$vm" '{.status.nics[0].address}')
wait_for 120 "the guest answering ping" ping -c 1 -W 1 "$address"
log "guest $address on $(field virtualmachine "$vm" '{.status.nodeName}')"

for n in kube01 kube02 kube03; do
  kubectl -n default run "$vm-probe-$n" --image=docker.io/library/python:3.13-alpine --restart=Never \
    --overrides="{\"spec\":{\"nodeName\":\"$n\",\"terminationGracePeriodSeconds\":1,\"tolerations\":[{\"operator\":\"Exists\"}]}}" \
    --command -- python3 -u -c "
import subprocess,time
while True:
    t=time.time(); ok=subprocess.run(['ping','-c','1','-W','1','$address'],stdout=subprocess.DEVNULL).returncode==0
    if ok: print(f'{t:.3f}',flush=True)
    time.sleep(max(0,0.1-(time.time()-t)))" >/dev/null
  # ipcache entry every ~0.1 s, timestamped inside the agent.
  kubectl -n kube-system exec "$(agent "$n")" -c cilium-agent -- sh -c \
    "while true; do echo \"\$(date +%s.%N) \$(cilium-dbg bpf ipcache get $address 2>/dev/null | tail -1)\"; sleep 0.1; done" \
    > "$work/ipcache-$n.log" 2>/dev/null &
  pids+=($!)
done
for n in kube01 kube02 kube03; do
  wait_for 120 "probe on $n" sh -c "kubectl -n default logs $vm-probe-$n 2>/dev/null | grep -q ."
done
# From the VM host: ping (unanswered sequence numbers) and its route to the guest.
ping -i 0.1 -W 1 -D -O "$address" > "$work/host-ping.log" 2>&1 &
pids+=($!)
( while true; do echo "$(date +%s.%N) $(ip route get "$address" | head -1)"; sleep 0.1; done ) > "$work/host-route.log" 2>&1 &
pids+=($!)

summary=()
for i in $(seq 1 "$moves"); do
  source_node=$(field virtualmachine "$vm" '{.status.nodeName}')
  source_pod=$(field virtualmachine "$vm" '{.status.podName}')
  target=kube0$(( (${source_node#kube0} % 3) + 1 ))
  start=$(date +%s.%N); since=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  log "move $i: $source_node -> $target"
  kubectl apply -f - >/dev/null <<EOF
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachineMigration
metadata: {name: $vm-move-$i, namespace: $ns, labels: {qrab-check: $vm}}
spec: {vmName: $vm, targetNode: $target}
EOF
  for _ in $(seq 1 600); do
    phase=$(field qvmm "$vm-move-$i" '{.status.phase}')
    [ "$phase" = Succeeded ] && break
    [ "$phase" = Failed ] && fail "move $i failed: $(field qvmm "$vm-move-$i" '{.status.message}')"
    sleep 1
  done
  [ "$phase" = Succeeded ] || fail "move $i did not finish"
  sleep 25
  done_at=$(date +%s.%N)
  # The manager's switch end, and each netd step's completion.
  switch_line=$(kubectl -n "$ns" logs "$source_pod" -c manager --timestamps 2>/dev/null | grep '"event":"migration_switched"' | tail -1)
  switched=$(ts "$(echo "$switch_line" | cut -d' ' -f1)")
  switch_ms=$(echo "$switch_line" | grep -o '"ms":[0-9]*' | cut -d: -f2)
  step() { # node pattern
    local line; line=$(kubectl -n qrab-node logs "$(netd_pod "$1")" --timestamps --since-time="$since" 2>/dev/null | grep -F "$2" | head -1)
    [ -n "$line" ] && awk -v a="$(ts "$(echo "$line" | cut -d' ' -f1)")" -v b="$switched" 'BEGIN {printf "%+.0f", (a-b)*1000}' || echo "?"
  }
  fwd=$(step "$source_node" "$address/32 forwarded via")
  act=$(step "$target" "nic-eth0-$target is active")
  stb=$(step "$source_node" "nic-eth0-$source_node is standby")
  # ipcache convergence: first entry naming the target after the switch.
  target_ip=$(node_ip "$target")
  converge=""
  for n in kube01 kube02 kube03; do
    c=$(awk -v s="$start" -v b="$switched" -v t="tunnelendpoint=$target_ip" '$1+0 > s && index($0, t) {printf "%+.0f", ($1-b)*1000; exit}' "$work/ipcache-$n.log")
    [ "$n" = "$target" ] && c=$(awk -v s="$start" -v b="$switched" '$1+0 > s && /tunnelendpoint=0.0.0.0|flags=<none>/ {printf "%+.0f", ($1-b)*1000; exit}' "$work/ipcache-$n.log")
    converge="$converge $n:${c:-?}ms"
  done
  # Per-node probe gaps and Hubble drops inside the move.
  gaps=""
  for n in kube01 kube02 kube03; do
    g=$(kubectl -n default logs "$vm-probe-$n" 2>/dev/null | awk -v s="$start" -v e="$done_at" '$1+0>=s && $1+0<=e {t=$1+0; if (p && t-p>g) g=t-p; p=t} END {printf "%.2f", g+0}')
    awk -v a="$g" -v m="$max_gap" 'BEGIN {exit !(a <= m)}' || fail "move $i: the probe on $n saw a ${g}s gap"
    gaps="$gaps $n:${g}s"
  done
  drops=""
  for n in kube01 kube02 kube03; do
    d=$(kubectl -n kube-system exec "$(agent "$n")" -c cilium-agent -- hubble observe --since "$since" --ip "$address" --verdict DROPPED -o compact 2>/dev/null | grep -v -c -i 'standby' || true)
    sd=$(kubectl -n kube-system exec "$(agent "$n")" -c cilium-agent -- hubble observe --since "$since" --ip "$address" --verdict DROPPED -o compact 2>/dev/null | grep -c -i 'standby' || true)
    [ "${d:-0}" -eq 0 ] || fail "move $i: $n dropped $d packets of $address for reasons other than the standby fence"
    drops="$drops $n:${sd:-0}"
  done
  # The VM host: unanswered pings (seconds after the switch end) and its route changes.
  lost=$(awk -v s="$start" -v e="$done_at" -v b="$switched" -F'[][]' '/no answer yet/ {t=$2+0; if (t>=s && t<=e) printf "%+.1f ", t-b}' "$work/host-ping.log")
  routes=$(awk -v s="$start" -v e="$done_at" -v b="$switched" '$1+0>=s && $1+0<=e {r=$0; sub(/^[^ ]+ /,"",r); if (r != last) {printf "%+.1fs:%s; ", $1-b, r; last=r}}' "$work/host-route.log" | sed 's/ dev [a-z0-9]*//g; s/ src [0-9.]*//g; s/ uid [0-9]*//g')
  summary+=("move $i host: unanswered pings at [${lost}] s; route changes: ${routes}")
  log "${summary[-1]}"
  summary+=("move $i $source_node->$target: switch ${switch_ms}ms; steps vs switch end: forward ${fwd}ms, target active ${act}ms, source standby ${stb}ms; ipcache names $target:$converge; probe gaps:$gaps; standby drops:$drops")
  log "${summary[-1]}"
done
log "summary:"; printf '  %s\n' "${summary[@]}"
log "PASS: handoff detail"
