# Evidence register and claim rules

The investigation begins at source commit `5f361b7` on 2026-11-17. Repository
links below identify the implementation owners; use the symbol/module names
when line numbers change. Later validated repairs belong in [audit.md](audit.md).
The [connection atlas](../connections.md) provides finer-grained stream IDs.

## Local evidence

| ID | Evidence | Supports | Does not establish |
| --- | --- | --- | --- |
| S01 | [Workspace manifest](../../Cargo.toml) and [legacy inventory](../legacy.md) | Active crate selection and retired scope | Which binaries a cluster actually runs |
| S02 | [VM API](../../projects/operator/src/api.rs) and [validation/rendering](../../projects/operator/src/resources.rs) | Public desired-state model, restricted Pod spec and config contract | API server admission or deployment success by itself |
| S03 | [VM controller](../../projects/operator/src/controller.rs) | Fresh reads, reconciliation, ownership checks and resource lifecycle | An exactly-once distributed transaction |
| S04 | [Manager supervisor](../../projects/manager/src/pod/supervisor.rs), [process ownership](../../projects/manager/src/pod/process.rs) | Direct child lifecycle and cleanup | Protection from a node/hypervisor administrator |
| S05 | [Manager persistent state](../../projects/manager/src/pod/persistent.rs) | VM-bound retained identity and lease | A storage-system or cross-host fencing mechanism by itself |
| S06 | [Control socket](../../projects/manager/src/pod/socket.rs), [Remote adapter](../../projects/manager/src/pod/remote.rs) | Bounded local API, peer checks and sidecar transport | End-user authorization inside manager |
| S07 | [Protocol clients](../../projects/qapi-protocol/src/) and [generated types](../../projects/qapi/src/lib.rs) | QMP/QGA transport and typed command contracts | A network-facing QAPI service |
| S08 | [NIC renderer](../../projects/operator/src/nic.rs), [address reconciler](../../projects/operator/src/address.rs) | Separate anchors, labels, /32 pools and FRR intent | Upstream Cilium duplicate-IP handoff support |
| S09 | [Node netd](../../projects/qrab-netd/src/main.rs), [authorization](../../projects/qrab-netd/src/authz.rs), [kernel work](../../projects/qrab-netd/src/sys.rs), [held process resources](../../projects/qrab-netd/src/process.rs), [namespace incarnation](../../projects/qrab-netd/src/anchor.rs) | Node-local plumbing, peer-to-Pod resolution and FD grants | Arbitrary Pod creators being safe in the trusted namespace |
| S10 | [Manager TAP launch](../../projects/manager/src/pod/tap.rs) and [NIC hotplug](../../projects/manager/src/pod/nics.rs) | Descriptor-based QEMU attachment | A userspace packet relay through manager or netd |
| S11 | [Operator migration](../../projects/operator/src/migration.rs), [manager migration](../../projects/manager/src/pod/migration.rs) | Target preparation, state tunnel, handoff and cleanup | OpenStack-to-qrab live migration or arbitrary QEMU-version compatibility |
| S12 | [netd conntrack](../../projects/qrab-netd/src/ct.rs), [peer policy](../../projects/qrab-netd/src/peer.rs), [peer TLS](../../projects/qrab-netd/src/peer_tls.rs) | Node-to-node scoped conntrack exchange | Bulk VM RAM or guest-disk transport |
| S13 | [Disk provisioner](../../projects/operator/src/provision.rs), [storage client](../../projects/operator/src/storage.rs), [storage guard](../../projects/operator/src/storage_guard.rs) | Export generations, CHAP lifecycle and safe replacement rules | That Pod deletion alone hard-fences old writers |
| S14 | [Storage helper](../../projects/iscsi-target/src/disk.rs), [host operations](../../projects/iscsi-target/src/host.rs), [journal](../../projects/iscsi-target/src/store.rs) | Narrow ZFS/LIO implementation, fence barriers and retained data | Generic CSI guest volumes or all Cinder backends |
| S15 | [BaseImage API](../../projects/operator/src/image.rs), [import controller](../../projects/operator/src/image_import.rs), [Job mode](../../projects/operator/src/image_job.rs) | Verified source imports, inspect/write stages and sealed generations | A complete Glance-compatible catalog/distribution service |
| S16 | [Dashboard launch](../../projects/dashboard/src/launch.rs), [cluster access](../../projects/dashboard/src/cluster.rs), [cleanup](../../projects/dashboard/src/cleanup.rs) | Browser workflow and managed-resource ownership | Kubernetes per-user impersonation; the backend uses its ServiceAccount |
| S17 | [Dashboard Redfish client](../../projects/dashboard/src/redfish.rs), [console client](../../projects/dashboard/src/bmc.rs) | Exact selected-VM proxy destinations and workload TLS | A broad dashboard-to-any-Pod proxy |
| S18 | [BMC authentication](../../projects/bmc/src/auth.rs), [console](../../projects/bmc/src/console.rs), [proxy](../../projects/bmc/src/proxy.rs) | User role checks, console admission and Redfish bridge | Direct end-user access to private QMP/VNC |
| S19 | [Redfish authentication](../../projects/redfish/src/auth.rs), [workload admission](../../projects/redfish/src/workload.rs), [routes](../../projects/redfish/src/lib.rs) | Separate user and service admission | A TLS certificate granting every Redfish action |
| S20 | [OIDC verifier](../../projects/oidc-bearer/src/lib.rs), [SPIFFE helper](../../projects/qrab-spiffe/src/lib.rs), [HTTP helper](../../projects/qrab-spiffe/src/http.rs) | Different human-bearer and workload-peer contracts | That one identity mechanism replaces every other credential |
| S21 | [KVM plugin](../../projects/kvm-device-plugin/src/lib.rs) and [node manifest](../../contrib/kubernetes/netd/daemonset.yaml) | Narrow device injection and separately privileged node service | No privileged platform components |
| S22 | [Guest agent](../../projects/qrab-guest-agent/src/agent.rs), [manager QGA worker](../../projects/manager/src/pod/agent.rs) | Bounded allowlisted guest cooperation | Trust in a hostile guest's inventory claims |
| S23 | [Guest identity registrations](../../projects/operator/src/guest_identity.rs), [manager provisioning](../../projects/manager/src/pod/identity.rs), [TPM issuer](../../projects/tpm-issuer/src/lib.rs) | Optional VM-UID-bound guest identity path | Hardware-backed isolation from the manager or node |
| S24 | [Guest relay framing](../../projects/qrab-spire-relay/src/wire.rs), [host relay](../../projects/manager/src/pod/spire_relay.rs) | Bounded opaque transport with fixed upstream | Relay-issued identities or TLS termination |
| S25 | [Proton broker](../../projects/proton/src/broker/runner.rs), [VPN implementation](../../projects/qrab-netd/src/vpn.rs) | Optional VPN profile lifecycle and anchor tunnel | A mandatory dependency of every VM |
| S26 | [Observability adapter](../../projects/qrab-observability/src/lib.rs), [Redfish history](../../projects/redfish/src/history.rs) | Current VM/Pod-bound scrape/history scope | Arbitrary PromQL or durable audit completeness |
| M01 | [Operator manifests/policies](../../contrib/kubernetes/operator/), [netd RBAC](../../contrib/kubernetes/netd/rbac.yaml) | Checked-in API/admission intent | Applied policy state in either cluster |
| M02 | [Image lock](../../contrib/images/images.lock.json), [build](../../contrib/images/build.sh), [publish](../../contrib/images/publish.sh) | Recorded digest/source pins and separate release operations | A running Pod having the latest source |
| L01 | [Dated platform evidence](../connections/platform-evidence.md) | Explicit historical rollout observations and captures | Current health or feature support in the other OpenStack cluster |
| L02 | [Guest identity evidence](../spire-vm.md) | The documented guest-identity implementation and dated verification | Every guest image having SPIRE enrolled |
| T01 | [Runtime check](../../contrib/check-runtime.sh), [testing guide](../testing.md) | Appropriate local verification scope | Real KVM, ZFS/LIO, Cilium or cluster acceptance |

