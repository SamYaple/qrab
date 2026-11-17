# Architecture boundary audit

This register contains source-backed findings and verified repairs from the
architecture investigation begun on 2026-11-17. Work claims, detailed progress
and operational notes live in the `qrab` project in towl. It is not a claim
that every possible violation has been found or every deployed image contains
these repairs.

## A01 — state-PVC adoption crossed the launch ownership boundary

**Affected source:** `resources::state_claim_adoption` in the operator and
`launch::render_pvc` in the dashboard. **Baseline:** `5f361b7`.

The operator previously granted a VM controller ownership of a state claim
when the claim carried `app.kubernetes.io/managed-by: qrab-dashboard` and had
no existing controller. It did not verify the intended VM or launch. A VM
referencing another dashboard-created claim could therefore acquire
garbage-collection authority over that identity volume. The manager's later
VM-identity check did not undo this earlier ownership mutation. A name reused
after an earlier launch was another ambiguity.

The repair requires both resources to carry the dashboard label and the same
valid random launch ID; the claim must also match the VM annotation,
namespace and referenced state-claim name. Deleting resources and claims
with an existing controller are not adopted. The patch retains exact
UID/resourceVersion preconditions and preserves other non-controller owners.
New dashboard PVCs record the launch provenance. Legacy/reused claims without
the proof remain retained; established controller ownership is unchanged.

The regression reproduced the foreign-VM adoption failure before the change.
It passes after the change, covering foreign/stale/missing provenance,
namespace/name mismatch, invalid IDs, deletion and existing controllers. A
dashboard test also checks the actual rendered VM/PVC contract against the
operator adoption function. The full `contrib/check-runtime.sh` passed:
formatting, workspace tests, generated CRD/policy comparisons and strict
Clippy. These checks are hermetic; no live cluster deletion was exercised.

**Merged source:** `719335966b72ba33332f53ad8d9a36812771c965`.
**Tracking:** qrab `work/state-pvc-adoption-launch-boundary`.
The primary main checkout was synchronized. This audit performed source work
and tests; image publication and deployment have separate evidence.

The provenance annotations are public trusted Kubernetes metadata, not secrets
or cryptographic credentials. An actor with broad authority to forge or
rewrite those objects remains inside the namespace administration trust
boundary. This fix scopes normal cleanup; it does not turn a namespace with
unrestricted Pod/resource writers into a hostile multi-tenant environment.

## Documentation boundary reconciled with the agreed deletion design

Some older operator comments/guides said state PVCs were never collected,
while the implemented dashboard deletion path already adopted dashboard
claims. The user-approved deletion design recorded in qrab towl
`container/vm-deletion-cleanup` permits collection of the dashboard launch's
own claim and retains manual claims. The repair updates the operator API
description, generated CRD, crate instructions and subsystem documentation to
state this narrow exception and its provenance checks.

PVC deletion is not backing-data destruction. The documented Retain-only
storage contract leaves a Released PV/zvol for a separately owned cleanup
workflow. Guest disk exports, retained identity state and group cleanup are
different ownership domains and must stay distinct in presentations.

## A02 — access grants incorrectly determined group cleanup authority

**Affected source:** dashboard `cleanup::reconcile_vm` and `cleanup::owned`.
**Repair baseline:** `0301347` (the source advanced independently during the
investigation).

The finalizer scanned current console admin/viewer grants instead of the
group UUIDs recorded at launch. Removing an original group from those grants
left it behind at deletion. A generated group from a previous launch of the
same namespace/VM name also passed the old name/description ownership check.
Access membership had incorrectly become lifecycle ownership.

Cleanup now uses the recorded launch ID and admin/viewer UUIDs, validates the
references before deletion and checks the exact launch-derived group name,
returned UUID and namespace/VM/role description. Current console lists do not
widen or erase that authority. Pre-launch-ID VMs look up only their exact
legacy names; incomplete modern provenance does not fall back to legacy.
Missing groups are already cleaned; invalid/mismatched provenance and service
outages keep the finalizer for a safe retry or explicit correction.

The amended regression failed before the repair because the original groups
were leaked. It passes after the repair and preserves platform, other-VM and
previous same-name-launch groups. Further coverage checks corrupt/missing
metadata, foreign UUIDs, legacy cleanup and outage behavior. Full dashboard
tests and `contrib/check-runtime.sh` passed. No live Kanidm groups were deleted
by these hermetic checks.

**Merged source:** `8a30586610cc80073076859454ce2554215fb2bd`.
**Tracking:** qrab `work/access-group-cleanup-launch-boundary`.
Primary main was synchronized. The audit performed no runtime release or
cluster rollout.

As with A01, the annotations are public trusted metadata. Broad namespace/API
administrators remain trusted. The repair does not claim an immutable or
cryptographic attestation of arbitrary client-supplied annotations.

