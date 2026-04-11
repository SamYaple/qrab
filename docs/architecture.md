# Architecture

Workspace crates and examples are listed in the root
[AGENTS.md](../AGENTS.md#workspace). This document covers dependency flow,
build phases, and non-crate paths.

## Key Non-Crate Paths

| Path | Purpose |
|---|---|
| `vendor/qemu/` | QEMU git submodule (schema source, must have full tag history) |
| `tests/containers/` | Dockerized multi-version QEMU test infrastructure |

## Dependency and Data Flow

```
vendor/qemu (all git release tags)
  |
  |  [build time — run manually]
  v
qapi-codegen
  |  nom parse -> IR -> unified merge -> TokenStream -> prettyplease
  v
projects/qapi/src/generated.rs   (QMP, ~22K lines, checked in)
projects/qapi/src/qga.rs         (QGA, ~1.2K lines, checked in)
  |
  |  [compile time — automatic]
  v
qapi-macros (#[qapi] proc macro expands attributes)
  |
  v
qapi crate (traits: QapiCommand, QapiEvent, PreconfigSafe + version gating)
  |
  |  [can be consumed directly here with a raw socket]
  v
qapi-protocol (async QMP/QGA clients, typed command execution)
  |
  v
manager (VmBuilder -> PreconfigVm -> Vm, high-level orchestration)
```

The `qapi` crate is non-prescriptive about control flow. You can use it
directly with a socket and your own code, bypassing `qapi-protocol` and
`manager` entirely. The layers above it are optional conveniences.

## Build-Time vs Runtime

- **Build time (manual):** `qapi-codegen` reads the QEMU git repo and writes
  `generated.rs` / `qga.rs`. These are checked in and only regenerated when
  QEMU versions are added.
- **Compile time (automatic):** `qapi-macros` expands `#[qapi(...)]` attributes
  into serde impls and trait impls. `QEMU_MIN_VERSION` / `QEMU_MAX_VERSION` env
  vars can prune types outside a version window.
- **Runtime:** `set_qemu_version(major, minor, patch)` gates which version-bound
  fields are serialized/deserialized. Set once per process via `OnceLock`.

## Toolchain Notes

| Item | Detail |
|---|---|
| Rust edition | `qapi-codegen` uses edition 2024; all other crates use edition 2021 |
| `syn` versions | `qapi-macros` depends on `syn` v1; other crates use `syn` v2. Both are compiled. |
| Workspace resolver | Resolver 3 (set in root `Cargo.toml`) |

## Future Direction

The `qapi` crate is the foundation for a larger system that includes a
hypervisor, a GraphQL-style database, and a Dioxus-based frontend. That work
exists on the `main` branch but has not been updated to use this rewritten
`qapi` foundation. It will be rebased and integrated once the `qapi` crate is
stable. Current priority is stabilizing `qapi` and improving test coverage so
it can serve as a reliable building block for all downstream consumers.
