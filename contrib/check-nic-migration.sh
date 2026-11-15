#!/usr/bin/env bash
# Live acceptance of tap NIC live migration (docs/networking.md, "Live
# migration"). Run on the VM host as an admin kubectl user, with HOST_PORT
# free. One CirrOS VM (provisioned disk, swtpm, UEFI, KVM, one NIC in a
# security group that admits ICMP and SSH and egress to the world) moves
# kube03 -> kube01 -> kube02 -> kube03 by VirtualMachineMigration while:
#   - the VM host pings the guest every 0.2 s;
#   - the guest reports its boot id to a listener here every second (its
#     egress, which must arrive from its own /32: no masquerade);
#   - the guest streams a counter over one TCP connection to a listener here
#     (an established flow that must survive every move without a reset);
#   - a Pod probes the guest's SSH every 0.5 s.
# After every move it requires:
#   - the migration Succeeded;
#   - the NIC's anchor and the VM host's BGP route are on the VM's new node
#     (single hop), and the new node's CiliumNode holds the NIC's pool;
#   - the guest's boot id, TPM state and UEFI variables are unchanged;
#   - no ping, report or stream gap exceeds MAX_GAP_S, and the stream is still
#     its first connection.
# It prints per-move downtime (the manager's switch and QEMU's downtime) and
# loss. Deletes the VM at the end. Its root disk is a retained 1 GiB clone
# (contrib/storage/README.md).
#
#   MOVES=20 bash contrib/check-nic-migration.sh [vm-name]   (default 3 moves)
set -euo pipefail
vm=${1:-nicmig}
ns=qrab
host_ip=${HOST_IP:-10.203.0.1}
host_port=${HOST_PORT:-18185}
stream_port=$((host_port + 1))
max_gap=${MAX_GAP_S:-2}
# Moves to make, cycling kube01 -> kube02 -> kube03 (the VM starts on kube03).
moves=${MOVES:-3}
work=$(mktemp -d)
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
fail() {
  log "FAIL: $*"
  kubectl -n "$ns" get virtualmachinemigrations -o wide 2>/dev/null | grep "$vm" || true
  kubectl -n "$ns" get pods -o wide 2>/dev/null | grep "$vm" || true
  exit 1
}
pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  kubectl -n default delete pod "$vm-ssh" --ignore-not-found --wait=false >/dev/null 2>&1 || true
  kubectl -n "$ns" delete virtualmachinemigration -l "qrab-check=$vm" --ignore-not-found >/dev/null 2>&1 || true
  kubectl -n "$ns" delete virtualmachine "$vm" --ignore-not-found --wait=true --timeout=180s >/dev/null 2>&1 || true
  kubectl -n "$ns" delete pvc -l "qrab-check=$vm" --ignore-not-found >/dev/null 2>&1 || true
  kubectl -n "$ns" delete pvc "$vm-state" --ignore-not-found >/dev/null 2>&1 || true
  kubectl -n "$ns" delete ciliumnetworkpolicy "sg-$vm" --ignore-not-found >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT
field() { kubectl -n "$ns" get "$1" "$2" -o jsonpath="$3"; }
wait_for() { # seconds description command...
  local limit=$1 what=$2; shift 2
  for _ in $(seq 1 "$limit"); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done
  fail "$what within ${limit}s"
}

python3 -u -c '
import http.server,sys,time
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        print(f"{time.time():.3f}", self.client_address[0], self.path, flush=True)
        self.send_response(200); self.end_headers()
    def log_message(self,*a): pass
http.server.ThreadingHTTPServer((sys.argv[1],int(sys.argv[2])),H).serve_forever()' "$host_ip" "$host_port" > "$work/reports.log" 2>&1 &
pids+=($!)
# One line per received counter line: time, connection number, line.
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
socketserver.ThreadingTCPServer((sys.argv[1],int(sys.argv[2])),H).serve_forever()' "$host_ip" "$stream_port" > "$work/stream.log" 2>&1 &
pids+=($!)