## A03 — network security guide retained a plaintext-era description

The detailed networking security section still called the node conntrack
exchange plain TCP, despite the implemented exact-node SPIFFE TLS path and
the newer connection-atlas/enrollment evidence. The atlas work corrects that
paragraph to match `peer_tls` and the existing rollout record. On-link address
checks remain additional authorization, not a substitute for peer identity.
This is a documentation repair, not a new transport implementation or rollout.

## A04 — verified image metadata could select extra unverified inputs

**Affected source:** operator `image_job::virtual_size` and the inspect/write
Job path. **Repair baseline:** `8a30586`.

The importer checked the downloaded file's digest, then allowed qemu-img to
convert qcow2 metadata that could name backing or external-data files. Those
extra local/network inputs are not covered by the top-level digest and may
cross the Job's filesystem/credential boundary. A harmless temporary marker
fixture demonstrated the conversion mechanism; no real credential or live
storage was read.

Both Job modes now run a bounded qcow2 dependency preflight before any
qemu-img invocation. It reads the fixed v2/v3 header, rejects backing
offset/length and the external-data feature, and fails on truncated or
unsupported fixed headers. Raw remains an opaque single file. This is not a
new general qcow2 parser: QEMU still performs full format validation.

Default tests require no QEMU and cover dependencies, truncated/malformed
headers, bounded reads and actual entrypoint refusal before tool launch. An
explicit fixture test passed on QEMU **11.1.3**, built locally from the pinned
`vendor/qemu` source with downloads disabled. It checks standalone v2/v3 and
extended-L2 inputs, rejects both dependency classes, and demonstrates the
unguarded baseline reading the separate benign marker. The full runtime
check also passed. This did not test live storage, libiscsi or a guest boot.

**Merged source:** `69ef4274d6ba973bb696371c97fb0716a1e4206c`.
**Tracking:** qrab `work/image-import-self-contained-qcow2`.
Primary main was synchronized. The audit performed no production image
build/publication or cluster rollout.

The legacy staging script accepts only a reviewed hardcoded CirrOS URL/digest,
not arbitrary BaseImage sources. The general inspect/write Job interface is
the repaired boundary. Prepared backing chains must be flattened in their
trusted preparation environment before import.

## A05 — host cgroup names could impersonate Pod identity

**Affected source:** netd `cgroup::pod_uid` and its use by socket-caller and
anchor-process lookup. **Repair baseline:** `93b9792`.

The old helper searched every cgroup path component for a `pod<UUID>`
substring. An unprivileged host user can create a transient user scope whose
name contains that text. A harmless local `systemd-run --user --scope`
fixture produced exactly such a kernel-reported path, and the old parser
returned its fake Pod UID. netd's caller mapping and anchor process selection
used that result as authority. A kernel-reported string still needs its
hierarchy and component semantics validated.

The repair accepts one host-view unified-cgroup-v2 `0::` record and exact Pod
components under kubelet's standard `/kubepods.slice` or `/kubepods` roots,
including their valid QoS paths. It rejects user/delegated roots, partial
names, extra records and unrooted views. The same corrected mapping protects
anchor PID selection, so a lower-numbered host lookalike cannot be chosen
instead of the real Pod process. Custom roots and cgroup-v1/multi-record
views fail closed; the node daemon requires the documented host unified view.

The regression failed before the fix and passes after it. Coverage includes
both drivers/all QoS classes, nested lookalikes, malformed/multiple records
and fake-proc caller/anchor selection. The netd suite, client-only
`--no-default-features` check and full runtime check passed. A bounded
read-only inspection confirmed the deployed netd's standard host-rooted
systemd view. No live VM descriptor grant, NIC control or namespace/network
mutation was attempted; unprivileged kernel-netns tests are not claimed as
live datapath proof.

**Merged source:** `4625eefabb3743b3ec775b4f547a6b873cd162bf`.
**Tracking:** qrab `work/netd-cgroup-hierarchy-authentication`.
Primary main was synchronized. This audit did not publish a runtime image or
roll out the repair; later release workflows require their own evidence.

## A06 — process identity checks could be separated from resource use

**Affected source:** netd caller mapping, anchor namespace selection and
resolver/VPN-file reads. **Repair baseline:** `2b601f4`.

The old paths read a numeric PID's cgroup and later reopened `/proc/<pid>`
resources. A PID/path replacement between those steps could substitute another
process, namespace or mounted file. A socket's saved numeric peer PID also
does not pin the original connector's lifetime. This was a source-identified
race; no live exploit or VM grant was attempted.

