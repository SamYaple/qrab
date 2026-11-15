#!/usr/bin/env bash
# Live check of a WireGuard VPN NIC (docs/networking.md, "VPN NICs"). Run on
# the VM host as an admin kubectl user, with HOST_PORT free.
#
# Without VPN_SECRET it stands up a lab provider: a privileged Pod in
# qrab-node running a WireGuard server that NATs to the world, and a Secret
# vpn-<vm> naming it. With VPN_SECRET=<name> it uses that Secret (e.g. a
# ProtonVPN config) and skips the provider-side steps.
#
# One CirrOS VM with one VPN NIC reports to a listener here every second.
# The script requires:
#   - every report arrives from the provider's side (never the anchor's pool
#     address: no leak), carrying the tunnel address the guest got by DHCP;
#   - stopping the provider's tunnel stops the reports and none leaks, and
#     restoring it brings them back (kill switch);
#   - the private key is absent from the VM Pod and from qrab-netd's logs;
#   - after a live migration the tunnel resumes from the new node.
#
#   bash contrib/check-vpn-nic.sh [vm-name]
set -euo pipefail
vm=${1:-vpnnic}
ns=qrab
host_ip=${HOST_IP:-10.203.0.1}
host_port=${HOST_PORT:-18188}
secret=${VPN_SECRET:-vpn-$vm}
lab=$([ -z "${VPN_SECRET:-}" ] && echo yes || echo no)
provider=vpn-provider-$vm
image=$(kubectl -n "$ns" get deploy qrab-operator -o jsonpath='{.spec.template.spec.containers[0].image}')
work=$(mktemp -d)
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
fail() {
  log "FAIL: $*"
  kubectl -n "$ns" get pods -o wide 2>/dev/null | grep "$vm" || true
  exit 1
}
pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  kubectl -n "$ns" delete virtualmachinemigration -l "qrab-check=$vm" --ignore-not-found >/dev/null 2>&1 || true
  kubectl -n "$ns" delete virtualmachine "$vm" --ignore-not-found --wait=true --timeout=180s >/dev/null 2>&1 || true
  kubectl -n "$ns" delete pvc "$vm-state" --ignore-not-found >/dev/null 2>&1 || true
  kubectl -n "$ns" delete pvc -l "qrab-check=$vm" --ignore-not-found >/dev/null 2>&1 || true
  kubectl -n "$ns" delete ciliumnetworkpolicy "sg-$vm" --ignore-not-found >/dev/null 2>&1 || true
  if [ "$lab" = yes ]; then
    kubectl -n "$ns" delete secret "$secret" --ignore-not-found >/dev/null 2>&1 || true
    kubectl -n qrab-node delete pod "$provider" --ignore-not-found --wait=false >/dev/null 2>&1 || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT
field() { kubectl -n "$ns" get "$1" "$2" -o jsonpath="$3"; }
wait_for() { # seconds description command...
  local limit=$1 what=$2; shift 2
  for _ in $(seq 1 "$limit"); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done
  fail "$what within ${limit}s"
}
reports_since() { awk -v s="$1" -v f="from=$vm&" '$3 ~ f && $1+0 > s {n++} END {exit !(n>0)}' "$work/reports.log"; }

python3 -u -c '
import http.server,sys,time
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        print(f"{time.time():.3f}", self.client_address[0], self.path, flush=True)
        self.send_response(200); self.end_headers()
    def log_message(self,*a): pass
http.server.ThreadingHTTPServer((sys.argv[1],int(sys.argv[2])),H).serve_forever()' "$host_ip" "$host_port" > "$work/reports.log" 2>&1 &
pids+=($!)

pexec() { kubectl -n qrab-node exec "$provider" -- sh -c "$1"; }
if [ "$lab" = yes ]; then
  log "starting the lab WireGuard provider"
  kubectl apply -f - >/dev/null <<EOF
apiVersion: v1
kind: Pod
metadata: {name: $provider, namespace: qrab-node, labels: {app: vpn-provider, qrab-check: $vm}}
spec:
  nodeName: kube01
  terminationGracePeriodSeconds: 1
  serviceAccountName: qrab-kvm-device-plugin
  automountServiceAccountToken: false
  tolerations: [{operator: Exists}]
  containers:
  - name: provider
    image: $image
    imagePullPolicy: Always
    command: [sleep, infinity]
    securityContext: {privileged: true, runAsUser: 0}
EOF
  wait_for 120 "the provider Pod" kubectl -n qrab-node wait --for=condition=Ready "pod/$provider" --timeout=1s
  provider_ip=$(kubectl -n qrab-node get pod "$provider" -o jsonpath='{.status.podIP}')
  server_key=$(pexec 'wg genkey'); client_key=$(pexec 'wg genkey')
  server_pub=$(pexec "echo $server_key | wg pubkey"); client_pub=$(pexec "echo $client_key | wg pubkey")
  pexec "ip link add wg0 type wireguard && echo $server_key | wg set wg0 private-key /dev/stdin listen-port 51820 peer $client_pub allowed-ips 10.66.0.2/32 && ip addr add 10.66.0.1/24 dev wg0 && ip link set wg0 up && echo 1 > /proc/sys/net/ipv4/ip_forward && nft add table ip lab && nft add chain ip lab post '{ type nat hook postrouting priority srcnat; }' && nft add rule ip lab post ip saddr 10.66.0.0/24 oifname eth0 masquerade"
  kubectl -n "$ns" create secret generic "$secret" --from-file=wg.conf=/dev/stdin >/dev/null <<EOF