| T02 | [Real QMP fixture](../../projects/qapi-protocol/tests/live_qemu.rs), [real QGA fixture](../../projects/qapi-protocol/tests/live_qga.rs) and [source preparation](../../contrib/images/prepare-source.py) | Explicit local 11.1.3 peer checks described in [verification](verification.md) | Guest boot, KVM, storage or a deployed release |
| T03 | [Kernel namespace fixtures](../../projects/qrab-netd/tests/netns.rs), [node wire faults](../../projects/qrab-netd/src/wire.rs), [manager NIC reconciliation](../../projects/manager/src/pod/nics.rs) | The explicitly exercised isolated mechanisms and stand-in ordering | Cilium/BGP, guest hot-unplug or application continuity acceptance |

| T04 | [Node status writers](../../projects/qrab-netd/src/main.rs) and [private API regressions](../../projects/qrab-netd/src/status_tests.rs) | Exact Pod UID/version conditional writes; explicit no-op native server dry-run verifies immutable UID and stale-version refusal | Persisted node rollout, guest readiness or disk fencing |

| T05 | [Isolated VPN/kernel fixtures](../../projects/qrab-netd/tests/netns.rs) with temporary official wireguard-tools 1.0.20260223 | All six explicit namespace fixtures executed without skips, including real WireGuard kill-switch and tunnel-only NAT-PMP | Production provider, Cilium/BGP or guest workload acceptance |

