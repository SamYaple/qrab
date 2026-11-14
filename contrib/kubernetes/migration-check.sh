#!/usr/bin/env bash
# Live acceptance of VM live migration (docs/migration.md). Boots a CirrOS VM
# with a provisioned disk, swtpm, UEFI and one NIC in security group nettest,
# then moves it with a VirtualMachineMigration and checks:
# - the guest keeps running: it reports its boot_id and uptime to the
#   qrab-nettest listener every second, and SSH answers on its NIC address;
# - the migration port 7800 refuses another Pod before, during and after;
# - the time-boxed policy exists only during the move;
# - the TPM state reached the target's own state PVC;
# - the source Pod, policy and the operator-created PVC are cleaned up.
# Needs: the operator and manager from the same release, the qrab-vm/qrab-nic
# ClusterSPIFFEIDs, namespace qrab-nettest with Pod listener (python
# http.server on 8000) and CNP qrab/sg-nettest. Deletes its VM at the end.
#
#   bash contrib/kubernetes/migration-check.sh [VM_NAME]
set -euo pipefail
vm=${1:-migcheck}
ns=qrab
listener=$(kubectl -n qrab-nettest get pod listener -o jsonpath='{.status.podIP}')
log() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*"; }
fail() { log "FAIL: $*"; exit 1; }
cleanup() {
  kubectl -n qrab-nettest delete pod migcheck-probe --ignore-not-found --wait=false >/dev/null || true
  kubectl -n "$ns" delete virtualmachine "$vm" --ignore-not-found --wait=true --timeout=180s >/dev/null || true
  kubectl -n "$ns" delete pvc "$vm-state" --ignore-not-found >/dev/null || true
  kubectl -n "$ns" delete virtualmachinemigration "$vm-move" --ignore-not-found >/dev/null || true
}
trap cleanup EXIT
field() { kubectl -n "$ns" get "$1" "$2" -o jsonpath="$3"; }
wait_for() { # seconds description command...
  local limit=$1 what=$2; shift 2
  for _ in $(seq 1 "$limit"); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done
  fail "$what within ${limit}s"
}
# Exit 0 if TCP $1:$2 connects from the probe Pod within 3 s.
reach() { kubectl -n qrab-nettest exec migcheck-probe -- python3 -c "import socket,sys; socket.create_connection(('$1',$2),timeout=3)" >/dev/null 2>&1; }

log "creating VM $vm"
kubectl apply -f - <<EOF
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
  nics: [{name: eth0, securityGroups: [nettest]}]
  cloudInit:
    hostname: $vm
    userData: |
      #!/bin/sh
      boot=\$(cat /proc/sys/kernel/random/boot_id)
      while true; do
        wget -q -T 2 -O /dev/null "http://$listener:8000/?from=$vm&boot=\$boot&up=\$(cut -d' ' -f1 /proc/uptime)"
        sleep 1
      done &
EOF
wait_for 300 "VM Running" sh -c "[ \"\$(kubectl -n $ns get virtualmachine $vm -o jsonpath='{.status.phase}')\" = Running ]"
address=$(field virtualmachine "$vm" '{.status.nics[0].address}')
source_pod=$(field virtualmachine "$vm" '{.status.podName}')
source_uid=$(field virtualmachine "$vm" '{.status.podUid}')
source_node=$(field virtualmachine "$vm" '{.status.nodeName}')
source_ip=$(field pod "$source_pod" '{.status.podIP}')
log "running: pod $source_pod ($source_uid) on $source_node, NIC $address, pod IP $source_ip"
spiffe_failures=$(kubectl get clusterspiffeid qrab-vm -o jsonpath='{.status.stats.entryFailures}')
[ "$spiffe_failures" = 0 ] || fail "ClusterSPIFFEID qrab-vm reports $spiffe_failures entry failures"

kubectl -n qrab-nettest apply -f - >/dev/null <<EOF
apiVersion: v1
kind: Pod
metadata: {name: migcheck-probe}
spec:
  terminationGracePeriodSeconds: 1
  tolerations: [{key: node-role.kubernetes.io/control-plane, operator: Exists, effect: NoSchedule}]
  containers:
  - name: probe
    image: docker.io/library/python:3.13-alpine
    command: [python3, -u, -c]
    args:
    - |
      import socket, time, datetime
      last = None
      while True:
          try:
              s = socket.create_connection(("$address", 22), timeout=1); s.settimeout(1)
              state = "ok " + s.recv(32).decode(errors="replace").strip(); s.close()
          except Exception as e:
              state = "fail " + type(e).__name__
          if state != last:
              print(datetime.datetime.utcnow().strftime("%H:%M:%S.%f")[:-3], state, flush=True)
              last = state
          time.sleep(0.5)
