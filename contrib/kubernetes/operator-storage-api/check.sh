#!/usr/bin/env bash
# Live acceptance of the operator storage API with a disposable VM in namespace
# qrab: provision, export, online grow, removal fence/Secret deletion, VM
# deletion fence and cleanup. Run by an administrator after
#   kubectl apply --server-side -k contrib/kubernetes/operator-storage-api
# has rolled out the operator. It never prints credentials.
set -euo pipefail
cd "$(dirname "$0")"
ns=qrab vm=qrab-disk-test secret=qrab-disk-test-disk-scratch
k() { kubectl -n "$ns" "$@"; }
status() { k get qvm "$vm" -o "jsonpath={$1}"; }
wait_for() { # description, jsonpath, expected value
  for _ in $(seq 1 120); do
    [ "$(status "$2" 2>/dev/null)" = "$3" ] && { echo "ok: $1"; return; }
    sleep 5
  done
  echo "FAIL: $1 (got '$(status "$2" 2>/dev/null)'; reason $(status .status.reason): $(status .status.message))" >&2
  exit 1
}

k rollout status deployment/qrab-operator --timeout=180s
k apply -f test-vm.yaml
uid=$(k get qvm "$vm" -o jsonpath='{.metadata.uid}')
echo "test VM UID $uid (needed for the root retire step at the end)"
wait_for "disk provisioned and exported" '.status.disks[0].phase' Active
wait_for "disk credential generation 1" '.status.disks[0].generation' 1
wait_for "VM running with the disk" '.status.phase' Running
k get secret "$secret" -o jsonpath='{.metadata.labels.app\.kubernetes\.io/component}{" "}{.metadata.ownerReferences[0].kind}{"\n"}'
pod_uid=$(status .status.podUid)
k get pod "$vm" -o jsonpath="{.metadata.annotations.qrab\.yapplesauce\.com/disk-capacity}{\"\n\"}"

# Online growth: the LUN grows, the annotation follows, the Pod is NOT replaced.
k patch qvm "$vm" --type=json -p '[{"op":"replace","path":"/spec/disks/0/sizeBytes","value":134217728}]'
wait_for "storage grew to 128MiB" '.status.disks[0].capacityBytes' 134217728
for _ in $(seq 1 60); do
  k get pod "$vm" -o jsonpath="{.metadata.annotations.qrab\.yapplesauce\.com/disk-capacity}" | grep -q 134217728 && break
  sleep 5
done
[ "$(status .status.podUid)" = "$pod_uid" ] || { echo "FAIL: growth restarted the VM" >&2; exit 1; }
grown=false
for _ in $(seq 1 60); do
  # Key order is not part of the log contract; match the parsed event.
  k logs "$vm" -c manager | python3 -c 'import json,sys
for line in sys.stdin:
    try: e=json.loads(line)
    except ValueError: continue
    if isinstance(e,dict) and e.get("event")=="disk_grown" and e.get("disk")=="scratch" and e.get("capacity_bytes")==134217728: sys.exit(0)
sys.exit(1)' && { grown=true; break; }
  sleep 5
done
k logs "$vm" -c manager | grep '"event":"disk_grow' | tail -3
$grown || { echo "FAIL: QEMU never grew the disk (no disk_grown event)" >&2; exit 1; }
[ "$(status .status.podUid)" = "$pod_uid" ] || { echo "FAIL: growth restarted the VM" >&2; exit 1; }
echo "ok: QEMU grew the disk online without a restart"
# Shrinking is refused by the CRD.
if k patch qvm "$vm" --type=json -p '[{"op":"replace","path":"/spec/disks/0/sizeBytes","value":67108864}]' 2>/dev/null; then
  echo "FAIL: shrink accepted" >&2; exit 1
fi
echo "ok: shrink refused"

# Removing the disk replaces the instance, then fences it and deletes its Secret.
k patch qvm "$vm" --type=json -p '[{"op":"remove","path":"/spec/disks"}]'
for _ in $(seq 1 120); do
  [ -z "$(status '.status.disks[*].id')" ] && ! k get secret "$secret" >/dev/null 2>&1 && break
  sleep 5
done
[ -z "$(status '.status.disks[*].id')" ] || { echo "FAIL: removed disk still recorded" >&2; exit 1; }
k get secret "$secret" >/dev/null 2>&1 && { echo "FAIL: removed disk Secret remains" >&2; exit 1; }
echo "ok: removed disk fenced and its Secret deleted"

# Re-add, then delete the VM: deletion fences the export before the finalizer goes.
k patch qvm "$vm" --type=merge -p '{"spec":{"disks":[{"id":"scratch","sizeBytes":134217728}]}}'
wait_for "re-added disk reattached with a rotated credential" '.status.disks[0].generation' 2
k delete qvm "$vm" --wait=true --timeout=300s
for _ in $(seq 1 60); do k get secret "$secret" >/dev/null 2>&1 || break; sleep 2; done
k get secret "$secret" >/dev/null 2>&1 && { echo "FAIL: owner GC kept the Secret after VM deletion" >&2; exit 1; }
k delete pvc qrab-disk-test-state --wait=true --timeout=300s
echo "ok: VM deleted after fencing; PVC removed"
echo "The fenced zvol is retained. As root, retire and dispose of it explicitly:"
echo "  sudo /usr/local/sbin/qrab-iscsi-target vm-disk-status --vm-uid $uid --disk scratch"
echo "  sudo /usr/local/sbin/qrab-iscsi-target vm-disk-retire --vm-uid $uid --disk scratch"
