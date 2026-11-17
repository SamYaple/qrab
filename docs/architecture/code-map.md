# Follow a requirement to its code owner

The workspace has 19 crates. Eighteen belong to the normal developer path;
`qapi-codegen` is selected explicitly for schema generation. A crate is a build
boundary, not necessarily a deployed service. The operator binary has distinct
VM-controller, image-controller and token-free image-Job modes; the manager,
BMC and Redfish can be shipped in one image while running as separate containers.

## Workspace map

| Crate / location | Owns | Does not own | Useful entry points |
| --- | --- | --- | --- |
| [`qapi-codegen`](../../projects/qapi-codegen/) | QEMU schema parsing, validation, IR and Rust generation | Running VM lifecycle | `src/parsers/`, `src/qapi_ir/`, `src/unified.rs` |
| [`qapi-macros`](../../projects/qapi-macros/) | Derives used by generated protocol types | Socket access, user authorization | `src/lib.rs` |
| [`qapi`](../../projects/qapi/) | Generated QMP/QGA command, reply, event and version metadata | QEMU supervision, Kubernetes, policy | `src/lib.rs`, generated outputs in `src/generated/` and `src/qga/` |
| [`qapi-protocol`](../../projects/qapi-protocol/) | Async QMP/QGA wire sessions, correlation, framing, timeouts and FD transport | Spawn/kill, KVM, firmware downloads, cloud orchestration | `src/qmp.rs`, `src/qga.rs`, `src/wire.rs`, `src/framing.rs` |
| [`manager`](../../projects/manager/) | One VM's local process lifecycle, persistent identity lease, QMP/QGA, hotplug, migration and bounded control API | Kubernetes API rights, cluster IPAM, user login, ZFS/LIO administration | `src/bin/qrab-manager.rs`, `src/pod/supervisor.rs`, `config.rs`, `socket.rs`, `migration.rs` |
| [`oidc-bearer`](../../projects/oidc-bearer/) | Shared verification of configured OIDC ID-token bearers | Browser login sessions or workload SVID issuance | `src/lib.rs` |
| [`bmc`](../../projects/bmc/) | Per-VM browser login, console access and RFB filtering; browser Redfish bridge | A general QMP gateway or Kubernetes controller | `src/auth.rs`, `console.rs`, `proxy.rs`, `rfb_filter.rs` |
| [`qrab-redfish`](../../projects/redfish/) | Authenticated VM management resources, power, inventory, events, telemetry, boot and approved firmware/media operations | Direct QMP ownership, cluster placement, arbitrary command execution | `src/lib.rs`, `auth.rs`, `workload.rs`, `tasks.rs` |
| [`qrab-operator`](../../projects/operator/) | VM/migration CRDs, rendered VM and NIC resources, storage reconciliation, image import modes and guest identity registration | Hypervisor process supervision, physical BGP sessions, kernel LIO/ZFS writes | `src/controller.rs`, `resources.rs`, `migration.rs`, `address.rs`, `image_import.rs`, `provision.rs` |
| [`qrab-dashboard`](../../projects/dashboard/) | User-facing VM discovery, launch/delete/access flows and VM-scoped Redfish/console proxying | Direct QMP, VM process ownership or a replacement Kubernetes API | `src/cluster.rs`, `launch.rs`, `cleanup.rs`, `redfish.rs`, `bmc.rs` |
| [`qrab-iscsi-target`](../../projects/iscsi-target/) | Narrow privileged ZFS/LIO storage verbs, durable disk/image journals, exports and fencing | User-facing cloud identity, Kubernetes scheduling or VM launch | `src/disk.rs`, `host.rs`, `store.rs` |
| [`qrab-guest-agent`](../../projects/qrab-guest-agent/) | Fixed in-guest QGA allowlist, freeze/thaw watchdog, boot/firmware reads and bounded update hook | Arbitrary guest shell/exec service, host privilege, Kubernetes credentials | `src/agent.rs`, `framing.rs`, `update.rs`, `secure_boot.rs` |
| [`qrab-spire-relay`](../../projects/qrab-spire-relay/) | Bounded guest-loopback to virtio-serial multiplexing and framing | TLS termination, workload attestation or guest identity issuance | `src/guest.rs`, `wire.rs`; host side in manager `spire_relay.rs` |
| [`qrab-kvm-device-plugin`](../../projects/kvm-device-plugin/) | Kubelet device-plugin registration and CDI injection of `/dev/kvm` and the netd socket mount | QEMU processes or guest packet forwarding | `src/lib.rs` |
| [`qrab-netd`](../../projects/qrab-netd/) | Privileged node-local TAP/vhost grants, anchor plumbing, DHCP/VPN, handoff/conntrack operations | User/VM orchestration, guest QMP or a second CNI | `src/main.rs`, `authz.rs`, `process.rs`, `anchor.rs`, `cilium.rs`, `sys.rs`, `peer.rs`, `peer_tls.rs` |
| [`qrab-spiffe`](../../projects/qrab-spiffe/) | Workload API consumption, exact SPIFFE peer policy, rotating TLS and HTTP helpers | Human roles, credential exchange policy or SPIRE server administration | `src/lib.rs`, `http.rs` |
| [`qrab-proton`](../../projects/proton/) | Workstation Proton login and in-cluster VPN-profile broker | A general network controller or guest credentials | `src/login.rs`, `broker/runner.rs`, `crd.rs` |
| [`qrab-observability`](../../projects/qrab-observability/) | API-backed VM/Pod discovery, authenticated scrape and constrained Prometheus-history bridge | General PromQL proxying or durable event/audit storage | `src/lib.rs`, `main.rs` |
| [`qrab-tpm-issuer`](../../projects/tpm-issuer/) | VM-bound software-TPM endorsement/device certificate issuance | Hardware confidential computing, general workload identity or user login | `src/lib.rs`, `tpm.rs`, `cert.rs`, `store.rs` |