EOF
wait_for 120 "probe Ready" kubectl -n qrab-nettest wait --for=condition=Ready pod/migcheck-probe --timeout=1s
wait_for 180 "guest SSH" sh -c "kubectl -n qrab-nettest logs migcheck-probe | tail -1 | grep -q '^.* ok SSH'"
wait_for 120 "guest reports" sh -c "kubectl -n qrab-nettest logs listener | grep -q 'from=$vm&boot='"
boot_before=$(kubectl -n qrab-nettest logs listener | grep "from=$vm&boot=" | tail -1 | sed 's/.*boot=\([^&]*\).*/\1/')
tpm_before=$(kubectl -n "$ns" exec "$source_pod" -c manager -- sha256sum /var/lib/qrab/identity/tpm/tpm2-00.permall | cut -d' ' -f1)
reach "$source_ip" 7800 && fail "another Pod reached the migration port before the move"
log "before: guest boot $boot_before, TPM state $tpm_before; port 7800 refused to another Pod"

kubectl apply -f - >/dev/null <<EOF
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachineMigration
metadata: {name: $vm-move, namespace: $ns}
spec: {vmName: $vm}
EOF
started=$(date +%s)
log "migration created"
wait_for 120 "Migrating" sh -c "[ \"\$(kubectl -n $ns get qvmm $vm-move -o jsonpath='{.status.phase}')\" = Migrating ] || [ \"\$(kubectl -n $ns get qvmm $vm-move -o jsonpath='{.status.phase}')\" = Succeeded ]"
kubectl -n "$ns" get ciliumnetworkpolicy "qrab-migration-$vm-move" >/dev/null 2>&1 \
  || [ "$(field qvmm "$vm-move" '{.status.phase}')" = Succeeded ] \
  || fail "no migration window policy while migrating"
reach "$source_ip" 7800 && fail "another Pod reached the migration port during the move"
for _ in $(seq 1 600); do
  phase=$(field qvmm "$vm-move" '{.status.phase}')
  [ "$phase" = Succeeded ] && break
  [ "$phase" = Failed ] && fail "migration failed: $(field qvmm "$vm-move" '{.status.message}')"
  sleep 1
done
[ "$phase" = Succeeded ] || fail "migration did not finish"
log "migration succeeded after $(( $(date +%s) - started ))s: $(field qvmm "$vm-move" '{.status.message}')"
wait_for 120 "cleanup" sh -c "[ \"\$(kubectl -n $ns get qvmm $vm-move -o jsonpath='{.status.cleaned}')\" = true ]"

target_pod=$(field virtualmachine "$vm" '{.status.podName}')
target_uid=$(field virtualmachine "$vm" '{.status.podUid}')
target_node=$(field virtualmachine "$vm" '{.status.nodeName}')
target_ip=$(field pod "$target_pod" '{.status.podIP}')
[ "$target_uid" != "$source_uid" ] && [ "$target_node" != "$source_node" ] || fail "the VM did not change Pod and node"
kubectl -n "$ns" get pod "$source_pod" >/dev/null 2>&1 && fail "the source Pod is still there"
kubectl -n "$ns" get ciliumnetworkpolicy "qrab-migration-$vm-move" >/dev/null 2>&1 && fail "the window policy is still there"
wait_for 60 "VM Running on the target" sh -c "[ \"\$(kubectl -n $ns get virtualmachine $vm -o jsonpath='{.status.phase}')\" = Running ]"
sleep 5
boot_after=$(kubectl -n qrab-nettest logs listener | grep "from=$vm&boot=" | tail -1 | sed 's/.*boot=\([^&]*\).*/\1/')
[ "$boot_after" = "$boot_before" ] || fail "the guest rebooted: boot $boot_before -> $boot_after"
tpm_after=$(kubectl -n "$ns" exec "$target_pod" -c manager -- sha256sum /var/lib/qrab/identity/tpm/tpm2-00.permall | cut -d' ' -f1)
[ "$tpm_after" = "$tpm_before" ] || fail "the target's TPM state differs from the source's"
reach "$target_ip" 7800 && fail "another Pod reached the migration port after the move"
kubectl -n qrab-nettest logs migcheck-probe | tail -1 | grep -q ' ok SSH' || fail "guest SSH is not answering after the move"
log "after: pod $target_pod ($target_uid) on $target_node, state PVC $(field virtualmachine "$vm" '{.status.stateVolumeClaim}'), same boot, same TPM state"
log "SSH reachability transitions:"
kubectl -n qrab-nettest logs migcheck-probe | sed 's/^/  /'
log "guest report gaps over 1.5 s:"
kubectl -n qrab-nettest logs listener | grep "from=$vm&boot=" | sed 's/.*up=\([0-9.]*\).*/\1/' \
  | awk 'NR>1 && $1-prev>1.5 {printf "  uptime %.1f: %.1f s since the previous report\n", $1, $1-prev} {prev=$1}'
log "PASS: live migration"
