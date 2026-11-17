# Boundary contracts

These contracts express the desired architecture and identify the source
that currently implements each seam. A contract can be narrower than the
capabilities of the underlying tool. A valid QMP command, for example, does
not automatically belong in the remote management API.

## Ownership and authority

| Boundary | Owner and permitted crossing | Required invariant | Violation to prevent | Source |
| --- | --- | --- | --- | --- |
| User → Kubernetes API | Kubernetes authenticates, authorizes and admits CR operations | Accepted identity and action policy precede persistent intent | Treating OIDC login alone as permission to create arbitrary privileged resources | S02, M01, U05 |
| Browser → dashboard | Dashboard session/role logic and Origin/CSRF checks | Invisible VMs remain invisible; mutations use the authenticated actor's allowed scope | A session cookie becoming a generic cluster-admin proxy | S16, S17 |
| Dashboard → Kubernetes | Dashboard ServiceAccount plus constrained admission | Backend checks the user before using its service authority | Claiming per-user Kubernetes impersonation when none occurs | S16, M01 |
| Dashboard → selected VM | Current controlled Pod binding, exact service SVID and user's bearer | Namespace, immutable VM UID and current Pod endpoint agree | Arbitrary destination proxying or cross-VM confused-deputy access | S17, S20 |
| Service peer → Redfish/BMC | Workload mTLS admission followed by user authorization | A service identity and a user role answer different questions | A dashboard SVID being enough to perform any VM action | S18–S20 |
| Sidecar → manager | Same-UID local control socket or trusted in-process control | Only bounded typed operations cross this seam | An unrestricted remote QMP/raw command gateway | S06 |
| Manager → QEMU | Private QMP/QGA/VNC sockets and owned child processes | Manager is the production QMP owner and reaps before releasing state | Two supervisors, leaked children or shared public QMP | S04–S07 |
| Protocol crate → runtime | Typed commands and Unix streams | No launcher/Kubernetes/hardware dependency in protocol behavior/tests | Requiring a guest boot to test JSON framing | S07, T01 |
| Operator → child resource | Controller ownership plus UID/resourceVersion binding | A name or familiar label is insufficient authority to patch/delete | Mutating a replacement or unrelated object's state | S02, S03, S08, A01 |
| VM → persistent identity | CR UID + PVC UID + runtime state binding/lease | Reuse/restart preserves identity; new VM creation does not clone old identity accidentally | Reinitializing corrupt state or taking a different VM's retained TPM | S05 |
| VM deletion → state-PVC GC | Exact launch/VM/claim provenance or established controller ownership | Manual, foreign and unproven claims remain outside automatic adoption | Label-only GC authority over another identity volume | A01, S16 |
| Manager → netd | Unix peer PID → exact host unified kubelet cgroup → watched local Pod → own NIC | Only this VM's own node-local NIC descriptors/controls | Arbitrary host scope names or user-supplied labels becoming authority | S09, S10, A05 |
| netd → anchor namespace | Separately privileged node helper | Plumb only intended anchors and grant bounded descriptors | Making every VM privileged or granting `/dev/net/tun` directly | S08–S10, S21 |
| Guest NIC → management | Separate network namespaces and Cilium identities | Guest packets do not share the VM management identity | A guest reaching loopback management listeners as a local peer | S08, S09 |
| Operator → network platform | Scoped pools/routes/labels through Kubernetes | FRR speaks BGP; Cilium owns policy/datapath | Manager programming cluster routes or netd becoming an alternate CNI | S08, M01 |
| Migration manager → manager | Exact same-VM SVID plus bounded protocol and temporary reachability policy | Transfer only compatible state for that immutable VM | Assuming an arbitrary same-name VM or claimed migration string is independent authorization | S11, S20 |
| netd → peer netd | Exact current node SVID plus same-VM/NIC/address authorization | Conntrack remains in node privilege boundary | Passing arbitrary conntrack records through a guest-adjacent manager | S12 |
| Operator → storage service | Exact service/exchange contract and scoped delegated verbs | Control authorization is separate from disk I/O | Manager acquiring root ZFS/LIO administration | S13, S14 |
| Replacement → native disk | Clean old process release or completed hard backend fence | No unknown old writer can coexist with a new export generation | Treating Pod deletion, a timeout or CHAP as a fence | S13, S14 |
| Image source → import stage | HTTPS + expected digest before image parsing | Job input/format/size/credentials remain bounded | Unverified image bytes reaching a privileged parser/writer | S15 |
| Guest QGA → host manager | Fixed command set and bounded frames/results | Guest output is untrusted; arbitrary exec is not exposed | Turning a cooperation channel into a general shell service | S22 |
| Guest workload → SPIRE | Separate guest agent/DevID and registered selectors | Guest workload identity is distinct from manager identity | Forwarding the Pod's manager SVID as guest attestation | S23, S24 |
| VM → Prometheus history | API-backed exact VM/Pod scope and fixed query templates | No arbitrary query or another VM's series | A telemetry endpoint becoming a general cluster query proxy | S26 |
| Source → release → cluster | Separate reviewed artifacts and dated live checks | Merge, publish and rollout are separate operations | Reporting a source fix as deployed behavior | M02, T01 |

Evidence IDs refer to [the register](evidence.md); A01 refers to the
[repaired adoption defect](audit.md). Ranges in the table denote the listed
adjacent register entries, not new composite evidence.

## Important trust assumptions

The node administrator, container runtime, hypervisor implementation,
privileged node helpers and storage administrator remain trusted. A software
TPM does not isolate its state from those authorities. Shared link encryption
does not eliminate endpoint compromise.

The management namespace is an administrative trust boundary. Labels,
annotations and ownerReferences are useful provenance under controlled API
write rights; they are not credentials against an actor allowed to forge the
same objects. The capability ledger does not assert hostile multi-tenancy
inside a namespace with arbitrary Pod creators, exec access or Secret-mount
rights. API admission and externally owned workload-use/image-pull policies
are part of the platform contract.

The manager control socket trusts its authorized local peers. BMC/Redfish are
therefore inside the VM's management authority boundary. Serving them in
separate containers reduces exposed filesystem/process access; it does not
make the bounded control API a second human-login system.

## Reconciliation is a protocol

A controller can crash between any two API/backend calls. Durable identity,
generation numbers, status, finalizers and backend journals make the next
attempt interpretable. A retry must not silently create a new identity,
re-export data to an unknown writer or delete an object that replaced the
one whose ownership was checked.

A finalizer is a cleanup coordination mechanism, not a security fence.
Force-deleting or stripping one bypasses its normal safety protocol; it is
not proof the underlying process, network endpoint or disk export is gone.
Error paths and cleanup paths require the same ownership checks as success.

## What counts as a violation during this audit

A confirmed violation crosses a stated authority/ownership boundary or loses
an invariant on a concrete path. It needs a reproducible scenario and an
appropriate verification method. Missing broad OpenStack features are scope
gaps unless a current interface promises them. Stale documentation is repaired
when it misstates the implemented/agreed boundary; it is not used to justify
silently changing a previously authorized lifecycle.
