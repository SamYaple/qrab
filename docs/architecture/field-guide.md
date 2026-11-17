# A guided walk through one VM

Imagine a VM named `web`. You want it to boot a known image, hold its data,
have one guest network interface and be manageable from a browser.

You create a `VirtualMachine` resource. Kubernetes checks who you are and
whether the request is allowed. The qrab operator reads the current object
and works out what must exist. The name `web` is the human handle; the VM's
immutable UID is the identity the runtime and its owned resources bind to.

If you asked for a disk, the operator uses the storage service to provision
it. A root storage helper manages the actual ZFS/LIO objects. QEMU will later
open the disk directly over iSCSI; your disk bytes do not take a detour
through the operator. The small filesystem volume holding firmware variables
and the virtual TPM is a different volume from the guest's root disk.

Kubernetes places the VM Pod on a node. Inside it, a Rust manager starts and
supervises QEMU and swtpm. QEMU is a child in the manager container's lifecycle
and resource boundary. There is no libvirt domain managing this VM outside
its Pod cgroup. QEMU and KVM still do the virtualization work; qrab did not
replace them with another hypervisor.

The manager creates private QMP, VNC and guest-agent sockets. You do not
connect to them from the internet. Redfish translates authenticated management
requests into bounded manager operations; BMC handles the graphical console.
The manager is the one process that owns production QMP control.

Your guest NIC gets another Pod: its anchor. That Pod has the address and
Cilium identity for the NIC. A privileged node helper creates its TAP and
vhost resources and gives QEMU the descriptors it needs. Packets move through
kernel queues into the anchor's namespace and Cilium. The guest does not
share the management Pod's network identity or its loopback listeners.

Security groups become anchor labels selected by real Cilium policies. A
label alone does not create a firewall. The platform must install the
intended baseline and allows. FRR still distributes routes using BGP; the
operator writes the small slice of routing intent belonging to the NIC.

When you open the dashboard, the existing OIDC provider identifies you.
The dashboard keeps your token on the server and checks whether you can see
or manage `web`. It calls the selected VM's service over workload mTLS and
forwards your user token. The service checks both its calling workload and
your role. Kubernetes API auth, a human login and a service certificate are
three different decisions.

If you power `web` off through Redfish, QEMU stops but manager stays reachable
to turn it back on. If you set `spec.running: false`, the operator removes
the VM Pod. If the node disappears, the operator cannot assume the old disk
writer is gone; safe recovery requires clean ownership evidence or a hard
storage fence.

If you live-migrate `web` within qrab, a target manager prepares empty state,
authenticates the same VM source and receives its memory/device/identity
state. Shared disks stay in storage. NIC ownership switches to a fenced
target anchor, with separate route and conntrack work. This is not an
OpenStack-to-qrab migration mechanism.

If you delete `web`, several owners clean up their pieces. Proven owned
control resources and a same-launch dashboard state claim can be collected.
Manual/unproven identity claims are retained. Guest exports are fenced;
retirement and a separately authorized host reaper govern eventual guest-disk
destruction. Access groups and guest registrations have their own cleanup.
"Delete" therefore has a specific meaning for each object.

That is the OpenStack comparison: Kubernetes owns the intent Nova previously
coordinated, manager replaces local libvirt supervision, Cilium replaces the
OVN guest fabric, and existing auth/identity systems replace Keystone's role
in the target. Image and volume responsibilities still need a complete
required subset, even though much of their foundation already exists.

For the technical version of this walk, see [lifecycle flows](lifecycle.md),
[code owners](code-map.md) and [the replacement map](replacement-map.md).
