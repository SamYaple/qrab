# qrab

Typed Rust bindings for QEMU's QMP and QGA protocols. Types are generated from
QEMU's QAPI JSON schemas across **all** release tags (v1.0 onward). A single
unified type system carries version lifecycle metadata (`since`, `until`,
`deprecated`) so one set of types works against any QEMU version at runtime.

## Workspace

| Crate | Path | Role |
|---|---|---|
| `qapi-codegen` | `projects/qapi-codegen/` | Parses QEMU QAPI JSON schemas from git history, generates Rust types |
| `qapi-macros` | `projects/qapi-macros/` | Proc macro expanding `#[qapi]` into serde/trait impls |
| `qapi` | `projects/qapi/` | Generated types + runtime traits (primary consumer-facing crate) |
| `qapi-protocol` | `projects/qapi-protocol/` | Async QMP/QGA clients + mock servers |
| `manager` | `projects/manager/` | High-level typestate VM lifecycle wrapper |

| Example | Path |
|---|---|
| `simple-vm` | `examples/simple-vm/` |
| `preconfig-vm` | `examples/preconfig-vm/` |
| `events` | `examples/events/` |

## Critical Rules

- **Source schemas are golden.** Never preprocess, strip, or transform QEMU
  schema files. Parsers handle the raw format.
- **Generated files are outputs.** `generated.rs` and `qga.rs` are produced by
  `qapi-codegen`. Edit the codegen, never the generated files.
- **All QEMU release tags are processed.** Tag discovery is automatic. Do not
  hardcode version lists.
- **Version gating is dual-layer.** Compile-time (`QEMU_MIN_VERSION` /
  `QEMU_MAX_VERSION` env vars) and runtime (`set_qemu_version()` OnceLock).
  Both must remain correct.

Full rationale: [docs/design-constraints.md](docs/design-constraints.md)

## Documentation Index

| Document | When to read |
|---|---|
| [docs/architecture.md](docs/architecture.md) | Orienting — dependency flow, build phases, toolchain notes |
| [docs/qapi-crate.md](docs/qapi-crate.md) | Working with the generated types and runtime traits |
| [docs/codegen.md](docs/codegen.md) | Modifying the parser, IR, or code generation pipeline |
| [docs/macro.md](docs/macro.md) | Debugging or extending the `#[qapi]` proc macro |
| [docs/protocol.md](docs/protocol.md) | Working on the async QMP/QGA clients or mock servers |
| [docs/manager.md](docs/manager.md) | Working on the VM lifecycle manager |
| [docs/design-constraints.md](docs/design-constraints.md) | Unsure what invariants to preserve |
| [docs/testing.md](docs/testing.md) | Writing tests or investigating CI |

## Quick Commands

```sh
cargo test --workspace

# Regenerate QMP types
cargo run -p qapi-codegen -- vendor/qemu > projects/qapi/src/generated.rs

# Regenerate QGA types
cargo run -p qapi-codegen -- vendor/qemu --qga > projects/qapi/src/qga.rs
```
