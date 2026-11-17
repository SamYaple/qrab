# Base images

A base image is a sealed, read-only ZFS snapshot
`tank/qrab-iscsi/image-<name>-g<generation>@sealed` that VM root disks clone
([iscsi.md](iscsi.md)). Generations never change; a new release or rebuild
is a new generation. Image admins stage and refresh images from the
dashboard's Images page ([dashboard.md](dashboard.md#base-images)); the
image importer turns each request into a sealed generation. No root command
runs per image, and no root code parses a downloaded file.

## BaseImage

```yaml
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: BaseImage
metadata: {name: cirros-g2, namespace: qrab}   # always <image>-g<generation>
spec:
  image: cirros            # 1-32 of [a-z0-9-]
  generation: 2            # 1-999999
  title: CirrOS 0.6.3
  os: cirros
  version: 0.6.3
  source:
    url: https://download.cirros-cloud.net/0.6.3/cirros-0.6.3-x86_64-disk.img
    sha256: 7d6355852aeb6dbcd191bcda7cd74f1536cfe5cbf8a10495a7283a8396e4b75b   # of the downloaded file
    format: qcow2          # or raw
  retired: false           # true stops new clones; irreversible
```

`kubectl -n qrab get bimg` shows each generation's phase. Everything but
`retired` is immutable (CEL), and a retired generation stays retired. The
importer refuses a BaseImage whose name is not `<image>-g<generation>` (that
rule exceeds the API server's CEL cost budget). Status: `phase` (Pending,
Inspecting, Staging, Writing, Sealing, Sealed, Retired, Failed), `sizeBytes`
(raw), `capacityBytes` (the sealed volume; clones must be at least this
big), `sha256` (of the raw image, as the storage host measured it),
`adopted`, `attempts` and `message`.

## The importer

`qrab-operator image-importer` (Deployment `qrab-image-importer`,
`contrib/kubernetes/image-importer`) has its own ServiceAccount and SPIFFE ID
`spiffe://yapplesauce.com/kubernetes/qrab/qrab-image-importer`, which the
exchange shim maps (policy `qrab-image-importer`) to a Kanidm service
account with `storage_read storage_provision storage_attach storage_publish
storage_retire`. The storage daemon lists that subject in
`TOWL_STORAGE_VM_IMAGE_SUBJECTS`, which allows only the `vm-image-stage-*`
verbs plus `vm-image-status` and `vm-image-retire`; the VM operator can call
none of them. Per BaseImage it:

1. reads `vm-image-status`: a generation that already exists (for example
   one made with `vm-image-import`) is **adopted** as Sealed, without Jobs;
2. runs the **inspect** Job: download over HTTPS (redirects must stay on
   HTTPS, at most 64 GiB), check the upstream SHA-256 *before* anything
   parses the file, then require a self-contained input before `qemu-img info`
   reports the virtual size. A bounded qcow2 v2/v3 fixed-header preflight
   rejects backing-file offset/length declarations and external data files
   before any qemu-img invocation. A wrong digest or dependency fails the
   image at once;
3. calls `vm-image-stage-create` with the size rounded up to whole MiB
   (at least 16 MiB), writes a fresh CHAP password to Secret `<bimg>-stage`
   (BaseImage-owned, key `password` only, confined by
   `secret-policy.yaml`) and calls `vm-image-stage-export`;
4. runs the **write** Job: download, check and preflight again, convert to raw, hash
   it, and write it onto the stage LUN with `qemu-img convert -n` over
   qemu's userspace libiscsi initiator, logging in as the stage's own
   initiator with the mounted CHAP password (a qemu `secret` object reads the
   file; it is never in argv);
5. calls `vm-image-stage-seal` with the Job's size and SHA-256: the storage
   host fences the export, hashes the volume itself and, only if it matches,
   renames it to `image-<name>-g<gen>` and seals it exactly as an import.

