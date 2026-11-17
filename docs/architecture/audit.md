# Architecture boundary audit

This register contains source-backed findings and verified repairs from the
architecture investigation begun on 2026-11-17. Work claims, detailed progress
and operational notes live in the `qrab` project in towl. It is not a claim
that every possible violation has been found or every deployed image contains
these repairs.

## A01 — state-PVC adoption crossed the launch ownership boundary

**Affected source:** `resources::state_claim_adoption` in the operator and
`launch::render_pvc` in the dashboard. **Baseline:** `5f361b7`.

The operator previously granted a VM controller ownership of a state claim
when the claim carried `app.kubernetes.io/managed-by: qrab-dashboard` and had
no existing controller. It did not verify the intended VM or launch. A VM
referencing another dashboard-created claim could therefore acquire
garbage-collection authority over that identity volume. The manager's later
VM-identity check did not undo this earlier ownership mutation. A name reused
after an earlier launch was another ambiguity.

The repair requires both resources to carry the dashboard label and the same
valid random launch ID; the claim must also match the VM annotation,
namespace and referenced state-claim name. Deleting resources and claims
with an existing controller are not adopted. The patch retains exact
UID/resourceVersion preconditions and preserves other non-controller owners.
New dashboard PVCs record the launch provenance. Legacy/reused claims without
the proof remain retained; established controller ownership is unchanged.

The regression reproduced the foreign-VM adoption failure before the change.
It passes after the change, covering foreign/stale/missing provenance,
namespace/name mismatch, invalid IDs, deletion and existing controllers. A
dashboard test also checks the actual rendered VM/PVC contract against the
operator adoption function. The full `contrib/check-runtime.sh` passed:
formatting, workspace tests, generated CRD/policy comparisons and strict
Clippy. These checks are hermetic; no live cluster deletion was exercised.

**Merged source:** `719335966b72ba33332f53ad8d9a36812771c965`.
**Tracking:** qrab `work/state-pvc-adoption-launch-boundary`.
The primary main checkout was synchronized. No image publication or deployment
followed the source repair.

The provenance annotations are public trusted Kubernetes metadata, not secrets
or cryptographic credentials. An actor with broad authority to forge or
rewrite those objects remains inside the namespace administration trust
boundary. This fix scopes normal cleanup; it does not turn a namespace with
unrestricted Pod/resource writers into a hostile multi-tenant environment.

## Documentation boundary reconciled with the agreed deletion design

Some older operator comments/guides said state PVCs were never collected,
while the implemented dashboard deletion path already adopted dashboard
claims. The user-approved deletion design recorded in qrab towl
`container/vm-deletion-cleanup` permits collection of the dashboard launch's
own claim and retains manual claims. The repair updates the operator API
description, generated CRD, crate instructions and subsystem documentation to
state this narrow exception and its provenance checks.

PVC deletion is not backing-data destruction. The documented Retain-only
storage contract leaves a Released PV/zvol for a separately owned cleanup
workflow. Guest disk exports, retained identity state and group cleanup are
different ownership domains and must stay distinct in presentations.

## A02 — access grants incorrectly determined group cleanup authority

**Affected source:** dashboard `cleanup::reconcile_vm` and `cleanup::owned`.
**Repair baseline:** `0301347` (the source advanced independently during the
investigation).

The finalizer scanned current console admin/viewer grants instead of the
group UUIDs recorded at launch. Removing an original group from those grants
left it behind at deletion. A generated group from a previous launch of the
same namespace/VM name also passed the old name/description ownership check.
Access membership had incorrectly become lifecycle ownership.

Cleanup now uses the recorded launch ID and admin/viewer UUIDs, validates the
references before deletion and checks the exact launch-derived group name,
returned UUID and namespace/VM/role description. Current console lists do not
widen or erase that authority. Pre-launch-ID VMs look up only their exact
legacy names; incomplete modern provenance does not fall back to legacy.
Missing groups are already cleaned; invalid/mismatched provenance and service
outages keep the finalizer for a safe retry or explicit correction.

The amended regression failed before the repair because the original groups
were leaked. It passes after the repair and preserves platform, other-VM and
previous same-name-launch groups. Further coverage checks corrupt/missing
metadata, foreign UUIDs, legacy cleanup and outage behavior. Full dashboard
tests and `contrib/check-runtime.sh` passed. No live Kanidm groups were deleted
by these hermetic checks.

**Merged source:** `8a30586610cc80073076859454ce2554215fb2bd`.
**Tracking:** qrab `work/access-group-cleanup-launch-boundary`.
Primary main was synchronized; no image build or deployment followed.

