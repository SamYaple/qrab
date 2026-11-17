# Presentation variants

The same evidence supports different stories. Keep the simple matched
OpenStack/qrab boundary pair at the center, and use detailed diagrams only
when their particular seam answers the audience's question. Raw diagrams,
captions and light/dark/print variants are in [the catalog](diagrams.md).

| Narrative | Open slides | Editable deck | PDF |
| --- | --- | --- | --- |
| Leadership / short | [HTML](exports/leadership.html) | [PPTX](exports/leadership.pptx) | [PDF](exports/leadership.pdf) |
| Engineering / detailed | [HTML](exports/engineering.html) | [PPTX](exports/engineering.pptx) | [PDF](exports/engineering.pdf) |
| Operations / workshop | [HTML](exports/operations.html) | [PPTX](exports/operations.pptx) | [PDF](exports/operations.pdf) |
| Network review | [HTML](exports/networking.html) | [PPTX](exports/networking.pptx) | [PDF](exports/networking.pdf) |
| Storage review | [HTML](exports/storage.html) | [PPTX](exports/storage.pptx) | [PDF](exports/storage.pdf) |
| Identity review | [HTML](exports/identity.html) | [PPTX](exports/identity.pptx) | [PDF](exports/identity.pdf) |
| OpenStack operator / cutover | [HTML](exports/operator.html) | [PPTX](exports/operator.pptx) | [PDF](exports/operator.pdf) |

The decks have speaker notes. HTML supports arrow keys, notes (`N`) and
fullscreen. Native boundary figures in PPTX are editable shapes; Mermaid
figures retain their separate editable source/SVG and use PNG in PowerPoint.
Use the [offline reader](atlas.html), [handbook PDF](exports/handbook.pdf),
[diagram portfolio](exports/diagrams.pdf), or [complete shareable bundle](exports/qrab-architecture.zip).
The bundle's reader/assets work offline; source-code and deeper subsystem
links still require the repository checkout. [Build instructions](../../contrib/docs/README.md)
explain regeneration and verification.

## Five-minute leadership story

**Opening:** We already run Kubernetes, Cilium and BGP. Today they host another
cloud control plane. The proposed change is to make those existing systems
own the VM lifecycle and network fabric directly, with a small VM-specific
operator and a manager beside QEMU.

**Show the OpenStack overview.** The services are in containers, but libvirt
still owns the QEMU domains in host cgroups. OVN supplies a separate guest
fabric while Cilium supplies Kubernetes networking. We are comparing that
specific deployment with a different qrab cluster.

**Show the matching qrab overview.** The VM is a Kubernetes object. The
operator reconciles it; the manager supervises QEMU in its container. Each
NIC has its own Cilium identity, separate from management. Kubernetes,
Cilium, FRR/BGP, QEMU/KVM and the underlying infrastructure remain.

**Explain the replacements.** Nova/libvirt move to Kubernetes, operator and
manager. Neutron/OVN move to Cilium, anchors, netd and scoped routing intent.
Keystone leaves the target, while existing OIDC, Kubernetes authorization and
SPIRE provide their distinct identity functions. Image and volume work is
already underway: verified imports, sealed images, clones, growth and fencing
exist. Finish the required image/volume recovery and product subset.

**Close on the decision:** accept the ownership model, prove the required
workload contract, then retire services when their last responsibility has
moved. This is a source-grounded design, not a measured cost/performance claim
or an assertion of full OpenStack compatibility.

Suggested visuals: `00-openstack-overview`, `00-qrab-overview`,
`03-service-replacements`, `05-cgroup-comparison`.

## Fifteen-minute engineering review

| Segment | Main claim | Best supporting material |
| --- | --- | --- |
| 1. Deployment versus ownership | Containerizing a service does not define where its VM processes live | Matched overview pair, cgroup comparison |
| 2. Control/data split | Kubernetes intent, bounded local commands, guest packets and disk I/O have different paths | Control/data map, code map |
| 3. Local VM lifecycle | One QMP owner; child cleanup and state leases survive failure/cancellation | Process containment, power state |
| 4. Network identity | Per-NIC anchor separates guest and management policy | Two-networks map, packet path |
| 5. Migration | Fenced target and explicit ownership transfer; local Cilium fork is a dependency | Migration streams/state/sequence |
| 6. Identity | Human grants, service admission and guest identity remain separate | Identity questions, management request |
| 7. Image/disk subset | Existing implementation is useful; fencing/recovery determine safe replacement | Storage control/data, image import, disk lifecycle |
| 8. Evidence and bugs | Concrete boundary defects were reproduced and repaired | Audit, rejected hypotheses and scoped verification evidence |
| 9. Remaining decisions | Workload-specific feature and operational acceptance | Capability ledger, transition plan |

