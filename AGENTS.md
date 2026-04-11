# AGENTS.md

Typed Rust bindings for QEMU's QMP and QGA protocols. Types are generated from
QEMU's QAPI JSON schemas across **all** release tags (v1.0 onward). A single
unified type system carries version lifecycle metadata (`since`, `until`,
`deprecated`) so one set of types works against any QEMU version at runtime.

## Critical Rules

Before working in this repo, internalize these constraints:

- **Source schemas are golden.** Never preprocess, strip, or transform QEMU
  schema files. Parsers handle the raw format. All content must be preserved.
- **Generated files are outputs.** `generated.rs` and `qga.rs` are produced by
  `qapi-codegen`. Edit the codegen, never the generated files.
- **All QEMU release tags are processed.** Tag discovery is automatic. Do not
  hardcode version lists.
- **Version gating is dual-layer.** Compile-time (`QEMU_MIN_VERSION` /
  `QEMU_MAX_VERSION` env vars) and runtime (`set_qemu_version()` OnceLock).
  Both must remain correct.

See [docs/design-constraints.md](docs/design-constraints.md) for full details.

## Documentation Index

| Document | Read if you are... |
|---|---|
| [docs/architecture.md](docs/architecture.md) | Orienting yourself — workspace map, dependency flow, crate roles |
| [docs/qapi-crate.md](docs/qapi-crate.md) | Working with or consuming the generated types and runtime traits |
| [docs/codegen.md](docs/codegen.md) | Modifying the parser, IR, or code generation pipeline |
| [docs/macro.md](docs/macro.md) | Debugging or extending the `#[qapi]` proc macro |
| [docs/protocol.md](docs/protocol.md) | Working on the async QMP/QGA clients or mock servers |
| [docs/manager.md](docs/manager.md) | Working on the high-level VM lifecycle manager |
| [docs/design-constraints.md](docs/design-constraints.md) | Unsure what invariants to preserve |
| [docs/testing.md](docs/testing.md) | Writing tests or investigating CI |

## Quick Commands

```sh
# Run all tests
cargo test --workspace

# Regenerate QMP types
cargo run -p qapi-codegen -- vendor/qemu > projects/qapi/src/generated.rs

# Regenerate QGA types
cargo run -p qapi-codegen -- vendor/qemu --qga > projects/qapi/src/qga.rs
```