| T06 | [Cilium provider capture/execution fixtures](../../projects/qrab-netd/src/cilium.rs) and [held process resources](../../projects/qrab-netd/src/process.rs) | Explicit original-peer/path replacement and distinct mount/PID namespace execution | Live Cilium bpftool/map behavior, release or rollout |

| T07 | [Endpoint ownership/HTTP fixtures](../../projects/qrab-netd/src/cilium/http.rs), [actual control handler regression](../../projects/qrab-netd/src/status_tests.rs) | Exact identity labels, unique attachment mutation and bounded local HTTP | Live handoff/traffic continuity or acceptance on every Cilium release |

| T08 | [Credential cleanup](../../projects/operator/src/provision.rs), [private API ownership/race fixtures](../../projects/operator/src/provision/secret_tests.rs) | Test-only metadata patch binds the intended VM/disk; delete carries checked Secret UID/version without reading credential data | A live rollout, arbitrary Secret access or backend writer exclusion |

| T09 | [Real manager launcher](../../projects/manager/tests/pod_runtime.rs), [UEFI/vTPM lifecycle](../../projects/manager/tests/pod_uefi.rs), [guest-agent fixture](../../projects/manager/tests/pod_guest_agent.rs) | Explicit local TCG guest boot, process cleanup, retained software-TPM identity, corruption refusal and one-filesystem freeze/thaw | KVM, Kubernetes/Cilium/BGP, hardware attestation, backend storage or application backup acceptance |

| T10 | [Real two-manager migration fixture](../../projects/manager/tests/pod_migration.rs) | Explicit local TCG guest resume, source migrated outcome and persisted TPM identity after a later cold boot | NIC/Cilium/BGP handoff, guest LUNs, production SPIRE or OpenStack transfer |

| T11 | [Direct supervision](../../projects/manager/src/pod/process.rs), [resource accounting](../../projects/manager/src/pod/resources.rs), dated local KVM/container fixture described in [verification](verification.md) | Current manager, QEMU and swtpm share one bounded local cgroup; guest boot/ping and original-process exit observed | Kubernetes placement/device injection, either cluster's tenant isolation, Cilium/BGP or backend disk acceptance |

| T12 | [Manager block-budget regression](../../projects/manager/src/pod/hotplug.rs), [operator resize decisions](../../projects/operator/tests/resize.rs) | Actual outgoing QMP requests stay within whole-block capacity/intent/ceiling; exact unrepresentable sizes take the restart path | A deployed resize, arbitrary guest onlining or live shrink |