The repair captures the original Unix peer with `SO_PEERPIDFD`, checks its
readiness/liveness during admission, and rechecks membership after the bounded
request read. Process-directory descriptors are opened before validation;
subsequent namespace, resolver and bounded VPN reads use `openat` relative to
that held directory. Namespace workers, queue grants and DHCP receive opened
namespace files. Numeric PID/path authorization helpers were removed.

Default tests replace/rename private fake-proc directories and demonstrate the
old path resolving replacement data while held directory/namespace handles
retain the original resource. Bounds, relative paths, symbolic PID directories
and close-on-exec behavior are covered. An explicit supported-kernel fixture
with a private child/Unix socket ran successfully and refuses the original
peer after exit even when a claimed numeric path is replaced. The netd suite,
client-only feature check and full runtime check passed. Kernel-netns tests
that self-skip under an unprivileged user are not live datapath evidence.

The server now probes `SO_PEERPIDFD` support at startup (Linux 6.5+) and fails
closed without it. The client-only build has no new kernel/API dependency.
Current native node metadata reports 7.0 kernels; the peer fixture ran locally
on 6.18.50. This does not claim feature/rollout acceptance on the nodes.

**Merged source:** `095edadb84e4a383ab1ef55a09839d873e403227`.
**Tracking:** qrab `work/netd-stable-process-authority`.
Primary main was synchronized; this audit did not build/publish runtime
images or roll out a cluster. The [authority perspective](authority.md) generalizes the review.

## A07 — a Pod UID did not identify a replacement NIC sandbox

**Affected source:** netd's plumbed/DHCP caches, grant/probe token and manager
NIC binding. **Repair baseline:** `c10ca28`.

A Kubernetes Pod can keep its UID while CRI recreates its network namespace.
The old cache and attachment token named only the UID. QEMU queues and the
DHCP socket could retain the old namespace even after the helper prepared a
new one; manager's token comparison would see no change.

A plumbed context now holds the validated process and opened namespace. Its
opaque token combines the Pod UID with that namespace's device/inode identity.
Healthy checks verify the held process still occupies the same namespace;
stale contexts invalidate readiness and stop their DHCP workers. Grants and
probes use the same current context. An agent restart observing the same
namespace produces the same token. Failed DHCP workers can be retried rather
than remaining cached as live.

Default file/context tests cover same-UID replacement, token stability,
process reassignment/exit and worker lifetime. The actual manager reconciliation
fixture demonstrates namespace-only token changes causing device removal,
guest-release confirmation, backend removal and fresh queue attachment, with
the same configured MAC and root port. Full runtime, node suite and client-only
checks passed.

An explicit isolated user/network-namespace run exercised four kernel fixtures:
namespace replacement/descriptor lifetime, TAP queue transfer, forwarding
routes and DHCP. The old namespace remained accessible through its file after
the namespace-holding task exited; the replacement did not contain its link.
Two VPN fixtures explicitly skipped without WireGuard tools. Dummy uplinks
are not a Cilium/BGP or real-guest hot-unplug proof.

**Merged source:** `69d96eea790363d333d116c0da0a4d17ce489ecb`.
**Tracking:** qrab `work/nic-anchor-namespace-incarnation`.
The audit changed source and performed isolated local tests; cluster sandbox
replacement and release operations require their own evidence.

## A08 — stream framing could reject valid replies and leak installed FDs

**Affected source:** netd `wire` reply send/receive and connection setup.
**Repair baseline:** `69d96ee` plus an independent image-pin-only update.

Grant decoding assumed one `recvmsg` contained all JSON, and probe/handoff
control assumed one read contained the reply. Unix streams permit fragmentation.
The older conntrack-count helper read to EOF, but its per-read timeout did not
bound a peer that kept dribbling bytes. A second reproduced failure was more
subtle: on `MSG_CTRUNC`, nix 0.29 refused to expose the installed subset of
SCM_RIGHTS descriptors, so the error path leaked them.

New senders terminate a bounded JSON line and finish short writes without
sending rights twice. The shared reader accumulates fragments within total
time/byte/descriptor bounds. It owns every delivered descriptor before refusing
ancillary truncation or parsing JSON, checks queue/count consistency, and closes
rights on every failure. Complete legacy JSON replies remain accepted at EOF.
A full Unix listen queue is a prompt retryable error rather than an indefinite
connect before read timeouts apply.

Both principal regressions failed before the change and pass after it. An EOF
sentinel verifies descriptor closure without racing process-wide FD counts.
Fifteen wire tests cover early/late rights, fragmented probe/control/counts,
malformed/oversized/refused replies, legacy EOF, deadline dribble, unexpected
rights and full listen queues. They pass in both node and client-only builds.
The full runtime and netd suites pass. An explicit isolated TAP fixture passed
real descriptor transfer and packet injection with the new decoder.

