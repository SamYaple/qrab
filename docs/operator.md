# Minimal VM operator

`qrab-operator` reconciles namespaced `VirtualMachine` resources in
`qrab.yapplesauce.com/v1alpha1`. It owns one Pod and ConfigMap per VM, plus
the NIC anchor Pods, CiliumNetworkPolicies and disk CHAP Secrets described below.
Each VM Pod runs `qrab-manager` (QEMU, OVMF UEFI and swtpm TPM2), plus the optional BMC and
Redfish sidecars. Guest NICs are opt-in (`spec.nics`); each is a kernel tap in a separate
NIC anchor Pod, so guest traffic never enters the VM Pod (see [Guest NICs](#guest-nics)). No guest Kubernetes token, VIF, arbitrary Pod template,
leader election, VM admission webhook or cluster-wide controller is involved.
Use one operator replica with `Recreate` deployment strategy.

PVCs, console Services/Ingresses/certificates and OIDC callback registration stay
explicit infrastructure resources. This keeps storage and identity retention
independent of controller garbage collection. The operator reads PVCs and
creates or deletes only a live migration's target state PVC, labelled for the
same VM UID ([migration.md](migration.md)); it cannot change VM specs or
manage another namespace.

## Create a VM

The operator uses the registered `qrab-operator` image-pull identity; VM Pods use
the separate `qrab-manager` identity without a mounted API token. Kubernetes
GitOps preserves both ServiceAccount UIDs and installs scoped pull/use guards.
Install that identity first, then apply the release-pinned manifests. Each
bundle and its apply mode is listed in
[contrib/kubernetes/README.md](../contrib/kubernetes/README.md). The root
kustomization is the platform (the operator with its storage client, and
monitoring); VMs are applied from their own directories, so a platform apply
never creates or retargets a VM:

```sh
kubectl apply -k contrib/kubernetes
kubectl wait --for=condition=Established crd/virtualmachines.qrab.yapplesauce.com
kubectl -n qrab rollout status deployment/qrab-operator --timeout=180s
kubectl -n qrab get qvm,pod,pvc
bash contrib/kubernetes/check.sh qrab <vm>   # for a console VM you created
```

With the storage service enabled (`contrib/kubernetes/operator-storage-api`,
as on this cluster), always update the operator through the root kustomization
or `kubectl apply -k contrib/kubernetes/operator-storage-api`. Applying
`contrib/kubernetes/operator` or `operator/deployment.yaml` on its own removes
the storage client (`QRAB_STORAGE_*` and its volumes) from the Deployment.
Every VM with `spec.disks`, including dashboard OS launches, then sits
`Blocked` with `StorageServiceUnavailable`. `contrib/kubernetes/operator` is
only for a cluster without the storage service.

The operator refuses to adopt resources with another owner UID, including an
unowned Pod with the same name. Do not delete or recreate the namespace or
image-pull ServiceAccounts.

The [qrab-demo sample](../contrib/kubernetes/examples/qrab-demo/vm.yaml) provides
a retained 1 GiB filesystem PVC on `zfs-block` and a VM CR. The qrab-demo VM
itself was retired, and leftover live console objects may still carry its
names, so copy and rename the sample before applying it
([why](../contrib/kubernetes/README.md#examplesqrab-demo)):

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
are sidecars in the same image. The BMC's UI manages the VM through Redfish
([bmc.md](bmc.md#redfish-proxy)), so Redfish is always rendered beside the
BMC:

```yaml
spec:
  console:                       # required while components.bmc is enabled
    publicUrl: https://qrab-demo.yapplesauce.com   # optional with dashboard bearer access
    allowedSubjects: [6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b]   # BMC admins
    viewerSubjects: []                                         # read-only BMC users
    adminGroups: []                                            # Kanidm group UUIDs
    viewerGroups: []
  components:
    bmc:
      enabled: true              # default; existing CRs keep the BMC
    redfish:
      # enabled: absent follows components.bmc.enabled; false is refused
      # while the BMC is enabled. Set true for Redfish without the BMC.
      adminSubjects: []          # may read and ComputerSystem.Reset
      viewerSubjects: []         # read-only
      adminGroups: []
      viewerGroups: []
```

### BMC users in Redfish

While the BMC is enabled the operator renders Redfish's lists as the union of
`components.redfish` and the console's: `console.allowedSubjects` and
`console.adminGroups` join `REDFISH_ADMIN_SUBJECTS` / `REDFISH_ADMIN_GROUPS`,
and `console.viewerSubjects` and `console.viewerGroups` join the viewer
lists. The BMC proxy forwards each user's own ID token, so a BMC admin or
viewer holds the same role in Redfish without being listed twice. Each
source's admin and viewer lists must still be disjoint; across the two
sources admin wins (a console admin listed as a Redfish viewer is a Redfish
admin), so the rendered lists stay disjoint. `components.redfish`'s own lists
add Redfish-only grants, for example for direct Redfish clients. Without the
BMC the console's lists grant nothing in Redfish.

### Access through Kanidm groups

Each VM may name a Kanidm admins group and a viewers group, by UUID, for the
console and for Redfish. Membership is then managed in Kanidm, for example
from the idm-dashboard person page ("Virtual machines"). It takes effect at
the person's next BMC login (sessions last at most 15 minutes) or next Redfish
token, without restarting the VM. Changing the CR's lists does restart it once.

The convention, which the idm-dashboard relies on to discover a VM's groups,
is `qrab_vm_<namespace>_<vm>_admins` and `qrab_vm_<namespace>_<vm>_viewers`.
Each group is a member of `qrab_vm_users`, the group scope-mapped to
`openid profile groups_uuid` on the `qrab-bmc` and `qrab-redfish` clients. As
a Kanidm administrator, create them and print the patch for the CR with:

```sh
bash contrib/kubernetes/vm-access-groups.sh create <namespace> <vm>
bash contrib/kubernetes/vm-access-groups.sh check <namespace> <vm>
```

Kanidm names are at most 64 characters, so `<namespace>_<vm>` must fit in 47.

### The creator is a viewer

The admission policies in `contrib/kubernetes/operator/creator-policy.yaml`
record the Kanidm subject that created a VM in the annotation
`qrab.yapplesauce.com/creator-subject`. They replace any client-supplied value
and refuse any later change. The operator adds that subject to the BMC and
Redfish viewers unless it is already an admin, so the creator can always
watch the VM; only explicit grants allow power actions and the console. VMs
created by certificate or ServiceAccount identities, such as Flux, have no
creator. The dashboard's ServiceAccount
(`system:serviceaccount:qrab:qrab-dashboard`) is the one exception: it creates
VMs for its signed-in user, so it must set the annotation itself to that
user's lowercase hyphenated subject UUID. Admission keeps its value and refuses
a missing or malformed one. The annotation stays immutable for everyone.
`contrib/kubernetes/operator/check-creator-policy.sh` checks the policies live
with server-side dry runs.

### Dashboard access and publicUrl

The dashboard (Kanidm client `qrab-dashboard`) is meant to be the cluster's
only browser OIDC client. It calls each VM's Redfish service (Pod IP, port
8444) for status, events and power and its BMC console (port 8443) server-side,
and forwards the signed-in user's ID token as `Authorization: Bearer`. With
`QRAB_BMC_BEARER_ISSUER` and `QRAB_BMC_BEARER_AUDIENCE` set, every BMC sidecar
receives `BMC_BEARER_ISSUER` and `BMC_BEARER_AUDIENCE` and Redfish lists the
pair in `REDFISH_TRUSTED_TOKENS`; both verify those tokens themselves. The VM's
subject and group lists decide that user's role, exactly as for the BMC login
([bmc.md](bmc.md#dashboard-bearer-tokens), [above](#bmc-users-in-redfish)).

`console.publicUrl` is then optional. Without it the BMC gets no
`BMC_PUBLIC_URL` or `BMC_OIDC_*`: it has no login, browser UI, Service,
Ingress, certificate or Kanidm callback, and is reachable only through the
dashboard. The operator refuses a BMC-enabled VM with neither a `publicUrl` nor
its own bearer settings, since that BMC would have no way in. With a
`publicUrl`, the per-VM browser console works as before, and the BMC also
accepts dashboard tokens. Network policy must admit the dashboard's Pods to
workload ports 8443 and 8444 of the VM Pods.

| components | Pod |
| --- | --- |
| BMC (default) | `manager` + `bmc` (HTTP8080, mTLS8443) + `redfish` (HTTP8081, mTLS8444; browser bridge uses loopback mTLS; metrics9181) |
| Redfish only (`bmc.enabled: false`, `redfish.enabled: true`) | `manager` + `redfish` (HTTP8081, mTLS8444, metrics9181); `console` may be omitted |
| none (`bmc.enabled: false`) | `manager` alone; the VM runs with no HTTP surface |

The CRD refuses `components.redfish.enabled: false` while the BMC is enabled.
Objects written under earlier CRDs store `enabled: false` (the old default);
the operator renders Redfish for them anyway, and CRD validation ratcheting
keeps admitting updates that leave `components` unchanged, such as the
dashboard's `spec.running` patch. Changing `components` requires removing the
stale value: `kubectl patch qvm <vm> --type=json -p
'[{"op":"remove","path":"/spec/components/redfish/enabled"}]'`.

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
`qrab-manager live`, so a paused or powered-off VM stays reachable. Sidecars have readiness on
`/readyz` (the manager's live probe over the socket) and liveness on `/healthz`,
and no startup probe, which would delay the manager's start.

The BMC sidecar receives `BMC_PUBLIC_URL`, `BMC_OIDC_ISSUER` and
`BMC_OIDC_CLIENT_ID` when `console.publicUrl` is set, `BMC_BEARER_ISSUER` and
`BMC_BEARER_AUDIENCE` when the operator has bearer settings, and the non-empty
`BMC_ALLOWED_SUBJECTS`, `BMC_VIEWER_SUBJECTS`, `BMC_ADMIN_GROUPS` and
`BMC_VIEWER_GROUPS`, and `BMC_REDFISH_URL=https://127.0.0.1:8444` with an
exact `BMC_REDFISH_SPIFFE_ID` for this VM. Each sidecar also mounts the
read-only Workload API socket and asserts its container-scoped
`QRAB_EXPECTED_SPIFFE_ID`. BMC8443 admits dashboard only; Redfish8444 admits
dashboard and this VM's BMC, with ordinary OIDC/role checks still required. The Redfish
sidecar receives `REDFISH_OIDC_ISSUER`, `REDFISH_OIDC_AUDIENCE` and the
non-empty `REDFISH_ADMIN_SUBJECTS` / `REDFISH_VIEWER_SUBJECTS` /
`REDFISH_ADMIN_GROUPS` / `REDFISH_VIEWER_GROUPS`
([merged with the console's](#bmc-users-in-redfish)). Its `REDFISH_TRUSTED_TOKENS` lists the BMC client
(`QRAB_OIDC_ISSUER|QRAB_OIDC_CLIENT_ID`) when the BMC is enabled with a
`console.publicUrl`, and the dashboard client
(`QRAB_BMC_BEARER_ISSUER|QRAB_BMC_BEARER_AUDIENCE`) when those are set, so
the BMC and dashboard can call Redfish with their users' ID tokens; the
Redfish role lists still decide access
([redfish.md](redfish.md#trusted-token-sources)). The operator's own
environment selects the issuers:

| Operator variable | Default |
| --- | --- |
| `QRAB_OIDC_ISSUER` / `QRAB_OIDC_CLIENT_ID` | `https://idm.yaple.net/oauth2/openid/qrab-bmc` / `qrab-bmc` |
| `QRAB_REDFISH_OIDC_ISSUER` / `QRAB_REDFISH_OIDC_AUDIENCE` | `https://idm.yaple.net/oauth2/openid/qrab-redfish` / `qrab-redfish` |
| `QRAB_BMC_BEARER_ISSUER` / `QRAB_BMC_BEARER_AUDIENCE` | unset; set together, e.g. `https://idm.yaple.net/oauth2/openid/qrab-dashboard` / `qrab-dashboard` |
| `QRAB_IDP_IP` | optional `hostAliases` IP for the enabled components' issuer hosts |

Changing components, subjects, the operator's bearer settings or this layout
changes the Pod template hash, so the operator replaces the VM Pod while
keeping its identity. Setting the bearer variables therefore restarts every
BMC- or Redfish-enabled VM once, and so does the operator release that made
Redfish mandatory beside the BMC (new `redfish` sidecar, `BMC_REDFISH_URL`,
merged role lists). The BMC's browser UI reaches Redfish over Pod loopback
through its `/bmc/redfish` session adapter, so the browser console needs no
Service, Ingress or policy entry for port 8081. Native Redfish clients do not
go through the BMC: the console Ingress routes the public `/redfish` prefix
straight to Service port `redfish` (8081), and the console's
CiliumNetworkPolicy admits 8081 from the ingress, as
`contrib/kubernetes/examples/qrab-demo/console.yaml` does. Without that route and
port a VM's Redfish is reachable only by the dashboard. A Redfish-only VM needs
the same explicit infrastructure; see [redfish.md](redfish.md#exposure).

The Redfish sidecar also declares port `metrics` (9181), its SPIFFE mTLS
Prometheus listener ([redfish.md](redfish.md#prometheus-export)). For each
Redfish-enabled VM the operator keeps an owned, additive CiliumNetworkPolicy
`<vm>-redfish-metrics`: it admits 9181 from Prometheus Pods
(`app.kubernetes.io/name=prometheus`) in the `monitoring` namespace and from
the operator's own Pods (`app.kubernetes.io/name=qrab-operator`, in the VMs'
namespace), and denies 9181 to every other endpoint and entity, without
enabling default deny or touching other ports. The operator reads only the
guest power state from it ([below](#power-restart-and-deletion)). It is deleted when Redfish is disabled. Adding the port
changes the Pod template, so the first operator rollout with it replaces each
Redfish-enabled VM Pod once. `contrib/kubernetes/monitoring/` holds the one
static PodMonitor and the Grafana dashboard ConfigMap.

For [metric history](redfish.md#metric-history) the Redfish sidecar also
receives `REDFISH_PROMETHEUS_URL=https://qrab-prometheus-history.monitoring.svc:9091`
and its own `QRAB_POD_NAME` / `QRAB_POD_NAMESPACE` (downward API), and the
operator keeps a second owned, additive CiliumNetworkPolicy
`<vm>-redfish-prometheus` selecting the VM Pod: egress to Prometheus Pods
(`app.kubernetes.io/name=prometheus`) in `monitoring` on SPIFFE mTLS TCP9091 and nothing
else, while explicitly denying the raw Prometheus9090 bypass. It never enables
default deny; the Service name resolves through the
VM's existing kube-dns egress (as in a console's `console.yaml`). It is deleted when
Redfish is disabled. The new environment changes the Pod template, so the
first operator rollout with it replaces each Redfish-enabled VM Pod once.

The console public URL must have a matching Service/Ingress, TLS certificate and
exact registered Kanidm callback. See [bmc.md](bmc.md). The operator does not
mutate the identity provider. A new console VM needs its own host, objects and callback
([sample](../contrib/kubernetes/examples/qrab-demo/console.yaml)).

### Admin bucket

Console features with data-exfiltration or device-injection risk are off by
default and opted into per VM:

```yaml
spec:
  console:
    adminBucket:
      audio: false          # guest audio playback (no microphone)
      usbRedirect: false    # browser USB redirection into the guest
```

Both default to false, and unknown fields are refused. When the BMC is enabled
and at least one feature is true, the BMC sidecar receives
`BMC_ADMIN_BUCKET` (for example `audio,usb-redirect`) and `vm.json` gains
`"admin_bucket": {"audio": true, "usb_redirect": false}`. With every feature
off, or the BMC disabled, neither is rendered, so the Pod template hash is
unchanged. Enabling or disabling a feature changes the hash and restarts the VM
once. `audio` adds an output-only Intel HDA device and VNC audio to QEMU;
`usbRedirect` is only recorded, since USB redirection is not built. Even when
enabled, only a console admin who requests a feature on a console connection
gets it; see [bmc.md](bmc.md#admin-bucket).

## Guest NICs

[networking.md](networking.md) is the full description. In short:

```yaml
spec:
  nics:
  - name: lan
    securityGroups: [web]
```

Guest NICs need `accelerator: kvm`, which the CRD enforces (the KVM device
brings qrab-netd's socket into the VM Pod), and stable NIC addresses
(`QRAB_NIC_ADDRESS_RANGE`, [Stable addresses](networking.md#stable-addresses)).
Without stable addresses the VM is `Blocked` with reason `InvalidSpec`.

- **Anchor Pods.** For each NIC the operator creates an anchor Pod
  `<vm>-nic-<name>-<node>`, controller-owned by the VM, which runs
  `qrab-manager anchor` (it waits for SIGTERM) as the `qrab-manager`
  ServiceAccount with no API token. It is restricted, with no ports or probes
  and, except for a VPN NIC's tunnel Secret, no volumes: its IP is the
  guest's (a VPN NIC's tunnel's outer address). It is annotated with the
  NIC's address pool
  (`ipam.cilium.io/ip-pool`, `ipam.cilium.io/require-pool-match: "true"`) and
  `qrab.yapplesauce.com/nic-datapath: tap`, which selects it for qrab-netd.
- **Anchor labels.** `app.kubernetes.io/component: vm-nic`,
  `app.kubernetes.io/instance: <vm>`, `qrab.yapplesauce.com/owner-uid`,
  `qrab.yapplesauce.com/nic: <name>`, and one
  `sg.qrab.yapplesauce.com/<group>: ""` per security group. Nothing is per
  Pod, so a second anchor of the same NIC shares its Cilium identity.
- **Default allow.** The operator creates no policy for a NIC, so **anchors
  are default-allow until a security-group or baseline policy selects them**.
- **Placement.** The anchor is created once the VM Pod is scheduled and is
  pinned to that node (`matchFields metadata.name`). A VM Pod on another node
  gets new anchors; anchors on other nodes are annotated
  `qrab.yapplesauce.com/retire-after` (20 s later) and then deleted.
- **Live migration.** Before the target VM Pod, the operator creates each
  NIC's anchor for the target node with `ipam.cilium.io/handoff-from` naming
  the source anchor and `qrab.yapplesauce.com/migration`, waits for its
  readiness gate, and gives the target `incoming.node_ip`. After the move it
  adds the NIC's pool to the new node's CiliumNode. A failed migration
  deletes them ([networking.md](networking.md#live-migration)).
- **Readiness and status.** qrab-netd builds the NIC's tap in the anchor's
  network namespace and sets the anchor's readiness gate
  `qrab.yapplesauce.com/nic-plumbed`. `status.nics` reports `name`, `mac`,
  `podName`, `ip` (the pool address), `address` and `ready` (the anchor's
  readiness).
- **Startup.** The VM Pod does not wait for anchors. Its config gets
  `nics: [{id, mac}]` and the seed's static addresses, and the manager waits
  for qrab-netd before starting QEMU. The MAC is `02:` plus five bytes of
  SHA-256 over the VM UID and NIC name.
- **VPN NICs.** With `vpn.secretName` (a Secret named `vpn-*`) the anchor
  mounts the Secret's `wg.conf` read-only at `/run/qrab-vpn` (mode 0400, so
  only qrab-netd, as root, reads it) and is annotated
  `qrab.yapplesauce.com/nic-vpn: wireguard`; the seed configures that NIC by
  DHCP ([networking.md](networking.md#vpn-nics)).
- **Changes.**
  - Adding or removing a NIC patches the VM ConfigMap without replacing the
    VM Pod: the template hash and the restart decision leave out `nics` and
    the seed's network-config, and KVM VMs with stable addresses get
    `nic_slots: 4` root ports, so the manager plugs NICs live
    ([networking.md](networking.md#hot-attach-and-detach)).
  - Security-group changes patch anchor labels in place.
  - A missing or failed anchor is recreated on the VM Pod's node with the
    same address.
  - Removing a NIC deletes its anchor and then releases its address.
    Deleting the VM stops its anchors before releasing their addresses.
- **Former relay policies.** A CiliumNetworkPolicy `<vm>-nic-<name>-relay`
  that the VM owns, left by the removed stream datapath, is deleted.
- **Selectors.** Anchors share `app.kubernetes.io/instance` with their VM.
  Anything meant for the VM Pod must also select `app.kubernetes.io/component:
  vm`, as `console.yaml`'s Service and policy do.
- **RBAC.** The operator needs `ciliumnetworkpolicies.cilium.io` get, list,
  watch, create, patch and delete in `rbac.yaml`, for the VM's Redfish and
  migration policies and the relay-policy cleanup.

## cloud-init seed

`spec.cloudInit` is opt-in. With it, the VM gets a read-only cloud-init NoCloud
disk labelled `cidata` and configures itself from it:

```yaml
spec:
  cloudInit:
    hostname: web-1          # DNS label; defaults to metadata.name
    userData: |              # verbatim, at most 32 KiB; default: empty #cloud-config
      #cloud-config
      ssh_authorized_keys: [ssh-ed25519 AAAA...]
```

- meta-data is generated: `instance-id` is the VM UID, so cloud-init runs its
  per-instance steps once for the lifetime of the CR, not on every Pod.
  `local-hostname` is `hostname` or the VM name. meta-data is written as a JSON
  object. cloud-init reads it as YAML, and JSON is a subset of YAML; CirrOS
  accepts only JSON. Names such as `no` or `0123` therefore stay strings.
- The operator puts the seed under `seed` in the VM ConfigMap's `vm.json`, and
  the manager builds the disk at each start (see manager.md). VMs without
  `cloudInit` render no seed; their ConfigMap and template hash are unchanged.
  Adding, changing or removing `cloudInit` changes the template hash and
  restarts the VM.
- **user-data is stored in the VM ConfigMap.** The operator owns that
  ConfigMap and only the VM Pod mounts it, but anyone who can read ConfigMaps
  in the namespace can read user-data, as with the CR itself. Use it for
  configuration and public keys. Do not put passwords or tokens there. The
  operator never copies user-data into status, events or logs, and its
  validation errors do not quote it. The BMC shows only its size.
- Validation: `hostname` must be a DNS label (1..63 lowercase letters, digits,
  hyphens), and `userData` must be at most 32 KiB.
- **Static network-config for NICs.** A VM with `spec.nics` always gets a
  seed, even without `cloudInit`. Its network-config v2 is rendered by
  `resources::seed_network_config`, with one entry per NIC:
  - the interface is matched by its guest MAC and named `ethN`;
  - the address is the NIC's stable address as /32;
  - the default route goes `on-link` via qrab-netd's fixed gateway
    `169.254.1.1` on the tap;
  - the nameserver is `169.254.1.1`, which qrab-netd redirects to the
    anchor's resolver.

  qrab-netd also answers DHCP on each tap with the same address, gateway and
  DNS, for guests that ignore the seed's network-config.

## Power, restart and deletion

```sh
kubectl -n qrab patch qvm "$vm" --type merge -p '{"spec":{"running":false}}'
kubectl -n qrab patch qvm "$vm" --type merge -p '{"spec":{"running":true}}'
kubectl -n qrab patch qvm "$vm" --type merge -p '{"spec":{"restartNonce":"restart-1"}}'
kubectl -n qrab get qvm "$vm" -o yaml
```

`running:true` means the operator keeps a VM Pod and replaces one that exited
or was deleted, about 10 s after it ended. A guest shutdown or power off (BMC,
dashboard, Redfish `GracefulShutdown`/`ForceOff`, or the guest itself) leaves
QEMU off inside the Pod, which stays Ready; Redfish `On` starts it again in
place ([manager.md](manager.md#power-lifecycle)). `running:false` deletes the
Pod and keeps the VM off; a new `restartNonce` replaces the Pod. Stop/restart
and spec changes reap the old instance before its replacement; they retain
firmware variables, TPM state and its endorsement key. ConfigMap drift is repaired and the Pod is
replaced so its effective config converges. Required Pod fields, image and
state mounts are checked without treating Kubernetes defaults as drift.

Deleting a CR garbage collects its owned Pod/ConfigMap. **Manual and unproven
state PVCs remain.** A new dashboard launch records the same random launch ID
on its VM and state claim; the operator makes that exact claim VM-owned so
garbage collection deletes it with the VM. Adoption requires both dashboard
labels, matching valid launch IDs, the claim's VM annotation, namespace and
referenced claim name, and no existing controller. A dashboard label alone
never grants deletion authority over identity storage. Legacy claims and
never-bound claims reused from another launch are left retained when this
provenance is absent. Already established controller ownership is unchanged.
Deleting a PVC does not establish that its backing PV or storage data was
destroyed; that follows the storage driver's reclaim contract.
`stateVolumeClaim` is immutable and status pins the PVC UID. A replacement PVC
with the same name is rejected. The runtime binds state to the immutable CR UID
as well as its name: creating a new CR over an old identity volume fails closed.
Restoring/migrating an identity to a new CR is a deliberate recovery operation,
not an automatic adoption path. Never copy a TPM volume to create another VM:
that duplicates its cryptographic identity. Apart from the exact dashboard
claim's GC ownership above, the operator deletes no PVC but a migration's own
target or, after success, the previous state PVC it created,
and does not reset missing/corrupt initialized TPM state.

Use `ReadWriteOncePod` for state where the driver supports it; single-node
`ReadWriteOnce` is also accepted. Each volume has a runtime owner lock, and
identity files are owner-only. `fsGroupChangePolicy: OnRootMismatch` prevents
routine remounts from broadening retained private-file access.

Status reports `observedGeneration`, `phase`, `ready`, `reason`, `message`, Pod
name/UID/node, the state PVC UID, in-place `resize` progress, provisioned
`disks`, `nics`, iSCSI release/recovery fields, `powerState`, `conditions`
and, after a live migration, `stateVolumeClaim` and `placementNode`
([migration.md](migration.md)). Invalid specs and ownership conflicts are
reported as Blocked rather than mutating unrelated resources.

While the VM Pod is Ready and the VM has Redfish, every reconcile (about every
15 s) reads the guest's Redfish `PowerState` from the Pod's metrics port
(`GET https://<pod IP>:9181/metrics`, `qrab_vm_power_state`, three-second
bound) into `status.powerState`, which `kubectl get qvm` shows as `Power`. The
read uses exact-VM SPIFFE mTLS and is read-only; the operator has no exec or control
socket access to the Pod. The phase follows the guest:

| Pod and guest | `phase` | `ready` | `reason` | `PoweredOff` condition |
| --- | --- | --- | --- | --- |
| Pod Ready, guest `On`, `PoweringOn` or `PoweringOff` | `Running` | true | `VMRunning` | False (`PowerState<state>`) |
| Pod Ready, guest `Off` (powered off in place) | `PoweredOff` | false | `GuestPoweredOff` | True (`GuestPoweredOff`) |
| Pod Ready, guest `Paused` | `Paused` | false | `GuestPaused` | False (`PowerStatePaused`) |
| Pod Ready, read failed or no current state | `Running` | true | `VMRunning` | Unknown (`PowerStateUnknown`, the error as message) |
| Pod Ready, Redfish disabled | `Running` | true | `VMRunning` | Unknown (`PowerStateUnavailable`) |
| `spec.running: false`, no Pod | `Stopped` | false | `PowerOff` | True (`PowerOff`) |

Other phases (Pending, Updating, Stopping, ...) carry no `powerState` and no
condition. A failed read never fails, replaces or restarts the Pod; guest boot
evidence is checked separately. `kubectl wait --for=condition=PoweredOff qvm/<vm>`
waits for a power off, and `--for=condition=PoweredOff=false` for power on.

## Resizing a running VM

The VM container is sized from the spec: requests `cpus` / `memoryMiB + 256Mi`,
limits `cpus + 1` / `memoryMiB + 512Mi`, with `resizePolicy: NotRequired` for
cpu and memory. The Pod records the size QEMU booted with in the
`qrab.yapplesauce.com/boot-size` annotation; its ConfigMap and template hash
stay at that boot size, so a size change is not a template change.

```sh
kubectl -n qrab patch qvm "$vm" --type merge -p '{"spec":{"memoryMiB":1024}}'
kubectl -n qrab get qvm "$vm" -o jsonpath='{.status.resize}'
```

- **Growth within the boot maxima**, with memory increments in whole 2 MiB
  blocks relative to the captured boot size, of a Running Pod patches `pods/resize`
  (strategic merge of the manager container's resources, with `metadata.uid` as
  a precondition). No Pod restart. The manager then hot-adds memory/vCPUs
  ([manager.md](manager.md#in-place-growth-hotplug)) once the kubelet has
  applied the new cgroup limits.
- **Shrink**, memory growth between those whole-block sizes, growth beyond
  `maxCpus`/`maxMemoryMiB`, a change of either ceiling,
  or a size change before the Pod is Running takes the existing controlled
  restart path (reason `ResizeRequiresRestart`; firmware/TPM identity retained).
  The guest is never shrunk in place, even though Kubernetes would apply a lower
  memory limit. Lowering a still-Deferred request also restarts: the kubelet may
  admit and apply the larger size (and the manager plug it) concurrently.
  An old Pod whose container limit was already changed to an unrepresentable
  memory size also takes this path, so its exact requested boot size is restored.
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

### Guest identity

`spec.guestIdentity` registers SPIFFE identities for code in the guest
([spire-vm.md](spire-vm.md#guest-workload-identity-design-2026-11-15)):

```yaml
guestIdentity:
  workloads:
  - name: web                 # DNS label, at most 40 characters, unique
    selectors: [unix:uid:1000, systemd:id:web.service]   # 1..8, unix: or systemd: only
```

At most 16 workloads are allowed. The operator writes cluster-scoped
`spire.spiffe.io/v1alpha1` ClusterStaticEntries with `className:
kubernetes-tpm`:
- `qrab-guest-node-<vm-uid>`: the node alias
  `spiffe://yapplesauce.com/qrab/guest-node/<vm-uid>`, parented by the SPIRE
  server. It selects `tpm_devid:subject:cn:qrab-vm-<vm-uid>` and
  `tpm_devid:issuer:cn:<CA CN>`. The CA CN is `QRAB_GUEST_DEVID_CA_CN`
  (default `qrab-devid-ca-1`).
- `qrab-guest-<vm-uid>-<name>` for each workload:
  `spiffe://yapplesauce.com/qrab/guest/<vm-uid>/<name>` under that alias,
  with the workload's selectors and a 10-minute X.509 SVID TTL.

`<vm-uid>` is the VM's UID, the same one in its manager Pod's
`kubernetes/qrab/vm/<vm-uid>` SVID. The entries are labelled with the VM's
owner UID, name and namespace. The finalizer `qrab.yapplesauce.com/guest-identity`
deletes them with the VM. Removing a workload or `guestIdentity` deletes their
entries, and the orphan sweep deletes the entries of VMs that are gone. Without
the CRD or `contrib/kubernetes/operator/guest-identity.yaml` (cluster RBAC and the
admission policy that confines the operator to these shapes), such a VM is
Blocked.

Such a VM's `vm.json` also carries `"spire_relay": {"enabled": true}` and
`"guest_identity": {"enabled": true, "issuer_url": ..., "issuer_id": ...}`,
from `QRAB_TPM_ISSUER_URL` (default
`https://qrab-tpm-issuer.qrab-tpm-issuer.svc:8443`) and `QRAB_TPM_ISSUER_ID`
(default `spiffe://yapplesauce.com/kubernetes/qrab-tpm-issuer/qrab-tpm-issuer`):
its manager provisions the vTPM and attaches the `qrab-identity` disk
([manager.md](manager.md#guest-identity)). Adding or removing
`guestIdentity` therefore restarts the VM; changing its workloads does not.

## Build and verification

The normal default tests and strict Clippy include the operator. The CRD is
generated from its Rust API and checked for drift:

```sh
cargo run --locked -p qrab-operator -- crd > contrib/kubernetes/operator/crd.json
bash contrib/check-runtime.sh
```

Build/publish `qemu`, then `manager`, then `operator` (and `dashboard`) with
the scripts under `contrib/images`; commit source first, publish, then pin all immutable references
and the manager default in the operator Deployment. Each image records its exact
source revision. No QEMU source is downloaded from upstream during these builds.

The explicit `manager/tests/pod_uefi.rs` test checks two UEFI boots with retained
EK identity, event-log availability, absence of a guest NIC, shutdown/reaping,
a crashed TPM child, and malformed retained TPM refusal. Live rollout checks
also compare EK fingerprints across a real controller-driven Pod replacement.

The lifecycle check is intentionally disruptive (Pod replacement, config
repair, stop/start and a rejected identity-adoption probe), so run it only
against a disposable VM. It never deletes the VM's PVC:

```sh
python3 contrib/tests/operator-lifecycle.py --namespace qrab --vm "$disposable_vm"
```

Native guest disks use QEMU/libiscsi with Secret file references and a pinned
storage node. Their drain/fence lifecycle is described in [iscsi.md](iscsi.md).
`spec.disks` entries are provisioned, grown, exported and fenced through the
storage service; the operator then owns `<vm>-disk-<id>` CHAP Secrets (RBAC
`secrets: create, patch, delete`, write-only and confined by
`secret-policy.yaml`) and needs the opt-in
`contrib/kubernetes/operator/storage-api` component. Without it such VMs
report `StorageServiceUnavailable`.
Removed-disk credential cleanup atomically tests the intended VM UID/disk
ownership using a no-op JSON Patch with a metadata-only response, then deletes
only that Secret UID/resourceVersion. This keeps Secret data unread and the
existing no-GET/list/watch RBAC. A colliding name or a changed Secret cannot
authorize deleting another VM's credential; missing credentials are idempotent.
The disposable legacy lifecycle script is intentionally limited to VMs without
native disks; failed native probes require hard-fence recovery.

### VM-platform rollout record (2026-11-12)

These dated records describe the cluster at the time. The qrab-demo VM they
verify has since been retired, as has `qrab-redfish-poc`, which later used its
host ([contrib/kubernetes/README.md](../contrib/kubernetes/README.md#examplesqrab-demo)).

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
`console.viewerSubjects`), Redfish (off; its Kanidm client existed, but
port 8081 had no Service/Ingress/Cilium path yet; the next record adds it), vCPU growth (needs KVM) and
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

### VM access through Kanidm groups, rollout record (2026-11-12, evening)

- Source `2f9a296`: manager `80a93bb0…` and operator `cea9e1ad…` (pins
  `093cb6a`). The gitops re-pin of the operator's SPIFFE identity and the
  KVM device plugin merged first (kubernetes-gitops PR #412).
- `creator-policy.yaml` is applied. `check-creator-policy.sh` passes with
  server-side dry runs: no creator for certificate identities, forged values
  removed or replaced, Kanidm users recorded, updates refused as immutable.
- Kanidm, through `vm-access-groups.sh setup` and `create qrab qrab-demo`:
  `qrab_vm_users` and `qrab_operators` map `openid profile groups_uuid` on
  `qrab-bmc` and `qrab-redfish`. `qrab_vm_qrab_qrab-demo_admins` is
  `5704ce8a…` (member sam) and `_viewers` is `4a520b85…`.
- qrab-demo carries both groups for the console and Redfish. The manager
  image change and the group patch shared one controlled replacement (Pod
  `4fb433fe`, 50 s, 0 restarts). `check.sh` PASS with EK `ec61ceb9…e844`
  unchanged. The BMC login now requests `groups_uuid`.
- idm-dashboard `9255e09` (person page "Virtual machines") is published as
  `e557a425…`; its gitops roll-out is a separate PR.

## Redfish service state and approved artifacts

Optional fields keep state/artifact access separated by container:

```yaml
spec:
  mediaVolumeClaim: approved-install-media
  firmwareCatalogVolumeClaim: approved-firmware-catalog
  components:
    redfish:
      enabled: true
      adminSubjects: ["<operator-subject-uuid>"]
      eventDestinations: ["https://events.example/redfish"]
      stateVolumeClaim: vm-redfish-state
```

These PVCs are explicit resources; the operator neither creates nor deletes
them. All claim roles must be distinct, including `spec.stateVolumeClaim`.
Media and firmware catalogs mount read-only only into the manager at
`/var/lib/qrab-media` and `/var/lib/qrab-firmware`. Redfish subscription state
mounts only into Redfish at `/var/lib/qrab-redfish`; it never gets the identity
PVC or manager runtime directory. Leaving fields absent retains prior defaults.

Event destinations are bounded exact HTTPS443 URLs without credentials, query
or fragment. The operator maintains an ownership-checked additive Cilium
policy for just those destination hosts/IPs, independently of guest NICs.
DNS interception retains existing kube-dns lookup behavior while exact HTTPS
rules constrain delivery; URL paths are enforced by the Redfish process.

Each service retains its own CPU/memory budget and receives
`QRAB_RESOURCE_SCOPE=container` for verified cgroup sampling. New workers and
bounded task/telemetry/subscription state execute within Redfish's allocation;
media and firmware operations execute within manager's allocation. See
[Redfish](redfish.md) for bounds, catalog permissions, task semantics and
current/pending lifecycle support. Source changes do not deploy these features.

### Explicit Secure Boot capability and telemetry cadence

`spec.secureBootCapable: true` selects the manager image's explicit
`OVMF_CODE_4M.secboot.fd` and SMM-protected pflash. It neither enrolls keys nor
sets SecureBootEnable. Use Redfish authenticated database updates, staged settings
and managed PowerCycle to control those separately. Retained NVRAM/TPM is never
reseeded when this capability changes; incompatible/malformed state fails closed.

Redfish defaults to on-request telemetry. Set
`spec.components.redfish.telemetryIntervalSeconds: 5` (allowed 5..3600) for explicit
periodic history. Active numeric triggers still sample every five seconds.
All work remains within independent manager/BMC/Redfish CPU and memory budgets;
no Redis or firmware helper daemon is added.

VM scrape and history identity, Pod/IP binding, raw9090 denial and the Pod-local
Prometheus adapter are described in [observability](observability.md).
