# Architecture

## Workspace Crates

| Crate | Path | Role |
|---|---|---|
| `qapi-codegen` | `projects/qapi-codegen/` | Parses QEMU QAPI JSON schemas from git history, generates Rust types |
| `qapi-macros` | `projects/qapi-macros/` | Proc macro expanding `#[qapi]` into Serialize/Deserialize/trait impls |
| `qapi` | `projects/qapi/` | Generated types + runtime traits. The primary consumer-facing crate |
| `qapi-protocol` | `projects/qapi-protocol/` | Async QMP and QGA clients + mock servers for testing |
| `manager` | `projects/manager/` | High-level typestate VM lifecycle wrapper |

## Examples

| Example | Path | Demonstrates |
|---|---|---|
| `simple-vm` | `examples/simple-vm/` | Minimal spawn, start, query, shutdown |
| `preconfig-vm` | `examples/preconfig-vm/` | Preconfig phase introspection with UEFI firmware |
| `events` | `examples/events/` | Async QMP event handling (STOP, RESUME, RESET, etc.) |

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

## Future Direction

The `qapi` crate is the foundation for a larger system that includes a
hypervisor, a GraphQL-style database, and a Dioxus-based frontend. That work
exists in incomplete form on the `main` branch and will be fully rewritten on
top of this new `qapi` crate. Current priority is stabilizing the `qapi` crate
and improving test coverage so it can serve as a reliable building block for all
consumers.
