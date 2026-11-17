# Architecture decisions for the target

These are proposed/confirmed design contracts for presentation and further
engineering. "Current evidence" points to the implementation; it is not a
claim that a separate organizational ADR process has approved every detail.

## D01 — Kubernetes is the declarative control plane

**Decision:** express VM and migration intent as Kubernetes resources and
reuse its API, admission, scheduling and resource lifecycle. **Reason:** avoid
a second general cloud state/placement/messaging plane. **Consequence:** the
operator must make retries, status, ownership and failure recovery precise;
VM-specific constraints remain explicit. **Current evidence:** S02, S03, M01.

## D02 — Manager is the local QEMU owner

**Decision:** supervise QEMU/swtpm directly in the manager container, with
private QMP/QGA/VNC and a bounded local control API. **Reason:** make process
ownership align with the VM Pod and keep protocol libraries reusable.
**Consequence:** manager owns child cleanup, leases, timeouts and hotplug;
management sidecars enforce user policy. **Current evidence:** S04–S07.

## D03 — Each guest NIC has its own Cilium identity

**Decision:** use separate restricted anchor Pods and a narrow privileged
node daemon for TAP/vhost grants. **Reason:** reuse the existing fabric while
separating guest packets from management. **Consequence:** per-NIC resource
cost, address/route reconciliation and a maintained handoff integration.
**Current evidence:** S08–S12.

## D04 — Identity mechanisms stay distinct

**Decision:** use Kubernetes API auth/RBAC, OIDC user roles and SPIFFE workload
admission at their respective boundaries, without Keystone in the target.
**Reason:** avoid confusing the human actor with the service peer.
**Consequence:** exact service identity is necessary but not sufficient for
user management actions; guest identity remains separate. **Current evidence:**
S17–S20, S23, S24.

## D05 — Storage fences determine replacement safety

**Decision:** require clean old-process release or hard backend fencing before
new native-disk ownership. **Reason:** API object disappearance is not proof
an old writer is gone. **Consequence:** unknown/failed ownership can block
recovery; credential generations and journals are part of the protocol.
**Current evidence:** S13, S14.

## D06 — Cleanup authority follows exact resource provenance

**Decision:** distinguish current access grants from lifecycle ownership and
scope GC/deletion to the created launch/resource. **Reason:** labels, names
and mutable grants can otherwise widen destructive authority. **Consequence:**
unproven legacy state stays retained; corrupt group provenance requires
correction rather than fallback. **Current evidence:** A01 and A02 in
[the audit](audit.md).

## D07 — Implement the required image/volume subset deliberately

**Decision:** build on the existing sealed-image and native-disk foundation,
with explicit backup, publication, retention and recovery requirements.
**Reason:** replacing a service name is not completing its product contract.
**Consequence:** no broad Glance/Cinder API equivalence, generic backend or
multi-writer claim without additional design and proof. **Current evidence:**
S13–S15 and [the capability ledger](capabilities.md).

## D08 — Migration transfers ownership, not just bytes

**Decision:** prepare a fenced target, authenticate the exact VM, transfer
supported state and adopt only from explicit completion evidence. Keep node
conntrack exchange and disk I/O in their own authority domains. **Reason:**
RAM copy alone does not preserve network/storage/identity safety.
**Consequence:** unsupported state refuses clearly; fork/platform contracts
must be versioned and tested. **Current evidence:** S08, S11, S12, S13.

## D09 — Release stages and evidence classes stay separate

**Decision:** distinguish source, manifests, dated live observations and desired
state; separate merge, publication and rollout. **Reason:** prevent a code fix
or unit test from being presented as deployed proof. **Consequence:** every
acceptance record names its scope and versions. **Current evidence:** M02,
T01 and the repository maintenance instructions.