As with A01, the annotations are public trusted metadata. Broad namespace/API
administrators remain trusted. The repair does not claim an immutable or
cryptographic attestation of arbitrary client-supplied annotations.

## A03 — network security guide retained a plaintext-era description

The detailed networking security section still called the node conntrack
exchange plain TCP, despite the implemented exact-node SPIFFE TLS path and
the newer connection-atlas/enrollment evidence. The atlas work corrects that
paragraph to match `peer_tls` and the existing rollout record. On-link address
checks remain additional authorization, not a substitute for peer identity.
This is a documentation repair, not a new transport implementation or rollout.

## A04 — verified image metadata could select extra unverified inputs

**Affected source:** operator `image_job::virtual_size` and the inspect/write
Job path. **Repair baseline:** `8a30586`.

The importer checked the downloaded file's digest, then allowed qemu-img to
convert qcow2 metadata that could name backing or external-data files. Those
extra local/network inputs are not covered by the top-level digest and may
cross the Job's filesystem/credential boundary. A harmless temporary marker
fixture demonstrated the conversion mechanism; no real credential or live
storage was read.

Both Job modes now run a bounded qcow2 dependency preflight before any
qemu-img invocation. It reads the fixed v2/v3 header, rejects backing
offset/length and the external-data feature, and fails on truncated or
unsupported fixed headers. Raw remains an opaque single file. This is not a
new general qcow2 parser: QEMU still performs full format validation.

Default tests require no QEMU and cover dependencies, truncated/malformed
headers, bounded reads and actual entrypoint refusal before tool launch. An
explicit fixture test passed on QEMU **11.1.3**, built locally from the pinned
`vendor/qemu` source with downloads disabled. It checks standalone v2/v3 and
extended-L2 inputs, rejects both dependency classes, and demonstrates the
unguarded baseline reading the separate benign marker. The full runtime
check also passed. This did not test live storage, libiscsi or a guest boot.

**Merged source:** `69ef4274d6ba973bb696371c97fb0716a1e4206c`.
**Tracking:** qrab `work/image-import-self-contained-qcow2`.
Primary main was synchronized. No production image build/publication or
cluster rollout followed.

The legacy staging script accepts only a reviewed hardcoded CirrOS URL/digest,
not arbitrary BaseImage sources. The general inspect/write Job interface is
the repaired boundary. Prepared backing chains must be flattened in their
trusted preparation environment before import.

## A05 — host cgroup names could impersonate Pod identity

**Affected source:** netd `cgroup::pod_uid` and its use by socket-caller and
anchor-process lookup. **Repair baseline:** `93b9792`.

The old helper searched every cgroup path component for a `pod<UUID>`
substring. An unprivileged host user can create a transient user scope whose
name contains that text. A harmless local `systemd-run --user --scope`
fixture produced exactly such a kernel-reported path, and the old parser
returned its fake Pod UID. netd's caller mapping and anchor process selection
used that result as authority. A kernel-reported string still needs its
hierarchy and component semantics validated.

The repair accepts one host-view unified-cgroup-v2 `0::` record and exact Pod
components under kubelet's standard `/kubepods.slice` or `/kubepods` roots,
including their valid QoS paths. It rejects user/delegated roots, partial
names, extra records and unrooted views. The same corrected mapping protects
anchor PID selection, so a lower-numbered host lookalike cannot be chosen
instead of the real Pod process. Custom roots and cgroup-v1/multi-record
views fail closed; the node daemon requires the documented host unified view.

The regression failed before the fix and passes after it. Coverage includes
both drivers/all QoS classes, nested lookalikes, malformed/multiple records
and fake-proc caller/anchor selection. The netd suite, client-only
`--no-default-features` check and full runtime check passed. A bounded
read-only inspection confirmed the deployed netd's standard host-rooted
systemd view. No live VM descriptor grant, NIC control or namespace/network
mutation was attempted; unprivileged kernel-netns tests are not claimed as
live datapath proof.

**Merged source:** `4625eefabb3743b3ec775b4f547a6b873cd162bf`.
**Tracking:** qrab `work/netd-cgroup-hierarchy-authentication`.
Primary main was synchronized. The source repair was not published in a
runtime image or rolled out.

## Checks and limits

The initial `5f361b7` runtime baseline passed before the repair. Future
findings must include a source location, a concrete boundary crossing or
failure mode, a regression or other appropriate proof, and the verified
change. Desired capability gaps are not automatically bugs: document them in
the capability ledger rather than silently expanding the platform's scope.
