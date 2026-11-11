# qrab

Rust libraries and tooling for launching and controlling QEMU. The repository
is being refactored toward a QEMU wrapper running in Kubernetes. The new `qrab-manager` pod runtime launches QEMU directly in the container
sandbox. The older manager/boot APIs remain separate while the refactor proceeds.

The protocol foundation is independent of systemd and Kubernetes:

| Crate | Responsibility |
| --- | --- |
| `qapi-codegen` | nom 8 parser, release-history merge, and Rust generation |
| `qapi-macros` | Serde and command/event traits for generated types |
| `qapi` | Generated QMP/QGA types and schema metadata |
| `qapi-protocol` | Async Unix-socket QMP/QGA clients and test servers |
| `manager` | QEMU process lifecycle and VM configuration |

The schema baseline is QEMU **11.1.3**, pinned in `vendor/qemu` from our
[local Gitea mirror](https://gitea.yapplesauce.com/mirrors/qemu). A lockfile
records the exact release commits used for historical lifecycle metadata.
Ordinary consumers build from the checked-in generated code without QEMU
sources, Python, systemd, KVM or a cluster.

See [pod runtime, images and Kubernetes launch](docs/manager.md),
[schema generation](docs/codegen.md), [protocol behavior](docs/protocol.md),
and [validation](docs/testing.md) for the core-library development workflow.
