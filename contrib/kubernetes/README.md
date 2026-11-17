# qrab Kubernetes manifests

Each directory is a separate bundle with its own apply mode. The cluster's
namespace, ServiceAccounts, ClusterSPIFFEIDs and pull identities live in
kubernetes-gitops, not here. Source changes, image builds and rollouts are
separate operations.

| Bundle | What it is | How it is applied |
| --- | --- | --- |
| `.` (root `kustomization.yaml`) | The platform as this cluster runs it: `operator-storage-api` plus `monitoring/` | `kubectl apply -k contrib/kubernetes`, client-side |
| `operator-storage-api/` | Operator, both CRDs, RBAC, NIC address pools, every admission policy, with the storage-service client component | `kubectl apply -k contrib/kubernetes/operator-storage-api`, client-side |
| `operator/` | The same without the storage client, for a cluster without the storage service | `kubectl apply -k contrib/kubernetes/operator`. On this cluster it strips `QRAB_STORAGE_*` from the operator: never apply it here |
| `monitoring/` | The static PodMonitor and the Grafana "qrab VM" dashboard ConfigMap | Through the root bundle (it has no kustomization of its own) |
| `dashboard/` | The VM dashboard; its Secret comes from `install-secret.py` | `kubectl apply -k contrib/kubernetes/dashboard` ([dashboard.md](../../docs/dashboard.md)); a `--server-side` apply needs `--force-conflicts` to take the image from the client-side field manager |
| `kvm-device-plugin/` | The `/dev/kvm` device plugin DaemonSet | Its own apply ([operator.md](../../docs/operator.md#kvm-acceleration)) |
| `examples/qrab-demo/` | Sample VM, UID-bound console template and iSCSI patch of the retired `qrab-demo` VM | Never applied as is (below) |

Apply `operator-storage-api` or the root bundle only from a checkout at current
origin/main. They carry the CRDs and every admission policy, so a stale
checkout rolls back whatever another session has just released.

The root bundle never contains a VM. A VM, its state claims and its console
objects are applied from their own directory, so a platform apply cannot create,
retarget or resurrect one. Client-side apply never prunes: objects that leave a
bundle stay live until they are deleted explicitly.

## Expected dry run

On a cluster at the pinned release, `kubectl apply --dry-run=server -k
contrib/kubernetes` reports every object `unchanged`, except the six
`ValidatingAdmissionPolicy`/`MutatingAdmissionPolicy` objects, which always
report `configured`. That is client-side apply noise, not drift: the three-way
patch resends their list fields (`matchConstraints.resourceRules`,
`validations`, ...) whole, and the server stores the same values. `kubectl diff
-k contrib/kubernetes` is the drift check; it prints nothing for these objects.
Any other `configured` or `created` line is real drift. Find who changed it
before applying.

## examples/qrab-demo

`qrab-demo` was the first operator VM (RAM-only ESP fixture, Sam's subject
allowlist, the optional native iSCSI probe disk). It no longer exists, and
neither does `qrab-redfish-poc`, which later used its public host: its bundle
was removed on 2026-11-16 because applying it recreated that deliberately
deleted VM. Its live console objects (Service, Ingress, CiliumNetworkPolicy,
Certificate and the `qrab-demo-tls` Secret) and its volumes were deleted on
2026-11-17. Applying `vm.yaml` here would create a new VM and a fresh
`qrab-demo-state` claim under the retired names.

The directory stays as the smallest complete example of a VM with its own
console, referenced by [operator.md](../../docs/operator.md#create-a-vm),
[bmc.md](../../docs/bmc.md) and [iscsi.md](../../docs/iscsi.md). It
deliberately has no `kustomization.yaml`. To use it, copy it and give the VM,
its claim, the Service/Ingress/Certificate/policy names, the selector's
`app.kubernetes.io/instance`, the host and the Kanidm callback new values.
Create the VM first, then `render-console.sh NAMESPACE VM` prints `console.yaml`
bound to its immutable UID (Service selector and both expected backend SPIFFE
IDs); it applies nothing and never creates a VM.

## Checks

All are read-only or server-side dry runs unless stated otherwise:

- `bash contrib/kubernetes/check.sh NAMESPACE VM [CONNECT_IP]` verifies a
  running console VM, e.g. `check.sh qrab my-vm`.
- `operator/check-creator-policy.sh EXISTING_VM` and
  `operator/check-dashboard-update-policy.sh EXISTING_VM` dry-run the
  admission policies against an existing VM.
- `operator/check-secret-policy.sh`, `operator/check-nic-address-policy.sh`,
  `operator/check-nic-handoff-policy.sh` and
  `operator/check-guest-identity-policy.sh` bind uniquely named copies of their
  policies and remove them on exit. The last needs spire-controller-manager's
  ClusterStaticEntry CRD.
- `redfish-live.py [ORIGIN] [--expect admin|viewer]` checks a VM's Redfish API
  with a real Kanidm ID token; as admin it pauses and resumes the VM.
- `operator-storage-api/check.sh`, `operator-storage-api/clone-check.sh IMAGE GEN`,
  `migration-check.sh` and `egress-check.sh` create and delete their own VMs.
  Each run leaves that VM's provisioned disk retained on the storage host
  (towl `work/test-vm-disk-leak`); reclaim them as described in
  [iscsi.md](../../docs/iscsi.md#destroying-retained-data).
