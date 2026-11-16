# qrab

Rust libraries and tooling for launching QEMU VMs in Kubernetes. The active
runtime is driven by the Rust `qrab-operator` and the namespaced
`VirtualMachine` and `VirtualMachineMigration` CRDs. In each VM pod
`qrab-manager` supervises QEMU through a private QMP socket; the BMC and Redfish
sidecars reach it through the manager's control socket. Video, keyboard and USB
mouse input use a private VNC socket. The live VM, `qrab-redfish-poc`, boots
signed media through Secure Boot-capable UEFI with a persistent swtpm TPM2 and
no guest NIC. `qrab-manager` also remains available as a standalone supervisor.

| Component | Responsibility |
| --- | --- |
| `qapi-macros` / `qapi` | Generated QMP/QGA types, Serde and command/event traits |
| `qapi-protocol` | Async Unix-socket QMP/QGA clients and bounded test servers |
| `manager` | Pod-native VM configuration, supervision, status, probes and live migration |
| `bmc` | Per-VM Kanidm OIDC login, noVNC display/input and the console UI |
| `qrab-redfish` | Per-VM Redfish service: power, inventory, events, telemetry, boot and firmware |
| `oidc-bearer` | Shared OIDC ID-token bearer verification for the BMC and Redfish |
| `qrab-operator` | VM and migration CRDs, Pod/ConfigMap/policy lifecycle, provisioned disks and status |
| `qrab-dashboard` | Cluster VM list, launch and deletion; power through Redfish and a proxied BMC console |
| `qrab-iscsi-target` | Root ZFS/LIO tool behind the storage service's VM-disk verbs |
| `qrab-guest-agent` | Static in-guest QGA daemon with a fixed command allowlist |
| `qrab-spire-relay` | Static in-guest relay from `127.0.0.1:8081` to the SPIRE server over a virtio-serial port |
| `qrab-kvm-device-plugin` | `/dev/kvm` device plugin for KVM-accelerated VMs |
| `qrab-spiffe` | SPIFFE X509-SVID mutual TLS (migration tunnel, Redfish workload admission) |
| `qrab-netd` | Per-node TAP/vhost descriptor grants, VPN anchors and NIC handoff |
| `qrab-proton` | Workstation ProtonVPN login and Kubernetes VPN-profile broker |
| `qapi-codegen` | Explicit schema parsing and generation tool |

## Development

```sh
cargo build --locked
cargo test --locked
bash contrib/check-runtime.sh
```

The default workspace selects every crate in the table except `qapi-codegen`.
These checks use committed generated code and require no QEMU source checkout,
Python, KVM, firmware, systemd or cluster access. Live-QEMU tests are explicit.
The QEMU image is built from **11.1.3**, pinned in `vendor/qemu` from our
[local Gitea mirror](https://gitea.yapplesauce.com/mirrors/qemu).

## Images and VM launch

See [manager.md](docs/manager.md) for the runtime images, strict VM config,
probes and shutdown behavior. Published digests and their source revision are
recorded in [images.lock.json](contrib/images/images.lock.json).

See the [connection atlas](docs/connections.md) for the complete stream inventory,
transport/encryption status, SPIFFE/SPIRE and OIDC boundaries, diagrams and
capture points across VM, network, storage and management paths.

See [iscsi.md](docs/iscsi.md) for native network disks, fencing and retained-data
cleanup, and [migration.md](docs/migration.md) for live migration.
See [operator.md](docs/operator.md) for CR-driven launch and persistent UEFI/TPM
identity, [bmc.md](docs/bmc.md) for OIDC and the per-VM console,
[redfish.md](docs/redfish.md) for the management API,
[dashboard.md](docs/dashboard.md) for the cluster dashboard,
[guest-agent.md](docs/guest-agent.md) for the guest agent and
[networking.md](docs/networking.md) for guest NICs.
Once the namespace/ServiceAccount image-pull identity, the storage service and
the OIDC client are enrolled, apply the platform and then each VM bundle on its
own ([contrib/kubernetes/README.md](contrib/kubernetes/README.md)):

```sh
kubectl apply -k contrib/kubernetes   # operator with storage client, monitoring
kubectl wait --for=condition=Established crd/virtualmachines.qrab.yapplesauce.com
kubectl apply -k contrib/kubernetes/redfish-poc
bash contrib/kubernetes/check.sh qrab qrab-redfish-poc
```

Read [protocol.md](docs/protocol.md) for the transport contract and
[testing.md](docs/testing.md) for live smoke tests. Parser/generator work uses
[codegen.md](docs/codegen.md) and the explicit `contrib/check-qapi.sh` check.

## Earlier experiments

Confidential-computing/SVSM projects and the legacy host manager were retired.
Their final source is preserved in the annotated tag
`eol/confidential-computing-svsm-2026-11-15`; see [legacy.md](docs/legacy.md).
