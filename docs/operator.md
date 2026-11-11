# Minimal VM operator

`qrab-operator` reconciles namespaced `VirtualMachine` resources in
`qrab.yapplesauce.com/v1alpha1`. It owns one Pod and ConfigMap per VM. Each Pod
runs the existing manager/BMC, QEMU, OVMF UEFI and swtpm TPM2, with guest
networking disabled. No guest Kubernetes token, VIF, arbitrary Pod template,
leader election, VM admission webhook or cluster-wide controller is involved.
Use one operator replica with `Recreate` deployment strategy.

PVCs, console Services/Ingresses/certificates and OIDC callback registration stay
explicit infrastructure resources. This keeps storage and identity retention
independent of controller garbage collection. The operator only reads PVCs;
it cannot create or delete them, change VM specs, or manage another namespace.

## Create a VM

The operator uses the registered `qrab-operator` image-pull identity; VM Pods use
the separate `qrab-manager` identity without a mounted API token. Kubernetes
GitOps preserves both ServiceAccount UIDs and installs scoped pull/use guards.
Install that identity first, then apply the release-pinned manifests:

```sh
kubectl apply -k contrib/kubernetes/operator
kubectl wait --for=condition=Established crd/virtualmachines.qrab.yapplesauce.com
kubectl -n qrab rollout status deployment/qrab-operator --timeout=180s
kubectl apply -k contrib/kubernetes
kubectl -n qrab get qvm,pod,pvc
bash contrib/kubernetes/check.sh
```

For the first migration only, remove the old manually managed `qrab-demo` Pod
before applying the CR. The operator refuses to adopt resources with another
owner UID, including an unowned Pod with the same name. Do not delete or recreate
the namespace or image-pull ServiceAccounts.

The [example](../contrib/kubernetes/vm.yaml) provides a retained 1 GiB filesystem
PVC on `zfs-block` and a VM CR:

```yaml
apiVersion: qrab.yapplesauce.com/v1alpha1
kind: VirtualMachine
metadata:
  name: qrab-demo
  namespace: qrab
spec:
  stateVolumeClaim: qrab-demo-state
  memoryMiB: 512
  cpus: 1
  running: true
  bootDisk: /opt/qrab/demo/boot.img
  console:
    publicUrl: https://qrab-demo.yapplesauce.com
    allowedSubjects: [6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b]
```

`image` may optionally override the operator's default manager-image digest in
the enrolled `zot.yapplesauce.com/qrab/manager` repository. The bundled ESP boot disk is fixed to `/opt/qrab/demo/boot.img`. CRs cannot
expose mounted credentials, TPM state or arbitrary container files as disks. The demo is a GPT/FAT ESP with
a systemd-stub UKI, kernel, command line and initramfs; it is a RAM-only terminal
guest, not a desktop or persistent OS disk. There is no BIOS/direct-kernel
fallback for operator VMs. Future guest disk/storage choices can extend this
small API without exposing arbitrary Pod specifications.

The console public URL must have a matching Service/Ingress, TLS certificate and
exact registered Kanidm callback. See [bmc.md](bmc.md). The operator does not
mutate the identity provider. The sample's existing URL and Sam allowlist remain
valid.

## Power, restart and deletion

```sh
kubectl -n qrab patch qvm qrab-demo --type merge -p '{"spec":{"running":false}}'
kubectl -n qrab patch qvm qrab-demo --type merge -p '{"spec":{"running":true}}'
kubectl -n qrab patch qvm qrab-demo --type merge -p '{"spec":{"restartNonce":"restart-1"}}'
kubectl -n qrab get qvm qrab-demo -o yaml
```

`running:true` means the operator restores an exited or deleted Pod. BMC
shutdown ends the current instance; the operator starts another after a short
backoff. Use `running:false` to keep the VM off. Stop/restart and spec changes
reap the old instance before its replacement; they retain firmware variables,
TPM state and its endorsement key. ConfigMap drift is repaired and the Pod is
replaced so its effective config converges. Required Pod fields, image and
state mounts are checked without treating Kubernetes defaults as drift.

