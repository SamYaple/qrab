# Native iSCSI guest disks

QEMU connects directly to an iSCSI target through libiscsi and presents the LUN
as a virtio disk. There is no kernel initiator, block PVC, host filesystem or disk
image file in this guest-disk path. The storage host exports an owned ZFS zvol
through Linux LIO. The small filesystem PVC for TPM/UEFI state stays separate.
Only QEMU in the VM Pod connects to the storage endpoint; optional guest NICs
use separate anchor Pods. The data path is plain iSCSI with CHAP authentication,
which does not encrypt block traffic. The storage provisioning API's TLS and
SPIFFE exchange are separate connections. See the
[storage diagram](connections/data-paths.md#storage-identity-exchange-and-data-io-are-different-paths)
and [connection inventory](connections.md#storage-firmware-and-local-device-connections).

## VM configuration

Provision the target and its exact VM-UID initiator ACL first. Then create a
namespace Secret containing the exact CHAP password bytes, without a trailing
newline. Passwords never belong in CRs, ConfigMaps, image layers or argv:

```yaml
spec:
  storageNode: kube03
  iscsiDisks:
  - id: data
    portal: 10.203.0.1:3260
    target: iqn.2026-11.com.yapplesauce:qrab.disk.VM-UID
    lun: 0
    username: CHAP-ACCOUNT
    passwordSecret:
      name: vm-disk-password
      key: password
```

Replace the example target/account with the provisioner's report. The operator
derives the initiator as `iqn.2026-11.com.yapplesauce:qrab.vm.<CR UID>`; a CR cannot
supply another initiator. It mounts a readonly Secret file under
`/run/qrab-auth/<id>/password` (0440, fsGroup 10001). QEMU uses a secret object and
`password-secret`; it never receives a credential-bearing URL. `username` is an
explicit nonsecret CHAP account identifier.

IDs are stable guest serials of 1..20 lowercase letters/digits/hyphens. Portal
must be literal IP:port, including brackets for IPv6. Disk endpoint identity is
immutable per ID; the runtime also retains append-only ID bindings beside the
TPM state. Readonly/presentation/credential changes do not repurpose that ID.
Rotate the Secret and change `restartNonce` to load new credentials: existing
QEMU processes do not hot-reload secret objects.

Optional `bootIndex: 0` requests UEFI priority ahead of the bundled ESP; the LUN
must already contain a valid UEFI-bootable OS. Index 1 is reserved for the demo
ESP; other boot indexes must be unique. Ordinary data disks omit it. The initial
live proof used a disposable 128MiB data LUN; ordinary volumes use provisioned
disks (below) instead of hand-declared LUNs.

## Provisioned disks and CoW base images

`spec.disks` asks the operator to manage the whole disk lifecycle through the
towl storage service instead of hand-declaring a target and Secret:

```yaml
spec:
  storageNode: kube03
  disks:
  - id: root
    sizeBytes: 10737418240          # grow-only; 16MiB..1TiB in whole MiB
    baseImage: {name: debian, generation: 3}   # optional, immutable
    bootIndex: 0
  - id: scratch
    sizeBytes: 1073741824           # no baseImage: blank thick zvol
```

Architecture: the operator calls the root `towl-storage-daemon` verbs
`vm-disk-*`/`vm-image-*`, which run qrab's reviewed
`/usr/local/sbin/qrab-iscsi-target` as a fixed wrapper. That one tool owns
`tank/qrab-iscsi`, the dedicated HBA and every LIO object, so ZFS/LIO
ownership and the kernel fence barriers stay in the code reviewed for the
demo; towl-zfs never touches this parent. The daemon authenticates the
operator's delegated Kanidm token (SPIFFE exchange, RFC 8693 actor, per-verb
scope) and passes CHAP only on the wrapper's stdin.

Per disk the operator:

1. reads `vm-disk-status`; if absent, creates a thick blank zvol
   (`vm-disk-create`) or a CoW clone of the sealed image generation
   (`vm-disk-clone`). A clone holds `image-<name>-g<gen>@sealed` and records that
   snapshot's GUID; every later read verifies the clone's ZFS origin and GUID.
2. grows the zvol when `sizeBytes` increases (`vm-disk-expand`, never shrinks)
   **online**: after the LUN has grown, the operator updates the Pod annotation
   `qrab.yapplesauce.com/disk-capacity` (`{"<id>": bytes}`). The kubelet projects
   it into the manager's downward-API file `/run/qrab-size/disk_capacity`; the
   manager compares it with the raw node's size from `query-named-block-nodes`
   and calls QMP `block_resize` on `iscsi-<id>` only to grow. libiscsi re-reads
   the LUN capacity and refuses a size the target lacks; virtio-blk notifies the
   guest, whose filesystem growth stays the guest's decision. No restart, and no
   operator→Pod network path is involved.
3. generates a fresh 32-hex CHAP secret for every export, writes it to the
   VM-owned Secret `<vm>-disk-<id>` (key `password`, generation annotation)
   **before** calling `vm-disk-export` with the exact derived initiator ACL.
   Credentials are write-only: the operator has no Secret `get/list/watch`, and
   the `ValidatingAdmissionPolicy` in `contrib/kubernetes/operator/secret-policy.yaml`
   lets it create, update or delete only Secrets controller-owned by a
   VirtualMachine, labelled `disk-credential` with the matching owner UID, named
   `<vm>-disk-<disk>` and holding only `password`. An update must keep the same
   owner, so it cannot take over a hand-declared credential.
   `contrib/kubernetes/operator/check-secret-policy.sh` checks this live in a
   disposable namespace. An interrupted export or fence (`Preparing`/`Fencing`)
   is fenced and rotated to the next generation once no VM process uses it.
   Status records each disk's capacity, credential generation and phase.
4. when a disk leaves `spec.disks`, waits until no launched VM process mounts
   it, then fences its export and deletes its Secret. The zvol is retained, and
   re-adding the same ID reattaches that data with a new credential. Removing
   the LIO objects (`vm-disk-retire`) stays an administrative storage verb, and
   destroying the data is manual ([below](#destroying-retained-data)).

A fence that outlives the storage daemon's wrapper budget leaves the journal in
`Fencing`; the operator reports `StorageFenceInProgress` and resumes it on the
next reconcile instead of treating it as a refusal. No export happens before a
fence completes. Fence calls wait up to 620s (the daemon bounds the drain at
600s, the ingress at 610s); every other storage call times out at 75s. Each
reconcile awaits only its own VM's calls, so a slow fence never delays other
VMs' reconciles. While a fence holds the target journal lock, other VMs' calls
may report `busy`; the operator treats that, like any outage, as transient and
retries without rewriting the VM's status.

Target names are derived and verified, never CR input:
`iqn.2026-11.com.yapplesauce:qrab.disk.<VM-UID>.<id>`, CHAP account
`qrab-<uid-hex>-<id>`, dataset `tank/qrab-iscsi/disk-<VM-UID>-<id>`. The total
logical size of every disk and image stays within the parent's ZFS quota. It is
created at 8GiB; an administrator may raise it within the reviewed 8GiB..4TiB
whole-GiB range with `zfs set quota=<N>G tank/qrab-iscsi` (leaving pool
headroom for thick reservations); values outside the range fail closed.
`iscsiDisks` and `disks` may be combined; IDs are unique across both lists.

Base images are administrative. There are two ways to create a generation:
**publish** a disk that a VM built, or **import** a verified raw image file on
the storage host. The result is the same sealed generation either way, and
`vm-image-status`, `vm-disk-clone` and `vm-image-retire` treat both kinds
identically.

To publish, build the image on an ordinary provisioned disk,
stop that VM (the disk is fenced on deletion, or fence it explicitly), then
publish it with an administrator token holding `storage_publish`:

```json
{"request_id":"publish-debian-3","operation":{"name":"vm-image-publish","image":"debian","image_generation":3,"source_vm_uid":"<builder VM UID>","source_disk":"root"}}
```

Publication requires a verified `Fenced` source, copies it with
`zfs send | zfs receive` into a readonly `image-debian-g3` and holds its
`@sealed` snapshot. Generations are immutable; publish a new generation for an
update. The operator identity cannot publish or retire. `vm-disk-retire` (also
administrative) removes a fenced disk's LIO objects and marks it retained.
`vm-image-retire` stops new clones of a generation and marks it retained; it is
refused while any disk journal (in any phase, including retired clones), clone
hold or ZFS origin still descends from the sealed snapshot. Data is never
destroyed by the API.

To import a stock OS image such as CirrOS, a storage-host administrator runs
the root-only `vm-image-import` command. It is not a storage-daemon verb and
no token scope reaches it: the daemon's fixed operation set and the wrapper's
`vm-*` dispatch table both exclude it, so it can't be called remotely. The
command reads a local file, so it runs only through `sudo` on the host, like
the target's other administrative commands. Convert the image to raw first.
The SHA-256 you pass is the hash of the **raw** file, not of the upstream
qcow2:

```sh
# Check the upstream qcow2 against its published checksum, then:
qemu-img convert -f qcow2 -O raw cirros-0.6.3-x86_64-disk.img /var/tmp/cirros-0.6.3.raw
sha256sum /var/tmp/cirros-0.6.3.raw
sudo /usr/local/sbin/qrab-iscsi-target vm-image-import --image cirros --image-generation 1 \
  --source /var/tmp/cirros-0.6.3.raw --sha256 <raw sha256>
```

Import applies publication's name and generation rules and refuses an
existing generation (journal or dataset). The source must be an absolute path
to a non-empty regular file whose size is a multiple of 512 bytes and at most
64GiB. The file is opened without following symlinks, and its type and size
are checked on the opened file. The zvol is that size rounded up to the next
whole MiB (minimum 16MiB, the same rule as disk capacities), counted against
the parent quota.

The tool creates the zvol, writes the file to its device node with plain file
I/O while hashing the same bytes, and fsyncs. It seals the zvol only if the
digest and length match: it sets readonly, creates the `@sealed` snapshot,
adds the `qrab-image` hold and records the journal. These are the same
properties and sealing steps as publication. If anything fails after the zvol
is created (a digest mismatch, a short read, or a write or seal error), the
tool destroys that zvol and removes its journal. Existing generations are
never touched.

If the import is interrupted (for example by a crash) or the cleanup itself
fails, an unsealed import journal remains. Rerunning the import for that
generation cleans it up and starts over. The journal records the digest and
size as `"import":{"sha256":...,"size_bytes":...}` and leaves the source VM
fields empty. Published journals are unchanged. Clones must be at least the
image's capacity (`capacity_bytes` in the report). Boot a clone with
`bootIndex: 0`, which requires a UEFI-bootable image.

The import holds the global journal lock for the whole copy. Storage-daemon
calls made meanwhile wait 20s and then report `busy`. The operator retries
these.

## Ownership and recovery

`spec.storageNode` is immutable and pins the VM's first node. The operator never
moves a native-disk VM on its own; a `VirtualMachineMigration` moves it live
([migration.md](migration.md)) and records the new node in `status.placementNode`. The target's
exact initiator ACL and CHAP credentials authorize a VM; they alone do not limit
the number of sessions using those credentials. MaxConnections is per session.

Native Pods start with an ownership scheduling gate. The operator removes it
only after the Pod UID is durably recorded, so an untracked process cannot start.
Native VMs and Pods receive drain finalizers. Before replacing/deleting a Pod,
the operator records its confirmed clean exit UID in status, then drops the Pod
finalizer. Parent deletion waits for this cleanup before dropping its own
finalizer. The manager reports forced/nonzero native child termination as a
failure and retains its state lock through actual child reaping. Failed,
vanished or unknown old process state blocks replacement. Timeouts, Pod deletion
and lease expiry are not treated as storage fences.

Use the bounded root target procedure in
[contrib/storage/README.md](../contrib/storage/README.md) to disable/drain the old
export and rotate CHAP before recovery. After verifying that hard fence and
updating the Secret, an administrator can acknowledge the exact old Pod UID:

```sh
kubectl -n qrab patch qvm VM --subresource=status --type=merge \
  -p '{"status":{"storageRecovery":"EXACT-OLD-POD-UID"}}'
```

This field is an operator acknowledgment, not a signed fence receipt. It cannot
be reused for another Pod UID.

When every native disk of a VM is in `spec.disks`, the operator performs this
recovery itself: on a failed, vanished or unknown owner it calls
`vm-disk-fence` for each disk, which returns only after the kernel ACL and LUN
teardown barriers complete, then records `status.storageRecovery` with reason
`StorageAutoFenced`. A fenced disk is re-exported only after the old Pod is
gone, with the next credential generation and a newly generated secret; a
running Pod whose disk is found fenced is replaced first. A storage-service
failure leaves the VM blocked. Any hand-declared `iscsiDisks` entry keeps the
administrator procedure above. Deleting a VM fences its provisioned disks
before the finalizer is removed; zvols are retained. A failed orphan whose parent was forcibly removed
remains a visible finalizer tombstone; hard-fence first, then explicitly remove
its Pod finalizer. The periodic sweep only collects genuinely VM-owned clean
orphans.

PVC state locks and RWOP still protect retained identity. Keep the target,
credentials, guest disk and TPM/NVRAM identity together through deliberate
recovery; never copy a TPM volume to make another identity.

## Destroying retained data

All VM storage is Retain-only, and no API destroys data. Deleting a VM, or
dropping a disk from `spec.disks`, fences the export and keeps the zvol, which
still counts against the `tank/qrab-iscsi` quota. Throwaway VMs therefore leave
one retained disk each; the live check scripts are tracked by towl
`work/test-vm-disk-leak`. Destroying a disk is a root operation on the storage
host `bb`, after you have confirmed that no VM will reattach it:

```sh
t=/usr/local/sbin/qrab-iscsi-target
# 1. Must report phase "fenced" (or "provisioned", never exported). The operator
#    fences on VM deletion and when the disk leaves spec.disks.
sudo $t vm-disk-status --vm-uid VM-UID --disk ID
# 2. Remove the LIO target; the zvol stays and the journal phase becomes "retired".
sudo $t vm-disk-retire --vm-uid VM-UID --disk ID
# 3. Clones only: release the hold on the image snapshot.
sudo zfs release qrab-clone-VM-UID.ID tank/qrab-iscsi/image-NAME-gGEN@sealed
# 4. Destroy the data.
sudo zfs destroy tank/qrab-iscsi/disk-VM-UID-ID
```

The retired journal `/var/lib/qrab-iscsi-target/<VM-UID>.<id>.json` stays, and
`vm-image-retire` still counts it as a descendant of the clone's image. Image
generations follow the same order: `vm-image-retire`, then
`zfs release qrab-image` on the `@sealed` snapshot, then `zfs destroy -r` the
image dataset.

State, media and Redfish claims on `zfs-block` behave the same way. The
`zfs-block.csi.yaple.net` provisioner refuses `DeleteVolume` ("Retain-only
storage: retire preserved datasets through the host operator"), so changing a
Released PV's reclaim policy to `Delete` only makes it retry. Destroy the
Released PV's zvol, `tank/kubernetes-block-storage/<spec.csi.volumeHandle>`
(`kubectl get pv <name> -o jsonpath='{.spec.csi.volumeHandle}'`), with
`zfs destroy` on `bb`, then delete the PV object.

## Checks

The image includes native libiscsi; authentication errors fail startup rather
than falling back to a local disk. The normal suite includes config, Secret,
immutable binding, clean/failed/unknown cleanup and orphan-order tests.

`projects/manager/tests/live_iscsi.py` and `contrib/images/iscsi-test-fixture.py`
provide isolated protocol acceptance with a disposable userspace target. That
file-backed fixture is not production ZFS or kernel LIO fencing evidence.

The guest's explicit `qrab-disk-probe initialize|verify [--serial]` helper only
accepts the reserved `qrab-probe` serial and a 64..256MiB disposable LUN.
Initialize refuses existing signatures and requires a full zero scan before
formatting ext4. Verify mounts readonly without journal replay and reports the
filesystem UUID and persisted token hash. No disk is autoformatted at boot.
The test-only `qrab-operator` example `vnc_probe` sends this fixed lowercase
command over the private Unix VNC socket when run through authenticated
`kubectl exec`; it is not an anonymous BMC endpoint or a shipped service.

The deployment-specific demo patch is
`contrib/kubernetes/examples/qrab-demo/iscsi-demo.patch.json`, kept with the
retired qrab-demo sample. Apply such a patch only after the reviewed root
installer has prepared that exact VM UID's target, fencing has been tested and
the matching `qrab-demo-iscsi` Secret has been created. It is deliberately not
included in the default kustomization: installing runtime capability must not
connect an unprovisioned or unfenced LUN.
