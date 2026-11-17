# Dedicated native iSCSI target

`qrab-iscsi-target` is a bounded root CLI for qrab's new ZFS-backed LIO targets.
It does not change existing CSI exports. It has no remote API of its own, shell
commands or caller-selected backing paths. Guest disks use QEMU/libiscsi
directly; the small existing filesystem PVC still holds TPM/NVRAM state.

## Storage-service wrapper mode

The `vm-disk-*` and `vm-image-*` subcommands are the fixed wrapper behind the
towl storage daemon's verbs of the same names (`docs/iscsi.md`). The one
exception is `vm-image-import` (below), which is administrator-only and
deliberately outside both the wrapper's dispatch table and the daemon's
operation set. The wrapper subcommands take an
exact flag set, print one JSON report on stdout and one
`{"error":"<code>","message":...}` line on stderr when refused. Only
`vm-disk-export` and `vm-image-stage-export` read a credential, from stdin;
reports never contain it.
Provisioned disks use journal schema 2 (`<uid>.<disk>.json`) and the dataset,
target and object names `disk-<uid>-<disk>`, `qrab.disk.<uid>.<disk>` and
`qrab-<uid>-<disk>`, so they cannot collide with the legacy demo record. Sealed
images are journaled as `image-<name>-g<gen>.json`, unsealed image stages as
`stage-<name>-g<gen>.json` (below). The global journal lock
waits up to 20s, then reports `busy`.

### Staging a base image for an in-cluster importer

The `vm-image-stage-*` verbs let an unprivileged importer write a new
generation over iSCSI while root only hashes and seals it. Root never parses
the image:

| Verb and exact flags | Effect |
|---|---|
| `vm-image-stage-create --image N --image-generation G --capacity-bytes B` | Thick zvol `tank/qrab-iscsi/stage-N-gG` of B bytes (whole MiB, 16MiB..64GiB, budgeted against the parent quota). Refused with `image-generation-exists` if the generation already exists as an image journal or dataset, or as a stage of another capacity. The same stage and capacity again is idempotent. |
| `vm-image-stage-export --image N --image-generation G --generation K` (CHAP on stdin) | Target `iqn.2026-11.com.yapplesauce:qrab.stage.N.gG` at the usual portal and LUN 0. Its only ACL is `iqn.2026-11.com.yapplesauce:qrab.importer.N.gG`, with CHAP user `qrab-stage-N-gG`. Generations and rotation work exactly as for `vm-disk-export`. |
| `vm-image-stage-seal --image N --image-generation G --size-bytes S --sha256 HEX` | Fences the export (same drain barriers as `vm-disk-fence`), then root reads and hashes the first S bytes of the zvol itself. S must be a non-zero multiple of 512 and at most the capacity (`invalid-size`). On a match, the tool removes the stage's LIO objects and renames the zvol to `image-N-gG` (no copy). It then seals it exactly as `vm-image-import` does, with `"staged":true` in the journal. A mismatch reports `sha256-mismatch` and leaves the stage fenced; re-export with the next generation to retry. |
| `vm-image-stage-abort --image N --image-generation G` | Fences, then destroys the stage zvol after checking its stage markers and recorded GUID, and removes its journal. Already absent is success. |
| `vm-image-stage-status --image N --image-generation G` | Verified state, or `stage-not-found`. |

Stage verbs report `{"ok":true,"stage":{...}}` with `phase` one of
`provisioned`, `preparing`, `active`, `fencing`, `fenced`, `sealing` or
`absent`. Seal returns the sealed `image` report. Once the digest matches, the
stage journal records `seal` (phase `sealing`). From then on, export and abort
refuse with `stage-sealing`, and only rerunning the same seal finishes it.
Rerunning a finished seal reports the image. Boot restore never re-enables a
stage export. An active stage found after a reboot is fenced by its next seal
or abort.

### Importing a base image from a raw file

`vm-image-import` turns a verified raw OS image into a sealed generation that
VMs clone, without a builder VM. It reads a local file, so the storage
daemon must never be able to call it. Like `vm-image-retire` and the target
recovery commands, an administrator runs it as root on the storage host:

