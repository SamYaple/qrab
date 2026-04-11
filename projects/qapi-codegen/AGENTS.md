# qapi-codegen

Parses QEMU's QAPI JSON schemas from every release tag and generates unified
Rust types with version lifecycle metadata.

See [../../docs/codegen.md](../../docs/codegen.md) for the full pipeline docs.
See [../../docs/design-constraints.md](../../docs/design-constraints.md) for invariants.

**Key rule:** Generated files (`generated.rs`, `qga.rs`) are outputs. Fix bugs
in this crate, never in the generated code.

```sh
cargo run -p qapi-codegen -- vendor/qemu > projects/qapi/src/generated.rs
cargo run -p qapi-codegen -- vendor/qemu --qga > projects/qapi/src/qga.rs
```