| T13 | [Manager growth](../../projects/manager/src/pod/hotplug.rs), dated local KVM growth record in [verification](verification.md) | Real cgroup changes gate CPU/memory requests; an odd budget floors device blocks safely; the same guest then onlines both resources | Kubernetes/kubelet placement/admission, arbitrary guest cooperation, live shrink or performance guarantees |

| T14 | [Media path policy and regression](../../projects/manager/src/pod/media.rs), isolated CLI configuration check described in [the audit](audit.md) | Actual boot path must resolve to the canonical image that passed the configured policy; matching-root size behavior retained | A guest boot, a claimed exploit of standard operator rendering or mutable/untrusted media-publisher safety |

## External sources

Checked on 2026-11-17. Versioned OpenStack 2026.1 documentation is used to
describe responsibilities, not to infer the version of the operator's cluster.
Some upstream pages retain an older "last updated" date inside a newer release
tree. The date of the evidence check does not make every diagram a new design.

| ID | Primary source | Use in this atlas |
| --- | --- | --- |
| U01 | [Nova system architecture](https://docs.openstack.org/nova/2026.1/admin/architecture.html) | API, scheduler, conductor, compute, persistence and placement duties |
| U02 | [Neutron OVN reference architecture](https://docs.openstack.org/neutron/2026.1/admin/ovn/refarch/refarch.html) | OVN control/database/controller/datapath boundaries and upstream comparison diagrams |
| U03 | [Glance overview](https://docs.openstack.org/glance/2026.1/) | Image discovery, metadata, registration and retrieval scope |
| U04 | [Cinder service overview](https://docs.openstack.org/cinder/2026.1/install/get-started-block-storage.html) | Persistent volumes, API/scheduler/volume/backup responsibilities |
| U05 | [Kubernetes access control](https://kubernetes.io/docs/concepts/security/controlling-access/) | Authentication, authorization and admission as separate API gates |
| U06 | [Cilium BGP scope](https://docs.cilium.io/en/stable/network/bgp-control-plane/bgp-control-plane/) | Route advertisement is distinct from datapath programming; not qrab's FRR implementation |
| U07 | [Cilium policy enforcement](https://docs.cilium.io/en/stable/security/policy/intro/) | Directional default policy and selection behavior |
| U10 | [Kubernetes container state and readiness](https://kubernetes.io/docs/concepts/workloads/pods/pod-lifecycle/#container-states) | Running container state and additional Pod readiness gates are distinct |
| U09 | [Kubernetes conditional API updates](https://kubernetes.io/docs/reference/using-api/api-concepts/#updates-to-existing-resources) | Object version conditions, conflict detection and retry requirements |
| U08 | [OpenStack architectural design](https://docs.openstack.org/arch-design/design.html) | Historical conceptual/logical overview; not the user's deployment topology |
| U11 | [OpenStack server task reference](https://docs.openstack.org/python-openstackclient/2026.1/cli/command-objects/server.html) | Familiar operation vocabulary for the task translator; not compatible qrab commands |
| U12 | [Nova evacuation](https://docs.openstack.org/nova/2026.1/admin/evacuate.html) | Failed-host recovery requires establishing the old host is not operational |
| U13 | [Keystone token scopes](https://docs.openstack.org/keystone/2026.1/admin/tokens-overview.html) | OpenStack API authorization context; distinct target identity questions |

## Rules for a defensible comparison

Attribute the host-cgroup/libvirt deployment model and the two clusters to the
operator's supplied context. Attribute general service duties to upstream.
Attribute qrab behavior to source and tests. Label desired contracts as desired.
Do not turn a dated deployment record into a fresh measurement.

The native MACsec and Cilium WireGuard records describe different segments.
CHAP, BGP, RBAC, an OIDC signature or a SPIFFE socket mount alone do not prove
payload confidentiality. Likewise, a private QMP socket is a local authority
boundary, not an authenticated public management service.
