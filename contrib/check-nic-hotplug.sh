#!/usr/bin/env bash
# Live check of guest NIC hot-attach, detach and re-attach (docs/networking.md,
# "Hot-attach"). Run on the VM host as an admin kubectl user, with HOST_PORT
# free. One CirrOS VM (KVM, one NIC) runs throughout; the script:
#   1. adds a second NIC to spec.nics and waits for the guest to reach the VM
#      host through it (the guest runs DHCP on any new interface and pings the
#      host through each one, reporting every success here);
#   2. removes it again and waits for the manager to unplug it;
#   3. deletes the first NIC's anchor Pod and waits for the guest to reach the
#      host through that NIC again (the manager re-plugs it with the new
#      anchor's queues);
#   4. adds the second NIC back and live-migrates the VM, then requires both
#      NICs to answer on the new node.
# Throughout, the VM Pod (until the migration) and the guest's boot id must not
# change. Prints each step's latency. Deletes the VM at the end.
#
#   bash contrib/check-nic-hotplug.sh [vm-name]
set -euo pipefail
vm=${1:-nichp}
ns=qrab
host_ip=${HOST_IP:-10.203.0.1}
host_port=${HOST_PORT:-18187}
limit=${LIMIT_S:-240}
work=$(mktemp -d)
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
fail() {
  log "FAIL: $*"
  kubectl -n "$ns" get pods -o wide 2>/dev/null | grep "$vm" || true
  pod=$(kubectl -n "$ns" get virtualmachine "$vm" -o jsonpath='{.status.podName}' 2>/dev/null || true)
  [ -n "$pod" ] && kubectl -n "$ns" logs "$pod" -c manager 2>/dev/null | grep -E '"nic_' | tail -20 || true
  exit 1
}
pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
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
# A report from NIC $1 (guest interface) newer than unix time $2.
reported_since() {
  awk -v n="nic=$1&" -v s="$2" -v f="from=$vm&" '$3 ~ f && $3 ~ n && $1+0 > s {found=1} END {exit !found}' "$work/reports.log"
}
since() { echo $(( $(date +%s) - ${1%.*} )); }
boot_id() { grep "from=$vm&" "$work/reports.log" | tail -1 | sed 's/.*boot=\([^&]*\).*/\1/'; }

python3 -u -c '
import http.server,sys,time
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        print(f"{time.time():.3f}", self.client_address[0], self.path, flush=True)
        self.send_response(200); self.end_headers()
    def log_message(self,*a): pass
http.server.ThreadingHTTPServer((sys.argv[1],int(sys.argv[2])),H).serve_forever()' "$host_ip" "$host_port" > "$work/reports.log" 2>&1 &
pids+=($!)

