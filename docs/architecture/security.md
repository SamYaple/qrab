# Security review: trust moves; it does not vanish

The desired simplification narrows exposed VM-specific control and reuses
Kubernetes/Cilium/identity policy. It retains a trusted node kernel,
hypervisor, privileged attachment helpers and storage administrator. It is
not a confidential-computing design and does not promise hostile multi-tenancy
against arbitrary writers in the management namespace.

## Assets and adversaries

| Asset / invariant | Relevant threat | Boundary / mitigation to assess |
| --- | --- | --- |
| A VM's memory, CPU and device state | Another workload or guest reaching control sockets | Private QMP/VNC/QGA session paths; bounded manager control; Pod/UID/file permissions |
| Per-VM management authority | Confused deputy or cross-VM proxy target | Current controlled Pod/VM UID binding, exact service SVID and separate user role |
| User tokens and sessions | Browser credential theft, CSRF or unintended forwarding | Server-side token storage, opaque cookies, exact Origin/CSRF, bounded token/session lifetime |
| UEFI/vTPM identity | Accidental reset, duplicate copy, foreign GC adoption | VM/PVC UID bindings, retained state lease, launch-specific deletion provenance |
| Guest network identity | Management reachability, spoofing or incorrect endpoint handoff | Separate anchor namespaces, Cilium identity/policy, netd peer authorization and standby fencing |
| Native guest disk integrity | Two writers after partition or unknown termination | Clean release evidence or hard backend fence and credential-generation rotation |
| Host network/kernel privilege | Unbounded FD grants or arbitrary namespace/control requests | Separate privileged netd, bounded local protocol, peer-Pod/own-NIC checks |
| Conntrack state | Cross-VM disclosure or state injection | Exact-node mTLS and API-backed same-VM/NIC/address export/import authorization |
| Executable artifacts | Unverified or mutable image/firmware input | Digest-pinned runtime releases, verified image import, sealed generation/catalog policies |
| Guest identity claims | Confusing Pod identity with guest workload identity | Separate guest agent/vTPM/selector registration; opaque relay |
| Monitoring data | Query escalation or another VM's series exposure | Fresh API-owned VM/Pod inventory and fixed scoped history templates |
| Cleanup authority | Mutable access grants or labels widening deletion scope | Exact provenance, existing-controller protection and UID/resourceVersion preconditions |

These controls need their actual implementations, tests and deployment
configuration checked; the table is a review model. Source references are in
[boundary contracts](boundaries.md), and confirmed defects/repairs are in
[the audit](audit.md).

## Human, workload and local authority

Workload mTLS establishes the service peer. It does not authorize the human's
power/console action. OIDC claims establish a subject/group set; application
roles still decide the operation. Kubernetes protects declarative API changes;
the dashboard uses a ServiceAccount rather than automatic per-user
impersonation. The manager local socket trusts authorized same-UID peers,
so its sidecars are inside its bounded management authority.

Labels and annotations are public provenance under protected API write
rights. They are not secrets or independent proof against a namespace
administrator. A user allowed arbitrary Pod creation, exec or Secret mounts
in that namespace has substantially different authority from a VM user.
Do not describe those two tenancy models as equivalent.

The [tenancy perspective](tenancy.md) separates current VM application roles,
Kubernetes API actors, workload peers and platform administrators, and states
the remaining contract for broader tenant self-service.

## Encryption ends somewhere

| Protection | Endpoint / scope | Outside that claim |
| --- | --- | --- |
| Frontend HTTPS | Browser/native client to ingress | The next backend hop unless separately secured |
| Workload mTLS | Exact admitted service peers on management/migration/conntrack paths | Human role, guest packet encryption, disk data at rest |
| Cilium Pod WireGuard | The verified configured managed Pod/node segment | Same-node local paths, host/world-origin cases outside its policy |
| Native MACsec | The documented node/gateway Ethernet peer segment | Local plaintext stacks and traffic beyond the peer |
| VPN NIC WireGuard | Anchor to configured provider peer | Guest-to-anchor plaintext and traffic after the provider |
| CHAP | iSCSI login authentication | Payload confidentiality or one-writer fencing |
| Unix socket permissions / peer credentials | Local process authority | Encryption or protection from privileged node administrators |

See the maintained [connection inventory](../connections.md),
[WireGuard guide](../node-encryption.md) and
[native-link guide](../node-link-encryption.md) for exact dated evidence and
exceptions. Do not infer encryption merely from a component being installed.

## Unsupported state should refuse clearly

A migration that cannot transfer management policy, attach auxiliary claims
safely or preserve identity must refuse before unsafe side effects. Unknown
old writers must remain blocked until fenced. A user request that names an
unowned resource must not silently adopt it. A timeout may mean the command
executed and its reply was lost; mutation retry policy must account for that.

The security goal is an understandable authority chain with tested refusal
paths. Fewer service names alone do not prove a smaller attack surface or
better security; exposure, privilege, dependency maintenance and recovery
semantics still need evidence.
