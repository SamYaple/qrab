# qrab

Rust libraries and tooling for launching QEMU VMs in Kubernetes. The active
runtime is `qrab-manager`: one container supervises one QEMU process through a
private QMP socket. The demo boots a real Linux guest without guest networking
or a VM controller.

| Component | Responsibility |
| --- | --- |
| `qapi-macros` / `qapi` | Generated QMP/QGA types, Serde and command/event traits |
| `qapi-protocol` | Async Unix-socket QMP/QGA clients and bounded test servers |
| `manager` | Pod-native VM configuration, supervision, status and probes |
| `qapi-codegen` | Explicit schema parsing and generation tool |

## Development

```sh
cargo build --locked
cargo test --locked
bash contrib/check-runtime.sh
```

The default workspace selects only the protocol libraries and pod manager.
These checks use committed generated code and require no QEMU source checkout,
Python, KVM, firmware, systemd or cluster access. Live-QEMU tests are explicit.
The QEMU image is built from **11.1.3**, pinned in `vendor/qemu` from our
[local Gitea mirror](https://gitea.yapplesauce.com/mirrors/qemu).

## Images and VM launch

See [manager.md](docs/manager.md) for the two runtime images, strict VM config,
probes and shutdown behavior. Published digests and their source revision are
recorded in [images.lock.json](contrib/images/images.lock.json).

Once the namespace/ServiceAccount image-pull identity is enrolled:

```sh
kubectl apply -k contrib/kubernetes
bash contrib/kubernetes/check.sh
```

Read [protocol.md](docs/protocol.md) for the transport contract and
[testing.md](docs/testing.md) for live smoke tests. Parser/generator work uses
[codegen.md](docs/codegen.md) and the explicit `contrib/check-qapi.sh` check.

## Earlier experiments

Host namespace/SEV/SVSM manager APIs are opt-in through `manager/legacy-host`.
The older BMC enables this explicitly. Confidential-guest boot/attestation
experiments and standalone UEFI crates remain available separately; see
[legacy.md](docs/legacy.md). They are not prerequisites for a normal VM pod.
