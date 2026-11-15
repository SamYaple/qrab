#!/usr/bin/env bash
# Live check that a guest's egress keeps its own NIC address (docs/networking.md,
# "Egress keeps the guest's address"). Run on the VM host, as an admin kubectl
# user, with nothing else listening on HOST_PORT. It starts a throwaway CirrOS VM
# with one NIC in security group egresscheck (egress to the world and DNS only),
# then requires:
#   1. the NIC endpoint carries Cilium's skip-masquerade flag (0x4) in its lxc entry;
#   2. a listener on the VM host (10.203.0.1) sees the guest's /32, not a node IP;
#   3. the guest reaches the internet (1.1.1.1's trace reports some address);
#   4. an ordinary pod still arrives at the same listener as its node's IP.
# It also requires the NIC's anchor to be plumbed by qrab-netd, to run on the VM
# Pod's node and to have no relay policy.
# Everything it creates is deleted on exit, except the VM's root disk: the
# storage service never destroys a disk, so every run keeps one 1 GiB CoW clone
# that counts against the block-storage budget at its logical size
# (contrib/storage/README.md). Run it sparingly; a run refused with
# StorageRefused budget-exhausted needs an administrator, not a retry.
#
#   bash contrib/kubernetes/egress-check.sh [vm-name]
set -euo pipefail
vm=${1:-egresscheck}
ns=qrab
host_ip=${HOST_IP:-10.203.0.1}
host_port=${HOST_PORT:-18180}
work=$(mktemp -d)
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
fail() {
  log "FAIL: $*"
  kubectl -n "$ns" get events --sort-by=.lastTimestamp 2>/dev/null | grep -E "$vm" | tail -15 || true
  kubectl -n "$ns" logs "$vm-nic-eth0" --all-containers --tail=30 2>/dev/null || true
  kubectl -n "$ns" get virtualmachine "$vm" -o jsonpath='{.status.phase} {.status.reason}: {.status.message}{"\n"}' 2>/dev/null || true
  kubectl -n "$ns" logs "$vm" --all-containers --tail=30 2>/dev/null || true
  exit 1
}
listener_pid=
cleanup() {
  [ -n "$listener_pid" ] && kill "$listener_pid" 2>/dev/null || true
  kubectl -n default delete pod "$vm-control" --ignore-not-found --wait=false >/dev/null 2>&1 || true
  kubectl -n "$ns" delete virtualmachine "$vm" --ignore-not-found --wait=true --timeout=180s >/dev/null 2>&1 || true
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
import http.server,sys
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        print(self.client_address[0], self.path, flush=True)
        self.send_response(200); self.end_headers(); self.wfile.write(b"ok\n")
    def log_message(self,*a): pass
http.server.ThreadingHTTPServer((sys.argv[1],int(sys.argv[2])),H).serve_forever()' "$host_ip" "$host_port" > "$work/listener.log" 2>&1 &
listener_pid=$!
log "host listener on $host_ip:$host_port"

log "creating VM $vm"
kubectl apply -f - >/dev/null <<EOF
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata: {name: sg-$vm, namespace: $ns}
spec:
  description: 'Security group $vm: egress to the world and DNS only (egress-check.sh)'
  endpointSelector: {matchLabels: {sg.qrab.yapplesauce.com/$vm: ""}}
  ingress: [{}]
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
        wan=\$(wget -q -T 3 -O - http://1.1.1.1/cdn-cgi/trace | sed -n 's/^ip=//p')
        wget -q -T 2 -O /dev/null "http://$host_ip:$host_port/?from=$vm&wan=\$wan"
        sleep 2
      done &
EOF
wait_for 300 "VM Running" sh -c "[ \"\$(kubectl -n $ns get virtualmachine $vm -o jsonpath='{.status.phase}')\" = Running ]"
address=$(field virtualmachine "$vm" '{.status.nics[0].address}')
nic_pod=$(field virtualmachine "$vm" '{.status.nics[0].podName}')
[ -n "$address" ] || fail "the NIC has no stable address (QRAB_NIC_ADDRESS_RANGE unset?)"
node=$(field pod "$nic_pod" '{.spec.nodeName}')
agent=$(kubectl -n kube-system get pods -l k8s-app=cilium --field-selector "spec.nodeName=$node" -o jsonpath='{.items[0].metadata.name}')
log "running: NIC $address in $nic_pod on $node (agent $agent)"
plumbed=$(field pod "$nic_pod" '{.status.conditions[?(@.type=="qrab.yapplesauce.com/nic-plumbed")].status}')
[ "$plumbed" = True ] || fail "anchor $nic_pod is not plumbed by qrab-netd ($plumbed)"
vm_node=$(field pod "$vm" '{.spec.nodeName}')
[ "$vm_node" = "$node" ] || fail "anchor on $node, VM Pod on $vm_node"
! kubectl -n "$ns" get ciliumnetworkpolicy "$nic_pod-relay" >/dev/null 2>&1 || fail "NIC $nic_pod has relay policy $nic_pod-relay"
log "ok: anchor plumbed on the VM Pod's node, no relay policy: $(field pod "$nic_pod" '{.status.conditions[?(@.type=="qrab.yapplesauce.com/nic-plumbed")].message}')"

pool_flag=$(kubectl get ciliumpodippool "qrab-nic-${address//./-}" -o jsonpath='{.metadata.annotations.ipam\.cilium\.io/skip-masquerade}')
[ "$pool_flag" = true ] || fail "pool qrab-nic-${address//./-} lacks ipam.cilium.io/skip-masquerade=true"
flags=$(kubectl -n kube-system exec "$agent" -c cilium-agent -- cilium-dbg bpf endpoint list | awk -v ip="$address:0" '$1==ip {for (i=1;i<=NF;i++) if ($i ~ /^flags=/) {sub("flags=","",$i); print $i}}')
[ -n "$flags" ] || fail "no lxc entry for $address on $node"
(( flags & 0x4 )) || fail "lxc entry for $address has flags=$flags, without skip-masquerade (0x4)"
log "ok: pool annotated and lxc flags=$flags carry skip-masquerade"

wait_for 300 "the guest reaching the host listener" grep -q "from=$vm" "$work/listener.log"
seen=$(grep "from=$vm" "$work/listener.log" | tail -1)
src=${seen%% *}
[ "$src" = "$address" ] || fail "the host saw the guest as $src, expected its NIC address $address"
log "ok: the VM host saw the guest as $src"
wait_for 120 "an internet reply" sh -c "grep \"from=$vm&wan=.\" '$work/listener.log'"
wan=$(grep -o "from=$vm&wan=[^ ]*" "$work/listener.log" | tail -1 | sed 's/.*wan=//')
log "ok: the guest reached 1.1.1.1, which saw $wan (the LAN router's NAT)"

kubectl -n default run "$vm-control" --image=docker.io/library/busybox:1.37 --restart=Never \
  --overrides='{"spec":{"terminationGracePeriodSeconds":1,"tolerations":[{"key":"node-role.kubernetes.io/control-plane","operator":"Exists","effect":"NoSchedule"}]}}' \
  -- wget -q -T 3 -O /dev/null "http://$host_ip:$host_port/?from=$vm-control" >/dev/null
wait_for 120 "the control pod reaching the host listener" grep -q "from=$vm-control" "$work/listener.log"
control_src=$(grep "from=$vm-control" "$work/listener.log" | tail -1 | cut -d' ' -f1)
control_node=$(kubectl -n default get pod "$vm-control" -o jsonpath='{.status.hostIP}')
[ "$control_src" = "$control_node" ] || fail "an ordinary pod arrived as $control_src, expected its node IP $control_node"
log "ok: an ordinary pod still arrives as its node IP $control_src"
log "PASS: guest egress keeps $address"
