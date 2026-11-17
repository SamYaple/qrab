# Desired state: Kubernetes owns intent; qrab owns the VM-specific seams

The desired architecture removes Nova, libvirt, Neutron/OVN and Keystone
from the qrab target while preserving the functions the operator actually
needs. Glance/Cinder replacement is a scoped product effort built on the
existing image/disk foundation. This is a target contract; a statement below
is not proof every path already satisfies it.

## 1. One declarative source of VM intent

Kubernetes resources own VM configuration, placement constraints and
long-lived lifecycle intent. The operator converges those resources and
reports precise status. It must reread current objects, validate controller
ownership before mutation, use UID/resourceVersion preconditions where races
matter, and keep retries safe. A same-name object is never sufficient proof
of identity or ownership.

Bounded imperative operations belong in the authenticated VM management
surface. Desired-state changes and actions have documented reconciliation
semantics: powering a guest off in place is different from requesting no VM
Pod with `spec.running: false`. The UI must not silently turn one into the
other.

Kubernetes remains responsible for admission, scheduling, container cgroups,
device assignment and Pod termination. qrab does not recreate a general
placement database or message broker beside it. Requirements that Kubernetes
does not satisfy by default—VM CPU compatibility, topology, safe migration
admission and capacity policy—remain explicit qrab/platform responsibilities.

## 2. One local QEMU owner per execution instance

The manager supervises QEMU, swtpm, private sockets, state leases and bounded
local control. Typed QMP/QGA libraries implement protocol behavior; they do
not grow launcher, Kubernetes or firmware-download dependencies. BMC and
Redfish enforce user authorization before asking the manager to act.

QEMU stays in its VM manager container's process/resource lifecycle. Node
helpers may grant narrowly scoped capabilities but do not become alternate
VM supervisors. A child must be terminated/reaped before state authority is
released. Failed/unknown ownership must block unsafe replacement rather than
being erased by a timeout.

## 3. One Cilium datapath, with separate identities

Guest NICs use the existing Cilium fabric. Every NIC has a policy identity
independent of the management Pod and of other guest NICs. A guest never
joins the management network namespace. Address ownership, DHCP/seed behavior,
DNS, routing, egress and policy defaults have named owners.

FRR/frr-k8s and BGP remain the routing foundation. qrab writes only the scoped
route intent it owns; it does not speak BGP inside manager or implement an
alternate overlay. A networking story that omits netd, address claims and
handoff is incomplete. The local Cilium fork required for live handoff is
treated as a versioned integration contract, with upgrade and rollback
acceptance, rather than described as an upstream guarantee.

Default-deny and required egress are platform policy decisions with tested
examples. Cilium being installed does not by itself establish guest isolation
or encrypted traffic. Encryption is documented per segment; guest application
security and transport protection remain distinct.

## 4. Three identity questions, three explicit answers

Who is the human? The existing OIDC identity provider answers. May that human
perform this operation? Kubernetes RBAC/admission or the management
application's VM-scoped roles answer, depending on the entrypoint. Which
service is calling? SPIFFE/SPIRE workload admission answers.

Keystone is absent from the target, but the identity provider and policy
administration remain. Kubernetes API authentication is not automatically a
Redfish login; a valid SVID is not automatically a console-admin grant. Guest
workload identity is separate again, VM-UID-bound and optional. Revocation,
expiry, replay handling, rotation and name reuse must preserve those scopes.

## 5. Storage control never substitutes for storage fencing

The storage backend owns durable bytes and hard export/session fences. The
operator owns orchestration and credential generation. QEMU owns its native
disk session. The control channel does not carry the block payload.

Every replacement/reattachment must either prove the old writer exited
cleanly or obtain the required backend hard fence. Pod deletion, node
unreachability, CHAP authentication and lease expiry are not fences. State
identity and guest disk data remain distinct durable objects with deliberate
backup/recovery requirements.

Deletion authority is scoped to exact provenance. VM-owned ephemeral control
resources can be collected; manual, foreign or unproven identity storage
cannot be swept merely because a name or label looks familiar. Retention,
retirement and irreversible destruction have separate owners and timelines.

## 6. Finish a deliberate image/volume subset

Build on sealed `BaseImage` generations, checksum-verified imports, constrained
Jobs, clone origin verification, disk export/growth and fencing. Define the
required image publication/sharing/distribution policy and the required volume
backup, restore, quotas, retention and recovery workflows before calling the
Glance/Cinder replacement complete.

Do not introduce a generic storage abstraction merely to imitate the old API.
Use the actual required backend contract, with a clear extension boundary if
another backend is later needed. Do not claim that CSI supplies the current
native guest LUN path: it supplies separate filesystem state volumes today.

## 7. Cross-node moves are explicit distributed ownership transfers

The target must authenticate the exact VM source, prepare an empty identity
state, remain fenced until handoff, and have compatible CPU/machine, storage
and auxiliary-volume access. The source retains ownership until the protocol
commits the transfer. The operator adopts the target only from documented
completion evidence. Cleanup must never fence the new owner as if it were a
failed old instance.

RAM/device/TPM/firmware migration, node conntrack exchange, temporary guest
forwarding and shared disk I/O are separate streams. Each has its own
authorization and failure semantics. Unsupported management state or
single-attach auxiliary claims should cause a clear refusal before target
side effects, until a complete transfer contract is implemented.

## 8. Evidence and release boundaries are first-class

Tests prove their stated scope. Protocol checks remain hardware/cluster-free;
kernel, boot, storage and cluster checks are explicit. A silently skipped
privileged test is not a datapath proof. Diagrams and source references must
distinguish source support, rendered manifests, dated live evidence and future
intent.

Source changes, image builds/publication and cluster rollouts remain separate
operations. This investigation authorizes code fixes and documentation, not
an automatic production deployment. Direct merges follow the repository's
maintenance policy; operational evidence stays in qrab towl.
