# qrab and OpenStack: architecture atlas

qrab uses Kubernetes as the VM control plane, QEMU as the hypervisor process,
and Cilium as the guest network datapath. The intended simplification is to
remove the parallel OpenStack control planes while keeping their required
responsibilities explicit.

This atlas compares **two different clusters**. The OpenStack baseline is the
deployment described by the operator: OpenStack services and libvirt run in
Kubernetes containers; libvirt launches QEMU in its traditional host cgroups;
OVN connects the guests; Cilium connects Kubernetes; frr-k8s/BGP provides the
routing foundation. The other cluster runs qrab. These documents do not claim
to have inspected the OpenStack cluster or identified its storage backend.

The source baseline for this investigation is `5f361b7`, dated 2026-11-17.
Later fixes and validation are identified in [the audit](audit.md). Source
support, manifest intent, dated deployment evidence, and desired state are
different evidence classes throughout the atlas.

## Choose a reading path

| Audience / time | Start here | What it answers |
| --- | --- | --- |
| Presenter / five minutes | [Executive brief](executive.md) | What remains, what is replaced, and why |
| OpenStack operator | [Service replacement map](replacement-map.md), [task translator](operator-translation.md) | Where each familiar responsibility and daily operation moves |
| Architecture review | [Current structure](current-state.md), [desired state](desired-state.md) | What the code does versus what we intend |
| Rust developer | [Code map](code-map.md), [boundary contracts](boundaries.md) | Which crate owns an operation and where to change it |
| Network engineer | [Network comparison](network.md) | OVN/Cilium separation versus per-NIC Cilium endpoints |
| Storage engineer | [Images and volumes](storage.md) | Existing image/disk work, fencing, and remaining scope |
| Identity / security reviewer | [Identity and trust](identity.md), [threat model](security.md) | Human, workload, guest and storage authority |
| Tenant / API policy reviewer | [Tenancy perspective](tenancy.md) | Separate VM roles, API rights, platform trust and remaining tenant-product scope |
| Cross-cutting architecture reviewer | [Authority and lifetimes](authority.md) | Bind a checked identity to the resource actually used |
| Team / system owner | [Operating model](operating-model.md) | Separate roles, processes, dependencies and operational ownership |
| Recovery reviewer | [Resource lifetimes and recovery](recovery.md) | Namespace replacement, held queues, power and safe restart |
| Backup / identity recovery owner | [Continuity](continuity.md) | What must survive together and which revocation clocks differ |
| Test / evidence reviewer | [Verification perspective](verification.md) | Exactly what local and real-peer checks establish |
| On-call engineer | [Failure and recovery](operations.md), [lifecycle flows](lifecycle.md) | Which failures stop which operations |
| Migration planner | [Transition plan](transition.md), [cutover story](cutover-story.md), [capability ledger](capabilities.md) | What must be proved before moving a workload |
| Skeptical reviewer | [Questions and answers](questions.md), [evidence register](evidence.md) | Limits, assumptions, and supporting source |
| Visual reader | [Diagram catalog](diagrams.md) | Reusable boundary, flow, sequence and state diagrams |
| OpenStack reference reader | [Original upstream diagrams](upstream-diagrams.md) | Attributed OVN reference figures, separate from the supplied deployment |
| New team member | [Field guide](field-guide.md), [glossary](glossary.md) | The same system in plain language |
| Decision recorder | [Architecture decisions](decisions.md) | Proposed invariants and their consequences |
| Presenter / multiple styles | [Presentation guide](presentations.md) | Short, technical and operations narratives |

The [existing connection atlas](../connections.md) remains the detailed source
for ports, credentials, encryption segments and capture locations. This atlas
explains ownership and the OpenStack comparison; it links to subsystem guides
instead of replacing their configuration and runbooks.

## The presentation in one table

| Keep the foundation | Replace the service | Keep doing the work |
| --- | --- | --- |
| Kubernetes API, admission, scheduler, kubelet and container runtime | Nova / Placement control plane and libvirt supervision | Placement, resource admission, process lifecycle, migration and recovery |
| Cilium, frr-k8s/BGP and the physical network | Neutron and the OVN guest fabric | Address claims, guest attachment, security policy, DHCP/seed configuration and route handoff |
| External identity provider, Kubernetes auth/RBAC, SPIRE | Keystone | Authenticate humans, authorize actions, identify workloads and rotate credentials |
| Storage devices and a separately owned storage service | Glance and Cinder APIs/control loops | Import, verify, catalog, clone, export, grow, fence, retain, recover and eventually destroy data |
| QEMU/KVM, Linux, firmware and persistent VM identity | Host-level libvirt domain ownership | Run and supervise QEMU inside the VM Pod; preserve identity across the intended lifecycle |

Removing a service is a design decision. Equivalent functionality, isolation,
operability and recovery are acceptance criteria, not consequences of deleting
its deployment. No OpenStack API compatibility or complete feature parity is
claimed.

## Evidence labels

**Implemented** means the inspected source contains the behavior. **Manifest**
means checked-in resources request it. **Dated live evidence** means an existing
record describes a particular release and test. **Desired** means the proposed
contract or remaining work. **Unknown** means the available evidence does not
establish the fact. An implemented feature is not automatically a deployed
feature, and a successful unit test is not a live-cluster proof.

## Maintaining this atlas

Update the code map and the affected contract when ownership changes. Update
the relevant diagram and its evidence references together. Put investigation
progress, bugs, commits and operational notes in the `qrab` project in towl;
the atlas contains reviewable architecture and dated evidence, not a live work
queue. Image publication and cluster rollout are separate operations.
