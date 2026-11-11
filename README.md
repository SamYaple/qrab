# qrab

Rust libraries and tooling for launching and controlling QEMU. The repository
is being refactored toward a QEMU wrapper running in Kubernetes. The existing
manager and host/guest boot utilities still contain the earlier deployment
model; Kubernetes delivery is a separate stage of the refactor.

The protocol foundation is independent of systemd and Kubernetes:

| Crate | Responsibility |
| --- | --- |
| `qapi-codegen` | nom 8 parser, release-history merge, and Rust generation |
| `qapi-macros` | Serde and command/event traits for generated types |
| `qapi` | Generated QMP/QGA types and schema metadata |
| `qapi-protocol` | Async Unix-socket QMP/QGA clients and test servers |
| `manager` | QEMU process lifecycle and VM configuration |

The schema baseline is QEMU **11.1.3**, pinned in `vendor/qemu`. A lockfile
records the exact release commits used for historical lifecycle metadata.
Ordinary consumers build from the checked-in generated code without QEMU
sources, Python, systemd, KVM or a cluster.

See [schema generation](docs/codegen.md), [protocol behavior](docs/protocol.md),
and [validation](docs/testing.md) for the core-library development workflow.