## The dependency spine

The production protocol spine is `qapi-macros → qapi → qapi-protocol → manager`.
BMC and Redfish depend on the manager's typed control abstractions; that Rust
dependency does not authorize them to take over the private QMP socket. The
operator does not depend on the manager crate: it renders the runtime config
contract and reconciles Kubernetes objects.

The dashboard depends on operator API/types and Proton profile types. The
observability adapter reuses operator discovery types and Redfish history
validation. netd's `agent` feature contains its Kubernetes/TLS node service;
manager and operator use `default-features = false` for its client/wire and
naming contracts. The manager also reuses guest-agent wire types and guest
SPIRE relay framing. This avoids duplicating transport definitions without
moving guest or node execution into the manager.

`qapi-codegen` is not a production dependency of `qapi`: generated files are
committed outputs. Change the parser/IR/generator/macros and regenerate them;
do not edit generated code. Protocol tests must remain runnable without
manager, KVM, firmware, systemd, a guest download or a Kubernetes cluster.

## Change guide

| Requirement | Start with | Then inspect |
| --- | --- | --- |
| New QMP/QGA schema command or type | QAPI schema/codegen | Protocol command traits and typed use in manager; run `check-qapi.sh` |
| New bounded VM operation | Manager control API | Socket/Remote adapters, Redfish authorization, UI and event semantics |
| New VM spec field | Operator API + validation | Rendered resources/config, CRD generation, restart/hash rules, dashboard |
| NIC attachment or policy labels | Operator NIC/address modules | netd authorization/plumbing, manager FD/hotplug path, Cilium integration |
| Cross-node NIC handoff | Operator migration | Manager switchover, netd peer TLS/conntrack, local Cilium fork contract |
| Guest disk lifecycle | Operator provision/storage modules | Storage-service verb contract and `iscsi-target` fence/journal behavior |
| Guest image import | Operator image/import/Job modules | Storage image verbs, dashboard catalog and source-verification limits |
| User permissions | Dashboard/BMC/Redfish auth | OIDC bearer verifier, creator/group admission and workload peer checks |
| Guest workload identity | Operator guest registration | TPM issuer, manager provisioning/relay, guest SPIRE agent configuration |
| Node hardware access | KVM plugin or netd | CDI, Pod Security, host namespace and admission boundaries |
| Metrics/history | Redfish + observability | Exact VM/Pod discovery, scope validation, Prometheus and policy |

## Repository boundary

[`contrib/kubernetes`](../../contrib/kubernetes/) expresses qrab-owned manifests,
policy examples and explicit live checks. [`contrib/images`](../../contrib/images/)
owns source preparation, image builds/publishing and digest locks. The pinned
QEMU source is `vendor/qemu`, fetched only from the local Gitea mirror.

Kubernetes, the container runtime, Cilium/its local fork, FRR/frr-k8s, the
storage daemon/exchange shim, Kanidm, SPIRE servers/CSI, DNS, registry,
monitoring deployment and native encryption configuration have separate
implementation or platform ownership. Some checked-in integration examples
live here; that does not make qrab their implementation owner.

Retired host/SVSM/confidential-computing projects live under the historical
archive tag, not the active runtime. See [legacy.md](../legacy.md). In
particular, the software vTPM path in the active tree must not be presented
as the old confidential-computing architecture.
