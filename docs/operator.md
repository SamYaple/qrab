# Minimal VM operator

`qrab-operator` reconciles namespaced `VirtualMachine` resources in
`qrab.yapplesauce.com/v1alpha1`. It owns one Pod and ConfigMap per VM. Each Pod
runs `qrab-manager` (QEMU, OVMF UEFI and swtpm TPM2) with guest networking
disabled, plus the optional BMC and Redfish sidecars. No guest Kubernetes token, VIF, arbitrary Pod template,
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

## Components

The `manager` container always owns the VM: `qrab-manager run` supervises QEMU
and swtpm and serves the control socket. The browser BMC and the Redfish API
are optional sidecars in the same image:

```yaml
spec:
  console:                       # required while components.bmc is enabled
    publicUrl: https://qrab-demo.yapplesauce.com
    allowedSubjects: [6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b]   # BMC admins
    viewerSubjects: []                                         # read-only BMC users
  components:
    bmc:
      enabled: true              # default; existing CRs keep the BMC
    redfish:
      enabled: false             # default
      adminSubjects: []          # may read and ComputerSystem.Reset
      viewerSubjects: []         # read-only
```

| components | Pod |
| --- | --- |
| BMC only (default) | `manager` + `bmc` sidecar on port 8080 |
| BMC and Redfish | `manager` + `bmc` (8080) + `redfish` (8081) |
| Redfish only | `manager` + `redfish` (8081); `console` may be omitted |
| none | `manager` alone; the VM runs with no HTTP surface |

Sidecars are native sidecars (`initContainers` with `restartPolicy: Always`):
they start before the manager, keep serving while it stops the VM, restart on
their own failure without touching the VM, and do not keep the Pod alive after
the manager exits. On SIGTERM they exit 0 within about two seconds, so the
iSCSI clean-release check still sees an orderly stop. The operator replaces a
Pod whose sidecars differ from the rendered set.

The manager serves the control socket at
`/run/qrab-control/manager/control.sock` on a separate 1 MiB in-memory
`control` volume (`control_socket: true`, `control_dir` in `vm.json`). Sidecars
mount only that volume. They never see the `runtime` volume with the QMP, VNC
and swtpm sockets, the identity PVC, iSCSI Secrets or the VM config. All
containers run as UID/GID 10001 (Pod `securityContext`), and the socket is 0600
in a 0700 directory with a same-UID peer check. The mount layout, not the UID, is
what keeps sidecars away from QMP/VNC. No group-based (0660) socket access is
used.