log "creating VM $vm"
kubectl apply -f - >/dev/null <<EOF
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata: {name: sg-$vm, namespace: $ns}
spec:
  description: 'Security group $vm (check-nic-hotplug.sh)'
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
  nics: [{name: lan, securityGroups: [$vm]}]
  cloudInit:
    hostname: $vm
    userData: |
      #!/bin/sh
      boot=\$(cat /proc/sys/kernel/random/boot_id)
      while true; do
        for d in /sys/class/net/eth*; do
          n=\${d##*/}
          a=\$(ip -4 -o addr show dev \$n | awk '{print \$4}' | cut -d/ -f1)
          if [ -z "\$a" ]; then
            ip link set \$n up
            udhcpc -i \$n -n -q -t 2 -T 1 >/dev/null 2>&1 &
          elif ping -c 1 -W 1 -I \$n $host_ip >/dev/null 2>&1; then
            wget -q -T 1 -O /dev/null "http://$host_ip:$host_port/?from=$vm&nic=\$n&ip=\$a&boot=\$boot" &
          fi
        done
        sleep 1
      done &
EOF
wait_for 420 "VM Running" sh -c "[ \"\$(kubectl -n $ns get virtualmachine $vm -o jsonpath='{.status.phase}')\" = Running ]"
wait_for 180 "the guest reporting through eth0" reported_since eth0 0
pod_uid=$(kubectl -n "$ns" get pod "$(field virtualmachine "$vm" '{.status.podName}')" -o jsonpath='{.metadata.uid}')
boot=$(boot_id)
log "running on $(field virtualmachine "$vm" '{.status.nodeName}'), pod uid $pod_uid, boot $boot"
same_pod() {
  [ "$(kubectl -n "$ns" get pod "$(field virtualmachine "$vm" '{.status.podName}')" -o jsonpath='{.metadata.uid}')" = "$pod_uid" ] \
    || fail "$1: the VM Pod was replaced"
  [ "$(boot_id)" = "$boot" ] || fail "$1: the guest rebooted"
}
summary=()

# 1. Hot-attach.
start=$(date +%s.%N)
kubectl -n "$ns" patch virtualmachine "$vm" --type json -p "[{\"op\":\"add\",\"path\":\"/spec/nics/-\",\"value\":{\"name\":\"wan\",\"securityGroups\":[\"$vm\"]}}]" >/dev/null
wait_for "$limit" "the guest reaching the host through its hot-added NIC" reported_since eth1 "$start"
attach_s=$(since "$start")
same_pod "hot-attach"
wan=$(field virtualmachine "$vm" '{.status.nics[?(@.name=="wan")].address}')
summary+=("hot-attach: eth1 ($wan) reachable after ${attach_s}s, same Pod and boot")
log "${summary[-1]}"

# 2. Detach.
start=$(date +%s.%N)
kubectl -n "$ns" patch virtualmachine "$vm" --type json -p '[{"op":"remove","path":"/spec/nics/1"}]' >/dev/null
pod=$(field virtualmachine "$vm" '{.status.podName}')
wait_for "$limit" "the manager unplugging wan" sh -c "kubectl -n $ns logs $pod -c manager | grep '\"event\":\"nic_detached\"' | grep -q '\"nic\":\"wan\"'"
detach_s=$(since "$start")
wait_for 120 "wan's anchor and address to go" sh -c "! kubectl -n $ns get pods -l qrab.yapplesauce.com/nic=wan -o name | grep -q . && ! kubectl get ciliumpodippool qrab-nic-${wan//./-} >/dev/null 2>&1"
same_pod "detach"
summary+=("detach: wan unplugged after ${detach_s}s, anchor and pool released")
log "${summary[-1]}"

# 3. Anchor replacement.
anchor=$(field virtualmachine "$vm" '{.status.nics[0].podName}')
old_anchor_uid=$(field pod "$anchor" '{.metadata.uid}')
start=$(date +%s.%N)
kubectl -n "$ns" delete pod "$anchor" --wait=false >/dev/null
wait_for 120 "a replacement anchor" sh -c "u=\$(kubectl -n $ns get pod $anchor -o jsonpath='{.metadata.uid}' 2>/dev/null); [ -n \"\$u\" ] && [ \"\$u\" != $old_anchor_uid ]"
replaced=$(date +%s)
wait_for "$limit" "the guest reaching the host through eth0 again" reported_since eth0 "$replaced"
reattach_s=$(since "$start")
kubectl -n "$ns" logs "$pod" -c manager | grep '"event":"nic_anchor_replaced"' | grep -q '"nic":"lan"' || fail "the manager did not report lan's new anchor"
same_pod "anchor replacement"
summary+=("anchor replacement: eth0 reachable again after ${reattach_s}s, same Pod and boot")
log "${summary[-1]}"

# 4. Live migration with a hot-attached NIC.
kubectl -n "$ns" patch virtualmachine "$vm" --type json -p "[{\"op\":\"add\",\"path\":\"/spec/nics/-\",\"value\":{\"name\":\"wan\",\"securityGroups\":[\"$vm\"]}}]" >/dev/null
start=$(date +%s.%N)
wait_for "$limit" "the re-added NIC" reported_since eth1 "$start"
source_node=$(field virtualmachine "$vm" '{.status.nodeName}')
target=kube01; [ "$source_node" = kube01 ] && target=kube02
log "migrating $source_node -> $target with two NICs"
kubectl apply -f - >/dev/null <<EOF
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachineMigration
metadata: {name: $vm-move, namespace: $ns, labels: {qrab-check: $vm}}
spec: {vmName: $vm, targetNode: $target}
EOF
for _ in $(seq 1 600); do
  phase=$(field qvmm "$vm-move" '{.status.phase}')
  [ "$phase" = Succeeded ] && break
  [ "$phase" = Failed ] && fail "migration failed: $(field qvmm "$vm-move" '{.status.message}')"
  sleep 1
done
[ "$phase" = Succeeded ] || fail "migration did not finish"
moved=$(date +%s)
wait_for 60 "eth0 after the move" reported_since eth0 "$moved"
wait_for 60 "eth1 after the move" reported_since eth1 "$moved"
[ "$(boot_id)" = "$boot" ] || fail "the guest rebooted during the migration"
for nic in lan wan; do
  a=$(kubectl -n "$ns" get pods -l "qrab.yapplesauce.com/nic=$nic,qrab.yapplesauce.com/owner-uid=$(field virtualmachine "$vm" '{.metadata.uid}')" -o jsonpath='{.items[*].spec.nodeName}')
  case " $a " in *" $target "*) ;; *) fail "NIC $nic has no anchor on $target ($a)" ;; esac
done
summary+=("migration with a hot-added NIC: both NICs answer on $target, same boot")
log "${summary[-1]}"

log "summary:"; printf '  %s\n' "${summary[@]}"
log "PASS: NIC hot-attach, detach, re-attach and migration"
