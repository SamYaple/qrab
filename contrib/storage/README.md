# Dedicated native iSCSI target

`qrab-iscsi-target` is a bounded root CLI for qrab's new ZFS-backed LIO targets.
It does not change existing CSI exports. It has no remote API of its own, shell
commands or caller-selected backing paths. Guest disks use QEMU/libiscsi
directly; the small existing filesystem PVC still holds TPM/NVRAM state.

## Storage-service wrapper mode

The `vm-disk-*` and `vm-image-*` subcommands are the fixed wrapper behind the
towl storage daemon's verbs of the same names (`docs/iscsi.md`). They take an
exact flag set, print one JSON report on stdout and one
`{"error":"<code>","message":...}` line on stderr when refused. Only
`vm-disk-export` reads a credential, from stdin; reports never contain it.
Provisioned disks use journal schema 2 (`<uid>.<disk>.json`) and the dataset,
target and object names `disk-<uid>-<disk>`, `qrab.disk.<uid>.<disk>` and
`qrab-<uid>-<disk>`, so they cannot collide with the legacy demo record. Sealed
images are journaled as `image-<name>-g<gen>.json`. The global journal lock
waits up to 20s, then reports `busy`.

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
   storage_attach`; never `storage_publish`/`storage_retire`), its `may_act`
   grant for the exchange shim actor, and the shim policy
   `qrab-vm-disk-operator` for caller
   `spiffe://yapplesauce.com/kubernetes/qrab/qrab-operator`.
4. Allow the `vm-disk-*` verbs and the top-level `credential` field in the
   storage-ingress shim, register the operator's ClusterSPIFFEID
   (`contrib/kubernetes/operator/storage-api/clusterspiffeid.yaml`) and publish
   the ingress CA as ConfigMap `qrab-storage-host-ca`; then apply the
   `contrib/kubernetes/operator/storage-api` component.

The first demo is fixed to VM UID `e309f751-5dda-4271-a0f4-4454c9527a59`, LUN0,
128MiB, host bridge `10.203.0.1:3260`. Its target is
`iqn.2026-11.com.yapplesauce:qrab.disk.e309f751-5dda-4271-a0f4-4454c9527a59`;
only initiator
`iqn.2026-11.com.yapplesauce:qrab.vm.e309f751-5dda-4271-a0f4-4454c9527a59`
is mapped. CHAP is mandatory and dynamic/demo ACL creation is disabled. CHAP
authenticates login; it does not encrypt the data transport.

## Ownership and limits

The tool uses only dedicated parent `tank/qrab-iscsi` (8GiB aggregate quota),
thick-reserved16KiB zvols of16..1024MiB, kernel HBA `iblock_47` and exact
VM-derived target names. Prototype volumes use `sync=always`. Existing parents,
zvols and targets without its root ownership journal/markers are refused.
Recorded ZFS GUID, identity, size, geometry, reservation and target topology
must continue to match. The legacy demo has no resize; provisioned disks grow
only (`vm-disk-expand`), CoW clones are thin but budgeted at their logical size,
and images are sealed snapshots. There is no destroy or foreign-adopt
operation. The existing `tank/kubernetes-block-storage` tree is unchanged.

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