[Interface]
PrivateKey = $client_key
Address = 10.66.0.2/32
DNS = 1.1.1.1

[Peer]
PublicKey = $server_pub
AllowedIPs = 0.0.0.0/0
Endpoint = $provider_ip:51820
EOF
  log "provider $provider_ip, tunnel 10.66.0.1 <-> 10.66.0.2"
else
  client_key=$(kubectl -n "$ns" get secret "$secret" -o jsonpath='{.data.wg\.conf}' | base64 -d | sed -n 's/^PrivateKey *= *//p')
fi

log "creating VM $vm"
kubectl apply -f - >/dev/null <<EOF
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata: {name: sg-$vm, namespace: $ns}
spec:
  description: 'Security group $vm (check-vpn-nic.sh): the tunnel only'
  endpointSelector: {matchLabels: {sg.qrab.yapplesauce.com/$vm: ""}}
  egress:
  - toEntities: [world, cluster]
    toPorts: [{ports: [{port: '51820', protocol: UDP}]}]
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
  nics: [{name: lan, securityGroups: [$vm], vpn: {secretName: $secret}}]
  cloudInit:
    hostname: $vm
    userData: |
      #!/bin/sh
      boot=\$(cat /proc/sys/kernel/random/boot_id)
      while true; do
        a=\$(ip -4 -o addr show dev eth0 | awk '{print \$4}' | cut -d/ -f1)
        wget -q -T 2 -O /dev/null "http://$host_ip:$host_port/?from=$vm&ip=\$a&boot=\$boot" &
        sleep 1
      done &
EOF
wait_for 420 "VM Running" sh -c "[ \"\$(kubectl -n $ns get virtualmachine $vm -o jsonpath='{.status.phase}')\" = Running ]"
pool=$(field virtualmachine "$vm" '{.status.nics[0].address}')
wait_for 240 "the guest reporting through the tunnel" reports_since 0
grep "from=$vm&" "$work/reports.log" | awk '{print $2}' | sort -u > "$work/sources"
log "running; pool address $pool, reports from $(tr '\n' ' ' < "$work/sources")"
grep -qx "$pool" "$work/sources" && fail "a report arrived from the pool address $pool (leak)"
tunnel_ip=$(grep "from=$vm&" "$work/reports.log" | tail -1 | sed 's/.*&ip=\([^&]*\).*/\1/')
[ "$lab" = no ] || [ "$tunnel_ip" = 10.66.0.2 ] || fail "the guest has $tunnel_ip, not the tunnel's 10.66.0.2"
boot=$(grep "from=$vm&" "$work/reports.log" | tail -1 | sed 's/.*boot=\([^&]*\).*/\1/')
summary=("tunnel: the guest has $tunnel_ip; its egress arrives from $(tr '\n' ' ' < "$work/sources")(never the pool $pool)")

# The key stays in the anchor.
key_prefix=${client_key:0:20}
vm_pod=$(field virtualmachine "$vm" '{.status.podName}')
if kubectl -n "$ns" exec "$vm_pod" -c manager -- sh -c "grep -rqsF '$key_prefix' /etc/qrab /run /var/lib/qrab /tmp" 2>/dev/null; then
  fail "the private key is visible in the VM Pod"
fi
node=$(field virtualmachine "$vm" '{.status.nodeName}')
netd=$(kubectl -n qrab-node get pods -o wide | awk -v n="$node" '/qrab-netd/ && $7 == n {print $1}')
kubectl -n qrab-node logs "$netd" | grep -qF "$key_prefix" && fail "qrab-netd logged the private key"
summary+=("key: absent from the VM Pod and qrab-netd's logs")

if [ "$lab" = yes ]; then
  # Kill switch: no tunnel, no egress, and nothing from the pool address.
  pexec 'ip link set wg0 down'
  down=$(date +%s); sleep 15
  late=$(awk -v s="$((down + 3))" -v f="from=$vm&" '$3 ~ f && $1+0 > s' "$work/reports.log" | wc -l)
  [ "$late" -eq 0 ] || fail "$late reports arrived with the provider's tunnel down"
  grep "from=$vm&" "$work/reports.log" | awk '{print $2}' | grep -qx "$pool" && fail "a report leaked from $pool with the tunnel down"
  pexec 'ip link set wg0 up'
  up=$(date +%s)
  wait_for 120 "the reports to resume" reports_since "$up"
  summary+=("kill switch: no egress and no leak for 15 s with the provider down; resumed $(( $(awk -v s="$up" -v f="from=$vm&" '$3 ~ f && $1+0 > s {print $1; exit}' "$work/reports.log" | cut -d. -f1) - up ))s after it came back")
fi

# Live migration: the target anchor's tunnel takes over.
target=kube01; [ "$node" = kube01 ] && target=kube02
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
wait_for 120 "the tunnel on $target" reports_since "$moved"
gap=$(awk -v f="from=$vm&" '$3 ~ f {t=$1+0; if (p && t-p>g) g=t-p; p=t} END {printf "%.1f", g}' "$work/reports.log")
[ "$(grep "from=$vm&" "$work/reports.log" | tail -1 | sed 's/.*boot=\([^&]*\).*/\1/')" = "$boot" ] || fail "the guest rebooted"
grep "from=$vm&" "$work/reports.log" | awk '{print $2}' | grep -qx "$pool" && fail "a report leaked from $pool around the migration"
summary+=("migration $node -> $target: tunnel resumed, same boot, largest report gap ${gap}s overall")

log "summary:"; printf '  %s\n' "${summary[@]}"
log "PASS: VPN NIC"
