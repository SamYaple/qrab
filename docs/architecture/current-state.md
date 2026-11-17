# Current structure: five planes and three kinds of ownership

This is the inspected source architecture, beginning at `5f361b7`. It is not
a fresh cluster inventory. [Evidence IDs](evidence.md) connect each plane to
its implementation; [capabilities](capabilities.md) separates implemented
behavior from remaining work.

## Declarative control plane

The Kubernetes API stores namespaced `VirtualMachine`,
`VirtualMachineMigration`, `BaseImage` and `VpnProfile` objects. The VM
operator watches the VM and relevant child objects and rereads current VM
state before reconciling. It validates the spec, checks resource ownership,
coordinates storage and network prerequisites, renders configuration and
creates or updates controlled Kubernetes resources. Status records observed
generation, phase, resource bindings and progress. (S02, S03, S08, S13, S15.)

Kubernetes admission and RBAC bound API operations. The scheduler and kubelet
place and start the rendered Pods; the device plugin advertises KVM capacity
and injects the required device/socket access. VM state/storage placement can
pin a node, and migration target selection is explicit operator logic rather
than a claim of general cloud scheduler parity. The operator deployment is
one replica with `Recreate`; horizontal controller HA/leader election is not
established by this architecture. (S21, M01.)

The image importer uses another mode of the operator binary and separate
identity/RBAC. Its Jobs execute token-free inspect/write modes. The optional
Proton broker separately reconciles VPN profiles. Neither belongs in the
critical path of a NIC-free VM using existing boot media. (S15, S25.)

## Per-VM execution plane

Each VM Pod has a manager container. It runs QEMU as a direct child inside
the container sandbox, keeps the persistent identity lease, creates private
session sockets, supervises swtpm/QEMU, drains events and publishes status.
QEMU owns the guest CPU, memory and virtual devices; Linux KVM supplies
hardware acceleration when requested. No libvirt domain or host systemd VM
service owns that QEMU process. (S04–S07.)

The VM Pod uses UID/GID 10001, drops capabilities, disables privilege
escalation and API-token automount, and uses a read-only root filesystem.
KVM VMs explicitly use the host user namespace (`hostUsers: true`) for the
device-open contract; that is distinct from `hostPID`, `hostNetwork` and
`hostIPC`, which are false. VM and anchor Pods have no manifest hostPath;
the privileged plugin/CDI provides narrowly scoped device/socket injection.
This is still a trusted hypervisor/node kernel boundary, not confidential
computing. (S02, S21.)

The manager's private QMP/VNC/QGA session directory is separate from the
control directory shared with BMC/Redfish. Sidecars call the bounded manager
control API; they do not open QMP themselves. A standalone/embedded BMC mode
can call the same control abstraction in process. That compatibility mode is
not a separate cluster control plane. (S06, S18, S19.)

## Guest network plane

A VM has no guest NIC unless requested. Each NIC gets a restricted anchor
Pod on the VM's node, with its own network namespace, stable /32 address,
Cilium endpoint identity and security-group labels. netd is the separately
privileged per-node agent: it enters the anchor namespace, creates TAP/vhost
resources and grants descriptors to the authorized manager through a local
Unix socket. QEMU's virtio/vhost queues carry packets; the control socket
does not relay them. Guest traffic does not traverse the VM management
network namespace. (S08–S10.)

The operator claims addresses through one-address Cilium IP pools and writes
scoped FRR configurations. frr-k8s/FRR speaks BGP. The anchor supplies routing,
DHCP compatibility and DNS redirection; NoCloud seed data supplies static
guest configuration. Security groups are labels selected by Cilium policies,
not a second Neutron rule database. An anchor without selecting policy is
default-allow; a platform baseline is required for a default-deny posture.
Live handoff of a duplicate address uses the local Cilium fork. (S08, S09,
S11, S12; [networking guide](../networking.md).)

## Storage and artifact plane

Guest block I/O and storage provisioning are different paths. For provisioned
disks, the operator calls a separately authorized storage service. That
service invokes the narrow root `qrab-iscsi-target` implementation to manage
ZFS zvols, LIO exports, generations and fences. The manager configures QEMU
libiscsi with a VM-derived initiator and CHAP secret file. QEMU connects
directly to LIO; disk payload does not flow through operator or manager's
control API. (S13, S14.)

Firmware variables and software-TPM identity live on a small filesystem PVC.
They are not the guest's root/data LUNs. A manager lease and VM/PVC UID
bindings protect the intended identity lifecycle; storage fencing protects
native disk writers. The two controls solve different problems. Deletion
authority depends on provenance: explicit manual state claims are retained;
the user-approved dashboard flow gives its own state claim a VM owner;
migration-created state claims have separate cleanup rules. See the
[ownership audit](audit.md) for the adoption fix and the exact contract.

`BaseImage` imports are already implemented: inspect verified downloads,
stage/export a volume, run a write Job, fence/hash/seal it, and advertise a
sealed generation for cloning. VM disks can clone those generations and grow
online. The constrained implementation is a starting point for the required
image/volume product, not complete Glance/Cinder equivalence. (S13–S15.)

## Management, identity and observation plane

The dashboard is the cluster browser entrypoint. It uses its ServiceAccount
for Kubernetes operations and its signed-in user's stored OIDC ID token for
VM-scoped Redfish/BMC calls. Redfish owns the management model; BMC owns
graphical console admission/transport. Workload mTLS checks the selected
service identity; user roles independently authorize the action. (S16–S20.)

SPIRE issues service SVIDs, including distinct manager, BMC and Redfish
identities. Optional guest identity uses the guest's vTPM and an in-guest
SPIRE agent; the guest relay forwards opaque bytes to a fixed upstream.
It does not hand the manager's identity to the guest. (S20, S23, S24.)

Redfish exports metrics/events. The observability bridge discovers current
VM/Pod bindings, scrapes over authenticated channels and validates constrained
history requests before local Prometheus access. Bounded histories can lose
old events; they are useful operational signals, not a durable complete
audit trail. (S26.)

## Ownership vocabulary

**Implementation owner** is the crate or external project responsible for
behavior. **Kubernetes controller owner** is an ownerReference used for
lifecycle/GC. **Runtime authority** is permission to affect a process, network
endpoint or data export. They must not be conflated: a label is not a
credential, a GC owner is not proof a disk is fenced, and a SPIFFE identity
is not a human role.