**Merged source:** `9e1dd3f0c8c605923efc575940082f839cad8297`.
**Tracking:** qrab `work/netd-reply-stream-framing`.
These checks exercise the local protocol and kernel mechanism; they do not
establish a deployed endpoint handoff or cross-node traffic outcome.

## A09 — delayed status could be published onto a same-name replacement

**Affected source:** netd readiness and VPN-port condition writers.
**Repair baseline:** `9e1dd3f`.

Local work held a Pod snapshot while plumbing or renewing a mapping, then
patched status by name without UID/version conditions. A replacement Pod could
receive the old result. Two fake-API regressions reproduced this through the
actual readiness and port-report entrypoints.

Both writers now use a shared conditional strategic patch with the observed
Pod UID and resourceVersion. Missing references fail closed; a rejected patch
is not cached as success. Successful kernel context retains its summary so
fresh reconciliation retries a conflicted status write from current watch data
without repeating plumbing or waiting for the sixty-second reverify interval.

Four focused tests cover replacement identity, version conflict/retry, missing
provenance and preservation of unrelated condition types. A no-op native API
server status **dry-run** accepted the matching snapshot, rejected a wrong UID
alone as immutable and rejected a stale version as Conflict. The dry-run
persisted no resource change and did not restart a Pod or VM.

The full runtime check passed with `RUST_TEST_THREADS=4`; strict node checks
also passed. These API conditions establish observation ownership, not backend
fencing or live guest health. Kubernetes documents
[conditional PATCH and conflict retry](https://kubernetes.io/docs/reference/using-api/api-concepts/#updates-to-existing-resources).

**Merged source:** `a5373c384c5679d2cc0218930bf8ff8f730da21b`.
**Tracking:** qrab `work/netd-status-pod-incarnation`.

## A10 — DHCP could serve stale configuration or report startup before binding

**Affected source:** netd applied-plan/DHCP worker lifecycle.
**Repair baseline:** `dc5d8c2`.

A healthy namespace-bound responder retained its original copied lease even
when later plumbing applied a different VPN address/DNS/MTU. Separately,
`serve_dhcp` reported thread creation as success before namespace entry or UDP
binding ran. A setup failure could therefore coexist with reported readiness.

The responder now binds its socket in the verified namespace before returning
success. That socket carries the namespace for subsequent I/O. Each request
reads one complete shared lease snapshot; reapplying a plan updates a healthy
same-namespace responder without rebinding port 67. A process-reference refresh
invalidates grants while preserving a potentially reusable namespace socket.
A changed namespace or removed Pod retires it. Failed responders invalidate
fresh readiness and retry; the socket closes before exit is published.

The invalid-namespace startup regression failed before the change and passes
after. An isolated real-link fixture received both the original and updated
address, DNS, MTU and lease duration from the same worker. A second bind failed
synchronously with AddrInUse while the first server continued to answer.
Default worker/readiness tests, client-only compilation and the full runtime
gate passed; the explicit isolated kernel suite exercised four paths while
two optional VPN fixtures skipped for missing tools.

This updates later DHCP answers; it does not force a guest to renew or rewrite
static NoCloud configuration. No live profile, Pod or route was changed.

**Merged source:** `033746a467bebccd87627e1530ec00a306a1a6ec`.
**Tracking:** qrab `work/netd-dhcp-lease-generation`.

## Validation repair: the HTTP power fixture depended on test order

A full gate exposed a deterministic isolated-test failure: the operator's
plain-HTTP power fixture built a default reqwest backend under
`rustls-no-provider` without initializing a provider. Another test's global
initialization could hide it. The fixture now initializes Ring explicitly and
reuses its client; production continues to supply its SPIFFE TLS configuration.
The isolated test and strict Clippy pass, followed by the combined full gate.

**Merged test repair:** `265afecf4327985086ae8c676f7ca0b2c7eb757c`.
**Tracking:** qrab `work/operator-power-http-provider-initialization`.
This is test isolation, not another production authorization defect.

The final combined gate used four test threads on the busy host. Timing
variance, isolated reruns and complete check logs are recorded in towl.

## Investigated concern rejected by the real policy contract

The audit initially suspected two `ingress: [{}]` CiliumNetworkPolicy examples
allowed unrelated ingress. Tests of the exact importer manifest and NIC
baseline through the local Cilium production policy repository denied that
traffic. The examples were preserved. The auxiliary simulator disagreed for
those inputs and was not used as the sole oracle. This is a rejected
hypothesis, not another repaired boundary bug; see [verification](verification.md)
and qrab `work/cilium-default-deny-ingress-policy` for the distinction.

## Checks and limits

The initial `5f361b7` runtime baseline passed before the repair. Future
findings must include a source location, a concrete boundary crossing or
failure mode, a regression or other appropriate proof, and the verified
change. Desired capability gaps are not automatically bugs: document them in
the capability ledger rather than silently expanding the platform's scope.
