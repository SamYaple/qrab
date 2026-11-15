# Live migration

A running VM moves to another node without a reboot. The guest's memory,
devices, TPM state and UEFI variables travel in QEMU's migration stream over a
SPIFFE mTLS tunnel between the two managers of the same VM. Disks are shared
iSCSI LUNs, so no disk data is copied. Guest NICs are taps in anchor Pods
pinned to the VM Pod's node; the operator does not drive their handoff yet (the
target manager needs `incoming.node_ip`, which the operator does not set), so a
VM with NICs cannot be live-migrated yet ([networking.md](networking.md#tap-datapath)).
Design and work items: towl
`container/vm-live-migration`.

## Identity and isolation

- Every VM pod's manager holds `spiffe://yapplesauce.com/kubernetes/qrab/vm/<vm-uid>`.
  The source and target pods of one VM therefore hold the same ID, and each
  side of the tunnel accepts exactly that ID (`projects/qrab-spiffe`, TLS 1.3,
  local trust domain only). Another VM's pod, or any other workload, fails the
  handshake.
- The source listens on TCP 7800 (`migration.port`). Two policies guard it:
  the standing `<vm>-migration-guard` denies the port to every entity and every
  endpoint that is not one of the VM's own VM Pods, and a deny always wins; the
  time-boxed `qrab-migration-<migration>` allows only the target Pod to reach
  the source Pod (by the `qrab.yapplesauce.com/pod` label), for the migration's
  lifetime. Outside a migration a VM has one Pod, so nothing can use the port.
- The target's hello names the VM (`state_id`) and the migration UID; the source
  refuses another VM, a protocol mismatch, local disks and hot-plugged growth.

## Moving a VM

```yaml
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachineMigration
metadata: {name: move-web-1, namespace: qrab}
spec:
  vmName: web
  targetNode: kube02   # optional; another Ready node is chosen otherwise
```

`kubectl get qvmm` shows the phase: `Pending` (waiting for a Ready VM),
`TargetStarting`, `Migrating`, then `Succeeded` or `Failed` with a message
and, when the cause is classified, a `reason` (`PoweredOff`). A
migration object is single-use and immutable; create a new one per move. One
migration per VM runs at a time; a second one created meanwhile fails.

The VM's own reconcile drives it (`projects/operator/src/migration.rs`), since
only that reconcile has the rendered disks, NIC addresses and boot size:

1. It picks the node (requested, else the first Ready, schedulable node other
   than the source with the KVM resource when needed), and names the target Pod
   `<vm>-m<id>` and its state PVC `<vm>-state-<id>` (same storage class and
   size, ReadWriteOncePod, VM-owned).
2. It creates the time-boxed policy `qrab-migration-<migration>`, the PVC, the
   target's ConfigMap (the VM's config plus `incoming`), and the target Pod
   pinned to the node, and records the target's UID before ungating it.
3. When the source's manager exits 0 with the termination message `migrated`,
   the target owns the guest and its disks. The operator strips `incoming` and
   points VM status at the target: `podName`, `podUid`, `nodeName`,
   `stateVolumeClaim`, `stateVolumeUid` and `placementNode`. From then on the
   normal reconcile renders exactly the target Pod, so it is not replaced.
4. Cleanup removes the source Pod, the policy and an operator-created previous
   state PVC after success (a PVC you created is left alone). After failure it
   removes the target Pod, policy, PVC and ConfigMap.

Anything else ending first fails the migration and leaves the source running:
the target stopping, the source exiting without `migrated`, the source Pod
being replaced, the VM stopping, or 10 minutes passing. A source that reports
`migrated` is never fenced by the storage guard, because its disks belong to
the target. If no migration adopts such a Pod, the VM blocks with reason
`HandedOver` instead of booting the guest a second time.

`spec.storageNode` is only the initial placement; `status.placementNode`
replaces it as the scheduling pin after a move, and the VM restarts there.

### Powered-off VMs

A VM powered off in place (Redfish/BMC power off, or the guest shutting
down) keeps its Pod Running and Ready while its manager waits in phase `off`
with no QEMU, so there is no guest to stream. Such a migration is refused, not
retried: the operator starts the target as usual, the source's manager answers
the target's hello with "the VM is powered off", and the target exits 1 with the
termination message `source-powered-off`. The migration then ends `Failed`
with reason `PoweredOff` and the message "the VM is powered off, so there is no
running guest to live-migrate; power it on and create a new migration", and is
cleaned up like any other failure (target Pod, policy, PVC and ConfigMap). The
VM stays powered off where it is. There is no cold move: a VM stopped with
`spec.running: false` restarts on its pinned node (`status.placementNode`,
else `spec.storageNode`).

## Manager protocol

`projects/manager/src/pod/migration.rs`.

1. The target manager (config `incoming: {migration, source}`) dials the
   source, sends `Hello`, and receives an `Offer`: the source's exact machine
   type (e.g. `pc-q35-11.1`) and every iSCSI binding it has recorded. The
   source's listener opens when its QEMU first runs and stays up for the
   manager's lifetime, across in-place power cycles; while the published phase
   is not `running` it refuses at once (`off`: "the VM is powered off", which
   the target reports as `Error::PoweredOff`).
2. The target prepares **empty** state (it refuses a retained identity, which
   would be a second, divergent copy), starts swtpm with a fresh directory and
   QEMU with the offered machine type and `-incoming defer`, calls
   `migrate-incoming unix:<session>/migrate-in.sock`, connects to that socket
   and sends `{"ready":true}`.
3. The source sets `return-path` and `events`, caps bandwidth at 1 GiB/s and
   runs `migrate unix:<session>/migrate-out.sock`. Each manager splices its
   QEMU's Unix socket with the TLS stream, so QEMU never sees a TCP socket.
4. Both managers poll `query-migrate` every 250 ms and keep publishing status,
   so probes stay green. On `completed` the target runs `cont`; the source
   QEMU (paused, disks inactive) is quit, and its manager exits 0 with outcome
   `{"reason":"migrated","shutdown_method":"migration"}`. On failure the source
   keeps running the guest, records `migration failed: ...` in its status
   detail, and accepts the next attempt.

TPM state rides the stream as swtpm state blobs; the target's swtpm writes them
to its own state directory. UEFI variables ride as pflash RAM and QEMU's
`pflash_post_load` writes them back to the target's `OVMF_VARS.fd`.

Limits: one migration at a time per VM; local `disk` images and hot-plugged
vCPUs/memory are refused (the target would need a different device layout);
SIGTERM during an outgoing migration cancels it and stops the VM normally.

## Testing

Unit tests in `migration.rs` cover the hello/offer frames, refusals and a
different VM's SVID being refused, with fixture SVIDs from
`qrab-spiffe`'s `fixture` feature. `tests/pod_control.rs` powers a supervised
mock VM off and checks that a target manager ends with `Error::PoweredOff`;
the operator's `tests/migration.rs` maps `source-powered-off` to `PoweredOff`.

`projects/manager/tests/pod_migration.rs` migrates a real UEFI + swtpm guest
between two managers (TCG), checks the guest agent answers on the target
without a reboot, then cold-boots the target's state and compares the TPM
endorsement key with the source's. It needs QEMU 11.1, swtpm, OVMF and the demo
guest, so run it inside the manager image, building with the image's Rust
toolchain:

```sh
T=target/migration-test
podman run --rm -v "$PWD":/src:ro -v "$PWD/$T":/target \
  -v "$HOME/.cargo/registry":/usr/local/cargo/registry:ro \
  -e CARGO_TARGET_DIR=/target -e CARGO_NET_OFFLINE=true -w /src \
  "$(sed -n 's/^ARG RUST_IMAGE=//p' contrib/images/Containerfile.manager)" \
  cargo test --release --locked -p manager --test pod_migration --no-run
podman run --rm -v "$PWD/$T":/target:ro \
  --entrypoint "$(ls $T/release/deps/pod_migration-* | grep -v '\.d$' | head -1 | sed "s|^$T|/target|")" \
  zot.yapplesauce.com/qrab/manager:<tag> --ignored --nocapture
```

First run, 2026-11-14 on manager `fe93197`: migration completed in under a
second for a 512 MiB TCG guest; the guest agent answered on the target, and the
cold-booted target reported the source's endorsement key.

## Redfish management-state boundary

Live migration currently refuses a VM with retained Secure Boot policy or
staged firmware metadata, or an enabled dynamic virtual-media policy. These
management records/graphs are not part of the migration offer yet; they must
not silently disappear or select different CODE on the target. Firmware staging
and migration admission share a bounded management lock. The incoming target
does not cold-apply policy to the placeholder VARS that is about to be replaced
by the authenticated migration stream. Unmodified default-image VMs retain the
existing migration behavior. Full policy/artifact transfer is a follow-up.