Deleting a CR garbage collects its owned Pod/ConfigMap; **the PVC remains**.
`stateVolumeClaim` is immutable and status pins the PVC UID. A replacement PVC
with the same name is rejected. The runtime binds state to the immutable CR UID
as well as its name: creating a new CR over an old identity volume fails closed.
Restoring/migrating an identity to a new CR is a deliberate recovery operation,
not an automatic adoption path. Never copy a TPM volume to create another VM:
that duplicates its cryptographic identity. The operator has no PVC deletion
permission and does not reset missing/corrupt initialized TPM state.

Use `ReadWriteOncePod` for state where the driver supports it; single-node
`ReadWriteOnce` is also accepted. Each volume has a runtime owner lock, and
identity files are owner-only. `fsGroupChangePolicy: OnRootMismatch` prevents
routine remounts from broadening retained private-file access.

Status reports `observedGeneration`, `phase`, `ready`, `reason`, `message`, Pod
name/UID/node and the state PVC UID. Ready means the manager/QEMU/swtpm/BMC
processes are healthy, including a deliberately paused VM; guest boot evidence
is checked separately. Invalid specs and ownership conflicts are reported as
Blocked rather than mutating unrelated resources.

## UEFI, TPM and SPIRE

Per-VM writable NVRAM and TPM state live under `/var/lib/qrab/identity` on the
PVC. OVMF CODE and its initial VARS template come from the published image;
subsequent boots reuse retained VARS. QMP, VNC and swtpm control use private Unix
sockets. Both child processes are supervised; a TPM crash fails the VM rather
than leaving a running guest without its identity device.

The guest exposes `/sys/firmware/efi`, `/dev/tpmrm0`, populated SHA256 PCRs and
an EFI event log. Its serial log records an EK public fingerprint that can be
compared across restarts. This is measured boot evidence, not a claim that a
PCR policy is enforced or that Secure Boot is enabled. See [spire-vm.md](spire-vm.md)
for the exact future DevID enrollment/trust/transport contract. Actual SPIRE
node authentication is not claimed while the guest has no server transport or
trusted DevID credentials.

## Build and verification

The normal default tests and strict Clippy include the operator. The CRD is
generated from its Rust API and checked for drift:

```sh
cargo run --locked -p qrab-operator -- crd > contrib/kubernetes/operator/crd.json
bash contrib/check-runtime.sh
```

Build/publish `qemu`, then `manager`, then `operator` with the scripts under
`contrib/images`; commit source first, publish, then pin all immutable references
and the manager default in the operator Deployment. Each image records its exact
source revision. No QEMU source is downloaded from upstream during these builds.

The explicit `manager/tests/pod_uefi.rs` test checks two UEFI boots with retained
EK identity, event-log availability, absence of a guest NIC, shutdown/reaping,
a crashed TPM child, and malformed retained TPM refusal. Live rollout checks
also compare EK fingerprints across a real controller-driven Pod replacement.

The disposable demo lifecycle check is intentionally disruptive (Pod replacement,
config repair, stop/start and a rejected identity-adoption probe), and never
deletes its PVC:

```sh
python3 contrib/tests/operator-lifecycle.py --namespace qrab --vm qrab-demo
```

Native guest disks use QEMU/libiscsi with Secret file references and a pinned
storage node. Their drain/fence lifecycle is described in [iscsi.md](iscsi.md).
`spec.disks` entries are provisioned, grown, exported and fenced through the
storage service; the operator then owns `<vm>-disk-<id>` CHAP Secrets (RBAC
`secrets: get, create, patch`) and needs the opt-in
`contrib/kubernetes/operator/storage-api` component. Without it such VMs
report `StorageServiceUnavailable`.
The disposable legacy lifecycle script is intentionally limited to VMs without
native disks; failed native probes require hard-fence recovery.
