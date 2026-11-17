# Capability ledger: what must be true before retiring a service

This ledger describes the required replacement subset. **Implemented** is a
source claim, not a fresh deployment certification. **Constrained** means
known scope limits. **Work remains** identifies a product/operational contract
that this investigation has not established as complete. Feature gaps are
not silently reclassified as boundary bugs.

| Capability | Source position | Remaining acceptance / limit | Evidence |
| --- | --- | --- | --- |
| Declarative VM lifecycle | Implemented | Prove target-cluster RBAC/admission and required tenancy/quotas | S02, S03, M01 |
| QEMU supervision without libvirt | Implemented | Validate chosen release/machine/firmware and workload behavior | S04–S07 |
| VM-specific container resources | Implemented | Required CPU topology, NUMA, pinning, hugepages and device-placement policies are separate scope decisions | S02, S04, S21 |
| In-place power / management availability | Implemented | Distinguish guest off, paused, Pod stopped and runtime failed | S04, S06, S19 |
| CPU/memory growth | Implemented / constrained | Boot ceilings, kubelet admission, whole 2 MiB memory increments relative to boot and guest onlining; unrepresentable sizes restart, no live shrink claim | S02–S04, T12 |
| Authenticated browser / Redfish / console | Implemented | Verify actual issuer/audience/role policy, expiry and deployed service enrollment | S17–S20 |
| Human identity without Keystone | Implemented foundation | Agree API RBAC, VM application roles and namespace administration model | S16, S18–S20, M01 |
| Workload identity / rotation | Implemented foundation | Platform SPIRE enrollment, service-specific peer policies and expiry evidence | S20, L01 |
| Optional guest workload identity | Implemented / optional | Guest-image enrollment, selector policy, revocation/recovery and software-vTPM trust model | S23, S24, L02 |
| Cilium-native guest attachment | Implemented | KVM + stable address range + netd + platform policy; host unified cgroup-v2 view, standard kubelet roots and SO_PEERPIDFD support (Linux 6.5+) | S08–S10 |
| Guest security groups | Implemented foundation | Deploy baseline/allow policies; label membership alone has no enforcement | S08, M01 |
| Stable externally routed guest IPv4 | Implemented / constrained | /32 range and external route acceptance; no general floating-IP or overlapping-tenant network claim | S08 |
| IPv6 / broad Neutron network parity | Work remains / outside current model | Decide required address/network/L2/router semantics before migration | S08, S09 |
| Optional VPN NIC | Implemented / constrained | IPv4/one-peer contract, key lifecycle and provider dependency | S25 |
| Same-VM qrab live migration | Implemented / constrained | Compatible machine/CPU/disks, supported management state and auxiliary claims | S11 |
| Live NIC handoff | Implemented / fork-dependent | Local Cilium fork version contract, failure/rollback/conntrack limits | S08, S11, S12 |
| OpenStack → qrab live migration | Not established | Use explicit workload conversion/cutover; no compatible live-migration API is claimed | S11 |
| Cold moves / automatic node evacuation | Not established as general product | Define identity transfer, fencing and placement/recovery policy | S05, S11, S13 |
| Image import / sealed generations | Implemented / constrained | Raw/qcow2 source contract, capacity, publication and distribution policy | S15 |
| Image sharing / multi-site distribution / full Glance API | Work remains / outside current interface | Define only the subset workloads require | S15, U03 |
| Provisioned VM root/data disks | Implemented | Exact backend enrollment, generation/fence behavior and capacity | S13, S14 |
| CoW clone / online disk growth | Implemented | Verify origin/GUID, backend growth and guest filesystem behavior | S13, S14 |
| Native disk recovery fencing | Implemented / constrained | Fully provisioned disks can self-fence; hand-declared disks retain explicit admin procedure | S13, S14 |
| Deleted-disk retirement / grace reaper | Added in source `0301347` | Verify storage verb scope and separate root timer installation; state-PV cleanup remains distinct | S13, S14 |
| General volume attach/detach / multi-writer / multi-backend | Not established as general interface | Specify required semantics; shared migration I/O is not multi-writer application support | S13, S14, U04 |
| Application-consistent backup / restore product | Work remains | Coordinate disks, identity state, guest freeze, access/guest registrations and recovery drills | S05, S13, S14, S22 |
| Durable complete audit history | Work remains | Bounded runtime events/telemetry do not supply complete immutable audit retention | S06, S26 |
| Operator horizontal HA | Not established | Current deployment is one replica/Recreate; evaluate coordination before adding replicas | S03, M01 |
| OpenStack client/API compatibility | Not claimed | Consumers must change interfaces or have an explicitly scoped adapter | S02, S19 |
| Full cloud feature catalog | Outside this comparison | Inventory actual Heat/Octavia/Designate/Manila/etc. dependencies rather than assuming absence | U01–U04 |

## Workload acceptance record

For each workload, record its required features, evidence class, tested source
and image digests, observed outcome and remaining owner. Include boot/firmware
mode, CPUs/memory, guest agent, disks/backup, NIC addressing/routes/policy,
identity/access, migration/downtime expectations and rollback. A checked box
without its evidence should not authorize retiring the old provider.

The [transition plan](transition.md) uses this ledger as a workload-specific
gate. It is an engineering acceptance condition, not an added human approval
gate on the already authorized source fixes and documentation work.
