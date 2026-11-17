# Boundary glossary

| Term | Meaning here |
| --- | --- |
| VM resource | Namespaced Kubernetes `VirtualMachine` intent/status; its UID is the immutable runtime identity binding |
| VM Pod | The management/execution Pod containing manager and optional BMC/Redfish sidecars |
| Manager container | The container/cgroup in which manager supervises QEMU/swtpm |
| QEMU | Hypervisor/device-model process; kept in the target |
| KVM | Linux kernel acceleration used through `/dev/kvm` when requested |
| QAPI | QEMU schema/type system and generated Rust command/event metadata; not a network service |
| QMP | QEMU's management protocol on a private Unix socket owned by manager |
| QGA | Guest-agent protocol over a private host socket/virtio-serial channel; guest cooperation is bounded and untrusted |
| BMC | Per-VM browser/console service and Redfish browser adapter |
| Redfish | Authenticated VM management model/API implemented by a per-VM sidecar |
| NIC anchor | Separate restricted Pod/netns/Cilium identity for one guest NIC |
| netd | Privileged per-node network helper; plumbs anchors and grants bounded TAP/vhost descriptors |
| TAP / vhost-net | Kernel mechanisms connecting guest virtio queues to the anchor datapath |
| SCM_RIGHTS | Unix descriptor transfer; carries authority to use an FD, not its packet payload |
| Cilium endpoint | Policy/datapath identity for the relevant Pod/anchor |
| Security group | NIC label selected by platform Cilium policy; the label alone is not enforcement |
| FRRConfiguration | Kubernetes route intent consumed by frr-k8s/FRR |
| BGP | Route advertisement protocol; not a packet tunnel or encryption mechanism |
| State PVC | Filesystem claim for UEFI/vTPM identity, distinct from guest root/data LUNs |
| Native guest disk | QEMU libiscsi backend to an authorized LIO export |
| CHAP | iSCSI login authentication; separate from encryption and hard fencing |
| Fence | Backend action that prevents/drains the old export/session before safe new ownership |
| Retain / retire / destroy | Preserve bytes; remove from active service with lifecycle metadata; irreversibly erase bytes—different operations/owners |
| BaseImage generation | Immutable verified/sealed clone source exposed by the image catalog |
| Launch ID | Public random provenance tying a dashboard launch to its created resources; not a credential |
| OwnerReference | Kubernetes lifecycle/GC relationship; not independently proof of storage fencing |
| Finalizer | Cleanup coordination before object removal; stripping it bypasses normal cleanup |
| OIDC subject / ID token | Human identity claims verified for the configured issuer/audience/lifetime |
| ServiceAccount | Kubernetes workload API principal; the dashboard's backend authority is separate from its browser user's identity |
| SPIFFE SVID | Workload identity certificate/credential used for exact peer admission |
| SPIRE | Workload identity issuance/attestation infrastructure; not a human login directory |
| Guest identity | Optional in-guest SPIRE/vTPM principal, separate from manager/Pod identity |
| vTPM | Software TPM state/interface; does not isolate secrets from trusted hypervisor/node administrators |
| Workload mTLS | Mutual TLS between admitted service peers; separate user-action authorization still applies |
| Handoff | Explicit standby/active endpoint ownership transfer during migration; uses a local Cilium fork |
| Reconciliation | Repeated convergence from current intent/observations, including partial progress and retries |
| Source evidence | Behavior present in inspected code/tests |
| Manifest intent | Behavior requested by checked-in resources; not proof those resources are applied |
| Dated live evidence | A bounded observation/test of named deployed versions at a stated time |
| Desired state | Proposed/required architecture; not automatically an implemented capability |

For OpenStack service names and their replacement owners, use the
[replacement map](replacement-map.md). For ports and stream IDs, use the
[connection atlas](../connections.md).
