# qrab

Typed Rust interface for controlling QEMU virtual machines over [QMP](https://www.qemu.org/docs/master/interop/qmp-intro.html) (QEMU Machine Protocol) and [QGA](https://www.qemu.org/docs/master/interop/qemu-ga.html) (QEMU Guest Agent).

Types are generated from QEMU's [QAPI](https://www.qemu.org/docs/master/devel/qapi-code-gen.html) JSON schema files. The codegen processes every release tag from v1.0 onward and annotates each type, field, and variant with lifecycle metadata (`added`, `deprecated`, `removed`), so one set of types works against any QEMU version at runtime.

## Workspace

| Crate | Purpose |
|-------|---------|
| `qapi-codegen` | Parses QAPI schemas from the QEMU git history and generates Rust types |
| `qapi-macros` | Proc-macro that produces version-aware serialization from `#[qapi]` attributes |
| `qapi` | Generated types and runtime traits |
| `qapi-protocol` | Async QMP and QGA clients, plus mock servers for testing |

## Generating types

The `vendor/qemu` submodule must point to a QEMU git repository. The codegen discovers release tags automatically.

```sh
cargo run -p qapi-codegen -- vendor/qemu > projects/qapi/src/generated.rs
cargo run -p qapi-codegen -- vendor/qemu --qga > projects/qapi/src/qga.rs
```

## Running tests

```sh
cargo test --workspace
```

Some integration tests in `qapi-protocol` spawn QEMU processes. Tests that require unavailable binaries or firmware skip automatically.