1. Download the upstream image and check it against the upstream checksum
   list (for CirrOS, the release's checksum file for the qcow2).
2. Convert it to raw in a root-controlled directory and hash the **raw** file:
   `qemu-img convert -f qcow2 -O raw cirros-0.6.3-x86_64-disk.img /var/tmp/cirros-0.6.3.raw`
   and `sha256sum /var/tmp/cirros-0.6.3.raw`.
3. Import a new, unused generation:

   ```sh
   sudo /usr/local/sbin/qrab-iscsi-target vm-image-import --image cirros --image-generation 1 \
     --source /var/tmp/cirros-0.6.3.raw --sha256 <raw sha256>
   ```

   The flags must be given in exactly this order. On success it prints
   `{"ok":true,"image":{...,"phase":"sealed","import":{"sha256":...,"size_bytes":...}}}`.
   On refusal it prints one `{"error":"<code>",...}` line on stderr. The codes
   are `invalid-image`, `invalid-sha256`, `invalid-source` (not an absolute
   path), `source-not-found`, `source-not-regular`, `source-empty`,
   `source-misaligned` (not a multiple of 512 bytes), `source-too-large`
   (over 64GiB), `image-generation-exists`, `budget-exhausted`,
   `source-changed` (short read) and `sha256-mismatch`.
4. Check it with the daemon's `vm-image-status` verb, or locally with
   `sudo /usr/local/sbin/qrab-iscsi-target vm-image-status --image cirros --image-generation 1`.
   Then delete the raw file.

A digest mismatch or any failure after the zvol is created destroys that new
zvol and its journal; existing images are never touched. If the tool is
killed mid-copy, or its cleanup fails, it leaves an unsealed import journal.
Rerunning the import for that generation cleans it up first. The copy holds
the global journal lock, so daemon calls report `busy` until it finishes.

Activation requires root and is staged, not automatic:

1. Build from merged source and record the SHA256 (`cargo build --locked
   --release -p qrab-iscsi-target`). Upgrade the installed tool with the reviewed
   installer, which verifies its owned marker and re-verifies the demo target:
   `sudo bash contrib/storage/install-demo.sh "$PWD/target/release/qrab-iscsi-target" SHA256`.
   The existing four demo sudo commands keep working unchanged.
2. Install a towl-storage-daemon build that has the `vm-disk-*` verbs, set
   `TOWL_STORAGE_VM_DISK_WRAPPER=true` and
   `TOWL_STORAGE_VM_DISK_SUBJECTS=<qrab operator subject UUID>` in
   `/etc/towl/storage-daemon.env` (preserve existing subjects/actors; the shim
   actor must already be in `TOWL_STORAGE_ALLOWED_ACTORS`) and restart it.
3. Create the Kanidm service account and scope map for the operator subject
   (`openid storage_read storage_provision storage_clone storage_expand
   storage_attach storage_retire`; never `storage_publish`), its `may_act`
   grant for the exchange shim actor, and the shim policy
   `qrab-vm-disk-operator` for caller
   `spiffe://yapplesauce.com/kubernetes/qrab/qrab-operator`.
4. Allow the `vm-disk-*` verbs and the top-level `credential` field in the
   storage-ingress shim, register the operator's ClusterSPIFFEID
   (`contrib/kubernetes/operator/storage-api/clusterspiffeid.yaml`) and publish
   the ingress CA as ConfigMap `qrab-storage-host-ca`; then apply the
   `contrib/kubernetes/operator/storage-api` component.
5. Install the reap timer:
   `sudo install -o root -g root -m 0644 contrib/storage/qrab-iscsi-reap.service contrib/storage/qrab-iscsi-reap.timer /etc/systemd/system/`,
   then `sudo systemctl daemon-reload && sudo systemctl enable --now qrab-iscsi-reap.timer`.
   Run `sudo qrab-iscsi-target vm-disk-reap --older-than-days 7 --mode plan`
   first to see what the first pass will stamp or destroy.

The first demo is fixed to VM UID `e309f751-5dda-4271-a0f4-4454c9527a59`, LUN0,
128MiB, host bridge `10.203.0.1:3260`. Its target is
`iqn.2026-11.com.yapplesauce:qrab.disk.e309f751-5dda-4271-a0f4-4454c9527a59`;
only initiator
`iqn.2026-11.com.yapplesauce:qrab.vm.e309f751-5dda-4271-a0f4-4454c9527a59`
is mapped. CHAP is mandatory and dynamic/demo ACL creation is disabled. CHAP
authenticates login; it does not encrypt the data transport.

## Ownership and limits

The tool uses only dedicated parent `tank/qrab-iscsi` (aggregate quota created
at 8GiB, accepted within the reviewed 8GiB..4TiB whole-GiB range),
thick-reserved16KiB zvols of16..1024MiB, kernel HBA `iblock_47` and exact
VM-derived target names. Prototype volumes use `sync=always`. Existing parents,
zvols and targets without its root ownership journal/markers are refused.
Recorded ZFS GUID, identity, size, geometry, reservation and target topology
must continue to match. The legacy demo has no resize; provisioned disks grow
only (`vm-disk-expand`), CoW clones are thin but budgeted at their logical size,
and images are sealed snapshots. There is no foreign-adopt operation and no
destroy of a sealed image. Three destroys exist, each limited to a zvol the
tool created itself and checked for ownership. `vm-image-stage-abort`, on an
unsealed stage, is the only one the API can reach. A failed `vm-image-import`
discards its zvol. The root-only `vm-disk-reap` destroys disks retired more
than N days ago; `qrab-iscsi-reap.timer` runs it daily with N=7
([docs/iscsi.md](../../docs/iscsi.md#the-reap-timer)). The existing
`tank/kubernetes-block-storage` tree is unchanged.

Manifests and passwords are root-owned0600 below
`/var/lib/qrab-iscsi-target` (0700). A global root lock serializes administration.
Interrupted `Preparing`/`Fencing` records are not blindly enabled. Handoff copies
are recipient-owned0600 files below root-owned `/var/lib/qrab-iscsi-handoff`;
these copies are never authoritative input. Passwords appear neither in argv
nor stdout/logs. The installer resolves the recipient with `id -u bob` (1002 on
the inspected host), not a guessed UID.

## Reviewed operator installation

Build from the merged clean source and record the actual binary SHA256:

```sh
cargo test --locked -p qrab-iscsi-target
cargo clippy --locked -p qrab-iscsi-target --all-targets --no-deps -- -D warnings
cargo build --locked --release -p qrab-iscsi-target
sha256sum target/release/qrab-iscsi-target
target/release/qrab-iscsi-target plan --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59 --size-mib 128
```

The plan is public derived configuration; privileged apply checks live ownership
and capacity. The following step requires the OS operator's root access. Review
`install-demo.sh`, the Rust tool and the service unit before running it:

```sh
sudo bash contrib/storage/install-demo.sh \
  "$PWD/target/release/qrab-iscsi-target" ACTUAL_REVIEWED_BINARY_SHA256
```

The installer verifies a root-private copied artifact before execution, installs
only `/usr/local/sbin/qrab-iscsi-target` and its owned marker/unit, starts the
standard configfs mount, loads the existing `target_core_iblock` and
`iscsi_target_mod` modules, provisions the dedicated demo, and writes the
credential handoff for bob. It refuses a foreign preexisting installed binary or
unit. By default it installs no sudo rule or general root command interface.
The optional explicit `--allow-demo-automation` flag installs, immediately after
verified binary/bootstrap and before target application, a `visudo`-checked
rule allowing bob only four exact commands for this one VM: status, fence,
apply128MiB and handoff to bob's resolved UID. There are no wildcard arguments,
bootstrap/install/update/restore grant, or root-shell permission. Existing foreign
rules are refused. Disclose and review this optional grant before choosing it.

`qrab-iscsi-target.service` restores only previously **Active** exports after
ZFS/network readiness, preserving their CHAP credentials across ordinary host
reboots. Existing active exports are verified without disruption. A malformed
journal or drift fails the service. Preparing/Fencing/Fenced records remain
unavailable; restoration can finish teardown, but cannot resume them. Restoration
is not a migration fence.

Import the recipient's private handoff into a Kubernetes Secret through a
credential-safe stdin/API operation. Never paste its contents into commands,
source control, work notes or chat. The JSON contains `chap_username`,
`chap_password`, `portal`, `target_iqn`, `initiator_iqn`, `lun`, `vm_uid` and
`generation`. The operator owns VM disk references, not this target root journal.

## Writer/fencing contract

The initial controller requires an explicit immutable storage node, scheduler
pinning and the existing state-volume flock/RWOP protections. It must observe a
clean process exit before replacement; failed, unknown or force-lost owners
remain blocked pending explicit administration. IQN ACLs and `MaxConnections=1`
are not themselves an exclusive-session/failover fence. Automatic cross-node
migration is outside this version.

The root recovery sequence is:

```sh
sudo /usr/local/sbin/qrab-iscsi-target fence --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59
sudo /usr/local/sbin/qrab-iscsi-target apply --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59 --size-mib 128
sudo /usr/local/sbin/qrab-iscsi-target handoff --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59 --recipient-uid "$(id -u bob)"
```

Fence first persists `Fencing`, disables the owned TPG, removes its mapped LUN
and exact initiator ACL, then unlinks the owned TPG LUN/backend export. ACL
removal waits for session destruction. LUN unlink waits for outstanding I/O
references through `core_tpg_remove_lun`/`transport_clear_lun_ref`; the tool also
flushes the backing block device and verifies no ALUA target-port members remain.
Only then is `Fenced` durable. It never treats elapsed time or Kubernetes state
as proof. Kernel teardown can wait for real I/O; an interrupted/hung/failed fence
never authorizes a new writer.

The subsequent apply increments the credential generation and rotates CHAP
before reenabling the export. Update the VM's Secret from the new private
handoff before an administrator acknowledges recovery. Old credentials cannot
reconnect. That acknowledgment is an operational action, not a cryptographic
fencing receipt. Data/zvol GUID and TPM identity are retained.

The kernel barriers are documented in the inspected Linux6.18 source:
[TPG disable](https://github.com/torvalds/linux/blob/v6.18/drivers/target/iscsi/iscsi_target_tpg.c),
[ACL deletion and LUN removal](https://github.com/torvalds/linux/blob/v6.18/drivers/target/target_core_tpg.c),
and [LUN-reference drain](https://github.com/torvalds/linux/blob/v6.18/drivers/target/target_core_transport.c).
The source inspection and hermetic tests do not replace the privileged live
session/old-password-rejection and guest data-persistence checks after install.
