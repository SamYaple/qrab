#!/usr/bin/env bash
# Conntrack handoff oracle (docs/networking.md, "Live migration"). Run on the
# VM host as an admin kubectl user.
#
# The guest's security group admits TCP 5000 in and nothing out. The guest
# serves a counter line every 0.5 s on 5000; a client here connects and only
# reads, so after the connection is set up every packet the client sends is a
# bare ACK of data it received. On the new node that flow exists only if its
# conntrack entry came along: without it the guest's next data segment is a
# new egress flow the group denies, the client has nothing to acknowledge, and
# the stream stalls. The check requires the stream to keep flowing (no gap
# over MAX_GAP_S) across MOVES live migrations (default 2), and the
# managers to report imported entries for each move (REQUIRE_COPY=0 skips
# that, to show the oracle failing on a release without the copy).
#
#   bash contrib/check-nic-ct-handoff.sh [vm-name]
set -euo pipefail
vm=${1:-nicct}
ns=qrab
moves=${MOVES:-2}
max_gap=${MAX_GAP_S:-3}
work=$(mktemp -d)
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
keep() { # with DEBUG_DIR: the client's log and the managers' logs
  [ -n "${DEBUG_DIR:-}" ] || return 0
  mkdir -p "$DEBUG_DIR"; cp "$work/stream.log" "$DEBUG_DIR/" 2>/dev/null || true
  for p in $(kubectl -n "$ns" get pods -o name | grep -E "/$vm(-m[0-9a-f]+)?\$"); do
    kubectl -n "$ns" logs "$p" -c manager --timestamps > "$DEBUG_DIR/${p#pod/}.log" 2>/dev/null || true
  done
}
fail() { log "FAIL: $*"; tail -5 "$work/stream.log" 2>/dev/null | sed 's/^/  /'; keep; exit 1; }
pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
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

log "creating VM $vm (security group: TCP 5000 in, nothing out)"
kubectl apply -f - >/dev/null <<EOF
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata: {name: sg-$vm, namespace: $ns}
spec:
  description: 'Security group $vm (check-nic-ct-handoff.sh): TCP 5000 in, no new egress'
  endpointSelector: {matchLabels: {sg.qrab.yapplesauce.com/$vm: ""}}
  ingress:
  - fromEntities: [world, cluster]
    toPorts: [{ports: [{port: '5000', protocol: TCP}]}]
  egress:
  - toEndpoints: [{matchLabels: {qrab.yapplesauce.com/nothing: matches}}]
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
  cloudInit:
    hostname: $vm
    userData: |
      #!/bin/sh
      while true; do
        i=0; while true; do i=\$((i + 1)); echo \$i; sleep 0.5; done | nc -l -p 5000
      done &
EOF
wait_for 420 "VM Running" sh -c "[ \"\$(kubectl -n $ns get virtualmachine $vm -o jsonpath='{.status.phase}')\" = Running ]"
address=$(field virtualmachine "$vm" '{.status.nics[0].address}')
# The client: connects once, never writes, logs each line it reads.
python3 -u -c '
import socket,sys,time
for _ in range(240):
    try:
        s=socket.create_connection((sys.argv[1],5000),timeout=2); break
    except OSError: time.sleep(1)
else: sys.exit("no connection")
s.settimeout(None)
f=s.makefile("rb")
for line in f:
    text=line.decode("utf-8","replace").strip()
    print(f"{time.time():.3f} {text}",flush=True)
print(f"{time.time():.3f} closed",flush=True)' "$address" > "$work/stream.log" 2>&1 &
pids+=($!)
wait_for 240 "the guest's stream" sh -c "[ \$(wc -l < $work/stream.log) -ge 5 ]"
log "guest $address on $(field virtualmachine "$vm" '{.status.nodeName}'); the client reads its stream"
summary=()
for i in $(seq 1 "$moves"); do
  source_node=$(field virtualmachine "$vm" '{.status.nodeName}')
  source_pod=$(field virtualmachine "$vm" '{.status.podName}')
  target=kube0$(( (${source_node#kube0} % 3) + 1 ))
  start=$(date +%s.%N)
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
  # The copy events, phase=entries, e.g. "precopy=1 delta=1": the target
  # manager pulls the precopy, the source asks for the delta (read before
  # the source Pod goes).
  target_pod=$(field virtualmachine "$vm" '{.status.podName}')
  events=$( (kubectl -n "$ns" logs "$target_pod" -c manager; kubectl -n "$ns" logs "$source_pod" -c manager) 2>/dev/null | grep '"event":"migration_conntrack"' || true)
  copies=$(echo "$events" | sed -n 's/.*"imported":\([0-9]*\).*"phase":"\([a-z]*\)".*/\2=\1/p' | tr '\n' ' ')
  copied=$(echo "$events" | grep -o '"imported":[0-9]*' | cut -d: -f2 | awk '{s+=$1} END {print s+0}')
  if [ -n "${CT_DEBUG:-}" ]; then
    a=$(kubectl -n kube-system get pods -l k8s-app=cilium --field-selector "spec.nodeName=$target" -o name | head -1)
    for t in 0 10 20; do
      log "conntrack on $target at +${t}s (node uptime $(kubectl -n kube-system exec "$a" -c cilium-agent -- cut -d' ' -f1 /proc/uptime)):"
      kubectl -n kube-system exec "$a" -c cilium-agent -- sh -c "cilium-dbg bpf ct list global 2>/dev/null | grep -F '$address'" | sed 's/^/    /'
      tail -1 "$work/stream.log" | sed 's/^/    stream: /'
      [ "$t" -lt 20 ] && sleep 10
    done
  fi
  sleep $((max_gap + 5))
  gap=$(awk -v s="$start" '$2 ~ /^[0-9]+$/ {t=$1+0; if (t>=s) {if (p && t-p>g) g=t-p; p=t}} END {printf "%.2f", g+0}' "$work/stream.log")
  last=$(tail -1 "$work/stream.log" | awk '{print $1}')
  now=$(date +%s)
  awk -v l="$last" -v n="$now" -v m="$max_gap" 'BEGIN {exit !(n - l <= m)}' \
    || fail "move $i: the guest's server-push stream stalled (last line $(awk -v l="$last" -v n="$now" 'BEGIN {printf "%.0f", n-l}')s ago)"
  awk -v g="$gap" -v m="$max_gap" 'BEGIN {exit !(g <= m)}' || fail "move $i: stream gap ${gap}s"
  # The copy itself, not a re-created entry: the source manager saw the
  # target import entries before the move streamed and after it.
  [ "${REQUIRE_COPY:-1}" = 0 ] || [ "$copied" -ge 1 ] || fail "move $i: no conntrack entry was imported (copies: ${copies:-none})"
  summary+=("move $i $source_node->$target: stream kept flowing, gap ${gap}s; copies: ${copies:-none}")
  log "${summary[-1]}"
done
keep
log "summary:"; printf '  %s\n' "${summary[@]}"
log "PASS: established flows keep their conntrack across moves"
