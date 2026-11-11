# qapi-codegen

Parses QEMU's QAPI JSON schemas from every release tag and generates unified
Rust types with version lifecycle metadata.

See [../../docs/codegen.md](../../docs/codegen.md) for the full pipeline docs.

**Key rule:** Generated files (under `qapi/src/generated/` and `qapi/src/qga/`) are outputs. Fix bugs
in this crate, never in the generated code.

```sh
cargo run --release --locked -p qapi-codegen -- vendor/qemu --output-dir projects/qapi/src/generated
cargo run --release --locked -p qapi-codegen -- vendor/qemu --qga --output-dir projects/qapi/src/qga
```