log "creating VM $vm"
kubectl apply -f - >/dev/null <<EOF
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata: {name: sg-$vm, namespace: $ns}
spec:
  description: 'Security group $vm (check-nic-migration.sh)'
  endpointSelector: {matchLabels: {sg.qrab.yapplesauce.com/$vm: ""}}
  ingress:
  - fromEntities: [world, cluster]
    icmps: [{fields: [{type: 8, family: IPv4}]}]
  - fromEntities: [world, cluster]
    toPorts: [{ports: [{port: '22', protocol: TCP}]}]
  egress:
  - toEntities: [world]
  - toEndpoints: [{matchLabels: {k8s:io.kubernetes.pod.namespace: kube-system, k8s-app: kube-dns}}]
    toPorts: [{ports: [{port: '53', protocol: UDP}, {port: '53', protocol: TCP}]}]
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
  cpus: 2
  memoryMiB: 512
  storageNode: kube03
  components: {bmc: {enabled: false}}
  disks: [{id: root, sizeBytes: 1073741824, bootIndex: 0, baseImage: {name: cirros, generation: 1}}]
  nics: [{name: eth0, securityGroups: [$vm]}]
  cloudInit:
    hostname: $vm
    userData: |
      #!/bin/sh
      boot=\$(cat /proc/sys/kernel/random/boot_id)
      while true; do
        wget -q -T 1 -O /dev/null "http://$host_ip:$host_port/?from=$vm&boot=\$boot" &
        sleep 1
      done &
      i=0; while true; do i=\$((i + 1)); echo \$i; sleep 1; done | nc $host_ip $stream_port &
EOF
wait_for 420 "VM Running" sh -c "[ \"\$(kubectl -n $ns get virtualmachine $vm -o jsonpath='{.status.phase}')\" = Running ]"
address=$(field virtualmachine "$vm" '{.status.nics[0].address}')
wait_for 180 "the guest reporting" grep -q "from=$vm&boot=" "$work/reports.log"
wait_for 60 "the guest's TCP stream" grep -q '^[0-9.]* 1 [0-9]' "$work/stream.log"
boot_before=$(grep "from=$vm&boot=" "$work/reports.log" | tail -1 | sed 's/.*boot=//')
log "running on $(field virtualmachine "$vm" '{.status.nodeName}'), guest $address, boot $boot_before"

ping -i 0.2 -W 1 -D -O "$address" > "$work/ping.log" 2>&1 &
pids+=($!)
kubectl -n default run "$vm-ssh" --image=docker.io/library/python:3.13-alpine --restart=Never \
  --overrides='{"spec":{"terminationGracePeriodSeconds":1,"tolerations":[{"operator":"Exists"}]}}' \
  --command -- python3 -u -c "
import socket,time
last=None
while True:
    try:
        s=socket.create_connection(('$address',22),timeout=1); s.settimeout(1); state='ok '+s.recv(16).decode(errors='replace').strip(); s.close()
    except Exception as e:
        state='fail '+type(e).__name__
    if state!=last: print(f'{time.time():.3f}',state,flush=True); last=state
    time.sleep(0.5)" >/dev/null
wait_for 180 "guest SSH" sh -c "kubectl -n default logs $vm-ssh 2>/dev/null | tail -1 | grep -q ' ok SSH'"