A failed write or a seal that measures another digest aborts the stage
(`vm-image-stage-abort`, the only API destroy, and only for an unsealed
stage) and retries with a new stage; the third failure leaves the image
Failed. Deleting an unsealed BaseImage aborts its stage (finalizer
`qrab.yapplesauce.com/image-stage`); deleting a sealed one never touches
the sealed volume. `retired: true` calls `vm-image-retire`, which the
storage host refuses while clones still descend from the snapshot (the
importer retries).

Kubernetes child names are reusable. The importer accepts a Job only when it
has the exact current BaseImage controller UID, and consumes a report only
from a Pod controlled by that observed Job UID. Successful reports require a
zero container exit code. Job cleanup checks UID/resourceVersion, including
the identity of a Job whose report was consumed. A create conflict must prove
the existing Job's owner before it is accepted.

Stage-Secret cleanup atomically tests its BaseImage owner and labels using a
no-op JSON patch, requests only metadata, then deletes that UID/version. It
does not require Secret get/list/watch or read the password back. A rotated
or recreated Secret is retained on conflict. Status/finalizer writes refresh
the API version while requiring the same BaseImage UID, generation, spec and
deletion state. The parent is also checked before reconciliation and storage
mutations, so a known stale object cannot change the backend before its later
status write is refused. Finalizer changes preserve other controllers' entries. These
API conditions do not make the backend storage operations one transaction;
intentional adoption of an existing sealed backend generation is unchanged.

Both Jobs run the importer's image (`qrab/operator`, which carries
`/opt/qemu/bin/qemu-img` built from `vendor/qemu` with libiscsi) as
`qrab-operator image-job inspect|write`, with no Kubernetes token
(`automountServiceAccountToken: false`), no Workload API socket, a read-only
root, UID 10001 and an emptyDir scratch volume. CiliumNetworkPolicy
`qrab-image-job` lets them reach only HTTPS on the internet, DNS and the
iSCSI portal `10.203.0.1:3260`. They report through their termination
message: `{"virtualSize":N}`, `{"sizeBytes":N,"sha256":"..."}` or
`{"error":"sha256-mismatch"|"import-failed","message":"..."}`.

Trust: the upstream SHA-256 an image admin enters is the root of trust. The
digest covers the downloaded file, not any file or network backing dependency
named by image metadata. Import Jobs therefore require standalone raw/qcow2
inputs; flatten a backing chain in its trusted preparation environment first.
The preflight checks dependency declarations only; QEMU remains responsible
for full format validation. The write Job converts only a self-contained file
that matches the digest, and the storage host seals
only bytes that match what the Job reported, so a short or damaged write is
caught. A compromised converter could still write other bytes, but only
for a file the admin pinned.

## Verification

The explicit single-file boundary fixture check needs only the pinned current
`qemu-img` and private temporary files, not a cluster, guest or storage target:

```sh
QRAB_TEST_QEMU_IMG=/path/to/current/qemu-img \
  cargo test --locked -p qrab-operator real_qcow2_fixtures_keep_the_single_file_boundary -- --ignored --nocapture
```

It checks standalone qcow2 v2/v3/extended-L2 images, refuses backing and
external-data dependencies, and demonstrates conversion of a benign external
marker in the unguarded baseline. Default image-job tests validate truncated
headers, dependency declarations and refusal before tool launch without QEMU.

## Rollout

1. Host (root): install `qrab-iscsi-target` with the stage verbs, a
   towl-storage-daemon with the importer class, set
   `TOWL_STORAGE_VM_IMAGE_SUBJECTS=<importer Kanidm subject UUID>` in
   `/etc/towl/storage-daemon.env` and restart the daemon; allow the
   `vm-image-stage-*` verbs in the storage-ingress shim.
2. Kanidm: service account for the importer with the scopes above, its
   `may_act` grant for the exchange shim actor, and shim policy
   `qrab-image-importer` for the importer's SPIFFE ID.
3. kubernetes-gitops: ServiceAccount `qrab-image-importer` with its pull
   identity, and `clusterspiffeid.yaml` pinned to the published digest.
4. `kubectl apply -k contrib/kubernetes/image-importer`, then adopt existing
   generations by creating their BaseImages (cirros g1).