For discussion, ask which invariant must remain true if a controller crashes
between any two calls. That question exposes the useful comparison with the
old ownership model more effectively than counting service binaries.

## Twenty-minute operations workshop

Use a failure-first sequence: the guest is off but management is up; a NIC
cannot reach the world; the storage API is down but guest I/O continues; a
node disappears; a migration has two visible Pods; a deletion waits on one
finalizer; a user can log in but cannot power a VM. In each case, identify the
authoritative owner, the evidence to collect and the unsafe inference to
avoid. The [operations matrix](operations.md) supplies the answers.

Then compare the old and target responsibilities: fewer duplicated control
planes should make those ownership questions easier to answer, but the
target still requires disciplined storage, networking, identity and release
operations. Use the current-state maps to assign on-call ownership and the
capability ledger to decide which drills must pass for a workload.

Suggested visuals: power state, failure domains, storage control/data,
migration streams, deletion ownership and release boundaries.

## One-page handout

Use [the executive brief](executive.md) and the matched overview pair. Keep
the replacement table and three statements: QEMU/KVM remain; one Cilium fabric
does not mean one shared guest/management identity; image/volume recovery work
has explicit remaining scope. Put the source baseline/evidence date in the
footer. Link the atlas rather than shrinking a detailed sequence onto a
single unreadable page.

## Whiteboard variant

Draw three boxes: Kubernetes intent, a VM Pod, and a NIC anchor. Put manager
and QEMU inside the VM Pod; put TAP/routing/Cilium identity in the anchor.
Draw a control line from API/operator to Pod configuration, a local manager
control line to QEMU, and a guest packet line to the anchor. Add storage
outside the Pod, with separate provisioning and I/O arrows. Finally add OIDC
for the human and SPIFFE for the service peers. This exposes every major
boundary without requiring the audience to learn the crate names first.

## Editing and exporting

The editable document sources live in this directory. Diagram JSON sources
describe deliberately positioned slide-sized vector layouts; Mermaid sources
describe technical flows/states/sequences. Rendered SVGs are portable text/vector
assets, and PNGs provide presentation-tool compatibility. The document build
tools under `contrib/docs` record and validate their input/output provenance.
Generated reading/presentation exports are documentation artifacts; no website
hosting, service deployment or external message is part of this task.

## Focused subsystem variants

The networking, storage and identity decks are additional audience-specific
narratives. Networking starts from the two fabrics in the supplied OpenStack
cluster, then follows guest endpoint identity, descriptor control and handoff.
Storage starts from surviving Glance/Cinder responsibilities, separates
provisioning from I/O, and finishes with import integrity, fences and recovery.
Identity separates human login, application roles, workload admission and
guest/storage authority. Each has its own notes and evidence references.

Use [the operating model](operating-model.md) to explain why a crate graph, a
Pod diagram and a responsibility map differ. Use [verification](verification.md)
for a skeptical review of what the collected tests actually establish.

The latest explicit networking verification includes the previously skipped
WireGuard cases; [verification](verification.md) distinguishes that local
provider fixture from production VPN and Cilium/BGP acceptance.

## OpenStack operator and cutover workshop

The operator deck starts from daily tasks, then follows one workload between
the two clusters. It separates guest Off from declarative Pod removal,
container resize from guest onlining, disk recovery from rescheduling, and
data cutover from network authority. It finishes with rollback after accepted
writes, the desired recovery bundle and the different credential/deletion
lifetimes. This is a practical handover story for an OpenStack operator.

Use [the task translator](operator-translation.md) as the desk reference,
[the cutover storyboard](cutover-story.md) as the rehearsal narrative and
[continuity](continuity.md) for the recovery discussion. Cross-platform
live migration and a complete backup/restore product remain unestablished.