state() { # sha256 of the TPM state and UEFI variables in the VM Pod
  local pod; pod=$(field virtualmachine "$vm" '{.status.podName}')
  kubectl -n "$ns" exec "$pod" -c manager -- sh -c 'cat /var/lib/qrab/identity/tpm/tpm2-00.permall /var/lib/qrab/identity/OVMF_VARS.fd | sha256sum' | cut -d' ' -f1
}
identity_before=$(state)
summary=()
step=0
targets=()
for i in $(seq 0 $((moves - 1))); do targets+=("kube0$((i % 3 + 1))"); done
for target in "${targets[@]}"; do
  step=$((step + 1))
  name="$vm-move-$step"
  source_node=$(field virtualmachine "$vm" '{.status.nodeName}')
  source_pod=$(field virtualmachine "$vm" '{.status.podName}')
  log "move $step: $source_node -> $target"
  start=$(date +%s.%N)
  kubectl apply -f - >/dev/null <<EOF
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachineMigration
metadata: {name: $name, namespace: $ns, labels: {qrab-check: $vm}}
spec: {vmName: $vm, targetNode: $target}
EOF
  for _ in $(seq 1 600); do
    phase=$(field qvmm "$name" '{.status.phase}')
    [ "$phase" = Succeeded ] && break
    [ "$phase" = Failed ] && fail "move $step failed: $(field qvmm "$name" '{.status.message}')"
    sleep 1
  done
  [ "$phase" = Succeeded ] || fail "move $step did not finish"
  done_at=$(date +%s.%N)
  switched=$(kubectl -n "$ns" logs "$source_pod" -c manager 2>/dev/null | grep -o '"event":"migration_switched"[^}]*' | grep -o '"ms":[0-9]*' | tail -1 | cut -d: -f2 || true)
  downtime=$(kubectl -n "$ns" logs "$source_pod" -c manager 2>/dev/null | grep -o '"downtime_ms":[0-9]*' | tail -1 | cut -d: -f2 || true)
  wait_for 120 "cleanup of move $step" sh -c "[ \"\$(kubectl -n $ns get qvmm $name -o jsonpath='{.status.cleaned}')\" = true ]"
  node=$(field virtualmachine "$vm" '{.status.nodeName}')
  [ "$node" = "$target" ] || fail "move $step: the VM runs on $node, not $target"
  anchor=$(field virtualmachine "$vm" '{.status.nics[0].podName}')
  [ "$(field pod "$anchor" '{.spec.nodeName}')" = "$target" ] || fail "move $step: anchor $anchor is not on $target"
  node_ip=$(kubectl get node "$target" -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}')
  wait_for 60 "the VM host's route to $address via $target" sh -c "ip route get $address | grep -q 'via $node_ip '"
  pool="qrab-nic-${address//./-}"
  wait_for 30 "$target's CiliumNode to request and hold $pool" sh -c \
    "[ \"\$(kubectl get ciliumnode $target -o jsonpath='{.spec.ipam.pools.requested[?(@.pool==\"$pool\")].pool} {.spec.ipam.pools.allocated[?(@.pool==\"$pool\")].cidrs[0]}')\" = '$pool $address/32' ]"
  # Gaps inside this move's window, from ping and from the guest's reports.
  ping_gap=$(awk -v s="$start" -v e="$done_at" -F'[][]' '/bytes from/ {t=$2+0; if (t>=s && t<=e) {if (p && t-p>g) g=t-p; p=t}} END {printf "%.2f", g+0}' "$work/ping.log")
  lost=$(awk -v s="$start" -v e="$done_at" -F'[][]' '/no answer yet/ {t=$2+0; if (t>=s && t<=e) n++} END {print n+0}' "$work/ping.log")
  report_gap=$(awk -v s="$start" -v e="$done_at" -v f="from=$vm&" '$3 ~ f {t=$1+0; if (t>=s && t<=e) {if (p && t-p>g) g=t-p; p=t}} END {printf "%.2f", g+0}' "$work/reports.log")
  stream_gap=$(awk -v s="$start" -v e="$done_at" '$2 == 1 && $3 ~ /^[0-9]+$/ {t=$1+0; if (t>=s && t<=e) {if (p && t-p>g) g=t-p; p=t}} END {printf "%.2f", g+0}' "$work/stream.log")
  wait_for 10 "the guest's stream on its first connection after move $step" sh -c "tail -1 '$work/stream.log' | grep -q '^[0-9.]* 1 [0-9]'"
  src=$(grep "from=$vm&" "$work/reports.log" | tail -1 | cut -d' ' -f2)
  [ "$src" = "$address" ] || fail "move $step: the guest's egress arrived from $src, not $address (masqueraded?)"
  awk -v a="$ping_gap" -v b="$report_gap" -v c="$stream_gap" -v m="$max_gap" 'BEGIN {exit !(a <= m && b <= m && c <= m)}' \
    || fail "move $step: ping gap ${ping_gap}s, report gap ${report_gap}s, stream gap ${stream_gap}s exceed ${max_gap}s"
  summary+=("move $step $source_node->$target: switch ${switched:-?} ms, QEMU downtime ${downtime:-?} ms, ping gap ${ping_gap}s ($lost unanswered), report gap ${report_gap}s, stream gap ${stream_gap}s, anchor $anchor")
  log "${summary[-1]}"
done

boot_after=$(grep "from=$vm&boot=" "$work/reports.log" | tail -1 | sed 's/.*boot=//')
[ "$boot_after" = "$boot_before" ] || fail "the guest rebooted: boot $boot_before -> $boot_after"
[ "$(state)" = "$identity_before" ] || fail "TPM state or UEFI variables differ after the moves"
resets=$(kubectl -n default logs "$vm-ssh" | grep -c ' fail ' || true)
log "SSH probe transitions:"; kubectl -n default logs "$vm-ssh" | sed 's/^/  /'
log "summary:"; printf '  %s\n' "${summary[@]}"
grep -q ' 1 closed$' "$work/stream.log" && fail "the guest's TCP stream was reset"
log "same boot, TPM and UEFI state and one TCP stream across ${step} moves; SSH probe failures: $resets"
log "PASS: tap NIC live migration"
