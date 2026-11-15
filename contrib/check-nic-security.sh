#!/usr/bin/env bash
# Live security check of guest NICs (docs/networking.md, "Security"). Run on
# the VM host as an admin kubectl user, with HOST_PORT and HOST_PORT+1 free.
#
# Two CirrOS VMs: "sec" in a security group that admits ICMP/SSH in and only
# the world out, and "peer" with no security group. "sec"'s guest fetches a
# probe list from the listener here, tries each target, and reports the
# result. The script requires:
#   - the world is reachable (control);
#   - its own VM Pod's BMC, Redfish and migration ports, the node's kubelet
#     and SSH, cluster DNS, the peer's NIC and the anchor's own addresses are
#     not;
#   - packets with a spoofed source address never arrive (UDP to a listener
#     here, which logs every datagram's source);
#   - from inside its VM Pod, qrab-netd refuses the peer VM's NIC.
#
#   bash contrib/check-nic-security.sh [prefix]
set -euo pipefail
prefix=${1:-nicsec}
ns=qrab
host_ip=${HOST_IP:-10.203.0.1}
http_port=${HOST_PORT:-18189}
udp_port=$((http_port + 1))
work=$(mktemp -d)
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
fail() { log "FAIL: $*"; exit 1; }
pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  for vm in "$prefix" "$prefix-peer"; do
    kubectl -n "$ns" delete virtualmachine "$vm" --ignore-not-found --wait=false >/dev/null 2>&1 || true
  done
  for vm in "$prefix" "$prefix-peer"; do
    kubectl -n "$ns" wait --for=delete "virtualmachine/$vm" --timeout=180s >/dev/null 2>&1 || true
    kubectl -n "$ns" delete pvc "$vm-state" --ignore-not-found >/dev/null 2>&1 || true
  done
  kubectl -n "$ns" delete ciliumnetworkpolicy "sg-$prefix" --ignore-not-found >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT
field() { kubectl -n "$ns" get "$1" "$2" -o jsonpath="$3"; }
wait_for() {
  local limit=$1 what=$2; shift 2
  for _ in $(seq 1 "$limit"); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done
  fail "$what within ${limit}s"
}
touch "$work/probes"
# HTTP: serves /probes and logs every request; UDP: logs every datagram's source.
python3 -u -c '
import http.server,socket,sys,threading,time
host,http_port,udp_port,probes=sys.argv[1],int(sys.argv[2]),int(sys.argv[3]),sys.argv[4]
def udp():
    s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM); s.bind((host,udp_port))
    while True:
        data,(src,_)=s.recvfrom(2048)
        text=data.decode("utf-8","replace").strip()
        print(f"{time.time():.3f} udp {src} {text}",flush=True)
threading.Thread(target=udp,daemon=True).start()
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        print(f"{time.time():.3f} http {self.client_address[0]} {self.path}",flush=True)
        body=open(probes,"rb").read() if self.path.startswith("/probes") else b""
        self.send_response(200); self.end_headers(); self.wfile.write(body)
    def log_message(self,*a): pass
http.server.ThreadingHTTPServer((host,http_port),H).serve_forever()' "$host_ip" "$http_port" "$udp_port" "$work/probes" > "$work/log" 2>&1 &
pids+=($!)

vm_yaml() { # name sg-or-empty user-data
  local groups=""; [ -n "$2" ] && groups="securityGroups: [$2]"
  cat <<EOF
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata: {name: $1-state, namespace: $ns}
spec: {accessModes: [ReadWriteOncePod], storageClassName: zfs-block, volumeMode: Filesystem, resources: {requests: {storage: 1Gi}}}
---
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachine
metadata: {name: $1, namespace: $ns}
spec:
  stateVolumeClaim: $1-state
  accelerator: kvm
  cpus: 1
  memoryMiB: 512
  storageNode: kube03
  components: {bmc: {enabled: true}}
  # A console nobody can open: its only subject is no user.
  console: {allowedSubjects: [00000000-0000-4000-8000-000000000000]}
  disks: [{id: root, sizeBytes: 1073741824, bootIndex: 0, baseImage: {name: cirros, generation: 1}}]
  nics: [{name: lan, $groups}]
  cloudInit:
    hostname: $1
    userData: |
$(printf '%s\n' "$3" | sed 's/^/      /')
EOF
}
# The guest runs each new probe line "name ip port" once: a TCP connect, or for
# "spoof" a UDP datagram from a forged source address.
guest='#!/bin/sh
host='"$host_ip"'; port='"$http_port"'; udp='"$udp_port"'
done_list=""
while true; do
  me=$(ip -4 -o addr show dev eth0 | awk "{print \$4}" | cut -d/ -f1 | head -1)
  wget -q -T 2 -O /tmp/probes "http://$host:$port/probes?from=sec&ip=$me" || true
  while read -r name ip p; do
    [ -n "$name" ] || continue
    case " $done_list " in *" $name "*) continue;; esac
    done_list="$done_list $name"
    if [ "$name" = spoof ]; then
      ip addr add "$ip/32" dev eth0 2>/dev/null
      echo "spoofed" | nc -u -s "$ip" -w 1 "$host" "$udp" 2>/dev/null
      echo "genuine" | nc -u -w 1 "$host" "$udp" 2>/dev/null
      ip addr del "$ip/32" dev eth0 2>/dev/null
      r=sent
    elif nc -w 2 "$ip" "$p" </dev/null >/dev/null 2>&1; then r=open; else r=closed; fi
    wget -q -T 2 -O /dev/null "http://$host:$port/result?name=$name&r=$r" || true
  done < /tmp/probes
  sleep 2
