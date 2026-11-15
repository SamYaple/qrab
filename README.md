# qrab

Rust libraries and tooling for launching QEMU VMs in Kubernetes. The active
runtime is driven by the Rust `qrab-operator` and a namespaced `VirtualMachine`
CRD. Each VM pod runs a Kanidm-authenticated browser console
and supervises QEMU through a private QMP socket. Video, keyboard and USB mouse
input use a private VNC socket. The demo boots Linux through UEFI with a persistent swtpm
TPM2 and no guest NIC. `qrab-manager` also remains available as a standalone supervisor.

| Component | Responsibility |
| --- | --- |
| `qapi-macros` / `qapi` | Generated QMP/QGA types, Serde and command/event traits |
| `qapi-protocol` | Async Unix-socket QMP/QGA clients and bounded test servers |
| `manager` | Pod-native VM configuration, supervision, status and probes |
| `bmc` | Per-VM Kanidm OIDC login, noVNC display/input and VM controls |
| `qrab-operator` | Minimal namespaced VM CRD, Pod/ConfigMap lifecycle and status |
| `qapi-codegen` | Explicit schema parsing and generation tool |

## Development

```sh
cargo build --locked
cargo test --locked
bash contrib/check-runtime.sh
```

The default workspace selects the protocol libraries, pod manager, BMC and operator.
These checks use committed generated code and require no QEMU source checkout,
Python, KVM, firmware, systemd or cluster access. Live-QEMU tests are explicit.
The QEMU image is built from **11.1.3**, pinned in `vendor/qemu` from our
[local Gitea mirror](https://gitea.yapplesauce.com/mirrors/qemu).

## Images and VM launch

See [manager.md](docs/manager.md) for the runtime images, strict VM config,
probes and shutdown behavior. Published digests and their source revision are
recorded in [images.lock.json](contrib/images/images.lock.json).

See [iscsi.md](docs/iscsi.md) for native network disks and fencing.
See [operator.md](docs/operator.md) for CR-driven launch and persistent UEFI/TPM
identity, and [bmc.md](docs/bmc.md) for OIDC and the per-VM console.
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
