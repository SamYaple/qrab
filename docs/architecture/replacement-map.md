# Replace control planes; assign every surviving responsibility

The comparison baseline is the operator's containerized OpenStack deployment:
Kubernetes/Cilium/frr-k8s already exist, libvirt launches VMs in host cgroups,
and OVN supplies guest networking. qrab runs in a different cluster. Nothing
here assumes the OpenStack release, installer, database topology, storage
backend, floating-IP configuration or identity federation configuration.

## Keep, replace, build

| Existing component | Disposition | qrab / platform owner | Responsibility that survives | Evidence / limit |
| --- | --- | --- | --- | --- |
| Kubernetes API and persistence | Keep | Kubernetes | Declarative objects, status, optimistic concurrency, watch and admission | qrab uses CRDs; etcd is platform-owned, not a qrab SQL replacement to administer separately |
| Scheduler / kubelet / runtime | Keep | Kubernetes | Resource admission, placement, container cgroups, restart/termination | qrab also pins placement for state/storage and explicitly chooses migration targets |
| Nova API | Replace | `VirtualMachine`, dashboard, Redfish | Create/delete, desired state, bounded imperative management | No Nova REST compatibility layer |
| Nova conductor / compute orchestration | Replace | Operator + per-VM manager | Coordinate resources, own local process lifecycle, recover safely | The manager alone does not replace all of Nova |
| Nova scheduler / Placement | Replace for required subset | Kubernetes resources, KVM plugin, operator placement | Inventory, capacity constraints and node selection | KVM resource presence is not full CPU/NUMA/PCI/topology placement parity |
| libvirt | Replace | Manager + QAPI/QMP/QGA libraries | Construct, supervise, query and control QEMU | QEMU remains; manager is the production private-QMP owner |
| QEMU / KVM | Keep | QEMU, Linux KVM; manager supervises | CPU/device emulation and virtualization | No new hypervisor is being implemented |
| Neutron API / agents | Replace for required subset | VM NIC spec, operator, netd, Cilium policies | NIC/address lifecycle, policy, guest configuration and routing | Not general Neutron API or feature parity |
| OVN northbound/southbound/northd/controllers | Remove from qrab guest path | Cilium + restricted NIC anchors + netd | Attach guests to a policy-governed datapath | Ordinary NIC datapath is kernel TAP/vhost; migration handoff adds fork integration |
| OVS guest datapath | Remove from this target guest path | Linux TAP/routing + Cilium eBPF | Forward guest packets | Does not assert every shared platform has no unrelated OVS use |
| Cilium | Keep and reuse | Platform CNI | Pod/anchor endpoint identity, policy and datapath | It already exists in both clusters; qrab is not adding a second CNI |
| frr-k8s / FRR / BGP | Keep | Platform FRR; operator writes scoped intent | Advertise/withdraw guest /32 routes | BGP exchanges reachability, not guest packets; qrab's route integration uses FRRConfiguration |
| Keystone | Remove from target | Existing IdP, Kubernetes auth/RBAC, application grants, SPIRE | Separate human login, action authorization and workload admission | SPIRE is not a human identity directory; Kubernetes auth is not automatically Redfish auth |
| Glance | Replace; finish required subset | BaseImage controller, import Jobs, storage service | Verify source, import, catalog, seal, retire and distribute images | Existing implementation covers a constrained raw/qcow2 generation workflow |
| Cinder | Replace; finish required subset | Disk reconciler, storage service, `qrab-iscsi-target` | Provision, attach/export, grow, fence, retain and recover volumes | Guest disks currently use QEMU libiscsi, not a generic CSI block attach path |
| Backing storage / CSI | Keep where needed | Storage platform | Durable bytes and small filesystem state volumes | CSI handles retained firmware/TPM state separately from native guest LUNs |
| Horizon | Replace for required subset | Dashboard + BMC + Redfish | Discovery, launch and management UI | Access is VM-scoped and OIDC-authorized |
| OpenStack-specific SQL / messaging | Remove when no consumers remain | Kubernetes object/watch model for qrab control | Durable intent and retryable coordination | Other services may still need databases; existing systems must be inventoried first |
| Monitoring / logs | Keep and adapt | Prometheus, qrab adapter, Redfish, platform logging | Health, metrics, events and investigation | Bounded event history is not a durable audit log |
| Guest operating systems / applications | Keep or reconfigure explicitly | Workload owners | Application behavior and data | No automatic conversion or cross-platform live migration is promised |

OpenStack's upstream [Nova architecture](https://docs.openstack.org/nova/2026.1/admin/architecture.html),
[OVN reference architecture](https://docs.openstack.org/neutron/2026.1/admin/ovn/refarch/refarch.html),
[Glance overview](https://docs.openstack.org/glance/2026.1/), and
[Cinder overview](https://docs.openstack.org/cinder/2026.1/install/get-started-block-storage.html)
support the service responsibilities above. qrab's owners are established by
the [local source register](evidence.md), not by those upstream documents.

## Three distinctions to keep on slides

**Deployment is not process ownership.** Running libvirt in a Pod does not
move its host-managed QEMU domains into a VM-specific Kubernetes resource
boundary. The proposed boundary puts QEMU under the manager container's
lifecycle and cgroup, with explicit allowances for per-node privileged helpers.

**One datapath is not zero network control.** Cilium replaces the OVN guest
fabric, but qrab must still allocate stable addresses, attach TAP queues,
configure guest networking and hand routes over during migration. Cilium BGP
support is a distinct upstream capability; qrab's inspected code uses
frr-k8s rather than assuming that Cilium BGP is enabled.
([Cilium BGP scope](https://docs.cilium.io/en/stable/network/bgp-control-plane/bgp-control-plane/))

**Identity mechanisms are not interchangeable.** Kubernetes protects its API;
OIDC supplies user identity to the management applications; SPIFFE identifies
service peers; per-VM roles authorize management. None of those alone fences
a storage writer, encrypts all guest traffic or attests a guest application.

## Responsibilities outside the comparison

The user did not identify deployments of Heat, Octavia, Designate, Barbican,
Manila, Swift, Ironic, Masakari or billing services. Their absence from the
target map is not proof they are unnecessary. If a workload depends on one,
record its API and operational requirements before retiring its provider.
The same rule applies to Nova/Neutron/Cinder features beyond the subset shown.
