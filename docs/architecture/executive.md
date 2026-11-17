# A VM platform with one Kubernetes control plane

The proposal is to keep the platform already required to run Kubernetes and
move VM-specific behavior into a small operator and a manager beside each
QEMU process. Cilium provides the guest network through a separate endpoint
for each guest NIC. Existing Kubernetes and identity infrastructure replace
the need for another cloud identity service.

## What changes

Today, the OpenStack cluster has two layers of orchestration. Kubernetes runs
the OpenStack services, but Nova and libvirt still manage the virtual machines.
It also has two networking systems: Cilium for Kubernetes and OVN for guests.
That is the operator's supplied deployment model, not a statement that every
OpenStack deployment has these boundaries.

With qrab, a `VirtualMachine` is a Kubernetes resource. The operator reconciles
its Pods and configuration; Kubernetes admits and places those Pods; the
manager supervises QEMU in the manager container. Guest NIC anchor Pods give
each NIC a Cilium identity independent of the VM's management interfaces.
frr-k8s/BGP remains responsible for route distribution. QEMU, KVM, the Linux
kernel, physical networking and backing storage remain necessary.

## The replacement story

| OpenStack piece | Intended replacement |
| --- | --- |
| Nova and Placement | Kubernetes admission/scheduling/resources plus qrab VM and migration controllers |
| libvirt | Per-VM Rust manager using typed QMP/QGA and direct child-process supervision |
| Neutron and OVN | Cilium endpoints/policies, NIC anchor Pods, qrab-netd, address/route reconciliation and frr-k8s |
| Keystone | Kubernetes API auth/RBAC for declarative operations; OIDC user grants for browser/Redfish; SPIFFE/SPIRE for workloads |
| Glance | `BaseImage` catalog/import controller, verified import Jobs and sealed image generations; broader catalog/distribution work remains |
| Cinder | Provisioned disk reconciliation and separately authorized storage verbs; broader volume lifecycle and recovery work remains |
| Horizon, if used | qrab dashboard, per-VM BMC console and Redfish management |

These are responsibility mappings, not interchangeable APIs. Nova is more than
a libvirt caller, Neutron is more than a switch, and identity is more than a
login screen. The detailed [replacement map](replacement-map.md) states the
surviving duties and limits. Upstream Nova documents scheduling, conductor,
compute, API and persistence as separate duties; the comparison preserves
them even when their implementation moves.
([Nova architecture](https://docs.openstack.org/nova/2026.1/admin/architecture.html))

## What is already present

The inspected source includes direct QEMU supervision, persistent UEFI/vTPM
identity, authenticated management, native guest networking, live migration,
image imports, copy-on-write disk clones, online growth and storage fencing.
It is more than a boot-only prototype. It is also deliberately narrower than
OpenStack: this atlas makes no claim of broad cloud API compatibility,
multi-region scheduling, full network virtualization parity or a general
multi-backend block-storage product.

The source has a real integration cost: live NIC handoff depends on a local
Cilium fork; privileged work still exists in the node network daemon, device
plugin and storage backend. The design confines those privileges rather than
pretending they disappear.

## The decision to present

Adopt the Kubernetes-native ownership model as the desired architecture.
Treat the existing image and disk implementation as a foundation for the
required Glance/Cinder subset. Move workloads only after proving their
network, identity, storage, backup and recovery requirements against the
[capability ledger](capabilities.md). Retire an OpenStack component when its
required responsibilities and rollback plan have an accepted replacement.

This is an architecture comparison, not a measured cost or performance claim.
The source inspection alone cannot establish lower operating cost, greater
reliability or faster networking. Those claims need comparable measurements.