The manager container receives `QRAB_NODE_NAME`, `QRAB_POD_NAME`,
`QRAB_POD_NAMESPACE` (downward API) and `QRAB_IMAGE`; admins see them in the
configuration view. Its startup, readiness and liveness probes run
`qrab-manager live`, so a paused VM stays reachable. Sidecars have readiness on
`/readyz` (the manager's live probe over the socket) and liveness on `/healthz`,
and no startup probe, which would delay the manager's start.

The BMC sidecar receives `BMC_PUBLIC_URL`, `BMC_OIDC_ISSUER`,
`BMC_OIDC_CLIENT_ID`, `BMC_ALLOWED_SUBJECTS` and, when set,
`BMC_VIEWER_SUBJECTS`. The Redfish sidecar receives `REDFISH_OIDC_ISSUER`,
`REDFISH_OIDC_AUDIENCE` and the non-empty `REDFISH_ADMIN_SUBJECTS` /
`REDFISH_VIEWER_SUBJECTS`. The operator's own environment selects the issuers:

| Operator variable | Default |
| --- | --- |
| `QRAB_OIDC_ISSUER` / `QRAB_OIDC_CLIENT_ID` | `https://idm.yaple.net/oauth2/openid/qrab-bmc` / `qrab-bmc` |
| `QRAB_REDFISH_OIDC_ISSUER` / `QRAB_REDFISH_OIDC_AUDIENCE` | `https://idm.yaple.net/oauth2/openid/qrab-redfish` / `qrab-redfish` |
| `QRAB_IDP_IP` | optional `hostAliases` IP for the enabled components' issuer hosts |

Changing components, subjects or this layout changes the Pod template hash, so
the operator replaces the VM Pod while keeping its identity. Service, Ingress
and network-policy entries for port 8081 are explicit infrastructure, like the
console's; see [redfish.md](redfish.md).

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
name/UID/node, the state PVC UID and in-place `resize` progress. Ready means the manager/QEMU/swtpm processes and the enabled
sidecars are healthy, including a deliberately paused VM; guest boot evidence
is checked separately. Invalid specs and ownership conflicts are reported as
Blocked rather than mutating unrelated resources.

## Resizing a running VM

The VM container is sized from the spec: requests `cpus` / `memoryMiB + 256Mi`,
limits `cpus + 1` / `memoryMiB + 512Mi`, with `resizePolicy: NotRequired` for
cpu and memory. The Pod records the size QEMU booted with in the
`qrab.yapplesauce.com/boot-size` annotation; its ConfigMap and template hash
stay at that boot size, so a size change is not a template change.

```sh
kubectl -n qrab patch qvm qrab-demo --type merge -p '{"spec":{"memoryMiB":1024}}'
kubectl -n qrab get qvm qrab-demo -o jsonpath='{.status.resize}'
```

- **Growth within the boot maxima** of a Running Pod patches `pods/resize`
  (strategic merge of the manager container's resources, with `metadata.uid` as
  a precondition). No Pod restart. The manager then hot-adds memory/vCPUs
  ([manager.md](manager.md#in-place-growth-hotplug)) once the kubelet has
  applied the new cgroup limits.
- **Shrink**, growth beyond `maxCpus`/`maxMemoryMiB`, a change of either ceiling,
  or a size change before the Pod is Running takes the existing controlled
  restart path (reason `ResizeRequiresRestart`; firmware/TPM identity retained).
  The guest is never shrunk in place, even though Kubernetes would apply a lower
  memory limit. Lowering a still-Deferred request also restarts: the kubelet may
  admit and apply the larger size (and the manager plug it) concurrently.
- `maxCpus` / `maxMemoryMiB` are optional, boot-time hot-plug ceilings, validated
  by CEL to be ≥ `cpus`/`memoryMiB`. Defaults: 4× the booted size (vCPUs at most
  64; memory at most +64 GiB). **The operator renders TCG, and Linux refuses
  hot-added vCPUs under TCG**, so the effective vCPU ceiling is the booted count
  and CPU growth restarts; memory growth is in place. The vCPU ceiling applies
  once an accelerator that guests recognise (KVM) is rendered.

`status.resize` reports `phase` (Applied, Proposed, InProgress, Deferred,
Infeasible, Error), the kubelet `message`, requested and applied guest sizes,
and the boot size and ceilings; `kubectl get qvm` shows it in the `Resize`
column. Phases come from `PodResizePending` (Deferred/Infeasible) and
`PodResizeInProgress` conditions, `observedGeneration` and the container
status resources. Kubernetes 1.37 rejects a request beyond the node's
allocatable capacity synchronously (HTTP 403); that is reported as Infeasible
and retried every reconcile. A Deferred resize is retried by the kubelet. The
only RBAC addition is `patch` on `pods/resize`. Existing VMs restart once when
an operator with this template (resizePolicy, downward-API `size` volume)
first reconciles them.

Live check on Kubernetes v1.37.1 / containerd 2.4.1, scratch namespace
`qrab-resize-test` (deleted afterwards), 2026-11-11. A plain Pod using the
cached `docker.io/library/python@sha256:0741d101…` image, the control-plane
toleration, `resizePolicy` NotRequired, requests 1/768Mi and limits 2/1Gi
(cgroup `cpu.max 200000 100000`, `memory.max 1073741824`):

```sh
kubectl -n qrab-resize-test patch pod resize-probe --subresource resize \
  -p '{"spec":{"containers":[{"name":"probe","resources":{"requests":{"cpu":"2","memory":"1280Mi"},"limits":{"cpu":"3","memory":"1536Mi"}}}]}}'
```

- Within 1 s `metadata.generation`/`status.observedGeneration` went 2/1 → 2/2,
  container status resources became 3/1536Mi, the container saw
  `cpu.max 300000 100000` and `memory.max 1610612736`, `restartCount` stayed 0
  and `startedAt` was unchanged.
- `--type merge` is refused (`only cpu and memory resources are mutable`,
  because it replaces the container list); use a strategic merge patch.
- cpu 500 on a 12-CPU node: refused synchronously with 403 `node didn't have
  enough allocatable resources: cpu`; nothing changed.
- memory 6Gi on a node with 95% memory requested: accepted, then
  `PodResizePending` reason `Deferred` (`Node didn't have enough resource:
  memory, requested: 6442450944, used: 46495956992, capacity: 50312679424`);
  the cgroup stayed at 1.5 GiB. Reverting the request cleared the condition.
- A memory *limit* decrease (2Gi → 1536Mi) is applied in place by the kubelet,
  which is why the operator never sends one for a running guest.
- Downward-API `limits.cpu` (divisor `1m`) and `limits.memory` (divisor `1Mi`)
  files, mounted read-only at `/run/qrab-size` (mode 0444, UID 10001,
  read-only root), refreshed about 1 s after the patch — once observed *before*
  `cpu.max` changed — and also showed the desired 6144 MiB while the resize was
  Deferred (cgroup still 2 GiB). Hence the manager gates on the cgroup.
- `metadata.uid` in a resize patch acts as a precondition (`metadata.uid:
  field is immutable` on mismatch); labels/annotations in it are ignored.

The rendered VM Pod is checked with the ignored tests (namespace needs a
`qrab-manager` ServiceAccount for the dry run; both passed in the scratch
namespace):

```sh
QRAB_TEST_NAMESPACE=qrab-resize-test \
QRAB_TEST_IMAGE=docker.io/library/python@sha256:0741d101873c12ab927e6f8653feb8862b9bd58771177acb1b885b95141f91b4 \
  cargo test -p qrab-operator --test resize --test contracts -- --ignored
```

They verify that API-server defaulting of the new template does not cause a
restart loop and that the operator's own resize patch reaches `Applied` with
`restartCount` 0. The guest-side hot-plug of the real VM Pod is verified at the
integration rollout.

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