done &'
log "creating VMs $prefix and $prefix-peer"
{
  cat <<EOF
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata: {name: sg-$prefix, namespace: $ns}
spec:
  description: 'Security group $prefix (check-nic-security.sh): ICMP/SSH in, world out'
  endpointSelector: {matchLabels: {sg.qrab.yapplesauce.com/$prefix: ""}}
  ingress:
  - fromEntities: [world, cluster]
    icmps: [{fields: [{type: 8, family: IPv4}]}]
  - fromEntities: [world, cluster]
    toPorts: [{ports: [{port: '22', protocol: TCP}]}]
  egress:
  - toEntities: [world]
EOF
  vm_yaml "$prefix" "$prefix" "$guest"
  vm_yaml "$prefix-peer" "" '#!/bin/sh
true'
} | kubectl apply -f - >/dev/null
for vm in "$prefix" "$prefix-peer"; do
  wait_for 420 "$vm Running" sh -c "[ \"\$(kubectl -n $ns get virtualmachine $vm -o jsonpath='{.status.phase}')\" = Running ]"
done
wait_for 180 "the guest fetching probes" grep -q 'http .* /probes?from=sec' "$work/log"
me=$(field virtualmachine "$prefix" '{.status.nics[0].address}')
peer=$(field virtualmachine "$prefix-peer" '{.status.nics[0].address}')
pod=$(field virtualmachine "$prefix" '{.status.podName}')
pod_ip=$(field pod "$pod" '{.status.podIP}')
node=$(field virtualmachine "$prefix" '{.status.nodeName}')
node_ip=$(kubectl get node "$node" -o jsonpath='{.status.addresses[?(@.type=="InternalIP")].address}')
dns=$(kubectl -n kube-system get svc kube-dns -o jsonpath='{.spec.clusterIP}')
log "guest $me in VM Pod $pod ($pod_ip) on $node ($node_ip); peer $peer"
cat > "$work/probes" <<EOF
world 1.1.1.1 443
bmc $pod_ip 8080
redfish $pod_ip 8081
migration $pod_ip 7800
kubelet $node_ip 10250
node-ssh $node_ip 22
cluster-dns $dns 53
peer-ssh $peer 22
anchor-uplink 169.254.1.2 22
gateway-ssh 169.254.1.1 22
spoof 10.244.128.250 0
EOF
expected() { case $1 in world) echo open ;; spoof) echo sent ;; *) echo closed ;; esac; }
summary=()
for name in $(awk '{print $1}' "$work/probes"); do
  wait_for 120 "probe $name" grep -q "/result?name=$name&" "$work/log"
  got=$(grep "/result?name=$name&" "$work/log" | tail -1 | sed 's/.*&r=//')
  [ "$got" = "$(expected "$name")" ] || fail "probe $name: $got (expected $(expected "$name"))"
  summary+=("$name: $got")
done
sleep 2
grep -q "udp $me genuine" "$work/log" || fail "the genuine UDP datagram did not arrive (control)"
if grep -q 'udp .* spoofed' "$work/log"; then
  fail "a datagram with a forged source arrived: $(grep 'udp .* spoofed' "$work/log")"
fi
summary+=("spoof: genuine datagram from $me arrived, forged 10.244.128.250 did not")

# Defence in depth: with the security group widened to the whole cluster,
# the VM Pod's own policies still refuse its migration port to its guest;
# what else answers is recorded for review.
kubectl -n "$ns" patch ciliumnetworkpolicy "sg-$prefix" --type merge -p '{"spec":{"egress":[{"toEntities":["world","cluster"]}]}}' >/dev/null
sleep 5
printf '%s\n' "wide-bmc $pod_ip 8080" "wide-redfish $pod_ip 8081" "wide-migration $pod_ip 7800" \
  "wide-kubelet $node_ip 10250" "wide-peer-ssh $peer 22" >> "$work/probes"
for name in wide-bmc wide-redfish wide-migration wide-kubelet wide-peer-ssh; do
  wait_for 120 "probe $name" grep -q "/result?name=$name&" "$work/log"
  got=$(grep "/result?name=$name&" "$work/log" | tail -1 | sed 's/.*&r=//')
  if [ "$name" = wide-migration ] && [ "$got" != closed ]; then
    fail "the guest reached its VM Pod's migration port"
  fi
  summary+=("$name (security group admits the cluster): $got")
done

# qrab-netd refuses another VM's NIC to this VM Pod.
netd=$(kubectl -n qrab-node get pods -l app.kubernetes.io/name=qrab-netd --field-selector "spec.nodeName=$node" -o name | head -1)
netd_bin=$(kubectl -n qrab-node exec "$netd" -- sh -c 'command -v qrab-netd')
kubectl -n qrab-node exec "$netd" -- cat "$netd_bin" > "$work/qrab-netd"
kubectl -n "$ns" exec -i "$pod" -c manager -- sh -c 'cat > /tmp/qrab-netd && chmod +x /tmp/qrab-netd' < "$work/qrab-netd"
peer_nic_refusal=$(kubectl -n "$ns" exec "$pod" -c manager -- /tmp/qrab-netd request lan-of-peer 1 2>&1 || true)
own=$(kubectl -n "$ns" exec "$pod" -c manager -- /tmp/qrab-netd request lan 1 2>&1 || true)
echo "$peer_nic_refusal" | grep -q refused || fail "qrab-netd did not refuse a NIC this VM does not have: $peer_nic_refusal"
echo "$own" | grep -q -E 'refused: .*held|received' || fail "unexpected answer for the VM's own NIC: $own"
summary+=("qrab-netd: refuses a NIC the VM lacks; its own NIC's tap is held by QEMU ($(echo "$own" | tail -1 | cut -c1-80))")

log "summary:"; printf '  %s\n' "${summary[@]}"
log "PASS: NIC security"
