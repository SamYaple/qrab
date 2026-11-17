# Skeptical questions for the architecture review

## Is this just OpenStack in Rust?

No. The intended control model is different: Kubernetes resources and existing
platform controllers own declarative intent and placement; a manager in each
VM Pod owns QEMU; Cilium already supplies the fabric. Some VM-specific
orchestration remains necessary. If qrab recreates general placement,
messaging, networking and identity control planes beside Kubernetes, it loses
the point of this design.

## Does putting libvirt in a Pod already solve this?

In the supplied deployment, the libvirt service is containerized but its VMs
still live in traditional libvirt-created host cgroups. qrab's execution
boundary places QEMU under its own manager container. This is a comparison of
that deployment model, not a universal claim about every way to run libvirt.

## Can we remove Nova just because QMP works?

QMP supplies local hypervisor control. Nova also coordinates lifecycle,
placement and resources. The replacement owner is Kubernetes **plus** the
operator **plus** manager, with explicit recovery/storage/network seams.
The [code map](code-map.md) shows that split.

## Does Cilium remove all the work Neutron did?

It removes the need for an OVN guest fabric in this target. Stable address
claims, guest configuration, attachment, policy, routes and handoff still have
owners. The current contract is narrower than general Neutron networking.
See the [network view](network.md).

## Why a Pod per guest NIC?

It gives the NIC an independent Cilium endpoint/IP/policy identity and keeps
guest packets out of the VM management namespace. Each NIC can have different
group labels. The cost is an extra Pod/endpoint per NIC, a privileged node
attachment helper and migration integration. That cost belongs in capacity
and operations planning.

## Is the networking all upstream Cilium?

The ordinary packet datapath uses Cilium and kernel TAP/vhost resources.
Duplicate-address standby/active live handoff depends on the local Cilium
fork and its node API. That fork is a real maintained dependency, not an
upstream compatibility promise.

## Why keep FRR if Cilium supports BGP?

The existing platform uses frr-k8s/FRR, and qrab writes scoped
FRRConfiguration resources. Replacing that integration is a separate design
choice. Advertising routes and programming the guest datapath are different
responsibilities.

## Can Kubernetes auth replace every Keystone and application role?

Kubernetes auth/RBAC protects its API. OIDC identifies users to management
apps; per-VM roles authorize their actions. SPIFFE/SPIRE identifies services.
The target removes Keystone but retains these explicit policy owners and the
external identity provider. A service SVID is not a human admin role.

## Is the guest disk just a Kubernetes PVC?

The firmware/vTPM state uses a small filesystem PVC. Current native guest
root/data disks use QEMU libiscsi to a ZFS/LIO backend, provisioned through
the storage service. Calling both "the VM volume" obscures fencing and
retention. See [storage](storage.md).

## Are Glance and Cinder already replaced?

There is a constrained foundation: image imports/sealed generations, clones,
exports, growth and fencing. Completion depends on the required image sharing,
distribution, backup/restore, attachment, quotas and recovery subset. No broad
OpenStack API or multi-backend equivalence is claimed.

## Does Kubernetes guarantee a single disk writer after node loss?

No such guarantee is assumed here. Missing Pods, unresponsive nodes and
expired leases are not hard disk fences. The storage guard needs clean old
process evidence or a completed backend fence before safe replacement.

## Does the vTPM make this confidential computing?

The active software-vTPM path provides stable guest identity interfaces and
optional guest SPIRE enrollment. The trusted node/hypervisor/manager/storage
administrator can still affect it. The retired SVSM experiments are not the
active runtime's trust model.

## Can every running VM migrate?

Migration has concrete admission limits: supported source state, compatible
machine/CPU/disks, no unsupported management policy, and suitably shared
auxiliary claims. Powered-off VMs have no running guest to live-migrate.
The current protocol is within qrab; cross-platform live migration is not
established.

## Does a green test mean the cluster is safe to cut over?

Only within that test's scope. Hermetic protocol/controller tests, real-QEMU
tests, privileged kernel tests and live workload/cluster acceptance establish
different facts. A test that silently skips is not live evidence. Use the
[capability ledger](capabilities.md) and [transition plan](transition.md).

## What did this review actually fix?

The repairs span cleanup provenance, held process/namespace authority,
network-generation lifetime, conditional API effects, actual device budgets
and input-path integrity. Later cases cover migration adoption/completion
retries and historical metrics crossing a reused Pod name. Each category has
reproduced refusal/retention cases and appropriate local checks.
The [audit](audit.md) records regression evidence and merged SHAs.
Source merges do not mean the fixes are already in deployed images.

## Is it cheaper, faster or more reliable?

Those are testable goals, not conclusions of this source review. A simpler
control-plane ownership model can reduce duplicated operational work, but
fair measurements must include the Cilium fork, node helpers, storage service,
identity dependencies and the workload's actual recovery requirements.
