# Transition view: move responsibilities and workloads deliberately

The OpenStack and qrab deployments are in different Kubernetes clusters.
This is not an in-place Neutron backend switch or a libvirt configuration
change. The transition must move workload data/configuration and explicitly
cut over authority, while the old environment remains available for the
defined rollback period.

## Phase 1 — inventory the workload contract

For each workload, record what it actually uses: firmware/Secure Boot/TPM,
machine/CPU/device expectations, RAM/CPU growth, root/data disks, snapshots
and backups, network address/routing/DNS/security groups, metadata/cloud-init,
user/service/guest identity, automation APIs, observability and downtime
limits. Record the OpenStack release and backend details from that cluster;
they were not supplied by this repository investigation.

Use the [replacement map](replacement-map.md) to assign owners, then the
[capability ledger](capabilities.md) to distinguish existing source support
from required additional work. Do not assume unused OpenStack services are
deployed, or deployed services are unused, without the inventory.

## Phase 2 — prove the platform foundations

| Foundation | Proof needed in the target cluster |
| --- | --- |
| Kubernetes | CRDs, protected namespace/API rights, admission, resource accounting and intended placement |
| QEMU/KVM/runtime | Pinned release/firmware, device injection, supported guest boot and clean lifecycle |
| Cilium/FRR | Address range, anchor identity, baseline policy, externally accepted routes, DNS and MTU |
| Cilium handoff fork | Version/upgrade contract and failure/rollback tests if live NIC migration is required |
| Identity | Correct issuer/audience, VM roles, exact SVID enrollment and rotation/expiry behavior |
| Images | Verified import and sealed-generation provenance, publication/retirement policy and capacity |
| Storage | Export/fence/rotation semantics, recovery, retention and authorized destruction |
| Observation | Logs, scoped metrics/history and useful failure evidence |

The existing qrab connection atlas and dated live checks are useful evidence
for their named releases. They are not automatic certification of a new
target cluster or of the separate OpenStack environment.

## Phase 3 — rehearse a representative workload

Start from a verified image or an explicitly supported disk import/conversion.
Validate the guest's boot mode, root/data layout, network configuration,
application behavior and management access. Exercise power-off/on, restart,
resource/disk growth, backup/restore and the required failure recovery paths.
If live migration matters, test the actual CPU/machine, disks, NIC flows and
auxiliary claims, including refusal paths.

Preserving an existing TPM/firmware identity is a separate recovery/migration
contract. qrab intentionally binds state to its immutable VM UID and refuses
casual reuse of another identity volume. Do not assume that copying an
OpenStack/libvirt TPM directory into a new VM is a supported identity transfer.
If identity continuity is required, design and prove it before cutover.

## Phase 4 — cut over one writer and one network authority

Quiesce or otherwise coordinate the application's data consistency according
to its own contract. Complete the supported data transfer, verify target
content and stop the old writer before the new one becomes authoritative.
Move addresses/routes/DNS through an explicit network plan; a stable qrab /32
is not by itself a cross-cluster address-transfer mechanism.

Prove the user-facing application and its management/observation paths.
Record the old/new resource identities, source/image versions, data state,
network authority and expected recovery behavior. No OpenStack-to-qrab live
migration or transparent API compatibility is established by this atlas.

## Phase 5 — retire services when their last responsibility moves

Neutron/OVN can leave when no remaining guest/network responsibility depends
on them. Nova/libvirt can leave when no remaining VM is under their ownership.
Keystone can leave only after its consumers and authorization relationships
have moved. Glance/Cinder can leave only after their image/volume/backup
dependencies and retained-data obligations have owners.

OpenStack-specific databases/message brokers are removed only after their
consumers are gone. Cilium, Kubernetes, frr-k8s/BGP, the physical network,
backing storage, identity provider and monitoring remain platform concerns.
Other OpenStack services are assessed from actual usage, not omitted by
analogy.

## Rollback is a data/authority plan

Before cutover, define the last point at which the old VM can restart without
losing accepted writes or creating two active writers/addresses. After the new
VM accepts writes, "turn the old VM back on" is not sufficient rollback: data
reconciliation or an application-specific failback is needed. Record who owns
the retained source, credentials, routes and eventual destruction. A snapshot
without its consistency and ownership contract is not a complete rollback.
