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
configuration view. Pods carry `kubectl.kubernetes.io/default-container:
manager`, so unqualified `kubectl exec`/`logs` reach it. Its startup, readiness and liveness probes run
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
  64; memory at most +64 GiB). **Linux refuses hot-added vCPUs under TCG**, so
  a TCG VM's effective vCPU ceiling is its booted count and CPU growth restarts;
  memory growth is in place. `maxCpus` applies to `accelerator: kvm` VMs
  (see [KVM acceleration](#kvm-acceleration)).

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

## KVM acceleration

`spec.accelerator` is `tcg` (default; absent in stored CRs, and TCG template
hashes are unchanged by the field) or `kvm`. A `kvm` VM renders
`"accelerator":"kvm"` (QEMU `-accel kvm -cpu host`), honours `maxCpus` for
in-place vCPU growth, and requests and limits one
`qrab.yapplesauce.com/kvm` extended resource. The VM Pod stays restricted:
no privileged container, capability, hostPath or supplemental host group.
It pins `hostUsers: true`, and the operator refuses a KVM Pod spec with
`hostUsers: false`: in a user namespace runc bind-mounts the host's 0660 node
instead of creating the CDI one, so UID 10001 could not open `/dev/kvm`.
Switching the accelerator restarts the VM. Without the device plugin on a node,
a `kvm` Pod stays Pending (`Insufficient qrab.yapplesauce.com/kvm`).

`qrab-kvm-device-plugin` (`projects/kvm-device-plugin`, shipped in the
operator image) is a kubelet v1beta1 device plugin. It advertises
`QRAB_KVM_DEVICES` (default 32) shareable slots per node, Healthy while
`/sys/class/misc/kvm/dev` exists. It writes the CDI spec
`/var/run/cdi/qrab.yapplesauce.com-kvm.json` (`/dev/kvm`, char major:minor from
sysfs, `fileMode` 0666 inside the container) and answers Allocate with only the
CDI device `qrab.yapplesauce.com/kvm=kvm`; containerd 2.x (CDI on by default) creates
the node in that container alone, so UID 10001 can open it without the host's
`kvm` group (gid 992, mode 0660). It re-registers after a kubelet restart.
Its DaemonSet ([daemonset.yaml](../contrib/kubernetes/kvm-device-plugin/daemonset.yaml))
runs as UID 0 to own sockets in the kubelet's root-owned directory, but is not privileged,
drops all capabilities, has a read-only root and mounts only
`/var/lib/kubelet/device-plugins` and `/var/run/cdi`. It needs a namespace with
privileged PSA for those hostPaths and lands through kubernetes-gitops.

Node evidence, 2026-11-12 (read-only, scratch namespace `qrab-kvm-probe`,
deleted afterwards): kube01–03 are KVM guests on an AMD EPYC 9354 host
(`kvm_amd` `nested=1`), started as `qemu-system-x86_64 -cpu host -smp 12
-m 48G`. From a restricted Pod on each node: `/sys/module/kvm_amd/parameters/nested`
= 1, `/sys/class/misc/kvm/dev` = `10:232`, 12 of 12 CPUs report `svm`, kernel
7.0.0-38-generic. `kubectl debug node/<n> --profile=general` shows
`/dev/kvm` as `crw-rw---- root:kvm(992)`, `kvm:x:992:` in `/etc/group`, only
`kubelet.sock` in `/var/lib/kubelet/device-plugins`, no `/etc/cdi` or
`/var/run/cdi`, and no CDI override in `/etc/containerd/config.toml`. No node advertises
a KVM resource yet. A VM Pod would run as a nested (L2) guest.

Live evidence, 2026-11-12, after the kubernetes-gitops DaemonSet (#403, operator
`3b85ece6…`) and the node pull bindings: kube01–03 each allocate 32
`qrab.yapplesauce.com/kvm`. The host node stays `crw-rw---- 0:992`. A disposable
VM `qrab-kvm-test` (`accelerator: kvm`, `cpus: 1`, `maxCpus: 4`, no disk or
BMC; deleted with its PVC afterwards) ran on kube03 with QEMU
`-accel kvm -cpu host -smp cpus=1,maxcpus=4` as UID 10001 in the host user
namespace, and saw `/dev/kvm` as `crw-rw-rw- 10001:10001`. containerd 2.4
therefore creates the CDI node with its `fileMode`, so no supplemental group is
rendered. Patching `cpus` 1→2→4 resized the same Pod UID in place, with 0
restarts and a CPU limit of 3 and then 5. The guest reported `QRAB_GUEST_RESOURCES cpus=2`
and then `cpus=4`. `cpus: 5` is rejected by the CRD (`cpus must not exceed maxCpus`).

The generated `projects/kvm-device-plugin/src/v1beta1.rs` is checked against a
fresh tonic-prost-build 0.14.6 run (default configuration, needs `protoc`):
`cargo test -p qrab-kvm-device-plugin --test regenerate -- --ignored`.

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
`secrets: create, patch, delete`, write-only and confined by
`secret-policy.yaml`) and needs the opt-in
`contrib/kubernetes/operator/storage-api` component. Without it such VMs
report `StorageServiceUnavailable`.
The disposable legacy lifecycle script is intentionally limited to VMs without
native disks; failed native probes require hard-fence recovery.

### VM-platform rollout record (2026-11-12)

Images built from main 503ea0e: manager
`zot.yapplesauce.com/qrab/manager@sha256:b6b8f19bd2e6fc10401addcecd0845a3ff8fcdb6534b29981bf2f53ba6fb2e0f`
(pin b7e2d00), operator
`zot.yapplesauce.com/qrab/operator@sha256:373cd49262f883b218421287f84254ca2fc22f9951cb5f92cce503cfc1f0e9d5`
(pin ac8b13c). The QEMU image was reused unchanged. One window applied the CRD,
RBAC, the `qrab-operator-disk-secrets` admission policy and binding, the
operator Deployment and `console.yaml` (Service `targetPort: 8080`). The
storage-api component, the KVM device plugin and the demo patch were not
applied.

The operator replaced qrab-demo by clean drain: old Pod ccbe3953 was recorded
in `storageReleasedPodUid`, new Pod c544ae20 is on kube03 with the same state
PVC 67cba3ee, and there was no storage recovery. Live checks on the new Pod:

| Check | Result |
| --- | --- |
| `contrib/kubernetes/check.sh` | PASS, including the published digests and private state |
| TPM EK | `ec61ceb9…e844`, unchanged |
| Guest NIC | absent (`QRAB_GUEST_NO_NIC_OK`) |
| Pod layout | `bmc` native sidecar (`restartPolicy: Always`); default container `manager` |
| iSCSI disk | native libiscsi blockdev with the same target and VM initiator, CHAP secret file, `qrab-probe` serial, read-write |
| Guest agent | connected (11.1.3); ping, freeze → `frozen`, thaw → `thawed`; `update-trigger channel=stable` succeeded, exit 0, `QRAB_UPDATE_HOOK_OK` |
| In-place memory | `memoryMiB` 512 → 768: `status.resize.phase=Applied`, Pod UID unchanged, restartCount 0, container limit 1Gi → 1280Mi, `guest_resized memory_mib=768`, guest reports `memory_kib=663088` |
| RSS at 768 MiB guest | manager 4 MiB, QEMU 453 MiB, container cgroup 445 MiB of 1280 MiB |

Not verified live in this window: iSCSI filesystem content (the test-only
probe copy into the Pod was not run), the BMC viewer role (no
`console.viewerSubjects`), Redfish (off; its Kanidm client now exists but
port 8081 has no Service/Ingress/Cilium path), vCPU growth (needs KVM) and
`spec.disks` provisioning (needs storage activation).

### Storage, KVM and Redfish rollout record (2026-11-12, later)

The earlier "not verified" items are now covered:

- **iSCSI content.** On c544ae20 the probe verify reported filesystem UUID `b7f594a0…`
  and token SHA-256 `7cbc00f2…6005`, the same as before the rollout.
- **Storage activation.** It finished after the kubernetes-gitops identity PR
  (#404). The operator storage-api component was then applied with operator
  `3b85ece6…`, and qrab-demo kept its Pod.
- **First storage check.** `operator-storage-api/check.sh` showed that online
  growth never reached QEMU. Every `query-named-block-nodes` failed to decode
  the `format-specific: file` of the pflash nodes, a QAPI shared-wire-name bug
  fixed in 0607fc5. The check had reported a false ok.
- **KVM.** The KVM device plugin DaemonSet (#403) and the node pull bindings
  were applied. The live KVM evidence is recorded [above](#kvm-acceleration).

Images rebuilt from 0d143c1 and pinned in 123e3b9:

- manager `zot.yapplesauce.com/qrab/manager@sha256:2458ab2a3d7854551debf22ec7718c669374cb56b2cc1086e03f51f62a1c9709`
- operator `zot.yapplesauce.com/qrab/operator@sha256:60b28e68c011afed4a90b927bc8eaee16d4343fda4c0de89ee5d89212b823cab`

kubernetes-gitops re-pinned the ClusterSPIFFEID and the DaemonSet to that
operator before it was applied. One apply covered three things: `console.yaml`
(Redfish `/redfish` route, Service port 8081, Cilium 8081), the
storage-api operator, and `spec.components.redfish` on qrab-demo with the
console's admin subject. That gave one clean drain, c544ae20 → b031b6d1.

| Check | Result |
| --- | --- |
| `contrib/kubernetes/check.sh` | PASS, including protected Redfish: 401 + `WWW-Authenticate: Bearer` on `/redfish`, the service root, `Systems/1` and `Reset`, with no token and with a malformed one |
| TPM EK | `ec61ceb9…e844`, unchanged |
| iSCSI content | `QRAB_DISK_PROBE_VERIFIED`, same UUID and token hash |
| Pod layout | native `bmc` and `redfish` sidecars |
| `operator-storage-api/check.sh` (VM 6ab10794) | PASS: Active gen 1; online growth to 128 MiB with `disk_grown` in the same Pod; shrink refused; removal fenced and its Secret deleted; re-add rotated to gen 2; deletion garbage-collected the Secret |
| `operator-storage-api/clone-check.sh qrab-test 1` (VM c97c60a9) | PASS: clone Active and VM Running at 128 MiB; baseImage change refused; deletion fenced the clone and garbage-collected its Secret. Before deletion, `disk-c97c60a9…-root` had origin `image-qrab-test-g1@sealed`, and the snapshot held `qrab-image` plus `qrab-clone-c97c60a9-217d-4acb-ba73-ac911673fcd3.root` (userrefs 2) |

Completed afterwards:

- `redfish-live.py` as an admin: a Kanidm ID token over HTTPS read
  `Systems/1` (PowerState On, Health OK), and `Reset` Pause then Resume
  reached Paused then On. qrab-demo kept Pod b031b6d1 with 0 restarts.
- Root retired the acceptance zvols and the clone `c97c60a9` (LIO objects
  removed, data retained). `qrab-test` g1 remains published.
- The host storage ingress runs spiffe-kanidm-shim 4078a3e (610 s fence
  timeout).

Still open: the viewer-role 403 check (`redfish-live.py --expect viewer`)
needs a second, non-admin account.
