# qrab

Typed Rust bindings for QEMU QMP/QGA protocols. Types generated from QEMU QAPI JSON schemas across all release tags (v1.0+). Unified type system carry version lifecycle metadata (`since`, `until`, `deprecated`) — one type set work against any QEMU version at runtime.

## Workspace

| Crate | Path | Role |
|---|---|---|
| `qapi-codegen` | `projects/qapi-codegen/` | Parse QEMU QAPI JSON schemas from git history, generate Rust types |
| `qapi-macros` | `projects/qapi-macros/` | Proc macro: `#[qapi]` → serde/trait impls |
| `qapi` | `projects/qapi/` | Generated types + runtime traits (consumer-facing) |
| `qapi-protocol` | `projects/qapi-protocol/` | Async QMP/QGA clients + mock servers |
| `manager` | `projects/manager/` | Typestate VM lifecycle wrapper |

Examples: `examples/simple-vm/`, `examples/preconfig-vm/`, `examples/events/`

## Critical Rules

- **Schemas golden.** Never preprocess/strip/transform QEMU schema files. Parsers handle raw format.
- **Generated files = outputs.** `generated/` and `qga/` dirs (`mod.rs`, `commands.rs`, `events.rs`) made by `qapi-codegen`. Edit codegen, not generated files.
- **All QEMU release tags processed.** Tag discovery auto. No hardcoded version lists.
- **Version gating dual-layer.** Compile-time (`QEMU_MIN_VERSION`/`QEMU_MAX_VERSION` env vars) + runtime (`set_qemu_version()` OnceLock). Both stay correct.

Full rationale: [docs/design-constraints.md](docs/design-constraints.md)

## Docs

| Doc | When |
|---|---|
| [docs/architecture.md](docs/architecture.md) | Orienting — deps, build phases, toolchain |
| [docs/qapi-crate.md](docs/qapi-crate.md) | Work with generated types + runtime traits |
| [docs/codegen.md](docs/codegen.md) | Modify parser, IR, or codegen pipeline |
| [docs/macro.md](docs/macro.md) | Debug/extend `#[qapi]` proc macro |
| [docs/protocol.md](docs/protocol.md) | Async QMP/QGA clients or mock servers |
| [docs/manager.md](docs/manager.md) | VM lifecycle manager |
| [docs/design-constraints.md](docs/design-constraints.md) | Invariants to preserve |
| [docs/testing.md](docs/testing.md) | Write tests or CI |
| [docs/audit-agents.md](docs/audit-agents.md) | Multi-perspective cleanup / refactor passes via parallel review agents |

## Quick Commands

```sh
cargo test --workspace

# Regen QMP types
cargo run -p qapi-codegen -- vendor/qemu --output-dir projects/qapi/src/generated

# Regen QGA types
cargo run -p qapi-codegen -- vendor/qemu --qga --output-dir projects/qapi/src/qga
```