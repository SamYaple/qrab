#!/usr/bin/env bash
# Live acceptance of a CoW clone through the operator: a disposable VM whose
# only disk is cloned from a published base image. Run after check.sh, once an
# administrator has published IMAGE generation GEN (docs/iscsi.md, "Base images
# are administrative"). It never prints credentials.
#
#   bash clone-check.sh IMAGE GEN [SIZE_BYTES]
set -euo pipefail
image=${1:?usage: clone-check.sh IMAGE GEN [SIZE_BYTES]}
gen=${2:?usage: clone-check.sh IMAGE GEN [SIZE_BYTES]}
size=${3:-134217728}
ns=qrab vm=qrab-clone-test secret=qrab-clone-test-disk-root
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

k apply -f - <<EOF
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: qrab-clone-test-state
spec:
  storageClassName: zfs-block
  accessModes: [ReadWriteOncePod]
  volumeMode: Filesystem
  resources:
    requests:
      storage: 1Gi
---
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachine
metadata:
  name: $vm
spec:
  stateVolumeClaim: qrab-clone-test-state
  memoryMiB: 512
  cpus: 1
  running: true
  bootDisk: /opt/qrab/demo/boot.img
  storageNode: kube03
  components:
    bmc: {enabled: false}
  console:
    publicUrl: https://qrab-clone-test.invalid
    allowedSubjects:
    - 6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b
  disks:
  - id: root
    sizeBytes: $size
    baseImage: {name: $image, generation: $gen}
EOF
uid=$(k get qvm "$vm" -o jsonpath='{.metadata.uid}')
echo "clone VM UID $uid"
wait_for "clone provisioned and exported" '.status.disks[0].phase' Active
wait_for "VM running with the clone" '.status.phase' Running
[ "$(status '.status.disks[0].capacityBytes')" = "$size" ] || { echo "FAIL: clone capacity differs" >&2; exit 1; }
if k patch qvm "$vm" --type=json -p '[{"op":"replace","path":"/spec/disks/0/baseImage/generation","value":999999}]' 2>/dev/null; then
  echo "FAIL: baseImage change accepted" >&2; exit 1
fi
echo "ok: baseImage is immutable"
echo "As root, confirm the CoW origin and the clone hold before deletion:"
echo "  sudo zfs get -H -o value origin tank/qrab-iscsi/disk-$uid-root    # expect tank/qrab-iscsi/image-$image-g$gen@sealed"
echo "  sudo zfs holds tank/qrab-iscsi/image-$image-g$gen@sealed           # expect qrab-image and qrab-clone-$uid.root"
read -r -p "Press Enter to delete the clone VM (or Ctrl-C to keep it for inspection) " _
k delete qvm "$vm" --wait=true --timeout=300s
for _ in $(seq 1 60); do k get secret "$secret" >/dev/null 2>&1 || break; sleep 2; done
k get secret "$secret" >/dev/null 2>&1 && { echo "FAIL: owner GC kept the Secret" >&2; exit 1; }
k delete pvc qrab-clone-test-state --wait=true --timeout=300s
echo "ok: clone VM deleted after fencing. As root, retire the fenced clone before the image:"
echo "  sudo /usr/local/sbin/qrab-iscsi-target vm-disk-retire --vm-uid $uid --disk root"
